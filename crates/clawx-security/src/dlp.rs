//! Data Loss Prevention (DLP) scanner.
//!
//! Scans content for sensitive patterns (SSH keys, AWS keys, API keys, tokens)
//! and produces a `DlpResult` indicating whether violations were found.

use regex::Regex;

use clawx_types::security::{DataDirection, DlpResult};

/// DLP scanner that checks content against a set of regex patterns.
#[derive(Debug)]
pub struct DlpScanner {
    patterns: Vec<(String, Regex)>,
}

impl DlpScanner {
    /// Create a DLP scanner with the default built-in patterns.
    pub fn default_patterns() -> Self {
        let patterns = vec![
            (
                "ssh_private_key".to_string(),
                Regex::new(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----").unwrap(),
            ),
            (
                "aws_access_key".to_string(),
                Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            ),
            (
                "api_key_assignment".to_string(),
                Regex::new(
                    r#"(?i)(api[_\-]?key|apikey|secret[_\-]?key)\s*[:=]\s*['"]?[a-zA-Z0-9]{20,}"#,
                )
                .unwrap(),
            ),
            (
                "github_token".to_string(),
                Regex::new(r"gh[ps]_[A-Za-z0-9_]{36,}").unwrap(),
            ),
        ];

        Self { patterns }
    }

    /// Create a DLP scanner with custom patterns.
    pub fn with_patterns(patterns: Vec<(String, Regex)>) -> Self {
        Self { patterns }
    }

    /// Scan content for DLP violations.
    ///
    /// Returns a `DlpResult` with `passed = true` if no violations found.
    pub fn scan(&self, content: &str, direction: DataDirection) -> DlpResult {
        let mut violations = Vec::new();

        for (name, regex) in &self.patterns {
            if regex.is_match(content) {
                violations.push(name.clone());
            }
        }

        let passed = violations.is_empty();

        let redacted_content = if !passed {
            Some(self.redact(content))
        } else {
            None
        };

        DlpResult {
            passed,
            direction,
            violations,
            redacted_content,
        }
    }

    /// Redact all matched patterns from content, replacing with `[REDACTED]`.
    fn redact(&self, content: &str) -> String {
        let mut result = content.to_string();
        for (_, regex) in &self.patterns {
            result = regex.replace_all(&result, "[REDACTED]").to_string();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::security::DataDirection;

    fn scanner() -> DlpScanner {
        DlpScanner::default_patterns()
    }

    // -----------------------------------------------------------------------
    // Clean content passes
    // -----------------------------------------------------------------------

    #[test]
    fn clean_content_passes() {
        let s = scanner();
        let result = s.scan("Hello, this is normal text.", DataDirection::Outbound);
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert!(result.redacted_content.is_none());
    }

    #[test]
    fn clean_code_passes() {
        let s = scanner();
        let result = s.scan(
            r#"fn main() { println!("Hello, world!"); }"#,
            DataDirection::Outbound,
        );
        assert!(result.passed);
    }

    // -----------------------------------------------------------------------
    // SSH key detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_rsa_private_key() {
        let s = scanner();
        let content = "Here is a key:\n-----BEGIN RSA PRIVATE KEY-----\nMIIEpA...\n-----END RSA PRIVATE KEY-----";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"ssh_private_key".to_string()));
    }

    #[test]
    fn detects_ec_private_key() {
        let s = scanner();
        let content = "-----BEGIN EC PRIVATE KEY-----\ndata\n-----END EC PRIVATE KEY-----";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"ssh_private_key".to_string()));
    }

    #[test]
    fn detects_openssh_private_key() {
        let s = scanner();
        let content = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1r...\n-----END OPENSSH PRIVATE KEY-----";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"ssh_private_key".to_string()));
    }

    #[test]
    fn detects_generic_private_key() {
        let s = scanner();
        let content = "-----BEGIN PRIVATE KEY-----\ndata\n-----END PRIVATE KEY-----";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"ssh_private_key".to_string()));
    }

    // -----------------------------------------------------------------------
    // AWS key detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_aws_access_key() {
        let s = scanner();
        let content = "aws_access_key_id = AKIAIOSFODNN7EXAMPLE";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"aws_access_key".to_string()));
    }

    #[test]
    fn aws_key_too_short_passes() {
        let s = scanner();
        // AKIA followed by only 10 chars (need 16)
        let content = "AKIA1234567890";
        let result = s.scan(content, DataDirection::Outbound);
        // Should not match aws_access_key pattern
        assert!(!result.violations.contains(&"aws_access_key".to_string()));
    }

    // -----------------------------------------------------------------------
    // API key detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_api_key_assignment() {
        let s = scanner();
        let content = "api_key = 'abcdefghijklmnopqrstuvwxyz'";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"api_key_assignment".to_string()));
    }

    #[test]
    fn detects_secret_key_with_colon() {
        let s = scanner();
        let content = "SECRET_KEY: ABCDEFGHIJKLMNOPQRSTUV";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"api_key_assignment".to_string()));
    }

    #[test]
    fn detects_apikey_no_separator() {
        let s = scanner();
        let content = "apikey=abcdefghijklmnopqrstuvwxyz";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
    }

    // -----------------------------------------------------------------------
    // GitHub token detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_github_pat() {
        let s = scanner();
        let content = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"github_token".to_string()));
    }

    #[test]
    fn detects_github_secret() {
        let s = scanner();
        let content = "ghs_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"github_token".to_string()));
    }

    // -----------------------------------------------------------------------
    // Multiple violations
    // -----------------------------------------------------------------------

    #[test]
    fn detects_multiple_violations() {
        let s = scanner();
        let content = "key: AKIAIOSFODNN7EXAMPLE\n-----BEGIN RSA PRIVATE KEY-----\ndata";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.len() >= 2);
    }

    // -----------------------------------------------------------------------
    // Direction preserved
    // -----------------------------------------------------------------------

    #[test]
    fn preserves_inbound_direction() {
        let s = scanner();
        let result = s.scan("clean text", DataDirection::Inbound);
        assert_eq!(result.direction, DataDirection::Inbound);
    }

    #[test]
    fn preserves_outbound_direction() {
        let s = scanner();
        let result = s.scan("clean text", DataDirection::Outbound);
        assert_eq!(result.direction, DataDirection::Outbound);
    }

    // -----------------------------------------------------------------------
    // Redaction
    // -----------------------------------------------------------------------

    #[test]
    fn redacts_sensitive_content() {
        let s = scanner();
        let content = "My key: AKIAIOSFODNN7EXAMPLE is secret";
        let result = s.scan(content, DataDirection::Outbound);
        assert!(!result.passed);
        let redacted = result.redacted_content.unwrap();
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    // -----------------------------------------------------------------------
    // Empty content
    // -----------------------------------------------------------------------

    #[test]
    fn empty_content_passes() {
        let s = scanner();
        let result = s.scan("", DataDirection::Outbound);
        assert!(result.passed);
    }

    // -----------------------------------------------------------------------
    // Custom patterns
    // -----------------------------------------------------------------------

    #[test]
    fn custom_pattern_matches() {
        let scanner = DlpScanner::with_patterns(vec![(
            "credit_card".to_string(),
            Regex::new(r"\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b").unwrap(),
        )]);
        let result = scanner.scan("card: 4111-1111-1111-1111", DataDirection::Outbound);
        assert!(!result.passed);
        assert!(result.violations.contains(&"credit_card".to_string()));
    }
}
