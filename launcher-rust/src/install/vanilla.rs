// install/vanilla.rs - Download a vanilla Minecraft version.
//
// Mirrors PowerShell Download-Version:
//   1. Fetch version_manifest_v2.json, find the version entry.
//   2. Download the per-version JSON.
//   3. Download client.jar (with SHA1 check).
//   4. Download asset index + assets (objects/<hash[:2]>/<hash>).
//   5. Download libraries (with legacy maven fallback for libs without `downloads`).
//
// `Progress` is a callback so the caller (UI) can show step + counts.

use crate::http;
use crate::maven::maven_rel_path;
use crate::versions::{Artifact, AssetIndex, Library, VersionJson};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Progress callback. `(label, current, total)` — total==0 means indeterminate.
pub type Progress = std::sync::Arc<dyn Fn(&str, usize, usize) + Send + Sync>;

/// Maven repositories tried (in order) for legacy libraries lacking `downloads`.
pub const MAVEN_REPOS: &[&str] = &[
    "https://libraries.minecraft.net",
    "https://maven.minecraftforge.net",
];

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ASSET_BASE: &str = "https://resources.download.minecraft.net";

/// Result of a download run.
pub type Result<T> = std::result::Result<T, String>;

/// Fetch the version manifest and return the list of (id, url) pairs.
#[derive(Debug, serde::Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(default)]
    r#type: String,
    url: String,
}

/// Get all release versions (id + url) from the manifest, newest first.
pub fn fetch_manifest() -> Result<Vec<(String, String)>> {
    let text = http::get_text(MANIFEST_URL)?;
    let m: Manifest = serde_json::from_str(&text).map_err(|e| format!("parse manifest: {e}"))?;
    Ok(m.versions.into_iter().map(|v| (v.id, v.url)).collect())
}

/// Download a vanilla version end-to-end.
pub fn download_version(
    version: &str,
    work_dir: &Path,
    progress: &Progress,
) -> Result<()> {
    let versions_dir = work_dir.join("versions");
    let lib_dir = work_dir.join("libraries");
    let assets_dir = work_dir.join("assets");
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");

    // ---- 1. Find the version in the manifest ----
    progress("[1/5] Fetching version manifest…", 0, 0);
    let entries = fetch_manifest()?;
    let url = entries
        .iter()
        .find(|(id, _)| id == version)
        .map(|(_, u)| u.clone())
        .ok_or_else(|| format!("Version {version} not found in manifest"))?;

    // ---- 2. Per-version JSON ----
    progress("[2/5] Reading version metadata…", 0, 0);
    let ver_text = http::get_text(&url)?;
    let ver: VersionJson =
        serde_json::from_str(&ver_text).map_err(|e| format!("parse version json: {e}"))?;

    let version_dir = versions_dir.join(version);
    fs::create_dir_all(&version_dir).map_err(|e| format!("mkdir version dir: {e}"))?;
    fs::create_dir_all(&lib_dir).map_err(|e| format!("mkdir libraries: {e}"))?;
    fs::create_dir_all(&indexes_dir).map_err(|e| format!("mkdir indexes: {e}"))?;
    fs::create_dir_all(&objects_dir).map_err(|e| format!("mkdir objects: {e}"))?;

    let vjson_path = version_dir.join(format!("{version}.json"));
    fs::write(&vjson_path, &ver_text).map_err(|e| format!("write version json: {e}"))?;

    // ---- 3. client.jar ----
    progress("[3/5] Downloading client.jar…", 0, 0);
    let dl = ver
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or("version json missing downloads.client")?;
    let client_jar = version_dir.join(format!("{version}.jar"));
    if !client_jar.exists() {
        let curl = dl
            .url
            .as_ref()
            .ok_or("client.jar url missing")?;
        http::download_file(curl, &client_jar, true)?;
        // SHA1 sanity (warn-only, like PowerShell).
        if let Some(expected_sha) = &dl.sha1 {
            if let Ok(actual) = sha1_hex(&client_jar) {
                if actual != *expected_sha {
                    eprintln!("warn: client.jar SHA1 mismatch (expected {expected_sha}, got {actual})");
                }
            }
        }
    }

    // ---- 4. Asset index + assets ----
    progress("[4/5] Downloading assets…", 0, 0);
    let asset_index = ver
        .asset_index
        .clone()
        .ok_or("version json missing assetIndex")?;
    let index_path = indexes_dir.join(format!("{}.json", asset_index.id));
    if !index_path.exists() {
        if let Some(idx_url) = &asset_index.url {
            http::download_file(idx_url, &index_path, true)?;
        }
    }
    if index_path.exists() {
        let idx_text = fs::read_to_string(&index_path)
            .map_err(|e| format!("read asset index: {e}"))?;
        download_assets(&idx_text, &objects_dir, progress)?;
    }

    // ---- 5. Libraries ----
    progress("[5/5] Downloading libraries…", 0, 0);
    download_libraries(&ver.libraries, &lib_dir, progress)?;

    progress("Done", 0, 0);
    Ok(())
}

