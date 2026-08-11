// install/optifine.rs - Install OptiFine: scrape optifine.net, bypass ad-wall, run installer.
//
// Mirrors PowerShell Get-OptiFineVersions + Get-OptiFineDirectUrl + Run-InstallOptiFine:
//   - optifine.net/downloads HTML is scraped with a regex.
//   - The real download URL hides behind an ad page (adloadx) which we scrape.
//   - Browser User-Agent required for ALL optifine.net requests.
use crate::http;
use crate::rules::compare_mc_version;
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

const DOWNLOADS_URL: &str = "https://optifine.net/downloads";

/// (mc_version -> Vec<build>) parsed from optifine.net/downloads.
/// A "build" is the part between "OptiFine_" and ".jar", e.g. "1.20.1_HD_U_I6".
pub fn fetch_versions() -> Result<BTreeMap<String, Vec<String>>, String> {
    let html = http::get_text_ua(DOWNLOADS_URL, http::BROWSER_UA)?;
    Ok(parse_versions(&html))
}

/// Parse the downloads HTML for OptiFine build filenames.
pub fn parse_versions(html: &str) -> BTreeMap<String, Vec<String>> {
    // Matches OptiFine_<mc>_HD_U_<letter><num>.jar
    // Capture 1 = full build (1.20.1_HD_U_I6), capture 2 = MC version (1.20.1).
    let re = Regex::new(
        r"OptiFine_((\d+\.\d+(?:\.\d+)?)_HD_U_[A-Z]\d+)\.jar",
    )
    .unwrap();
    let mut by_mc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for cap in re.captures_iter(html) {
        let full = &cap[1];
        let mc = &cap[2];
        let list = by_mc.entry(mc.to_string()).or_default();
        if !list.contains(&full.to_string()) {
            list.push(full.to_string());
        }
    }
    by_mc
}

/// Sort MC versions newest-first.
pub fn sorted_mc_versions(by_mc: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut v: Vec<String> = by_mc.keys().cloned().collect();
    v.sort_by(|a, b| compare_mc_version(b, a));
    v
}

/// Resolve the direct download URL for an OptiFine file by scraping the ad page.
/// `file_name` is e.g. "OptiFine_1.20.1_HD_U_I6.jar".
pub fn resolve_direct_url(file_name: &str) -> Option<String> {
    let ad_url = format!("https://optifine.net/adloadx?f={file_name}");
    let html = http::get_text_ua(&ad_url, http::BROWSER_UA).ok()?;
    // The ad page contains: downloadx?f=<file>&x=<token>
    let re = Regex::new(r#"downloadx\?f=([^"'<>\s]+)"#).unwrap();
    let cap = re.captures(&html)?;
    Some(format!("https://optifine.net/downloadx?f={}", &cap[1]))
}

/// Install OptiFine: resolve the direct URL, download the installer, run its GUI.
/// `build` is e.g. "1.20.1_HD_U_I6".
pub fn install_optifine(
    build: &str,
    work_dir: &Path,
    java_exe: &Path,
) -> Result<(), String> {
    let file_name = format!("OptiFine_{build}.jar");

    // Resolve the real download URL through the ad page.
    let direct_url = resolve_direct_url(&file_name)
        .ok_or_else(|| format!("Could not resolve download URL for {file_name}"))?;

    // Download the installer.
    let installers_dir = work_dir.join("installers");
    fs::create_dir_all(&installers_dir).map_err(|e| format!("mkdir installers: {e}"))?;
    let installer_path = installers_dir.join(&file_name);
    if !installer_path.exists() {
        http::download_file_ua(&direct_url, &installer_path, http::BROWSER_UA, true)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_versions_from_html() {
        let html = r#"<html>
        <a href="adloadx?f=OptiFine_1.20.1_HD_U_I6.jar">1.20.1</a>
        <a href="adloadx?f=OptiFine_1.20.1_HD_U_I5.jar">1.20.1</a>
        <a href="adloadx?f=OptiFine_1.7.10_HD_U_E7.jar">1.7.10</a>
        <a href="other">not a match</a>
        </html>"#;
        let by_mc = parse_versions(html);
        assert_eq!(by_mc.len(), 2);
        assert_eq!(by_mc["1.20.1"].len(), 2);
        assert!(by_mc["1.20.1"].contains(&"1.20.1_HD_U_I6".to_string()));
        assert!(by_mc["1.20.1"].contains(&"1.20.1_HD_U_I5".to_string()));
        assert_eq!(by_mc["1.7.10"], vec!["1.7.10_HD_U_E7"]);
    }

    #[test]
    fn resolve_direct_url_from_ad_page() {
        let ad_html = r#"<html>
        <script>ads...</script>
        <a href="downloadx?f=OptiFine_1.20.1_HD_U_I6.jar&x=abc123">Download</a>
        </html>"#;
        let re = Regex::new(r#"downloadx\?f=([^"'<>\s]+)"#).unwrap();
        let cap = re.captures(ad_html).unwrap();
        assert_eq!(&cap[1], "OptiFine_1.20.1_HD_U_I6.jar&x=abc123");
    }

    #[test]
    fn sorted_versions_newest_first() {
        let mut by_mc = BTreeMap::new();
        by_mc.insert("1.7.10".into(), vec!["1.7.10_HD_U_E7".into()]);
        by_mc.insert("1.20.1".into(), vec!["1.20.1_HD_U_I6".into()]);
        let sorted = sorted_mc_versions(&by_mc);
        assert_eq!(sorted[0], "1.20.1");
        assert_eq!(sorted[1], "1.7.10");
    }

    #[test]
    fn parse_versions_live() {
        // Live test against optifine.net — skip on network failure.
        match fetch_versions() {
            Ok(by_mc) => {
                assert!(!by_mc.is_empty(), "no OptiFine versions found");
                eprintln!("optifine versions: {} MC versions", by_mc.len());
            }
            Err(e) => eprintln!("skipping optifine parse (network): {e}"),
        }
    }
}
