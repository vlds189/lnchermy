// versions.rs - Mojang version JSON structs + inheritsFrom merge + argument expansion.
use crate::maven::dedup_libraries;
use crate::rules::{rules_allowed, Rule};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------
// Raw JSON structs (serde-flavored). Fields are optional where Mojang
// version JSONs may omit them.
// ------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VersionJson {
    #[serde(default, rename = "mainClass")]
    pub main_class: Option<String>,
    #[serde(default, rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default, rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(default, rename = "assetIndex")]
    pub asset_index: Option<AssetIndex>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default, rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: i32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AssetIndex {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LibDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

/// An argument entry is either a plain string OR an object { rules, value }.
/// serde flattens both into this enum via a custom deserialize.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    Str(String),
    Cond {
        #[serde(default)]
        rules: Vec<Rule>,
        value: ArgPayload,
    },
}

/// `value` is either a single string or an array of strings.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum ArgPayload {
    One(String),
    Many(Vec<String>),
}

impl ArgPayload {
    pub fn into_strings(self) -> Vec<String> {
        match self {
            ArgPayload::One(s) => vec![s],
            ArgPayload::Many(v) => v,
        }
    }
}

// ------------------------------------------------------------------
// Resolved version: result of merging a child + its parent (inheritsFrom).
// ------------------------------------------------------------------

/// A version after `inheritsFrom` merging — libraries deduped, arguments
/// concatenated, inherited fields filled from the parent.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub main_class: String,
    pub inherits_from: Option<String>,
    pub libraries: Vec<Library>,
    pub jvm_args: Vec<ArgValue>,
    pub game_args: Vec<ArgValue>,
    pub minecraft_arguments: Option<String>,
    pub asset_index: Option<AssetIndex>,
    pub assets: Option<String>,
    pub java_version_major: i32,
}

impl Default for ResolvedVersion {
    fn default() -> Self {
        ResolvedVersion {
            main_class: "net.minecraft.client.main.Main".to_string(),
            inherits_from: None,
            libraries: Vec::new(),
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            minecraft_arguments: None,
            asset_index: None,
            assets: None,
            java_version_major: 17,
        }
    }
}

/// Load + recursively resolve a version JSON from the versions/ folder.
/// Merges the parent (inheritsFrom) into the child: libraries (parent+child,
/// deduped), arguments (parent+child concatenated), and inherits missing
/// fields (mainClass, assetIndex, javaVersion, minecraftArguments) from parent.
///
/// `versions_root` is the absolute path to the `versions/` directory.
pub fn load_resolved(version_id: &str, versions_root: &Path) -> Option<ResolvedVersion> {
    let json_path = versions_root.join(version_id).join(format!("{version_id}.json"));
    let text = std::fs::read_to_string(&json_path).ok()?;
    let raw: VersionJson = serde_json::from_str(&text).ok()?;
    Some(resolve(raw, versions_root))
}

