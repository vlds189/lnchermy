// install/forge.rs - Install Forge: parse maven-metadata.xml, download installer, run GUI.
//
// Mirrors PowerShell Get-ForgeMetadata + Run-InstallForge:
//   - maven-metadata.xml has every build; group by MC version.
//   - promotions_slim.json gives recommended/latest labels (best-effort).
//   - installer requires launcher_profiles.json to exist (we create it).
//   - run `java -jar installer.jar` with cwd=work_dir so it installs here.
use crate::http;
use crate::rules::compare_mc_version;
use std::fs;
use std::path::Path;
use std::process::Command;

const METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const PROMOS_URLS: &[&str] = &[
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
    "https://maven.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
];

/// (mc_version, forge_version) pairs parsed from maven-metadata.xml, grouped
/// by MC version. `builds[0]` is the newest for that MC version (metadata order).
pub fn fetch_metadata() -> Result<std::collections::BTreeMap<String, Vec<String>>, String> {
    let xml = http::get_text(METADATA_URL)?;
    let builds = parse_metadata(&xml);
    // Group by MC version: split "1.20.1-47.3.0" on first '-'.
    let mut by_mc: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for full in builds {
        if let Some(dash) = full.find('-') {
            let mc = full[..dash].to_string();
            let fg = full[dash + 1..].to_string();
            by_mc.entry(mc).or_default().push(fg);
        }
    }
    Ok(by_mc)
}

/// Extract all version strings from maven-metadata.xml.
fn parse_metadata(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in extract_tags(xml, "version") {
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

/// Naive XML tag extractor (avoids pulling a full XML parser dependency).
fn extract_tags(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        if let Some(end) = after.find(&close) {
            out.push(after[..end].trim().to_string());
            rest = &after[end + close.len()..];
        } else {
            break;
        }
    }
    out
}

/// Fetch promotions (recommended/latest per MC version). Best-effort: returns
/// empty map on failure.
pub fn fetch_promos() -> std::collections::HashMap<String, String> {
    for url in PROMOS_URLS {
        if let Ok(text) = http::get_text(url) {
            if let Ok(map) = parse_promos(&text) {
                return map;
            }
        }
    }
    std::collections::HashMap::new()
}

fn parse_promos(json: &str) -> Result<std::collections::HashMap<String, String>, String> {
    #[derive(serde::Deserialize)]
    struct Promos {
        promos: std::collections::HashMap<String, String>,
    }
    let p: Promos = serde_json::from_str(json).map_err(|e| format!("parse promos: {e}"))?;
    Ok(p.promos)
}

/// Sort MC versions newest-first.
pub fn sorted_mc_versions(by_mc: &std::collections::BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut v: Vec<String> = by_mc.keys().filter(|k| k.starts_with("1.")).cloned().collect();
    v.sort_by(|a, b| compare_mc_version(b, a));
    v
}

/// The default (recommended or latest) Forge build for an MC version.
pub fn default_build(
    mc: &str,
    builds: &[String],
    promos: &std::collections::HashMap<String, String>,
) -> String {
    // Try recommended, then latest from promos.
    if let Some(r) = promos.get(&format!("{mc}-recommended")) {
        if builds.contains(r) {
            return r.clone();
        }
    }
    if let Some(l) = promos.get(&format!("{mc}-latest")) {
        if builds.contains(l) {
            return l.clone();
        }
    }
    // Fall back to the first build (newest in metadata order).
    builds.first().cloned().unwrap_or_default()
}

/// Install Forge: download the installer jar and run its GUI.
/// `forge_version` is e.g. "47.4.5", `mc` is e.g. "1.20.1".
pub fn install_forge(
    mc: &str,
    forge_version: &str,
    work_dir: &Path,
    java_exe: &Path,
) -> Result<(), String> {
    let full = format!("{mc}-{forge_version}");

    // Ensure launcher_profiles.json exists (Forge installer requires it).
    ensure_launcher_profiles(work_dir)?;

    // Download installer jar.
    let installer_name = format!("forge-{full}-installer.jar");
    let installers_dir = work_dir.join("installers");
    fs::create_dir_all(&installers_dir).map_err(|e| format!("mkdir installers: {e}"))?;
    let installer_path = installers_dir.join(&installer_name);
    if !installer_path.exists() {
        let url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/{installer_name}"
        );
        http::download_file(&url, &installer_path, true)?;
    }

    // Run the installer GUI (blocks until the user closes it).
    let status = Command::new(java_exe)
        .arg("-jar")
        .arg(&installer_path)
        .current_dir(work_dir)
        .status()
        .map_err(|e| format!("failed to run installer: {e}"))?;
    if !status.success() {
        return Err(format!("installer exited with code {:?}", status.code()));
    }
    Ok(())
}

