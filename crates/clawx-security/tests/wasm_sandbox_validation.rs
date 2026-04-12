//! WASM sandbox policy-layer validation tests.
//!
//! Verifies that the sandbox configuration and enforcement layer correctly
//! rejects operations that exceed resource limits, and that domain
//! whitelisting is enforced.

use std::time::{Duration, Instant};

use clawx_security::wasm_sandbox::{SandboxConfig, WasmSandbox, SubprocessEnvCleaner};

// ---------------------------------------------------------------------------
// Test 1: Memory limit enforcement
// ---------------------------------------------------------------------------

#[test]
fn wasm_memory_limit_rejects_over_256mb() {
    let sandbox = WasmSandbox::new(SandboxConfig::default());

    // Exactly at the limit should pass
    let at_limit = 256 * 1024 * 1024;
    assert!(
        sandbox.check_memory(at_limit).is_ok(),
        "allocation at exactly 256 MB should be allowed"
    );

    // 1 byte over should fail
    let over_limit = at_limit + 1;
    let result = sandbox.check_memory(over_limit);
    assert!(result.is_err(), "allocation > 256 MB should be rejected");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("memory limit exceeded"),
        "error should mention memory limit, got: {err_msg}"
    );
}

#[test]
fn wasm_memory_limit_custom_config() {
    let config = SandboxConfig {
        max_memory_bytes: 64 * 1024 * 1024, // 64 MB
        ..SandboxConfig::default()
    };
    let sandbox = WasmSandbox::new(config);

    assert!(sandbox.check_memory(32 * 1024 * 1024).is_ok());
    assert!(sandbox.check_memory(65 * 1024 * 1024).is_err());
}

// ---------------------------------------------------------------------------
// Test 2: Fuel exhaustion
// ---------------------------------------------------------------------------

#[test]
fn wasm_fuel_exhaustion_rejects_over_budget() {
    let sandbox = WasmSandbox::new(SandboxConfig::default());
    let budget = sandbox.config().fuel_budget;

    // At budget should pass
    assert!(sandbox.check_fuel(budget).is_ok());

    // Over budget should fail
    let result = sandbox.check_fuel(budget + 1);
    assert!(result.is_err(), "fuel consumption over budget should fail");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("fuel budget exceeded"),
        "error should mention fuel budget, got: {err_msg}"
    );
}

#[test]
fn wasm_fuel_custom_budget() {
    let config = SandboxConfig {
        fuel_budget: 500,
        ..SandboxConfig::default()
    };
    let sandbox = WasmSandbox::new(config);

    assert!(sandbox.check_fuel(500).is_ok());
    assert!(sandbox.check_fuel(501).is_err());
}

// ---------------------------------------------------------------------------
// Test 3: Execution timeout
// ---------------------------------------------------------------------------

#[test]
fn wasm_timeout_enforcement() {
    let config = SandboxConfig {
        max_execution_time: Duration::from_secs(5),
        ..SandboxConfig::default()
    };
    let sandbox = WasmSandbox::new(config);

    // Recent start should pass
    let started = Instant::now();
    assert!(sandbox.check_timeout(started).is_ok());

    // Simulated old start should fail
    let old_start = Instant::now() - Duration::from_secs(10);
    let result = sandbox.check_timeout(old_start);
    assert!(result.is_err(), "expired execution should be rejected");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("execution timeout"));
}

// ---------------------------------------------------------------------------
// Test 4: Domain whitelist enforcement
// ---------------------------------------------------------------------------

#[test]
fn domain_whitelist_blocks_unlisted_domains() {
    let sandbox = WasmSandbox::new(SandboxConfig::default())
        .with_allowed_domains(vec!["api.openai.com".to_string()]);

    assert!(
        sandbox.is_domain_allowed("https://api.openai.com/v1/chat"),
        "whitelisted domain should be allowed"
    );
    assert!(
        !sandbox.is_domain_allowed("https://evil.com/steal"),
        "non-whitelisted domain should be blocked"
    );
    assert!(
        !sandbox.is_domain_allowed("https://api.openai.com.evil.com/v1"),
        "subdomain impersonation should be blocked"
    );
}

#[test]
fn domain_whitelist_empty_blocks_all() {
    let sandbox = WasmSandbox::new(SandboxConfig::default());
    // No domains whitelisted = block everything
    assert!(!sandbox.is_domain_allowed("https://api.example.com"));
    assert!(!sandbox.is_domain_allowed("https://localhost:8080"));
}

#[test]
fn domain_whitelist_wildcard_matching() {
    let sandbox = WasmSandbox::new(SandboxConfig::default())
        .with_allowed_domains(vec!["*.example.com".to_string()]);

    assert!(sandbox.is_domain_allowed("https://api.example.com/v1"));
    assert!(sandbox.is_domain_allowed("https://cdn.example.com/assets"));
    assert!(sandbox.is_domain_allowed("https://example.com/root"));
    assert!(!sandbox.is_domain_allowed("https://notexample.com"));
}

// ---------------------------------------------------------------------------
// Test 5: HTTP response size limit
// ---------------------------------------------------------------------------

#[test]
fn http_response_size_enforcement() {
    let config = SandboxConfig {
        max_http_response_bytes: 1024, // 1 KB
        ..SandboxConfig::default()
    };
    let sandbox = WasmSandbox::new(config);

    assert!(sandbox.check_http_response_size(512).is_ok());
    assert!(sandbox.check_http_response_size(1024).is_ok());
    assert!(sandbox.check_http_response_size(1025).is_err());
}

// ---------------------------------------------------------------------------
// Test 6: Secret isolation
// ---------------------------------------------------------------------------

#[test]
fn secret_exists_only_for_registered() {
    let sandbox = WasmSandbox::new(SandboxConfig::default())
        .with_available_secrets(vec!["ALLOWED_KEY".to_string()]);

    assert!(sandbox.secret_exists("ALLOWED_KEY"));
    assert!(!sandbox.secret_exists("AWS_SECRET_ACCESS_KEY"));
    assert!(!sandbox.secret_exists(""));
}

// ---------------------------------------------------------------------------
// Test 7: Subprocess environment cleaning
// ---------------------------------------------------------------------------

#[test]
fn subprocess_env_cleaner_blocks_sensitive_vars() {
    use std::collections::HashMap;

    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin".to_string());
    env.insert("ANTHROPIC_API_KEY".to_string(), "sk-secret".to_string());

    let result = SubprocessEnvCleaner::validate_env(&env);
    assert!(result.is_err(), "should reject env with ANTHROPIC_API_KEY");

    // Clean env should pass
    let clean = SubprocessEnvCleaner::safe_env();
    assert!(SubprocessEnvCleaner::validate_env(&clean).is_ok());
}