fn resolve(raw: VersionJson, versions_root: &Path) -> ResolvedVersion {
    // Merge parent first if present.
    let (mut libs, mut jvm, mut game, parent) = match &raw.inherits_from {
        Some(parent_id) => {
            let parent = load_resolved(parent_id, versions_root);
            match parent {
                Some(p) => {
                    // libraries: parent first, child after
                    let mut libs = p.libraries.clone();
                    libs.extend(raw.libraries.clone());
                    // arguments: concatenate
                    let mut jvm = p.jvm_args.clone();
                    let mut game = p.game_args.clone();
                    if let Some(args) = &raw.arguments {
                        jvm.extend(args.jvm.clone());
                        game.extend(args.game.clone());
                    }
                    (libs, jvm, game, Some(p))
                }
                None => (
                    raw.libraries.clone(),
                    raw.arguments.as_ref().map(|a| a.jvm.clone()).unwrap_or_default(),
                    raw.arguments.as_ref().map(|a| a.game.clone()).unwrap_or_default(),
                    None,
                ),
            }
        }
        None => (
            raw.libraries.clone(),
            raw.arguments.as_ref().map(|a| a.jvm.clone()).unwrap_or_default(),
            raw.arguments.as_ref().map(|a| a.game.clone()).unwrap_or_default(),
            None,
        ),
    };

    // Dedup libraries by group:artifact[:classifier], keeping the last.
    libs = dedup_libraries(&libs, |l: &Library| l.name.as_str());

    // Fill missing fields from parent.
    let main_class = raw
        .main_class
        .clone()
        .or_else(|| parent.as_ref().map(|p| p.main_class.clone()))
        .unwrap_or_else(|| "net.minecraft.client.main.Main".to_string());
    let asset_index = raw
        .asset_index
        .clone()
        .or_else(|| parent.as_ref().and_then(|p| p.asset_index.clone()));
    let assets = raw
        .assets
        .clone()
        .or_else(|| parent.as_ref().and_then(|p| p.assets.clone()));
    let minecraft_arguments = raw
        .minecraft_arguments
        .clone()
        .or_else(|| parent.as_ref().and_then(|p| p.minecraft_arguments.clone()));
    let java_version_major = raw
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .or_else(|| parent.as_ref().map(|p| p.java_version_major))
        .unwrap_or(17);

    ResolvedVersion {
        main_class,
        inherits_from: raw.inherits_from.clone(),
        libraries: libs,
        jvm_args: jvm,
        game_args: game,
        minecraft_arguments,
        asset_index,
        assets,
        java_version_major,
    }
}

// ------------------------------------------------------------------
// Library path resolution + native detection
// ------------------------------------------------------------------

/// Resolve the on-disk path of a library jar. Prefers `downloads.artifact.path`,
/// falls back to building the path from maven coordinates.
pub fn resolve_lib_path(lib: &Library, lib_dir: &Path, classifier: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = classifier {
        if let Some(downloads) = &lib.downloads {
            if let Some(classifiers) = &downloads.classifiers {
                if let Some(cl) = classifiers.get(c) {
                    if let Some(path) = &cl.path {
                        return Some(join_maven_path(lib_dir, path));
                    }
                }
            }
        }
    } else if let Some(downloads) = &lib.downloads {
        if let Some(art) = &downloads.artifact {
            if let Some(path) = &art.path {
                return Some(join_maven_path(lib_dir, path));
            }
        }
    }
    // Fall back to maven coordinates.
    let rel = crate::maven::maven_rel_path(&lib.name, classifier)?;
    Some(join_maven_path(lib_dir, &rel))
}

fn join_maven_path(lib_dir: &Path, rel: &str) -> PathBuf {
    let mut p = lib_dir.to_path_buf();
    for seg in rel.split('/') {
        p.push(seg);
    }
    p
}

/// Detect whether a library is a native jar for the given OS, and return the
/// classifier to use if so. Handles both formats:
///  - Old (<1.19): `natives` map -> classifier per OS
///  - New (1.19+): separate entry whose name contains `:natives-<os>`
pub fn native_classifier(lib: &Library, os: &str, arch: &str) -> Option<String> {
    if let Some(natives) = &lib.natives {
        if let Some(c) = natives.get(os) {
            return Some(c.replace("${arch}", arch));
        }
    }
    if lib.name.contains(&format!(":natives-{os}")) {
        // The classifier is everything after the 3rd colon.
        if let Some(idx) = lib.name.find(':') {
            if let Some(idx2) = lib.name[idx + 1..].find(':') {
                let abs2 = idx + 1 + idx2;
                if let Some(idx3) = lib.name[abs2 + 1..].find(':') {
                    return Some(lib.name[abs2 + 1 + idx3 + 1..].to_string());
                }
            }
        }
        // Fallback: split into 4 parts.
        let parts: Vec<&str> = lib.name.splitn(4, ':').collect();
        if parts.len() == 4 {
            return Some(parts[3].to_string());
        }
    }
    None
}

// ------------------------------------------------------------------
// Argument expansion
// ------------------------------------------------------------------

