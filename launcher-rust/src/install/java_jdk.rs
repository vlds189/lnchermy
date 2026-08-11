// install/java_jdk.rs - Install portable Java (Eclipse Temurin / Adoptium).
//
// Mirrors PowerShell Run-InstallJava:
//   - Download a JDK zip from the Adoptium latest-binary endpoint.
//   - Extract and rename to jdk-<major>.
use crate::http;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ADOPTIUM_BASE: &str =
    "https://api.adoptium.net/v3/binary/latest";

/// Build the Adoptium download URL for a given major version.
pub fn adoptium_url(major: u32) -> String {
    format!("{ADOPTIUM_BASE}/{major}/ga/windows/x64/jdk/hotspot/normal/eclipse")
}

/// Install a portable JDK. Downloads the zip, extracts, renames to jdk-<major>.
/// Returns the path to the installed java.exe.
pub fn install_jdk(major: u32, work_dir: &Path) -> Result<PathBuf, String> {
    let url = adoptium_url(major);
    let zip_path = work_dir.join(format!("java-{major}-download.zip"));
    let extract_dir = work_dir.join(format!("java-extract-{major}"));
    let target_dir = work_dir.join(format!("jdk-{major}"));

    // Download.
    http::download_file(&url, &zip_path, true)?;

    // Clean previous extraction.
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).map_err(|e| format!("clean extract dir: {e}"))?;
    }
    fs::create_dir_all(&extract_dir).map_err(|e| format!("mkdir extract dir: {e}"))?;

    // Extract zip.
    extract_zip(&zip_path, &extract_dir)?;

    // Find the inner jdk folder (contains bin/java.exe).
    let jdk_folder = find_jdk_folder(&extract_dir)?;
    if jdk_folder.is_none() {
        return Err("no bin/java.exe found in extracted archive".into());
    }
    let jdk_folder = jdk_folder.unwrap();

    // Rename to jdk-<major>, replacing existing.
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|e| format!("remove old jdk: {e}"))?;
    }
    fs::rename(&jdk_folder, &target_dir).map_err(|e| format!("rename jdk: {e}"))?;

    // Cleanup temp.
    let _ = fs::remove_dir_all(&extract_dir);
    let _ = fs::remove_file(&zip_path);

    let java_exe = target_dir.join("bin").join("java.exe");
    if !java_exe.exists() {
        return Err(format!(
            "installed JDK missing java.exe at {}",
            java_exe.display()
        ));
    }
    Ok(java_exe)
}

/// Extract a zip archive to a directory (flat — no password support needed).
fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("open zip archive: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("read zip entry {i}: {e}"))?;
        let outpath = match entry.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue,
        };
        if entry.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| format!("mkdir {}: {e}", outpath.display()))?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {e}", p.display()))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("create {}: {e}", outpath.display()))?;
            io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("write {}: {e}", outpath.display()))?;
        }
    }
    Ok(())
}

/// Find the first subdirectory of `extract_dir` that contains bin/java.exe.
fn find_jdk_folder(extract_dir: &Path) -> Result<Option<PathBuf>, String> {
    // Could be directly in extract_dir or one level down.
    let direct = extract_dir.join("bin").join("java.exe");
    if direct.exists() {
        return Ok(Some(extract_dir.to_path_buf()));
    }
    let entries = fs::read_dir(extract_dir).map_err(|e| format!("read extract dir: {e}"))?;
    for entry in entries.flatten() {
        let java = entry.path().join("bin").join("java.exe");
        if java.exists() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoptium_url_format() {
        assert_eq!(
            adoptium_url(17),
            "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse"
        );
        assert_eq!(
            adoptium_url(8),
            "https://api.adoptium.net/v3/binary/latest/8/ga/windows/x64/jdk/hotspot/normal/eclipse"
        );
    }

    #[test]
    fn adoptium_url_reachable() {
        // Live check: the URL should at least respond (302 redirect to GitHub).
        let url = adoptium_url(17);
        assert!(
            http::url_exists(&url),
            "Adoptium JDK 17 URL should be reachable"
        );
    }
}
