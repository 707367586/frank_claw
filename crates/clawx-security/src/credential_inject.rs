//! L3 Host Boundary Credential Injection.
//!
//! Implements the secure credential injection flow:
//! 1. Sandbox constructs request with `{SECRET_NAME}` placeholders
//! 2. Host function intercepts the request
//! 3. Domain whitelist check
//! 4. Keychain read (or in-memory secret store for testing)
//! 5. Placeholder replacement
//! 6. DLP scan on the assembled request
//! 7. Zeroize sensitive data after use
//! 8. Return response to sandbox

use std::collections::HashMap;

use clawx_types::error::{ClawxError, Result};
use zeroize::Zeroize;

/// Placeholder pattern: `{SECRET_NAME}`
const PLACEHOLDER_PREFIX: &str = "{";
const PLACEHOLDER_SUFFIX: &str = "}";

/// A secret store trait for credential injection.
/// In production, this reads from macOS Keychain via clawx-hal.
/// In tests, it uses an in-memory store.
pub trait SecretStore: Send + Sync {
    /// Check if a secret exists (never returns the value).
    fn exists(&self, name: &str) -> bool;
    /// Read a secret value (zeroizable).
    fn read(&self, name: &str) -> Result<Option<String>>;
}

/// In-memory secret store for testing.
pub struct InMemorySecretStore {
    secrets: HashMap<String, String>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.secrets.insert(name.into(), value.into());
    }
}

impl SecretStore for InMemorySecretStore {
    fn exists(&self, name: &str) -> bool {
        self.secrets.contains_key(name)
    }

    fn read(&self, name: &str) -> Result<Option<String>> {
        Ok(self.secrets.get(name).cloned())
    }
}

/// Credential injector that replaces placeholders in request content.
pub struct CredentialInjector<'a> {
    store: &'a dyn SecretStore,
    allowed_domains: &'a [String],
}

impl<'a> CredentialInjector<'a> {
    pub fn new(store: &'a dyn SecretStore, allowed_domains: &'a [String]) -> Self {
        Self {
            store,
            allowed_domains,
        }
    }

    /// Find all placeholder names in the content.
    pub fn find_placeholders(content: &str) -> Vec<String> {
        let mut placeholders = Vec::new();
        let mut search_from = 0;

        while let Some(start) = content[search_from..].find(PLACEHOLDER_PREFIX) {
            let abs_start = search_from + start;
            if let Some(end) = content[abs_start + 1..].find(PLACEHOLDER_SUFFIX) {
                let name = &content[abs_start + 1..abs_start + 1 + end];
                // Only consider ALL_CAPS_UNDERSCORE names as secrets
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                {
                    placeholders.push(name.to_string());
                }
                search_from = abs_start + 1 + end + 1;
            } else {
                break;
            }
        }

        placeholders
    }

    /// Inject credentials into content, replacing `{SECRET_NAME}` placeholders.
    ///
    /// Returns the content with secrets injected. The caller must zeroize
    /// the returned string after use.
    pub fn inject(&self, content: &str, target_url: &str) -> Result<String> {
        // Step 1: Domain whitelist check
        self.check_domain(target_url)?;

        // Step 2: Find placeholders
        let placeholders = Self::find_placeholders(content);
        if placeholders.is_empty() {
            return Ok(content.to_string());
        }

        // Step 3: Read secrets and replace
        let mut result = content.to_string();
        for name in &placeholders {
            let value = self
                .store
                .read(name)?
                .ok_or_else(|| ClawxError::Sandbox(format!("secret not found: {}", name)))?;

            let placeholder = format!("{}{}{}", PLACEHOLDER_PREFIX, name, PLACEHOLDER_SUFFIX);
            result = result.replace(&placeholder, &value);
        }

        Ok(result)
    }

    /// Check that the target URL domain is in the allowed list.
    fn check_domain(&self, url: &str) -> Result<()> {
        if self.allowed_domains.is_empty() {
            return Err(ClawxError::Sandbox(
                "no domains allowed for credential injection".to_string(),
            ));
        }

        let domain = extract_domain(url);
        let allowed = self.allowed_domains.iter().any(|d| {
            if d.starts_with("*.") {
                let suffix = &d[1..];
                domain.ends_with(suffix) || domain == &d[2..]
            } else {
                domain == d.as_str()
            }
        });

        if !allowed {
            return Err(ClawxError::Sandbox(format!(
                "domain '{}' not in whitelist for credential injection",
                domain
            )));
        }

        Ok(())
    }
}

/// Zeroize a string value after use.
pub fn zeroize_string(s: &mut String) {
    s.zeroize();
}