/// Replace ${var} placeholders in a single string.
pub fn resolve_placeholders(text: &str, vars: &HashMap<String, String>) -> String {
    let mut out = text.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Expand an argument list (jvm or game) applying OS/feature rules and
/// ${var} placeholders. Returns the flat list of argument strings.
pub fn expand_arguments(
    args: &[ArgValue],
    vars: &HashMap<String, String>,
    enabled_features: &HashMap<String, bool>,
) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            ArgValue::Str(s) => out.push(resolve_placeholders(s, vars)),
            ArgValue::Cond { rules, value } => {
                if !rules_allowed(rules, enabled_features) {
                    continue;
                }
                for s in value.clone().into_strings() {
                    out.push(resolve_placeholders(&s, vars));
                }
            }
        }
    }
    out
}

/// Expand the legacy `minecraftArguments` single string (1.12.2 and older).
pub fn expand_legacy_args(
    template: &str,
    vars: &HashMap<String, String>,
) -> Vec<String> {
    let resolved = resolve_placeholders(template, vars);
    resolved
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_substitute() {
        let mut vars = HashMap::new();
        vars.insert("version_name".into(), "1.20.1".into());
        vars.insert("auth_player_name".into(), "Steve".into());
        assert_eq!(
            resolve_placeholders("--version ${version_name}", &vars),
            "--version 1.20.1"
        );
        assert_eq!(
            resolve_placeholders("--username ${auth_player_name}!", &vars),
            "--username Steve!"
        );
    }

    #[test]
    fn native_classifier_old_format() {
        let mut natives = HashMap::new();
        natives.insert("windows".to_string(), "natives-windows".to_string());
        let lib = Library {
            name: "org.lwjgl.lwjgl:lwjgl-platform:2.9.1".into(),
            rules: vec![],
            natives: Some(natives),
            downloads: None,
        };
        assert_eq!(
            native_classifier(&lib, "windows", "64"),
            Some("natives-windows".into())
        );
    }

    #[test]
    fn native_classifier_new_format() {
        let lib = Library {
            name: "org.lwjgl:lwjgl:3.3.1:natives-windows".into(),
            rules: vec![],
            natives: None,
            downloads: None,
        };
        assert_eq!(
            native_classifier(&lib, "windows", "64"),
            Some("natives-windows".into())
        );
        // not a native for linux
        assert_eq!(native_classifier(&lib, "linux", "64"), None);
    }

    #[test]
    fn expand_simple_args() {
        let mut vars = HashMap::new();
        vars.insert("x".into(), "42".into());
        let feats = HashMap::new();
        let args = vec![
            ArgValue::Str("-cp".into()),
            ArgValue::Str("${x}".into()),
        ];
        assert_eq!(expand_arguments(&args, &vars, &feats), vec!["-cp", "42"]);
    }

    #[test]
    fn expand_excludes_disallowed_feature() {
        let vars = HashMap::new();
        let feats = HashMap::new(); // no demo
        let arg = ArgValue::Cond {
            rules: vec![Rule {
                action: "allow".into(),
                os: None,
                features: Some([("is_demo_user".to_string(), true)].into_iter().collect()),
            }],
            value: ArgPayload::One("--demo".into()),
        };
        assert!(expand_arguments(&[arg], &vars, &feats).is_empty());
    }

    #[test]
    fn expand_legacy_arg_string() {
        let mut vars = HashMap::new();
        vars.insert("auth_player_name".into(), "Steve".into());
        let template = "--username ${auth_player_name} --version 1.7.10";
        assert_eq!(
            expand_legacy_args(template, &vars),
            vec!["--username", "Steve", "--version", "1.7.10"]
        );
    }

    #[test]
    fn dedup_libraries_in_resolve() {
        // guava 15.0 (parent) + guava 17.0 (child) -> keep 17.0
        let libs = vec![
            Library {
                name: "com.google.guava:guava:15.0".into(),
                rules: vec![],
                natives: None,
                downloads: None,
            },
            Library {
                name: "com.google.guava:guava:17.0".into(),
                rules: vec![],
                natives: None,
                downloads: None,
            },
        ];
        let deduped = dedup_libraries(&libs, |l: &Library| l.name.as_str());
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].name, "com.google.guava:guava:17.0");
    }
}
