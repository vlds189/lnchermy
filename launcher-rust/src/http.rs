// http.rs - HTTP helpers wrapping reqwest blocking.
//
// IMPORTANT: a single reqwest::blocking::Client is reused for ALL requests.
// Creating a new client per call would rebuild the connection pool + TLS state
// each time, making bulk downloads (thousands of assets) 10-100x slower.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

/// User-Agent sent on requests that don't override it.
pub const DEFAULT_UA: &str = concat!(
    "mc-launcher/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/vlds189/lnchermy)"
);

/// A browser-like UA for endpoints that block non-browser clients (optifine.net).
pub const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

/// Shared client with connection pooling + 30s timeout. Reused across ALL
/// requests so connections are kept alive between downloads.
static SHARED_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(DEFAULT_UA)
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .build()
        .expect("reqwest client build")
});

/// Shared client with browser UA (for optifine.net).
static BROWSER_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(BROWSER_UA)
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .build()
        .expect("reqwest browser client build")
});

/// GET a text body (default UA). Errors propagated as String.
pub fn get_text(url: &str) -> Result<String, String> {
    let resp = SHARED_CLIENT
        .get(url)
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    resp.text().map_err(|e| format!("read body {url}: {e}"))
}

/// GET a text body with browser UA (optifine.net).
pub fn get_text_ua(url: &str, _ua: &str) -> Result<String, String> {
    let resp = BROWSER_CLIENT
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
    download_with_client(&SHARED_CLIENT, url, dest)
}

/// Download with browser UA (optifine.net).
pub fn download_file_ua(url: &str, dest: &Path, _ua: &str, force: bool) -> Result<(), String> {
    if dest.exists() && !force {
        return Ok(());
    }
    download_with_client(&BROWSER_CLIENT, url, dest)
}

fn download_with_client(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("dat")
    ));
    // Large files (e.g. 180 MB JDK zips) easily exceed the client's 30s total
    // timeout, so override it per-request with a generous limit. The body is
    // streamed to disk instead of buffered in RAM (resp.bytes() would peak at
    // >200 MB for a JDK download).
    let mut resp = client
        .get(url)
        .timeout(Duration::from_secs(20 * 60))
        .send()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    io::copy(&mut resp, &mut f).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    f.sync_all().ok();
    drop(f);
    fs::rename(&tmp, dest).map_err(|e| format!("rename {tmp:?} -> {dest:?}: {e}"))?;
    Ok(())
}

/// HEAD probe: returns true if the URL responds 2xx.
pub fn url_exists(url: &str) -> bool {
    SHARED_CLIENT
        .head(url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
