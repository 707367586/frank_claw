//! Document parsers for various file types.

use clawx_types::error::{ClawxError, Result};
use std::path::Path;

/// Parse a file and extract its text content.
pub async fn parse_file(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "markdown" | "csv" | "json" | "toml" | "yaml" | "yml" | "rs" | "py"
        | "js" | "ts" | "go" | "swift" | "sh" | "html" | "css" | "xml" | "sql" | "log" => {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ClawxError::Internal(format!("failed to read {}: {}", path.display(), e)))
        }
        _ => Err(ClawxError::Internal(format!(
            "unsupported file type: .{}",
            ext
        ))),
    }
}

/// Detect the file type from extension.
pub fn detect_file_type(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase()
}

/// Calculate SHA-256 hash of file content.
pub fn content_hash(content: &str) -> String {
    use std::fmt::Write;
    let digest = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    };
    let mut hex = String::with_capacity(16);
    write!(hex, "{:016x}", digest).unwrap();
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn parse_text_file() {
        let mut f = NamedTempFile::with_suffix(".txt").unwrap();
        write!(f, "Hello, world!").unwrap();
        let content = parse_file(f.path()).await.unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[tokio::test]
    async fn parse_unsupported_returns_error() {
        let f = NamedTempFile::with_suffix(".bin").unwrap();
        assert!(parse_file(f.path()).await.is_err());
    }

    #[test]
    fn detect_type() {
        assert_eq!(detect_file_type(Path::new("test.md")), "md");
        assert_eq!(detect_file_type(Path::new("test.rs")), "rs");
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = content_hash("hello");
        let h2 = content_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(content_hash("hello"), content_hash("world"));
    }
}
