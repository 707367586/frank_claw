//! SkillLoader: WASM skill verification, installation, and sandbox preparation.

use clawx_types::error::{ClawxError, Result};
use clawx_types::skill::{Skill, SkillManifest};
use clawx_security::signing::verify_skill_signature;
use clawx_security::wasm_sandbox::{SandboxConfig, WasmSandbox};
use sqlx::SqlitePool;

/// Configuration for skill loading.
pub struct SkillLoaderConfig {
    /// Optional Ed25519 public key hex for signature verification.
    pub public_key_hex: Option<String>,
    /// Whether to require valid signatures. Default: false (allow unsigned).
    pub require_signature: bool,
    /// Sandbox configuration override.
    pub sandbox_config: SandboxConfig,
}

impl Default for SkillLoaderConfig {
    fn default() -> Self {
        Self {
            public_key_hex: None,
            require_signature: false,
            sandbox_config: SandboxConfig::default(),
        }
    }
}

/// SkillLoader: validates, verifies, and prepares skills for sandboxed execution.
pub struct SkillLoader {
    config: SkillLoaderConfig,
    pool: SqlitePool,
}

impl SkillLoader {
    pub fn new(pool: SqlitePool, config: SkillLoaderConfig) -> Self {
        Self { config, pool }
    }

    /// Install a skill: validate manifest -> verify signature -> store in DB -> return Skill.
    pub async fn install(
        &self,
        manifest: &SkillManifest,
        wasm_bytes: &[u8],
        signature: Option<&str>,
    ) -> Result<Skill> {
        // 1. Validate manifest
        self.validate_manifest(manifest)?;

        // 2. Verify signature if provided or required
        if let Some(sig) = signature {
            if let Some(ref pk) = self.config.public_key_hex {
                let valid = verify_skill_signature(wasm_bytes, sig, pk)?;
                if !valid {
                    return Err(ClawxError::Skill("invalid skill signature".into()));
                }
            }
        } else if self.config.require_signature {
            return Err(ClawxError::Skill("signature required but not provided".into()));
        }

        // 3. Store in DB
        let skill = crate::skill_repo::install_skill(
            &self.pool,
            manifest,
            wasm_bytes,
            signature,
        ).await?;

        Ok(skill)
    }

    /// Load a skill and prepare a sandbox for execution based on its manifest capabilities.
    pub async fn load(&self, skill: &Skill) -> Result<WasmSandbox> {
        let sandbox = WasmSandbox::new(self.config.sandbox_config.clone())
            .with_allowed_domains(skill.manifest.capabilities.net_http.clone())
            .with_available_secrets(skill.manifest.capabilities.secrets.clone());

        Ok(sandbox)
    }

    /// Validate a skill manifest before installation.
    fn validate_manifest(&self, manifest: &SkillManifest) -> Result<()> {
        if manifest.name.is_empty() {
            return Err(ClawxError::Validation("skill name cannot be empty".into()));
        }
        if manifest.version.is_empty() {
            return Err(ClawxError::Validation("skill version cannot be empty".into()));
        }
        if manifest.entrypoint.is_empty() {
            return Err(ClawxError::Validation("skill entrypoint cannot be empty".into()));
        }
        if !manifest.entrypoint.ends_with(".wasm") {
            return Err(ClawxError::Validation("skill entrypoint must be a .wasm file".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use clawx_types::skill::CapabilityDeclaration;
    use clawx_security::signing::{generate_keypair, sign_skill};
    use std::time::Duration;

    fn make_manifest(name: &str) -> SkillManifest {
        SkillManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: Some(format!("{} skill", name)),
            author: Some("test-author".to_string()),
            entrypoint: "main.wasm".to_string(),
            capabilities: CapabilityDeclaration::default(),
        }
    }

    fn make_loader(pool: SqlitePool, config: SkillLoaderConfig) -> SkillLoader {
        SkillLoader::new(pool, config)
    }

    // ----- install tests -----

    #[tokio::test]
    async fn install_skill_stores_in_db() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());
        let manifest = make_manifest("greeting");

        let skill = loader.install(&manifest, b"fake-wasm", None).await.unwrap();
        assert_eq!(skill.name, "greeting");
        assert_eq!(skill.version, "1.0.0");

        // Verify it's actually in the DB
        let fetched = crate::skill_repo::get_skill(&db.main, &skill.id)
            .await
            .unwrap()
            .expect("skill should exist in DB");
        assert_eq!(fetched.id, skill.id);
    }

    #[tokio::test]
    async fn install_skill_with_valid_signature() {
        let db = Database::in_memory().await.unwrap();
        let (pk_hex, sk_hex) = generate_keypair();
        let wasm = b"signed-wasm-bytes";
        let sig_hex = sign_skill(wasm, &sk_hex).unwrap();

        let config = SkillLoaderConfig {
            public_key_hex: Some(pk_hex),
            require_signature: true,
            sandbox_config: SandboxConfig::default(),
        };
        let loader = make_loader(db.main.clone(), config);
        let manifest = make_manifest("signed-skill");

        let skill = loader.install(&manifest, wasm, Some(&sig_hex)).await.unwrap();
        assert_eq!(skill.name, "signed-skill");
        assert_eq!(skill.signature.as_deref(), Some(sig_hex.as_str()));
    }

    #[tokio::test]
    async fn install_skill_with_invalid_signature_fails() {
        let db = Database::in_memory().await.unwrap();
        let (pk_hex, _sk_hex) = generate_keypair();
        let wasm = b"wasm-bytes";
        // Use a different key to sign (generates wrong signature)
        let (_other_pk, other_sk) = generate_keypair();
        let bad_sig = sign_skill(wasm, &other_sk).unwrap();

        let config = SkillLoaderConfig {
            public_key_hex: Some(pk_hex),
            require_signature: true,
            sandbox_config: SandboxConfig::default(),
        };
        let loader = make_loader(db.main.clone(), config);

        let result = loader.install(&make_manifest("bad-sig"), wasm, Some(&bad_sig)).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Skill(msg)) => assert!(msg.contains("invalid skill signature")),
            other => panic!("expected Skill error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn install_skill_requires_signature_when_configured() {
        let db = Database::in_memory().await.unwrap();
        let config = SkillLoaderConfig {
            public_key_hex: Some("deadbeef".repeat(4)), // 32 bytes hex
            require_signature: true,
            sandbox_config: SandboxConfig::default(),
        };
        let loader = make_loader(db.main.clone(), config);

        let result = loader.install(&make_manifest("unsigned"), b"wasm", None).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Skill(msg)) => assert!(msg.contains("signature required")),
            other => panic!("expected Skill error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn install_unsigned_skill_allowed_by_default() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());

        let skill = loader.install(&make_manifest("unsigned-ok"), b"wasm", None).await.unwrap();
        assert_eq!(skill.name, "unsigned-ok");
        assert!(skill.signature.is_none());
    }

    // ----- validate_manifest tests -----

    #[tokio::test]
    async fn validate_manifest_empty_name_fails() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());
        let mut manifest = make_manifest("test");
        manifest.name = String::new();

