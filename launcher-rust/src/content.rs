// content.rs - Download mods/resourcepacks/shaderpacks from a JSON index.
//
// Mirrors PowerShell Run-DownloadContent. The index is user-provided (URL from
// settings). Structure:
//   { "versions": { "<mc>": { "mods": [...], "resourcepacks": [...], "shaderpacks": [...] } } }
use crate::http;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Map category -> target subfolder under the game dir.
pub fn category_folder(cat: &str) -> Option<&'static str> {
    match cat {
        "mods" => Some("mods"),
        "resourcepacks" => Some("resourcepacks"),
        "shaderpacks" => Some("shaderpacks"),
        _ => None,
    }
}

/// A single content file entry in the index.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContentFile {
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub size: Option<String>,
}

/// The full content index.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContentIndex {
    pub versions: HashMap<String, HashMap<String, Vec<ContentFile>>>,
}

/// Fetch and parse the content index from a URL.
pub fn fetch_index(url: &str) -> Result<ContentIndex, String> {
    let text = http::get_text(url)?;
    serde_json::from_str(&text).map_err(|e| format!("parse content index: {e}"))
}

/// Download a set of files into the target folder. Returns (ok, failed) counts.
pub fn download_files(
    files: &[ContentFile],
    dest_dir: &Path,
    progress: &dyn Fn(&str, usize, usize),
) -> (usize, usize) {
    fs::create_dir_all(dest_dir).ok();
    let total = files.len();
    let mut ok = 0;
    let mut failed = 0;
    for (i, f) in files.iter().enumerate() {
        if f.url.is_empty() {
            progress(&format!("[{}/{i}] {} — no URL, skipped", i + 1, f.name), i + 1, total);
            continue;
        }
        let target = dest_dir.join(&f.name);
        progress(
            &format!("[{}/{i}] {} …", i + 1, f.name),
            i + 1,
            total,
        );
        match http::download_file(&f.url, &target, true) {
            Ok(()) => ok += 1,
            Err(_) => failed += 1,
        }
    }
    (ok, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_index() {
        let json = r#"{
          "versions": {
            "1.20.1": {
              "mods": [
                {"name": "sodium.jar", "url": "https://example.com/sodium.jar", "size": "1 MB"}
              ],
              "resourcepacks": [
                {"name": "faithful.zip", "url": "https://example.com/faithful.zip"}
              ]
            }
          }
        }"#;
        let idx: ContentIndex = serde_json::from_str(json).unwrap();
        assert_eq!(idx.versions.len(), 1);
        let mc = &idx.versions["1.20.1"];
        assert_eq!(mc["mods"].len(), 1);
        assert_eq!(mc["mods"][0].name, "sodium.jar");
        assert_eq!(mc["resourcepacks"].len(), 1);
        assert!(mc["resourcepacks"][0].size.is_none());
    }

    #[test]
    fn category_mapping() {
        assert_eq!(category_folder("mods"), Some("mods"));
        assert_eq!(category_folder("resourcepacks"), Some("resourcepacks"));
        assert_eq!(category_folder("shaderpacks"), Some("shaderpacks"));
        assert_eq!(category_folder("unknown"), None);
    }
}
