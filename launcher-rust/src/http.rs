// http.rs - HTTP helpers wrapping reqwest blocking.
//
// Centralizes User-Agent handling, file downloads with atomic writes, and
// simple GET-text helpers. The PowerShell launcher used Invoke-WebRequest;
// reqwest with rustls replaces it without any system dependency.

use std::fs;
use std::io::Write;
use std::path::Path;

/// User-Agent sent on requests that don't override it. Most Mojang / Forge /
/// Adoptium endpoints accept any UA; only optifine.net requires a browser UA,
/// which callers pass explicitly via `get_text_ua` / `download_file_ua`.
pub const DEFAULT_UA: &str = concat!(
    "mc-launcher/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/vlds189/lnchermy)"
);

/// A browser-like UA for endpoints that block non-browser clients (optifine.net).
pub const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

/// Build a blocking reqwest client with our default UA + rustls.
pub fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(DEFAULT_UA)
        .build()
        .expect("reqwest client build")
}

/// Build a client with a custom User-Agent.
pub fn client_with_ua(ua: &str) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(ua)
        .build()
        .expect("reqwest client build")
}

/// GET a text body (default UA). Errors propagated as String.
pub fn get_text(url: &str) -> Result<String, String> {
    let resp = client()
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| format!("read body {url}: {e}"))
}

/// GET a text body with a custom UA.
pub fn get_text_ua(url: &str, ua: &str) -> Result<String, String> {
    let resp = client_with_ua(ua)
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| format!("read body {url}: {e}"))
}

/// Download a URL to a file atomically (temp + rename). Skips if the file
/// already exists, unless `force` is true.
pub fn download_file(url: &str, dest: &Path, force: bool) -> Result<(), String> {
    if dest.exists() && !force {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("dat")
    ));
    let resp = client()
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| format!("read body {url}: {e}"))?;
    let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    f.write_all(&bytes)
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    f.sync_all().ok();
    drop(f);
    fs::rename(&tmp, dest).map_err(|e| format!("rename {tmp:?} -> {dest:?}: {e}"))?;
    Ok(())
}

/// Download with a custom UA (optifine.net etc.).
pub fn download_file_ua(url: &str, dest: &Path, ua: &str, force: bool) -> Result<(), String> {
    if dest.exists() && !force {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("dat")
    ));
    let resp = client_with_ua(ua)
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| format!("read body {url}: {e}"))?;
    let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    f.write_all(&bytes)
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    f.sync_all().ok();
    drop(f);
    fs::rename(&tmp, dest).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// HEAD probe: returns true if the URL responds 2xx.
pub fn url_exists(url: &str) -> bool {
    client()
        .head(url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
