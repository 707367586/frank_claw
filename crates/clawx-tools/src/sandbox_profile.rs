//! Generate `sandbox-exec(1)` profile text scoped to a workspace directory.
//!
//! The generated policy: deny by default, allow read everywhere (so the
//! child can load dylibs, read its own binary, etc.) but only allow
//! file-write under the workspace tree. No outbound network.

use std::path::Path;

pub fn workspace_profile(workspace: &Path) -> String {
    // Absolute canonical path required by sandbox-exec.
    let ws = workspace.display();
    format!(
        r#"(version 1)
(deny default)
(allow process*)
(allow signal (target self))
(allow sysctl-read)
(allow mach-lookup)
(allow ipc-posix-shm)
(allow file-read*)
(allow file-write*
    (subpath "{ws}")
    (subpath "/private/tmp")
    (subpath "/tmp")
    (subpath "/private/var/folders"))
(deny network*)
"#,
        ws = ws
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn profile_includes_workspace_subpath() {
        let got = workspace_profile(&PathBuf::from("/ws/demo"));
        assert!(got.contains(r#"(subpath "/ws/demo")"#));
        assert!(got.contains("(deny network*)"));
    }
}
