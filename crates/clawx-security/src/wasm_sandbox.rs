//! L2 WASM Dual-Metering Sandbox.
//!
//! Provides a sandboxed execution environment for Skills using fuel metering,
//! memory limits, and timeout control. In this version, we implement the sandbox
//! interface and resource limiting without the full wasmtime dependency —
//! the actual WASM execution engine can be plugged in when wasmtime is available.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use clawx_types::error::{ClawxError, Result};

/// Configuration for the WASM sandbox.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum memory in bytes (default: 256 MB).
    pub max_memory_bytes: usize,
    /// Maximum execution time.
    pub max_execution_time: Duration,
    /// Fuel budget for metering (default: 1_000_000).
    pub fuel_budget: u64,
    /// Maximum HTTP response body size (default: 10 MB).
    pub max_http_response_bytes: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256 MB
            max_execution_time: Duration::from_secs(30),
            fuel_budget: 1_000_000,
            max_http_response_bytes: 10 * 1024 * 1024, // 10 MB
        }
    }
}

/// Result of a sandboxed execution.
#[derive(Debug, Clone)]
pub struct SandboxResult {
    /// Output from the WASM module.
    pub output: String,
    /// Fuel consumed during execution.
    pub fuel_consumed: u64,
    /// Peak memory usage in bytes.
    pub peak_memory_bytes: usize,
    /// Execution duration.
    pub execution_time: Duration,
    /// HTTP requests made by the module.
    pub http_requests: Vec<SandboxHttpRequest>,
}

/// An HTTP request made from within the sandbox.
#[derive(Debug, Clone)]
pub struct SandboxHttpRequest {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub response_bytes: usize,
}

/// Host functions exposed to the WASM module (only 4 allowed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostFunction {
    /// Make an HTTP request (subject to domain whitelist + DLP).
    HttpRequest,
    /// Check if a secret exists (returns bool only, never the value).
    SecretExists,
    /// Write a log message.
    Log,
    /// Get current time in milliseconds.
    NowMillis,
}

/// WASM Sandbox for executing skill modules with resource limits.
pub struct WasmSandbox {
    config: SandboxConfig,
    /// Domain whitelist for HTTP requests.
    allowed_domains: Vec<String>,
    /// Available secrets (names only — values injected at host boundary).
    available_secrets: Vec<String>,
    /// Logs captured during execution.
    logs: Vec<String>,
}

