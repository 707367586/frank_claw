//! Capability checker and path safety validation for sandbox enforcement.

use clawx_types::security::{Capability, SecurityDecision};

/// Maps a `Capability` to its string representation used in agent config.
pub fn capability_to_str(cap: Capability) -> &'static str {
    match cap {
        Capability::FsRead => "fs_read",
        Capability::FsWrite => "fs_write",
        Capability::NetHttp => "net_http",
        Capability::ExecShell => "exec_shell",
        Capability::SecretInject => "secret_inject",
    }
}

/// Stateless capability checker.
///
/// Checks whether a requested capability is present in the agent's granted
/// capabilities list.
pub struct CapabilityChecker;

impl CapabilityChecker {
    /// Check if the requested capability is in the granted capabilities list.
    pub fn check(capabilities: &[String], requested: Capability) -> SecurityDecision {
        let required = capability_to_str(requested);
        if capabilities.iter().any(|c| c == required) {
            SecurityDecision::Allow
        } else {
            SecurityDecision::Deny {
                reason: format!(
                    "capability '{}' not granted to agent",
                    required
                ),
            }
        }
    }
}

/// Check if a file path is safe (no traversal, within allowed directories).
///
/// Uses canonical path resolution when the path exists on disk to prevent
/// symlink-based traversal. Falls back to string-based checks for
/// non-existent paths.
pub fn check_path_safety(path: &str, allowed_dirs: &[String]) -> SecurityDecision {
    // Reject path traversal in the raw string first
    if path.contains("..") {
        return SecurityDecision::Deny {
            reason: "path traversal detected: '..' component in path".to_string(),
        };
    }

    // If no allowed dirs specified, allow all non-traversal paths
    if allowed_dirs.is_empty() {
        return SecurityDecision::Allow;
    }

    // Try to canonicalize for symlink-safe comparison; fall back to the raw path
    let resolved = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());

    // Check if resolved path is under any allowed directory
    for dir in allowed_dirs {
        let canonical_dir = std::fs::canonicalize(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| dir.clone());

        let normalized_dir = if canonical_dir.ends_with('/') {
            canonical_dir.clone()
        } else {
            format!("{}/", canonical_dir)
        };

        if resolved.starts_with(&normalized_dir) || resolved == canonical_dir.trim_end_matches('/') {
            return SecurityDecision::Allow;
        }
    }

    SecurityDecision::Deny {
        reason: format!("path '{}' is outside allowed directories", path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::security::{Capability, SecurityDecision};

    // -----------------------------------------------------------------------
    // CapabilityChecker
    // -----------------------------------------------------------------------

    #[test]
    fn allows_granted_capability() {
        let caps = vec!["fs_read".to_string(), "net_http".to_string()];
        let decision = CapabilityChecker::check(&caps, Capability::FsRead);
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[test]
    fn denies_missing_capability() {
        let caps = vec!["fs_read".to_string()];
        let decision = CapabilityChecker::check(&caps, Capability::ExecShell);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn denies_empty_capabilities() {
        let caps: Vec<String> = vec![];
        let decision = CapabilityChecker::check(&caps, Capability::FsWrite);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn allows_all_capabilities_when_all_granted() {
        let caps = vec![
            "fs_read".to_string(),
            "fs_write".to_string(),
            "net_http".to_string(),
            "exec_shell".to_string(),
            "secret_inject".to_string(),
        ];
        assert_eq!(
            CapabilityChecker::check(&caps, Capability::FsRead),
            SecurityDecision::Allow
        );
        assert_eq!(
            CapabilityChecker::check(&caps, Capability::FsWrite),
            SecurityDecision::Allow
        );
        assert_eq!(
            CapabilityChecker::check(&caps, Capability::NetHttp),
            SecurityDecision::Allow
        );
        assert_eq!(
            CapabilityChecker::check(&caps, Capability::ExecShell),
            SecurityDecision::Allow
        );
        assert_eq!(
            CapabilityChecker::check(&caps, Capability::SecretInject),
            SecurityDecision::Allow
        );
    }

    #[test]
    fn capability_to_str_mappings() {
        assert_eq!(capability_to_str(Capability::FsRead), "fs_read");
        assert_eq!(capability_to_str(Capability::FsWrite), "fs_write");
        assert_eq!(capability_to_str(Capability::NetHttp), "net_http");
        assert_eq!(capability_to_str(Capability::ExecShell), "exec_shell");
        assert_eq!(capability_to_str(Capability::SecretInject), "secret_inject");
    }

    // -----------------------------------------------------------------------
    // Path safety
    // -----------------------------------------------------------------------

    #[test]
    fn rejects_path_traversal_with_dotdot() {
        let allowed = vec!["/home/user".to_string()];
        let decision = check_path_safety("/home/user/../etc/passwd", &allowed);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn rejects_dotdot_at_start() {
        let allowed = vec!["/home/user".to_string()];
        let decision = check_path_safety("../../etc/shadow", &allowed);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn rejects_dotdot_with_no_allowed_dirs() {
        let decision = check_path_safety("/tmp/../etc/passwd", &[]);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn allows_path_within_allowed_dir() {
        let allowed = vec!["/home/user/project".to_string()];
        let decision = check_path_safety("/home/user/project/src/main.rs", &allowed);
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[test]
    fn allows_exact_allowed_dir() {
        let allowed = vec!["/home/user/project".to_string()];
        let decision = check_path_safety("/home/user/project", &allowed);
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[test]
    fn rejects_path_outside_allowed_dirs() {
        let allowed = vec!["/home/user/project".to_string()];
        let decision = check_path_safety("/etc/passwd", &allowed);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn allows_any_path_when_no_allowed_dirs() {
        let decision = check_path_safety("/any/path/file.txt", &[]);
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[test]
    fn rejects_path_with_similar_prefix_but_different_dir() {
        let allowed = vec!["/home/user/project".to_string()];
        // "/home/user/project-other/file" should not match "/home/user/project"
        let decision = check_path_safety("/home/user/project-other/file", &allowed);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn allows_with_multiple_allowed_dirs() {
        // Use canonical temp dir to handle macOS /tmp -> /private/tmp symlink
        let tmp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let tmp_str = tmp.to_string_lossy().to_string();
        let allowed = vec!["/home/user".to_string(), tmp_str];
        // Use a path under the canonical temp dir so both resolve consistently
        let test_path = tmp.join("file.txt");
        assert_eq!(
            check_path_safety(test_path.to_str().unwrap(), &allowed),
            SecurityDecision::Allow
        );
        assert!(matches!(
            check_path_safety("/etc/passwd", &allowed),
            SecurityDecision::Deny { .. }
        ));
    }

    #[test]
    fn rejects_empty_path_with_allowed_dirs() {
        let allowed = vec!["/home/user".to_string()];
        let decision = check_path_safety("", &allowed);
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }
}
