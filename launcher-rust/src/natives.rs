// natives.rs - Extract native libraries (jar -> .dll) into a folder.
use std::fs;
use std::io;
use std::path::Path;

/// Extract native jars into `dest_dir`, skipping META-INF and directory entries.
/// Mirrors PowerShell Extract-Natives. Overwrites existing extraction dir.
pub fn extract_natives(native_jars: &[std::path::PathBuf], dest_dir: &Path) -> io::Result<()> {
    // Start fresh.
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)?;
    }
    fs::create_dir_all(dest_dir)?;

    for jar in native_jars {
        if !jar.exists() {
            continue;
        }
        let file = fs::File::open(jar)?;
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("warn: cannot open native jar {}: {e}", jar.display());
                continue;
            }
        };
        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.name().to_string();
            // Skip directories and META-INF.
            if name.ends_with('/') {
                continue;
            }
            if name.starts_with("META-INF/") {
                continue;
            }
            let dest = dest_dir.join(&name);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = fs::File::create(&dest)?;
            io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}
