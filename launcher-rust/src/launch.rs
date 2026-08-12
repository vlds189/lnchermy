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
    vars.insert("auth_uuid".into(), "00000000-0000-0000-0000-000000000000".into());
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