/// Create a minimal launcher_profiles.json if it doesn't exist. The Forge
/// installer refuses to run without this "official launcher" marker.
pub fn ensure_launcher_profiles(work_dir: &Path) -> Result<(), String> {
    let path = work_dir.join("launcher_profiles.json");
    if path.exists() {
        return Ok(());
    }
    let game_dir = work_dir.to_string_lossy().replace('\\', "\\\\");
    let json = format!(
        r#"{{
  "profiles": {{
    "mc_console": {{
      "name": "mc_console",
      "type": "latest-release",
      "icon": "Grass",
      "lastVersionId": "latest-release",
      "gameDir": "{game_dir}"
    }}
  }},
  "selectedProfile": "mc_console",
  "clientToken": "mc-console-offline-0001",
  "authenticationDatabase": {{}},
  "launcherVersion": {{ "name": "2.1.0", "format": 0 }}
}}"#
    );
    fs::write(&path, json).map_err(|e| format!("write launcher_profiles: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_extracts_versions() {
        let xml = r#"<?xml version="1.0"?>
<metadata>
  <groupId>net.minecraftforge</groupId>
  <artifactId>forge</artifactId>
  <versioning>
    <versions>
      <version>1.20.1-47.3.0</version>
      <version>1.20.1-47.2.18</version>
      <version>1.19.2-43.2.0</version>
    </versions>
  </versioning>
</metadata>"#;
        let v = parse_metadata(xml);
        assert_eq!(v.len(), 3);
        assert!(v.contains(&"1.20.1-47.3.0".to_string()));
    }

    #[test]
    fn group_by_mc() {
        let xml = r#"<metadata><versioning><versions>
          <version>1.20.1-47.3.0</version>
          <version>1.20.1-47.2.18</version>
          <version>1.19.2-43.2.0</version>
        </versions></versioning></metadata>"#;
        let builds = parse_metadata(xml);
        let mut by_mc = std::collections::BTreeMap::new();
        for full in builds {
            if let Some(dash) = full.find('-') {
                by_mc
                    .entry(full[..dash].to_string())
                    .or_insert_with(Vec::new)
                    .push(full[dash + 1..].to_string());
            }
        }
        assert_eq!(by_mc["1.20.1"].len(), 2);
        assert_eq!(by_mc["1.19.2"].len(), 1);
    }

    #[test]
    fn extract_tags_basic() {
        let xml = "<a><x>1</x><y>2</y><x>3</x></a>";
        assert_eq!(extract_tags(xml, "x"), vec!["1", "3"]);
        assert_eq!(extract_tags(xml, "y"), vec!["2"]);
        assert_eq!(extract_tags(xml, "z"), Vec::<String>::new());
    }

    #[test]
    fn sorted_mc_versions_newest_first() {
        let mut by_mc = std::collections::BTreeMap::new();
        by_mc.insert("1.7.10".into(), vec!["10.13.4.1614".into()]);
        by_mc.insert("1.20.1".into(), vec!["47.4.5".into()]);
        by_mc.insert("1.19.2".into(), vec!["43.2.0".into()]);
        let sorted = sorted_mc_versions(&by_mc);
        assert_eq!(sorted[0], "1.20.1");
        assert_eq!(sorted[1], "1.19.2");
        assert_eq!(sorted[2], "1.7.10");
    }
}
