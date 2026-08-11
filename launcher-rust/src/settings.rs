// settings.rs - Persistent launcher settings (mc_console_settings.json)
// PascalCase JSON keys match the PowerShell version for cross-compatibility.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

impl Theme {
    pub fn toggle(self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
        }
    }
}

/// On-disk JSON representation. Field names are PascalCase to stay
/// compatible with the PowerShell launcher's mc_console_settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(rename = "RAM_MIN", default = "default_ram_min")]
    pub ram_min: String,
    #[serde(rename = "RAM_MAX", default = "default_ram_max")]
    pub ram_max: String,
    #[serde(rename = "ContentIndexUrl", default)]
    pub content_index_url: String,
    #[serde(rename = "Username", default = "default_username")]
    pub username: String,
    #[serde(rename = "Theme", default)]
    pub theme: Theme,
}

fn default_ram_min() -> String { "2G".to_string() }
fn default_ram_max() -> String { "4G".to_string() }
fn default_username() -> String { "Player".to_string() }

impl Default for SettingsFile {
    fn default() -> Self {
        SettingsFile {
            ram_min: default_ram_min(),
            ram_max: default_ram_max(),
            content_index_url: String::new(),
            username: default_username(),
            theme: Theme::Dark,
        }
    }
}

/// In-memory settings used throughout the app.
#[derive(Debug, Clone)]
pub struct Settings {
    pub ram_min: String,
    pub ram_max: String,
    pub content_index_url: String,
    pub username: String,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        let f = SettingsFile::default();
        Settings::from_file(f)
    }
}

impl Settings {
    fn from_file(f: SettingsFile) -> Self {
        Settings {
            ram_min: f.ram_min,
            ram_max: f.ram_max,
            content_index_url: f.content_index_url,
            username: f.username,
            theme: f.theme,
        }
    }

    fn to_file(&self) -> SettingsFile {
        SettingsFile {
            ram_min: self.ram_min.clone(),
            ram_max: self.ram_max.clone(),
            content_index_url: self.content_index_url.clone(),
            username: self.username.clone(),
            theme: self.theme,
        }
    }

    /// Load settings from `path`. Returns defaults on any error (missing file,
    /// parse error) so the launcher always starts in a usable state.
    pub fn load(path: &Path) -> Settings {
        match fs::read_to_string(path) {
            Ok(text) => {
                let parsed: Result<SettingsFile, _> = serde_json::from_str(&text);
                match parsed {
                    Ok(f) => Settings::from_file(f),
                    Err(_) => Settings::default(),
                }
            }
            Err(_) => Settings::default(),
        }
    }

    /// Save settings to `path`. Best-effort: errors are returned to the caller
    /// so the UI can surface them, but never panic.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let file = self.to_file();
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("serialize: {e}"))?;
        // Atomic write: temp file + rename, so a crash never leaves a
        // truncated settings file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    /// Validate a Minecraft username (3-16 chars, letters/digits/underscore).
    pub fn is_valid_username(name: &str) -> bool {
        (3..=16).contains(&name.chars().count())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert_eq!(s.ram_min, "2G");
        assert_eq!(s.ram_max, "4G");
        assert_eq!(s.username, "Player");
        assert_eq!(s.theme, Theme::Dark);
    }

    #[test]
    fn roundtrip() {
        let tmp = std::env::temp_dir().join("mc_set_test.json");
        let _ = std::fs::remove_file(&tmp);
        let mut s = Settings::default();
        s.username = "Steve".into();
        s.ram_max = "8G".into();
        s.theme = Theme::Light;
        s.save(&tmp).unwrap();
        let loaded = Settings::load(&tmp);
        assert_eq!(loaded.username, "Steve");
        assert_eq!(loaded.ram_max, "8G");
        assert_eq!(loaded.theme, Theme::Light);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn missing_file_uses_defaults() {
        let s = Settings::load(Path::new("definitely_does_not_exist.json"));
        assert_eq!(s.username, "Player");
    }

    #[test]
    fn corrupt_file_uses_defaults() {
        let tmp = std::env::temp_dir().join("mc_corrupt_test.json");
        std::fs::write(&tmp, "{ this is not json").unwrap();
        let s = Settings::load(&tmp);
        assert_eq!(s.username, "Player");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn username_validation() {
        assert!(Settings::is_valid_username("Steve"));
        assert!(Settings::is_valid_username("Player_99"));
        assert!(!Settings::is_valid_username("ab"));          // too short
        assert!(!Settings::is_valid_username("ThisNameIsWayTooLong12345")); // too long
        assert!(!Settings::is_valid_username("Bad Name!"));   // invalid chars
        assert!(!Settings::is_valid_username("Ник"));         // non-ascii
    }
}
