// java.rs - Find a suitable Java runtime + read its major version.
use std::path::{Path, PathBuf};
use std::process::Command;

/// Read the major Java version from a java.exe. Handles the Java 8 `"1.8"`
/// special case (Java 9+ report the major version directly). Returns 0 on failure.
/// Mirrors PowerShell Get-JavaVersion.
pub fn get_java_version(exe: &Path) -> i32 {
    let output = match Command::new(exe).arg("-version").output() {
        Ok(o) => o,
        Err(_) => return 0,
    };
    // Java prints -version to stderr.
    let text = String::from_utf8_lossy(&output.stderr);
    parse_java_version(&text)
}

/// Pure parser: extract major version from `java -version` output text.
pub fn parse_java_version(text: &str) -> i32 {
    // Java 8: 'openjdk version "1.8.0_302"' -> 8
    // Java 9+: 'openjdk version "17.0.2"' or '"21"' -> 17 / 21
    for line in text.lines() {
        if let Some(rest) = line.find("version \"") {
            let start = rest + "version \"".len();
            if let Some(end) = line[start..].find('"') {
                let ver = &line[start..start + end];
                // Java 8 special case: 1.8.x
                if ver.starts_with("1.8") {
                    return 8;
                }
                // Java 9+: take the leading integer segment.
                let major: String = ver
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = major.parse::<i32>() {
                    return n;
                }
            }
        }
    }
    0
}

/// Find a Java executable suitable for `min_version`.
///
/// Scans `work_dir` for `jdk-*` folders, preferring an EXACT major match
/// (e.g. Java 8 for 1.7.10) because old Minecraft cannot run on newer JVMs
/// even if they meet the minimum. Falls back to the lowest available Java
/// >= min_version, then to a system `java` on PATH.
/// Mirrors PowerShell Find-Java.
pub fn find_java(work_dir: &Path, min_version: i32) -> Option<PathBuf> {
    let mut candidates: Vec<(i32, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(work_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("jdk-") {
            continue;
        }
        let exe = entry.path().join("bin").join("java.exe");
        if !exe.exists() {
            // Non-windows: try `java` without .exe
            let exe_unix = entry.path().join("bin").join("java");
            if !exe_unix.exists() {
                continue;
            }
            let major = get_java_version(&exe_unix);
            if major > 0 {
                candidates.push((major, exe_unix));
            }
            continue;
        }
        let major = get_java_version(&exe);
        if major > 0 {
            candidates.push((major, exe));
        }
    }

    // Prefer exact match.
    for (major, exe) in &candidates {
        if *major == min_version {
            return Some(exe.clone());
        }
    }
    // Fall back to lowest available >= min_version.
    let mut qualified: Vec<&(i32, PathBuf)> =
        candidates.iter().filter(|(m, _)| *m >= min_version).collect();
    qualified.sort_by_key(|(m, _)| *m);
    if let Some((_, exe)) = qualified.first() {
        return Some(exe.clone());
    }

    // System Java on PATH.
    if let Ok(out) = Command::new("java").arg("-version").output() {
        let text = String::from_utf8_lossy(&out.stderr);
        let major = parse_java_version(&text);
        if major >= min_version {
            return Some(PathBuf::from("java"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_java8() {
        let s = "openjdk version \"1.8.0_302\"\nOpenJDK Runtime Environment ...\n";
        assert_eq!(parse_java_version(s), 8);
    }

    #[test]
    fn parse_java17() {
        let s = "openjdk version \"17.0.2\" 2022-01-18\nOpenJDK Runtime Environment ...\n";
        assert_eq!(parse_java_version(s), 17);
    }

    #[test]
    fn parse_java21() {
        let s = "openjdk version \"21.0.5\" 2024-10-15 LTS\n";
        assert_eq!(parse_java_version(s), 21);
    }

    #[test]
    fn parse_garbage() {
        assert_eq!(parse_java_version("not java output"), 0);
    }
}