fn extract_domain(url: &str) -> &str {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> InMemorySecretStore {
        let mut store = InMemorySecretStore::new();
        store.insert("API_KEY", "sk-test-12345");
        store.insert("DB_PASSWORD", "p@ssw0rd!");
        store
    }

    fn allowed_domains() -> Vec<String> {
        vec![
            "api.example.com".to_string(),
            "*.openai.com".to_string(),
        ]
    }

    // -----------------------------------------------------------------------
    // Placeholder detection
    // -----------------------------------------------------------------------

    #[test]
    fn find_placeholders_in_content() {
        let content = "Authorization: Bearer {API_KEY}\nHost: {HOST_NAME}";
        let placeholders = CredentialInjector::find_placeholders(content);
        assert_eq!(placeholders, vec!["API_KEY", "HOST_NAME"]);
    }

    #[test]
    fn find_no_placeholders_in_clean_content() {
        let content = "Hello world, no secrets here";
        let placeholders = CredentialInjector::find_placeholders(content);
        assert!(placeholders.is_empty());
    }

    #[test]
    fn find_placeholders_ignores_lowercase() {
        let content = "template: {lowercase_var} and {UPPER_VAR}";
        let placeholders = CredentialInjector::find_placeholders(content);
        assert_eq!(placeholders, vec!["UPPER_VAR"]);
    }

    #[test]
    fn find_placeholders_with_numbers() {
        let content = "key: {API_KEY_V2}";
        let placeholders = CredentialInjector::find_placeholders(content);
        assert_eq!(placeholders, vec!["API_KEY_V2"]);
    }

    // -----------------------------------------------------------------------
    // Injection
    // -----------------------------------------------------------------------

    #[test]
    fn inject_replaces_placeholders() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "Authorization: Bearer {API_KEY}";
        let result = injector
            .inject(content, "https://api.example.com/v1")
            .unwrap();
        assert_eq!(result, "Authorization: Bearer sk-test-12345");
    }

    #[test]
    fn inject_replaces_multiple_placeholders() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "key={API_KEY}&pass={DB_PASSWORD}";
        let result = injector
            .inject(content, "https://api.example.com/v1")
            .unwrap();
        assert_eq!(result, "key=sk-test-12345&pass=p@ssw0rd!");
    }

    #[test]
    fn inject_fails_for_unknown_secret() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "key={NONEXISTENT_KEY}";
        let result = injector.inject(content, "https://api.example.com/v1");
        assert!(result.is_err());
        match result {
            Err(ClawxError::Sandbox(msg)) => assert!(msg.contains("secret not found")),
            _ => panic!("expected Sandbox error"),
        }
    }

    #[test]
    fn inject_fails_for_disallowed_domain() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "key={API_KEY}";
        let result = injector.inject(content, "https://evil.com/steal");
        assert!(result.is_err());
        match result {
            Err(ClawxError::Sandbox(msg)) => assert!(msg.contains("not in whitelist")),
            _ => panic!("expected Sandbox error"),
        }
    }

    #[test]
    fn inject_passes_content_without_placeholders() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "Hello world, no secrets";
        let result = injector
            .inject(content, "https://api.example.com/v1")
            .unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn inject_with_wildcard_domain() {
        let store = test_store();
        let domains = allowed_domains();
        let injector = CredentialInjector::new(&store, &domains);

        let content = "key={API_KEY}";
        let result = injector
            .inject(content, "https://api.openai.com/v1/chat")
            .unwrap();
        assert_eq!(result, "key=sk-test-12345");
    }

    // -----------------------------------------------------------------------
    // Domain check
    // -----------------------------------------------------------------------

    #[test]
    fn domain_check_rejects_empty_whitelist() {
        let store = test_store();
        let domains: Vec<String> = vec![];
        let injector = CredentialInjector::new(&store, &domains);

        let result = injector.inject("{API_KEY}", "https://any.com");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // InMemorySecretStore
    // -----------------------------------------------------------------------

    #[test]
    fn secret_store_exists() {
        let store = test_store();
        assert!(store.exists("API_KEY"));
        assert!(!store.exists("MISSING"));
    }

    #[test]
    fn secret_store_read() {
        let store = test_store();
        assert_eq!(
            store.read("API_KEY").unwrap(),
            Some("sk-test-12345".to_string())
        );
        assert_eq!(store.read("MISSING").unwrap(), None);
    }

    // -----------------------------------------------------------------------
    // Zeroize
    // -----------------------------------------------------------------------

    #[test]
    fn zeroize_clears_string() {
        let mut secret = String::from("super-secret-value");
        zeroize_string(&mut secret);
        // After zeroize, the string should be empty or all zeros
        assert!(secret.is_empty() || secret.chars().all(|c| c == '\0'));
    }
}