impl WasmSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            allowed_domains: Vec::new(),
            available_secrets: Vec::new(),
            logs: Vec::new(),
        }
    }

    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }

    pub fn with_available_secrets(mut self, secrets: Vec<String>) -> Self {
        self.available_secrets = secrets;
        self
    }

    /// Check if a domain is allowed for HTTP requests.
    pub fn is_domain_allowed(&self, url: &str) -> bool {
        if self.allowed_domains.is_empty() {
            return false; // No domains allowed by default
        }
        // Extract domain from URL
        let domain = extract_domain(url);
        self.allowed_domains.iter().any(|d| {
            if d.starts_with("*.") {
                let suffix = &d[1..]; // ".example.com"
                domain.ends_with(suffix) || domain == &d[2..]
            } else {
                domain == d.as_str()
            }
        })
    }

    /// Check if a secret name is available (returns bool, never the value).
    pub fn secret_exists(&self, name: &str) -> bool {
        self.available_secrets.iter().any(|s| s == name)
    }

    /// Validate that memory usage is within limits.
    pub fn check_memory(&self, current_bytes: usize) -> Result<()> {
        if current_bytes > self.config.max_memory_bytes {
            return Err(ClawxError::Sandbox(format!(
                "memory limit exceeded: {} bytes > {} bytes max",
                current_bytes, self.config.max_memory_bytes
            )));
        }
        Ok(())
    }

    /// Validate that fuel consumption is within budget.
    pub fn check_fuel(&self, consumed: u64) -> Result<()> {
        if consumed > self.config.fuel_budget {
            return Err(ClawxError::Sandbox(format!(
                "fuel budget exceeded: {} > {} max",
                consumed, self.config.fuel_budget
            )));
        }
        Ok(())
    }

    /// Validate execution time.
    pub fn check_timeout(&self, started: Instant) -> Result<()> {
        let elapsed = started.elapsed();
        if elapsed > self.config.max_execution_time {
            return Err(ClawxError::Sandbox(format!(
                "execution timeout: {:?} > {:?} max",
                elapsed, self.config.max_execution_time
            )));
        }
        Ok(())
    }

    /// Validate HTTP response size.
    pub fn check_http_response_size(&self, size: usize) -> Result<()> {
        if size > self.config.max_http_response_bytes {
            return Err(ClawxError::Sandbox(format!(
                "HTTP response too large: {} bytes > {} bytes max",
                size, self.config.max_http_response_bytes
            )));
        }
        Ok(())
    }

    /// Record a log message from the sandbox.
    pub fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    /// Get captured logs.
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// Get the sandbox configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Get current time in milliseconds (safe host function).
    pub fn now_millis(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Extract domain from a URL string.
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

/// Environment cleaner for subprocess execution (L9 T2).
/// Clears the environment and only passes safe variables.
pub struct SubprocessEnvCleaner;

impl SubprocessEnvCleaner {
    /// Get the safe environment variables for a subprocess.
    /// Only PATH, HOME, and LANG are preserved.
    pub fn safe_env() -> HashMap<String, String> {
        let mut env = HashMap::new();
        if let Ok(path) = std::env::var("PATH") {
            env.insert("PATH".to_string(), path);
        }
        if let Ok(home) = std::env::var("HOME") {
            env.insert("HOME".to_string(), home);
        }
        if let Ok(lang) = std::env::var("LANG") {
            env.insert("LANG".to_string(), lang);
        }
        env
    }

    /// Check that no sensitive env vars leak into subprocess.
    pub fn validate_env(env: &HashMap<String, String>) -> Result<()> {
        let forbidden = [
            "AWS_SECRET_ACCESS_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "ZHIPU_API_KEY",
            "DATABASE_URL",
            "SECRET_KEY",
            "PRIVATE_KEY",
        ];
        for key in &forbidden {
            if env.contains_key(*key) {
                return Err(ClawxError::Sandbox(format!(
                    "forbidden environment variable in subprocess: {}",
                    key
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // SandboxConfig
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_values() {
        let config = SandboxConfig::default();
        assert_eq!(config.max_memory_bytes, 256 * 1024 * 1024);
        assert_eq!(config.max_execution_time, Duration::from_secs(30));
        assert_eq!(config.fuel_budget, 1_000_000);
        assert_eq!(config.max_http_response_bytes, 10 * 1024 * 1024);
    }

    // -----------------------------------------------------------------------
    // Memory limits
    // -----------------------------------------------------------------------

    #[test]
    fn check_memory_within_limit() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_memory(100 * 1024 * 1024).is_ok());
    }

    #[test]
    fn check_memory_exceeds_limit() {
        let sandbox = WasmSandbox::new(SandboxConfig {
            max_memory_bytes: 1024,
            ..SandboxConfig::default()
        });
        let result = sandbox.check_memory(2048);
        assert!(result.is_err());
        match result {
            Err(ClawxError::Sandbox(msg)) => assert!(msg.contains("memory limit exceeded")),
            _ => panic!("expected Sandbox error"),
        }
    }

    // -----------------------------------------------------------------------
    // Fuel budget
    // -----------------------------------------------------------------------

    #[test]
    fn check_fuel_within_budget() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_fuel(500_000).is_ok());
    }

    #[test]
    fn check_fuel_exceeds_budget() {
        let sandbox = WasmSandbox::new(SandboxConfig {
            fuel_budget: 100,
            ..SandboxConfig::default()
        });
        let result = sandbox.check_fuel(200);
        assert!(result.is_err());
        match result {
            Err(ClawxError::Sandbox(msg)) => assert!(msg.contains("fuel budget exceeded")),
            _ => panic!("expected Sandbox error"),
        }
    }

    // -----------------------------------------------------------------------
    // Timeout
    // -----------------------------------------------------------------------

    #[test]
    fn check_timeout_within_limit() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        let started = Instant::now();
        assert!(sandbox.check_timeout(started).is_ok());
    }

    #[test]
    fn check_timeout_exceeded() {
        let sandbox = WasmSandbox::new(SandboxConfig {
            max_execution_time: Duration::from_millis(0),
            ..SandboxConfig::default()
        });
        let started = Instant::now() - Duration::from_secs(1);
        let result = sandbox.check_timeout(started);
        assert!(result.is_err());
        match result {
            Err(ClawxError::Sandbox(msg)) => assert!(msg.contains("execution timeout")),
            _ => panic!("expected Sandbox error"),
        }
    }

    // -----------------------------------------------------------------------
    // HTTP response size
    // -----------------------------------------------------------------------

    #[test]
    fn check_http_response_within_limit() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(sandbox.check_http_response_size(1024).is_ok());
    }

    #[test]
    fn check_http_response_exceeds_limit() {
        let sandbox = WasmSandbox::new(SandboxConfig {
            max_http_response_bytes: 100,
            ..SandboxConfig::default()
        });
        let result = sandbox.check_http_response_size(200);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Domain whitelist
    // -----------------------------------------------------------------------

    #[test]
    fn domain_not_allowed_by_default() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        assert!(!sandbox.is_domain_allowed("https://api.example.com/v1"));
    }

    #[test]
    fn domain_allowed_exact_match() {
        let sandbox = WasmSandbox::new(SandboxConfig::default())
            .with_allowed_domains(vec!["api.example.com".to_string()]);
        assert!(sandbox.is_domain_allowed("https://api.example.com/v1"));
        assert!(!sandbox.is_domain_allowed("https://evil.com/v1"));
    }

    #[test]
    fn domain_allowed_wildcard() {
        let sandbox = WasmSandbox::new(SandboxConfig::default())
            .with_allowed_domains(vec!["*.example.com".to_string()]);
        assert!(sandbox.is_domain_allowed("https://api.example.com/v1"));
        assert!(sandbox.is_domain_allowed("https://cdn.example.com/assets"));
        assert!(!sandbox.is_domain_allowed("https://evil.com/v1"));
    }

    // -----------------------------------------------------------------------
    // Secret existence check
    // -----------------------------------------------------------------------

    #[test]
    fn secret_exists_when_registered() {
        let sandbox = WasmSandbox::new(SandboxConfig::default())
            .with_available_secrets(vec!["API_KEY".to_string(), "DB_PASS".to_string()]);
        assert!(sandbox.secret_exists("API_KEY"));
        assert!(sandbox.secret_exists("DB_PASS"));
        assert!(!sandbox.secret_exists("NOT_REGISTERED"));
    }

    // -----------------------------------------------------------------------
    // Logging
    // -----------------------------------------------------------------------

    #[test]
    fn sandbox_captures_logs() {
        let mut sandbox = WasmSandbox::new(SandboxConfig::default());
        sandbox.log("step 1 done".to_string());
        sandbox.log("step 2 done".to_string());
        assert_eq!(sandbox.logs().len(), 2);
        assert_eq!(sandbox.logs()[0], "step 1 done");
    }

    // -----------------------------------------------------------------------
    // now_millis host function
    // -----------------------------------------------------------------------

    #[test]
    fn now_millis_returns_reasonable_value() {
        let sandbox = WasmSandbox::new(SandboxConfig::default());
        let ms = sandbox.now_millis();
        // Should be after 2020-01-01
        assert!(ms > 1_577_836_800_000);
    }

    // -----------------------------------------------------------------------
    // Domain extraction
    // -----------------------------------------------------------------------

    #[test]
    fn extract_domain_from_various_urls() {
        assert_eq!(extract_domain("https://api.example.com/v1"), "api.example.com");
        assert_eq!(extract_domain("http://localhost:8080/path"), "localhost");
        assert_eq!(extract_domain("https://example.com"), "example.com");
    }

    // -----------------------------------------------------------------------
    // SubprocessEnvCleaner
    // -----------------------------------------------------------------------

    #[test]
    fn safe_env_contains_only_allowed_vars() {
        let env = SubprocessEnvCleaner::safe_env();
        // Should only contain PATH, HOME, LANG (if they exist)
        for key in env.keys() {
            assert!(
                key == "PATH" || key == "HOME" || key == "LANG",
                "unexpected env var: {}",
                key
            );
        }
    }

    #[test]
    fn validate_env_rejects_sensitive_vars() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("AWS_SECRET_ACCESS_KEY".to_string(), "secret".to_string());
        let result = SubprocessEnvCleaner::validate_env(&env);
        assert!(result.is_err());
    }

    #[test]
    fn validate_env_accepts_safe_vars() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());
        assert!(SubprocessEnvCleaner::validate_env(&env).is_ok());
    }
}
