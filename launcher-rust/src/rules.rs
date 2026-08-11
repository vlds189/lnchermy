// rules.rs - Rule evaluation (OS + features) for libraries & conditional args.
// Mirrors PowerShell Test-RulesAllowed + Compare-McVersion.
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rule {
    pub action: String, // "allow" | "deny"
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// Returns the OS name as expected by Mojang rules (windows / linux / osx).
pub fn os_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "osx"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        "unknown"
    }
}

/// Returns "64" on a 64-bit target, else "32" — matches PowerShell $arch.
pub fn arch() -> &'static str {
    const_width()
}

const fn const_width() -> &'static str {
    #[cfg(target_pointer_width = "64")]
    {
        "64"
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        "32"
    }
}

/// Evaluate Mojang rules against the current OS and the set of enabled features.
///
/// Mirrors Test-RulesAllowed: if rules is empty, always allowed. Otherwise walk
/// every rule; a rule "applies" if its os condition matches the current OS/arch
/// AND every feature it lists matches the enabled set. The last applying rule
/// wins (allowed = action == "allow").
pub fn rules_allowed(rules: &[Rule], enabled_features: &HashMap<String, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let os = os_name();
    let mut allowed = false;
    for rule in rules {
        let mut applies = true;

        // OS condition
        if let Some(os_rule) = &rule.os {
            if let Some(name) = &os_rule.name {
                applies = name == os;
            }
            if applies {
                if let Some(arch_req) = &os_rule.arch {
                    // PowerShell: x86 rule does not apply on 64-bit.
                    let is64 = arch() == "64";
                    if arch_req == "x86" && is64 {
                        applies = false;
                    }
                }
            }
        }

        // Features condition: rule applies only if ALL listed features match.
        if applies {
            if let Some(feats) = &rule.features {
                for (f, want) in feats {
                    let have = enabled_features.get(f).copied().unwrap_or(false);
                    if *want != have {
                        applies = false;
                        break;
                    }
                }
            }
        }

        if applies {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

/// Compare Minecraft version strings. Returns Ordering. Segments are compared
/// numerically when both are numeric, else lexicographically (case-insensitive).
/// Missing segments are treated as "0". Mirrors PowerShell Compare-McVersion.
pub fn compare_mc_version(a: &str, b: &str) -> std::cmp::Ordering {
    let aa: Vec<&str> = a.split('.').collect();
    let bb: Vec<&str> = b.split('.').collect();
    let max = aa.len().max(bb.len());
    for i in 0..max {
        let av = aa.get(i).copied().unwrap_or("0");
        let bv = bb.get(i).copied().unwrap_or("0");
        let (an, a_ok) = parse_num(av);
        let (bn, b_ok) = parse_num(bv);
        let ord = if a_ok && b_ok {
            an.cmp(&bn)
        } else {
            av.to_ascii_lowercase()
                .cmp(&bv.to_ascii_lowercase())
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

fn parse_num(s: &str) -> (i64, bool) {
    match s.parse::<i64>() {
        Ok(n) => (n, true),
        Err(_) => (0, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(action: &str) -> Rule {
        Rule {
            action: action.into(),
            os: None,
            features: None,
        }
    }

    #[test]
    fn empty_rules_allowed() {
        let f = HashMap::new();
        assert!(rules_allowed(&[], &f));
    }

    #[test]
    fn allow_windows_only_on_windows() {
        let f = HashMap::new();
        let r = Rule {
            action: "allow".into(),
            os: Some(OsRule {
                name: Some("windows".into()),
                arch: None,
                version: None,
            }),
            features: None,
        };
        if os_name() == "windows" {
            assert!(rules_allowed(&[r.clone()], &f));
        }
        // deny on linux/osx would make allowed=false there
    }

    #[test]
    fn demo_feature_excluded_by_default() {
        let f = HashMap::new(); // no features enabled
        let r = Rule {
            action: "allow".into(),
            os: None,
            features: Some([("is_demo_user".to_string(), true)].into_iter().collect()),
        };
        // demo arg should NOT be allowed when is_demo_user is false
        assert!(!rules_allowed(&[r], &f));
    }

    #[test]
    fn version_ordering() {
        use std::cmp::Ordering;
        assert_eq!(compare_mc_version("1.20.1", "1.20.1"), Ordering::Equal);
        assert_eq!(compare_mc_version("1.21.0", "1.20.1"), Ordering::Greater);
        assert_eq!(compare_mc_version("1.7.10", "1.7.9"), Ordering::Greater);
        assert_eq!(compare_mc_version("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(compare_mc_version("1.2", "1.2.0"), Ordering::Equal);
    }
}