        let result = loader.install(&manifest, b"wasm", None).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Validation(msg)) => assert!(msg.contains("name cannot be empty")),
            other => panic!("expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn validate_manifest_empty_version_fails() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());
        let mut manifest = make_manifest("test");
        manifest.version = String::new();

        let result = loader.install(&manifest, b"wasm", None).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Validation(msg)) => assert!(msg.contains("version cannot be empty")),
            other => panic!("expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn validate_manifest_non_wasm_entrypoint_fails() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());
        let mut manifest = make_manifest("test");
        manifest.entrypoint = "main.js".to_string();

        let result = loader.install(&manifest, b"wasm", None).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Validation(msg)) => assert!(msg.contains("must be a .wasm file")),
            other => panic!("expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn validate_manifest_empty_entrypoint_fails() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());
        let mut manifest = make_manifest("test");
        manifest.entrypoint = String::new();

        let result = loader.install(&manifest, b"wasm", None).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Validation(msg)) => assert!(msg.contains("entrypoint cannot be empty")),
            other => panic!("expected Validation error, got: {:?}", other),
        }
    }

    // ----- load tests -----

    #[tokio::test]
    async fn load_skill_creates_sandbox_with_capabilities() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());

        let mut manifest = make_manifest("net-skill");
        manifest.capabilities.net_http = vec!["api.example.com".to_string()];
        manifest.capabilities.secrets = vec!["API_KEY".to_string()];

        let skill = loader.install(&manifest, b"wasm", None).await.unwrap();
        let sandbox = loader.load(&skill).await.unwrap();

        assert!(sandbox.is_domain_allowed("https://api.example.com/v1"));
        assert!(!sandbox.is_domain_allowed("https://evil.com"));
        assert!(sandbox.secret_exists("API_KEY"));
        assert!(!sandbox.secret_exists("OTHER_KEY"));
    }

    #[tokio::test]
    async fn load_skill_sandbox_has_default_config() {
        let db = Database::in_memory().await.unwrap();
        let loader = make_loader(db.main.clone(), SkillLoaderConfig::default());

        let manifest = make_manifest("simple-skill");
        let skill = loader.install(&manifest, b"wasm", None).await.unwrap();
        let sandbox = loader.load(&skill).await.unwrap();

        let config = sandbox.config();
        assert_eq!(config.max_memory_bytes, 256 * 1024 * 1024);
        assert_eq!(config.max_execution_time, Duration::from_secs(30));
        assert_eq!(config.fuel_budget, 1_000_000);
    }

    #[tokio::test]
    async fn load_skill_sandbox_with_custom_config() {
        let db = Database::in_memory().await.unwrap();
        let custom_config = SandboxConfig {
            max_memory_bytes: 64 * 1024 * 1024,
            max_execution_time: Duration::from_secs(10),
            fuel_budget: 500_000,
            max_http_response_bytes: 1024 * 1024,
        };
        let loader_config = SkillLoaderConfig {
            sandbox_config: custom_config,
            ..SkillLoaderConfig::default()
        };
        let loader = make_loader(db.main.clone(), loader_config);

        let manifest = make_manifest("custom-skill");
        let skill = loader.install(&manifest, b"wasm", None).await.unwrap();
        let sandbox = loader.load(&skill).await.unwrap();

        let config = sandbox.config();
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(config.max_execution_time, Duration::from_secs(10));
        assert_eq!(config.fuel_budget, 500_000);
    }
}
