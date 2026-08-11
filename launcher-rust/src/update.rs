// update.rs - Self-update: compare versions against version.json in the repo,
// then download a new binary and swap it in.
use crate::http;
use std::path::{Path, PathBuf};

const REPO_OWNER: &str = "vlds189";
const REPO_NAME: &str = "lnchermy";
const REPO_BRANCH: &str = "main";

fn version_url() -> String {
    format!(
        "https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/{REPO_BRANCH}/version.json"
    )
}

/// URL to the latest published .exe in the repo's releases (raw asset).
/// We use the GitHub releases download path; falls back to raw if unreleased.
fn binary_url() -> String {
    // Primary: GitHub releases asset (recommended path for distribution).
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/latest/download/mc-launcher.exe")
}

#[derive(serde::Deserialize)]
struct VersionFile {
    version: String,
}

/// Fetch the latest published version string. Errors on network failure.
pub fn check_latest() -> Result<String, String> {
    let url = version_url();
    let resp = reqwest::blocking::get(&url)
        .map_err(|e| format!("fetch {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp
        .text()
        .map_err(|e| format!("read body: {e}"))?;
    let parsed: VersionFile =
        serde_json::from_str(&body).map_err(|e| format!("parse version.json: {e}"))?;
    Ok(parsed.version)
}

/// Compare dotted version strings. Returns true if `latest` is strictly newer
/// than `current`. Mirrors the PowerShell Compare-Version logic.
pub fn is_newer(latest: &str, current: &str) -> bool {
    compare(latest, current) > 0
}

/// Returns -1 / 0 / 1 like a typical comparator. Segments are compared
/// numerically when both are numeric, else lexicographically.
pub fn compare(a: &str, b: &str) -> i32 {
    let aa: Vec<&str> = a.split('.').collect();
    let bb: Vec<&str> = b.split('.').collect();
    let max = aa.len().max(bb.len());
    for i in 0..max {
        let av = aa.get(i).copied().unwrap_or("0");
        let bv = bb.get(i).copied().unwrap_or("0");
        let (an, a_ok) = parse_num(av);
        let (bn, b_ok) = parse_num(bv);
        if a_ok && b_ok {
            if an != bn {
                return if an < bn { -1 } else { 1 };
            }
        } else {
            // At least one non-numeric segment: case-insensitive string compare.
            let cmp = av.to_ascii_lowercase().cmp(&bv.to_ascii_lowercase());
            if cmp != std::cmp::Ordering::Equal {
                return match cmp {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Greater => 1,
                    _ => 0,
                };
            }
        }
    }
    0
}

fn parse_num(s: &str) -> (i64, bool) {
    match s.parse::<i64>() {
        Ok(n) => (n, true),
        Err(_) => (0, false),
    }
}

/// Outcome of a self-update attempt.
pub enum UpdateOutcome {
    /// Update downloaded and swapped in. Caller should restart.
    Updated(PathBuf),
    /// No newer version available (already up to date).
    UpToDate,
}

/// Check for an update and, if one is available, download + swap the binary.
/// Returns the path the user should re-launch.
pub fn check_and_update(current_version: &str) -> Result<UpdateOutcome, String> {
    let latest = check_latest()?;
    if !is_newer(&latest, current_version) {
        return Ok(UpdateOutcome::UpToDate);
    }
    let exe_path = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    download_and_swap(&exe_path)?;
    Ok(UpdateOutcome::Updated(exe_path))
}

/// Download a new binary and swap it in place of `exe_path`.
///
/// On Windows a running .exe cannot be overwritten, but it CAN be renamed.
/// So we: rename current -> `.bak`, write the new .exe to the final path.
/// If the download fails mid-way we restore the .bak so the app stays usable.
pub fn download_and_swap(exe_path: &Path) -> Result<(), String> {
    let url = binary_url();
    let bak = exe_path.with_extension("exe.bak");

    // Download to a temp file first, so a failed download never corrupts the
    // running binary.
    let tmp = exe_path.with_extension("exe.new");
    http::download_file(&url, &tmp, true)?;
    // Sanity check: a real .exe should be at least a few hundred KB.
    let size = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if size < 100_000 {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("downloaded file too small ({size} bytes) — not a valid .exe"));
    }

    // Move the current binary aside (Windows allows renaming a running exe).
    if bak.exists() {
        let _ = std::fs::remove_file(&bak);
    }
    if exe_path.exists() {
        std::fs::rename(exe_path, &bak).map_err(|e| format!("backup current exe: {e}"))?;
    }

    // Move the new binary into place.
    if let Err(e) = std::fs::rename(&tmp, exe_path) {
        // Restore the backup so the app still runs.
        let _ = std::fs::rename(&bak, exe_path);
        return Err(format!("install new exe: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert_eq!(compare("1.2.0", "1.2.0"), 0);
        assert!(is_newer("1.3.0", "1.2.0"));
        assert!(is_newer("1.2.1", "1.2.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.2.0", "1.3.0"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        // double-digit sanity: 10 > 9, not 1>1 then 0<9
        assert!(is_newer("1.10.0", "1.9.0"));
        // different segment count, equal value
        assert_eq!(compare("1.2", "1.2.0"), 0);
    }
}
