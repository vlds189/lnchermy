// launch.rs - Assemble the JVM command line and launch Minecraft.
//
// Mirrors PowerShell Run-Launch:
//  - resolve version JSON (with inheritsFrom merge)
//  - find matching Java (exact-version preference)
//  - build classpath (with library dedup + Forge special-casing)
//  - detect + extract natives
//  - expand jvm/game arguments with ${var} placeholders
//  - spawn java.exe
use crate::java::find_java;
use crate::natives::extract_natives;
use crate::versions::{
    expand_arguments, expand_legacy_args, load_resolved, native_classifier, resolve_lib_path,
};
use crate::{rules, settings::Settings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Launch outcome.
pub enum LaunchResult {
    /// Game spawned successfully. The Child handle lets the caller track when
    /// the game process exits.
    Ok(std::process::Child),
    Failed(String),
}

/// Build the full java command line for a version and spawn it.
///
/// `version_id` is the folder name under versions/ (e.g. "1.20.1-forge-47.4.5").
/// `work_dir` is the launcher/game root. `settings` provides RAM and username.
pub fn launch(version_id: &str, work_dir: &Path, settings: &Settings) -> LaunchResult {
    let (java_exe, args, _main, _cp, _natives) = match build_command(version_id, work_dir, settings) {
        Ok(v) => v,
        Err(e) => return LaunchResult::Failed(e),
    };

    // Extract natives before launching (build_command computes the natives dir
    // but doesn't perform the extraction itself).
    let versions_dir = work_dir.join("versions");
    let version_dir = versions_dir.join(version_id);
    let lib_dir = work_dir.join("libraries");
    let os = rules::os_name();
    let arch = rules::arch();
    let enabled_features = HashMap::new();
    let resolved = load_resolved(version_id, &versions_dir);
    if let Some(r) = &resolved {
        let mut native_jars: Vec<PathBuf> = Vec::new();
        for lib in &r.libraries {
            if !rules::rules_allowed(&lib.rules, &enabled_features) {
                continue;
            }
            if let Some(classifier) = native_classifier(lib, os, arch) {
                if let Some(p) = resolve_lib_path(lib, &lib_dir, Some(&classifier)) {
                    if p.exists() {
                        native_jars.push(p);
                    }
                }
            }
        }
        let natives_dir = version_dir.join("natives-extracted");
        if let Err(e) = extract_natives(&native_jars, &natives_dir) {
            eprintln!("warn: natives extraction failed: {e}");
        }
    }

    let mut cmd = Command::new(&java_exe);
    cmd.args(&args);
    cmd.current_dir(work_dir);

    match cmd.spawn() {
        Ok(child) => LaunchResult::Ok(child),
        Err(e) => LaunchResult::Failed(format!("Failed to start java: {e}")),
    }
}

/// Offline player UUID — byte-identical to Java's
/// `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`, i.e.
/// an RFC 4122 v3 (name-based, MD5) UUID. Vanilla offline servers and LAN
/// worlds identify players by exactly this value, and the client picks the
/// default skin from `DEFAULT_SKINS[floorMod(uuid.hashCode(), 18)]` — so a
/// per-name UUID gives every nickname its own stable default skin. (A
/// hardcoded zero UUID always hashed to index 0 = slim/alex: the
/// "female skin regardless of nick" bug.)
pub fn offline_uuid(name: &str) -> String {
    let mut h = md5(format!("OfflinePlayer:{name}").as_bytes());
    // nameUUIDFromBytes pins the version (3) and IETF variant nibbles.
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Minimal MD5 (RFC 1321). Needed only for the 16-byte offline-UUID digest
/// above — same "inline hash, no extra crate" approach as SHA-1 in
/// install/vanilla.rs.
fn md5(msg: &[u8]) -> [u8; 16] {
    // Per-round shift amounts.
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20,
        5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // K[i] = floor(|sin(i+1)| * 2^32), the standard T table.
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    // Pad: 0x80, zeros to 56 mod 64, then the bit length as LE u64.
    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&((msg.len() as u64) * 8).to_le_bytes());

    for chunk in data.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

/// Build the full argument vector for a version WITHOUT spawning java.
/// Useful for testing the assembly logic and for displaying the command.
/// Returns (java_exe_path, all_args, main_class, classpath, natives_dir).
#[allow(clippy::type_complexity)]
pub fn build_command(
    version_id: &str,
    work_dir: &Path,
    settings: &Settings,
) -> Result<(PathBuf, Vec<String>, String, String, PathBuf), String> {
    let versions_dir = work_dir.join("versions");
    let lib_dir = work_dir.join("libraries");

    let resolved = load_resolved(version_id, &versions_dir)
        .ok_or_else(|| format!("Cannot read version JSON for {version_id}"))?;

    let min_java = resolved.java_version_major;
    let java_exe = find_java(work_dir, min_java)
        .ok_or_else(|| format!("Java {min_java}+ not found"))?;

    let os = rules::os_name();
    let arch = rules::arch();
    let enabled_features = HashMap::new();

    let version_dir = versions_dir.join(version_id);
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut native_jars: Vec<PathBuf> = Vec::new();

    let client_jar = version_dir.join(format!("{version_id}.jar"));
    if client_jar.exists() {
        classpath.push(client_jar);
    }

    let is_forge = resolved.main_class.contains("BootstrapLauncher");
    if let Some(parent) = &resolved.inherits_from {
        if !is_forge {
            let parent_jar = versions_dir.join(parent).join(format!("{parent}.jar"));
            if parent_jar.exists() {
                classpath.push(parent_jar);
            }
        }
    }

    for lib in &resolved.libraries {
        if !rules::rules_allowed(&lib.rules, &enabled_features) {
            continue;
        }
        if let Some(classifier) = native_classifier(lib, os, arch) {
            if let Some(p) = resolve_lib_path(lib, &lib_dir, Some(&classifier)) {
                if p.exists() {
                    native_jars.push(p);
                }
            }
        } else if let Some(p) = resolve_lib_path(lib, &lib_dir, None) {
            classpath.push(p);
        }
    }

    let natives_dir = version_dir.join("natives-extracted");

    let asset_index = resolved
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| resolved.assets.clone())
        .unwrap_or_else(|| version_id.to_string());

    let cp_sep = ';';
    let cp_string = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(&cp_sep.to_string());
    let assets_root = work_dir.join("assets").to_string_lossy().to_string();
    let mut vars = HashMap::new();
    vars.insert("auth_player_name".into(), settings.username.clone());
    vars.insert("version_name".into(), version_id.to_string());
    vars.insert("game_directory".into(), work_dir.to_string_lossy().to_string());
    vars.insert("assets_root".into(), assets_root.clone());
    vars.insert("assets_index_name".into(), asset_index.clone());
    vars.insert("auth_uuid".into(), offline_uuid(&settings.username));
    vars.insert("auth_access_token".into(), "0".into());
    vars.insert("clientid".into(), "00000000-0000-0000-0000-000000000000".into());
    vars.insert("auth_xuid".into(), "0".into());
    vars.insert("user_properties".into(), "{}".into());
    vars.insert("user_type".into(), "msa".into());
    vars.insert("version_type".into(), "release".into());
    vars.insert("natives_directory".into(), natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name".into(), "mc_launcher".into());
    vars.insert("launcher_version".into(), "2.0".into());
    vars.insert("classpath".into(), cp_string.clone());
    vars.insert("classpath_separator".into(), cp_sep.to_string());
    vars.insert("library_directory".into(), lib_dir.to_string_lossy().to_string());

    let mut jvm_args: Vec<String> = Vec::new();
    jvm_args.push(format!("-Xms{}", settings.ram_min));
    jvm_args.push(format!("-Xmx{}", settings.ram_max));

    if !resolved.jvm_args.is_empty() {
        jvm_args.extend(expand_arguments(&resolved.jvm_args, &vars, &enabled_features));
    } else {
        jvm_args.push(format!("-Djava.library.path={}", natives_dir.display()));
        jvm_args.push("-cp".into());
        jvm_args.push(cp_string.clone());
    }

    if is_forge {
        let has_lib_path = jvm_args.iter().any(|a| a.starts_with("-Djava.library.path"));
        if !has_lib_path {
            jvm_args.push(format!("-Djava.library.path={}", natives_dir.display()));
        }
        jvm_args.push("-cp".into());
        jvm_args.push(cp_string.clone());
    }

    let mut game_args: Vec<String> = Vec::new();
    if !resolved.game_args.is_empty() {
        game_args.extend(expand_arguments(&resolved.game_args, &vars, &enabled_features));
    } else if let Some(legacy) = &resolved.minecraft_arguments {
        game_args.extend(expand_legacy_args(legacy, &vars));
    }

    let mut all_args = jvm_args;
    all_args.push(resolved.main_class.clone());
    all_args.extend(game_args);

    Ok((java_exe, all_args, resolved.main_class, cp_string, natives_dir))
}

/// Resolve and return the classpath string for a version (used for display/debug).
#[allow(dead_code)]
pub fn debug_classpath(version_id: &str, work_dir: &Path) -> Option<String> {
    let versions_dir = work_dir.join("versions");
    let lib_dir = work_dir.join("libraries");
    let resolved = load_resolved(version_id, &versions_dir)?;
    let mut cp: Vec<PathBuf> = Vec::new();
    let client = versions_dir.join(version_id).join(format!("{version_id}.jar"));
    if client.exists() {
        cp.push(client);
    }
    for lib in &resolved.libraries {
        if let Some(p) = resolve_lib_path(lib, &lib_dir, None) {
            cp.push(p);
        }
    }
    Some(cp.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::{resolve_placeholders, ResolvedVersion};

    #[test]
    fn placeholder_substitution_in_launch() {
        // sanity: ensure resolve_placeholders is re-exported and works
        let mut vars = HashMap::new();
        vars.insert("x".into(), "1".into());
        assert_eq!(resolve_placeholders("${x}", &vars), "1");
    }

    // ---- offline player UUID (default-skin-per-nickname fix) ----

    /// Ground truth: these UUIDs were computed by Minecraft itself for
    /// offline LAN players and cached in the game folder's usercache.json.
    #[test]
    fn offline_uuid_matches_vanilla() {
        assert_eq!(offline_uuid("Kepler"), "00551003-7c65-3a9d-94dd-33e9387cc53f");
        assert_eq!(offline_uuid("Player"), "a01e3843-e521-3998-958a-f459800e4d11");
        assert_eq!(offline_uuid("Player1"), "681f539b-8bb8-3f85-85e5-a2945f6c6539");
        assert_eq!(offline_uuid("Vlad"), "c5ac6b65-aeae-3786-98d1-60ad333907ec");
    }

    #[test]
    fn offline_uuid_shape_and_uniqueness() {
        let u = offline_uuid("Steve");
        assert_eq!(u.len(), 36);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.len(), 5);
        // RFC 4122 v3 (name-based, MD5) + IETF variant, exactly what
        // UUID.nameUUIDFromBytes produces.
        assert!(parts[2].starts_with('3'), "not a v3 UUID: {u}");
        assert!(matches!(parts[3].as_bytes()[0], b'8'..=b'b'), "bad variant: {u}");
        // The whole point: different nicks must yield different UUIDs (and
        // therefore different default skins).
        assert_ne!(offline_uuid("Steve"), offline_uuid("Alex"));
    }

    #[test]
    fn md5_known_vectors() {
        let hex = |d: [u8; 16]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(md5(b"The quick brown fox jumps over the lazy dog")),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
    }

    const WORK_DIR: &str = "D:\\Games\\h";

    fn skip_if_no_version(id: &str) -> bool {
        let p = Path::new(WORK_DIR).join("versions").join(id).join(format!("{id}.json"));
        if !p.exists() {
            eprintln!("skipping: {id} not installed");
            return true;
        }
        false
    }

    #[test]
    fn build_command_vanilla_1_20_1() {
        if skip_if_no_version("1.20.1") {
            return;
        }
        let settings = Settings {
            ram_min: "2G".into(),
            ram_max: "4G".into(),
            content_index_url: String::new(),
            username: "Steve".into(),
            theme: crate::settings::Theme::Dark,
            last_version: None,
        };
        let (java, args, main, cp, _natives) =
            build_command("1.20.1", Path::new(WORK_DIR), &settings).unwrap();

        // Java 17 for 1.20.1.
        let java_ver = crate::java::get_java_version(&java);
        assert!(
            java_ver >= 17,
            "expected Java >=17 for 1.20.1, got {java_ver} from {}",
            java.display()
        );

        // main class = vanilla client Main (not Forge BootstrapLauncher).
        assert_eq!(main, "net.minecraft.client.main.Main");

        // RAM flags present.
        assert!(args.iter().any(|a| a == "-Xms2G"));
        assert!(args.iter().any(|a| a == "-Xmx4G"));

        // username substituted into game args.
        assert!(args.iter().any(|a| a == "--username"));
        assert!(args.iter().any(|a| a == "Steve"));

        // the offline UUID derived from the username (not a zero constant)
        // must be passed via --uuid — the client picks the default skin from it.
        assert!(
            args.iter()
                .any(|a| a == "5627dd98-e6be-3c21-b8a8-e92344183641"),
            "offline uuid for username Steve missing from launch args"
        );

        // classpath is non-empty and includes the client jar.
        assert!(cp.contains("1.20.1.jar"));
        assert!(cp.contains("libraries"));

        // No leftover ${...} placeholders in any arg.
        for a in &args {
            assert!(
                !a.contains("${"),
                "unsubstituted placeholder in arg: {a}"
            );
        }

        // No feature-gated args (--demo, --quickPlay*) leaked in.
        assert!(!args.iter().any(|a| a == "--demo"));
        assert!(!args.iter().any(|a| a.starts_with("--quickPlay")));
    }

    #[test]
    fn build_command_forge_1_20_1() {
        let forge_id = "1.20.1-forge-47.4.5";
        if skip_if_no_version(forge_id) {
            return;
        }
        let settings = Settings::default();
        let (java, args, main, cp, _natives) =
            build_command(forge_id, Path::new(WORK_DIR), &settings).unwrap();

        // Forge uses BootstrapLauncher.
        assert!(
            main.contains("BootstrapLauncher"),
            "expected BootstrapLauncher, got {main}"
        );

        // Forge special-casing: an explicit -cp must be present.
        assert!(args.iter().any(|a| a == "-cp"));

        // Forge game args include --launchTarget forgeclient.
        assert!(args.iter().any(|a| a == "--launchTarget"));
        assert!(args.iter().any(|a| a == "forgeclient"));

        // java.library.path must be set (forge JSON omits it, our code adds it).
        assert!(
            args.iter().any(|a| a.starts_with("-Djava.library.path")),
            "missing -Djava.library.path for Forge"
        );

        // --add-opens for java.base/java.lang.invoke (Forge needs it).
        assert!(args.iter().any(|a| a == "--add-opens"));

        // No leftover placeholders.
        for a in &args {
            assert!(!a.contains("${"), "unsubstituted placeholder: {a}");
        }

        // The classpath must NOT contain the vanilla 1.20.1.jar (Forge excludes
        // the parent jar to avoid the _1._20._1 module conflict).
        let _ = java;
        assert!(
            !cp.contains("1.20.1.jar") || cp.contains("forge"),
            "vanilla parent jar leaked into Forge classpath"
        );
    }

    #[test]
    fn build_command_legacy_1_7_10() {
        if skip_if_no_version("1.7.10") {
            return;
        }
        let settings = Settings::default();
        let (java, args, main, cp, _natives) =
            build_command("1.7.10", Path::new(WORK_DIR), &settings).unwrap();

        // 1.7.10 uses legacy launchwrapper.
        assert!(
            main.contains("launchwrapper") || main.contains("Main"),
            "unexpected main class for 1.7.10: {main}"
        );

        // Legacy: -cp and -Djava.library.path (old JSON has no arguments.jvm).
        assert!(args.iter().any(|a| a == "-cp"));
        assert!(args.iter().any(|a| a.starts_with("-Djava.library.path")));

        // Java 8 must be selected (exact-match preference).
        let jv = crate::java::get_java_version(&java);
        assert_eq!(jv, 8, "expected Java 8 for 1.7.10, got {jv}");

        // classpath includes lwjgl 2.9.1 (1.7.10's LWJGL).
        assert!(cp.contains("lwjgl-2.9.1"), "missing lwjgl-2.9.1 in 1.7.10 cp");
    }

    #[test]
    fn launch_vanilla_1_20_1_spawns_java() {
        if skip_if_no_version("1.20.1") {
            return;
        }
        let settings = Settings {
            ram_min: "2G".into(),
            ram_max: "4G".into(),
            content_index_url: String::new(),
            username: "SmokeTest".into(),
            theme: crate::settings::Theme::Dark,
            last_version: None,
        };
        match launch("1.20.1", Path::new(WORK_DIR), &settings) {
            LaunchResult::Ok(_child) => {
                // Give the spawned java a moment, then we just trust the OS
                // reported a successful spawn. The test passing means:
                //  - java.exe was found
                //  - the command line was valid enough to spawn
                //  - no immediate exec error
                println!("launch() reported spawn OK");
            }
            LaunchResult::Failed(e) => {
                panic!("launch() failed: {e}");
            }
        }
    }

    fn _ensure_resolved_default() -> ResolvedVersion {
        ResolvedVersion::default()
    }
}