/// Download all asset objects listed in an asset index JSON.
fn download_assets(index_text: &str, objects_dir: &Path, progress: &Progress) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Index {
        objects: HashMap<String, AssetObject>,
    }
    #[derive(serde::Deserialize)]
    struct AssetObject {
        hash: String,
        size: u64,
    }
    let idx: Index =
        serde_json::from_str(index_text).map_err(|e| format!("parse asset index: {e}"))?;
    let total = idx.objects.len();
    let mut done = 0usize;
    let mut new = 0usize;
    let mut cached = 0usize;
    let mut failed = 0usize;
    for (_name, obj) in &idx.objects {
        done += 1;
        let hash = obj.hash.to_ascii_lowercase();
        let sub = &hash[..2];
        let target = objects_dir.join(sub).join(&hash);
        if target.exists() && fs::metadata(&target).map(|m| m.len() == obj.size).unwrap_or(false) {
            cached += 1;
        } else {
            let url = format!("{ASSET_BASE}/{sub}/{hash}");
            match http::download_file(&url, &target, true) {
                Ok(()) => new += 1,
                Err(_) => failed += 1,
            }
        }
        if done % 100 == 0 || done == total {
            progress(
                &format!("[4/5] Assets: {done}/{total} (new {new}, cached {cached}, failed {failed})"),
                done,
                total,
            );
        }
    }
    Ok(())
}

/// Download all libraries, with the legacy maven fallback for libs lacking
/// `downloads.artifact`.
fn download_libraries(libs: &[Library], lib_dir: &Path, progress: &Progress) -> Result<()> {
    let total = libs.len();
    let mut done = 0usize;
    let mut missing = 0usize;
    for lib in libs {
        done += 1;
        // Artifact (or legacy maven fallback).
        if let Some(dl) = &lib.downloads {
            if let Some(art) = &dl.artifact {
                if let Some(path) = &art.path {
                    let target = join_maven(lib_dir, path);
                    if !target.exists() {
                        if let Some(url) = &art.url {
                            let _ = http::download_file(url, &target, true);
                        }
                    }
                }
            } else {
                // No artifact field: legacy maven fallback.
                missing += download_legacy(lib, lib_dir);
            }
            // Native classifiers.
            if let Some(classifiers) = &dl.classifiers {
                for (_key, cl) in classifiers {
                    if let Some(path) = &cl.path {
                        let target = join_maven(lib_dir, path);
                        if !target.exists() {
                            if let Some(url) = &cl.url {
                                let _ = http::download_file(url, &target, true);
                            }
                        }
                    }
                }
            }
        } else {
            // No downloads at all: legacy maven fallback.
            missing += download_legacy(lib, lib_dir);
        }
        if done % 10 == 0 || done == total {
            progress(&format!("[5/5] Libraries: {done}/{total}"), done, total);
        }
    }
    if missing > 0 {
        eprintln!("warn: {missing} libraries could not be found in any maven repo");
    }
    Ok(())
}

/// Try each maven repo for a legacy library. Returns 1 if still missing, 0 if ok.
fn download_legacy(lib: &Library, lib_dir: &Path) -> usize {
    let Some(rel) = maven_rel_path(&lib.name, None) else { return 0 };
    let target = join_maven(lib_dir, &rel);
    if target.exists() {
        return 0;
    }
    for repo in MAVEN_REPOS {
        let url = format!("{repo}/{rel}");
        if http::download_file(&url, &target, true).is_ok() {
            return 0;
        }
    }
    eprintln!("missing: {}", lib.name);
    1
}

fn join_maven(lib_dir: &Path, rel: &str) -> PathBuf {
    let mut p = lib_dir.to_path_buf();
    for seg in rel.split('/') {
        p.push(seg);
    }
    p
}

/// Compute the SHA1 hex digest of a file (used for client.jar verification).
fn sha1_hex(path: &Path) -> Result<String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    use std::fmt::Write;
    let digest = sha1(&data);
    let mut out = String::with_capacity(40);
    for b in digest {
        write!(&mut out, "{b:02x}").unwrap();
    }
    Ok(out)
}

/// Minimal SHA-1 implementation (no external dependency for one-off checks).
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }
    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(sha1_hex_bytes(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            sha1_hex_bytes(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            sha1_hex_bytes(b"The quick brown fox jumps over the lazy dog"),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }

    fn sha1_hex_bytes(data: &[u8]) -> String {
        use std::fmt::Write;
        let digest = sha1(data);
        let mut out = String::with_capacity(40);
        for b in digest {
            write!(&mut out, "{b:02x}").unwrap();
        }
        out
    }

    #[test]
    fn fetch_manifest_reaches_mojang() {
        // Live test: hit the real manifest endpoint. Skip on network failure
        // rather than fail (CI may be offline).
        match fetch_manifest() {
            Ok(list) => {
                assert!(!list.is_empty(), "manifest empty");
                assert!(
                    list.iter().any(|(id, _)| id == "1.20.1"),
                    "1.20.1 not in manifest"
                );
                eprintln!("manifest OK: {} versions", list.len());
            }
            Err(e) => {
                eprintln!("skipping fetch_manifest (network): {e}");
            }
        }
    }

    #[test]
    fn download_legacy_library_resolves() {
        // The launchwrapper jar (no downloads field, legacy maven fallback)
        // must resolve from libraries.minecraft.net. This is the most fragile
        // piece of the download pipeline (it broke in the PowerShell version).
        let tmp = std::env::temp_dir().join("mc_legacy_lib_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let lib = Library {
            name: "net.minecraft:launchwrapper:1.12".into(),
            rules: vec![],
            natives: None,
            downloads: None,
        };
        let missing = download_legacy(&lib, &tmp);
        assert_eq!(missing, 0, "launchwrapper should download from maven");
        let jar = tmp.join("net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar");
        assert!(jar.exists(), "launchwrapper jar missing at {}", jar.display());
        // Sanity: must be a non-trivial jar.
        let size = std::fs::metadata(&jar).map(|m| m.len()).unwrap_or(0);
        assert!(size > 10000, "launchwrapper jar too small: {size} bytes");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
