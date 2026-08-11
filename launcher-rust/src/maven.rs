// maven.rs - Maven path construction + library dedup (Forge override logic).
use std::path::{Path, PathBuf};

/// `group:artifact:version[:classifier]` -> maven repository relative path.
/// Mirrors PowerShell Get-MavenRelPath.
pub fn maven_rel_path(name: &str, classifier: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = name.splitn(4, ':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

/// Join a library root dir with a relative maven path (forward slashes -> OS sep).
pub fn join_maven(lib_dir: &Path, rel: &str) -> PathBuf {
    let mut p = lib_dir.to_path_buf();
    for seg in rel.split('/') {
        p.push(seg);
    }
    p
}

/// Parse the coordinates of a maven library name.
/// Returns (group, artifact, version, Option<classifier>).
pub fn parse_coords(name: &str) -> Option<(&str, &str, &str, Option<&str>)> {
    let parts: Vec<&str> = name.splitn(4, ':').collect();
    match parts.len() {
        3 => Some((parts[0], parts[1], parts[2], None)),
        4 => Some((parts[0], parts[1], parts[2], Some(parts[3]))),
        _ => None,
    }
}

/// Dedup key for a library: `group:artifact` plus `:classifier` if present,
/// WITHOUT the version. This makes version overrides collapse (guava 15.0 vs
/// 17.0 -> one entry) while classifier variants stay distinct (lwjgl base vs
/// :natives-windows).
pub fn dedup_key(name: &str) -> String {
    match parse_coords(name) {
        Some((g, a, _, Some(c))) => format!("{g}:{a}:{c}"),
        Some((g, a, _, None)) => format!("{g}:{a}"),
        None => name.to_string(),
    }
}

/// De-duplicate a list of libraries by coordinate, keeping the LAST occurrence
/// of each group:artifact[:classifier]. Mirrors PowerShell dedup logic.
pub fn dedup_libraries<T>(libs: &[T], name_of: impl Fn(&T) -> &str) -> Vec<T>
where
    T: Clone,
{
    use std::collections::HashMap;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<T> = Vec::new();
    for lib in libs {
        let key = dedup_key(name_of(lib));
        if let Some(idx) = seen.get(&key) {
            // replace existing
            out[*idx] = lib.clone();
        } else {
            seen.insert(key, out.len());
            out.push(lib.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_basic() {
        let p = maven_rel_path("org.lwjgl:lwjgl:3.3.1", None).unwrap();
        assert_eq!(p, "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar");
    }

    #[test]
    fn rel_path_classifier() {
        let p =
            maven_rel_path("org.lwjgl:lwjgl:3.3.1", Some("natives-windows")).unwrap();
        assert_eq!(
            p,
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );
    }

    #[test]
    fn coords_parse() {
        let (g, a, v, c) = parse_coords("com.google.guava:guava:17.0").unwrap();
        assert_eq!(g, "com.google.guava");
        assert_eq!(a, "guava");
        assert_eq!(v, "17.0");
        assert_eq!(c, None);

        let (g, a, v, c) =
            parse_coords("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap();
        assert_eq!(g, "org.lwjgl");
        assert_eq!(a, "lwjgl");
        assert_eq!(v, "3.3.1");
        assert_eq!(c, Some("natives-windows"));
    }

    #[test]
    fn dedup_key_collapses_versions() {
        assert_eq!(dedup_key("com.google.guava:guava:15.0"), "com.google.guava:guava");
        assert_eq!(dedup_key("com.google.guava:guava:17.0"), "com.google.guava:guava");
    }

    #[test]
    fn dedup_key_keeps_classifier() {
        assert_eq!(
            dedup_key("org.lwjgl:lwjgl:3.3.1"),
            "org.lwjgl:lwjgl"
        );
        assert_eq!(
            dedup_key("org.lwjgl:lwjgl:3.3.1:natives-windows"),
            "org.lwjgl:lwjgl:natives-windows"
        );
    }

    #[test]
    fn dedup_libraries_keeps_last_override() {
        let libs = vec![
            "com.google.guava:guava:15.0".to_string(),
            "org.lwjgl:lwjgl:3.3.1".to_string(),
            "com.google.guava:guava:17.0".to_string(), // override
            "org.lwjgl:lwjgl:3.3.1:natives-windows".to_string(),
        ];
        let out = dedup_libraries(&libs, |s| s.as_str());
        // guava 15.0 should be replaced by 17.0, both lwjgl entries kept.
        assert!(out.contains(&"com.google.guava:guava:17.0".to_string()));
        assert!(!out.contains(&"com.google.guava:guava:15.0".to_string()));
        assert!(out.contains(&"org.lwjgl:lwjgl:3.3.1".to_string()));
        assert!(out
            .contains(&"org.lwjgl:lwjgl:3.3.1:natives-windows".to_string()));
        assert_eq!(out.len(), 3);
    }
}
