//! Skill CRUD repository -- thin layer over SQLite `skills` table.

use chrono::Utc;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::SkillId;
use clawx_types::skill::{Skill, SkillManifest, SkillStatus};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Row returned by SELECT on the skills table.
#[derive(Debug, sqlx::FromRow)]
struct SkillRow {
    id: String,
    name: String,
    version: String,
    manifest: String,
    status: String,
    hash: String,
    signature: Option<String>,
    installed_at: String,
    updated_at: String,
}

impl TryFrom<SkillRow> for Skill {
    type Error = ClawxError;

    fn try_from(row: SkillRow) -> Result<Self> {
        Ok(Skill {
            id: SkillId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid skill id: {}", e)))?,
            name: row.name,
            version: row.version,
            manifest: serde_json::from_str(&row.manifest)
                .map_err(|e| ClawxError::Database(format!("invalid manifest json: {}", e)))?,
            status: SkillStatus::from_str(&row.status)
                .map_err(|e| ClawxError::Database(format!("invalid skill status: {}", e)))?,
            hash: row.hash,
            signature: row.signature,
            installed_at: chrono::DateTime::parse_from_rfc3339(&row.installed_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid installed_at: {}", e)))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid updated_at: {}", e)))?,
        })
    }
}

/// Compute SHA-256 hex digest for wasm bytes.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Install a new skill (insert into DB).
pub async fn install_skill(
    pool: &SqlitePool,
    manifest: &SkillManifest,
    wasm_bytes: &[u8],
    signature: Option<&str>,
) -> Result<Skill> {
    let id = SkillId::new();
    let now = Utc::now();
    let manifest_json = serde_json::to_string(manifest)
        .map_err(|e| ClawxError::Internal(format!("serialize manifest: {}", e)))?;
    let hash = sha256_hex(wasm_bytes);
    let installed_at = now.to_rfc3339();
    let updated_at = now.to_rfc3339();

    sqlx::query(
        "INSERT INTO skills (id, name, version, manifest, status, hash, signature, installed_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(&manifest.name)
    .bind(&manifest.version)
    .bind(&manifest_json)
    .bind(SkillStatus::default().to_string())
    .bind(&hash)
    .bind(signature)
    .bind(&installed_at)
    .bind(&updated_at)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("install skill: {}", e)))?;

    get_skill(pool, &id)
        .await?
        .ok_or_else(|| ClawxError::Internal("skill not found after insert".into()))
}

/// Get a single skill by ID.
pub async fn get_skill(pool: &SqlitePool, id: &SkillId) -> Result<Option<Skill>> {
    let row: Option<SkillRow> =
        sqlx::query_as("SELECT * FROM skills WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get skill: {}", e)))?;

    row.map(Skill::try_from).transpose()
}

/// List all installed skills.
pub async fn list_skills(pool: &SqlitePool) -> Result<Vec<Skill>> {
    let rows: Vec<SkillRow> =
        sqlx::query_as("SELECT * FROM skills ORDER BY installed_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list skills: {}", e)))?;

    rows.into_iter().map(Skill::try_from).collect()
}

/// Enable a skill.
pub async fn enable_skill(pool: &SqlitePool, id: &SkillId) -> Result<Skill> {
    set_skill_status(pool, id, SkillStatus::Enabled).await
}

/// Disable a skill.
pub async fn disable_skill(pool: &SqlitePool, id: &SkillId) -> Result<Skill> {
    set_skill_status(pool, id, SkillStatus::Disabled).await
}

/// Internal: set status and updated_at for a skill.
async fn set_skill_status(
    pool: &SqlitePool,
    id: &SkillId,
    status: SkillStatus,
) -> Result<Skill> {
    // Verify skill exists first
    let _ = get_skill(pool, id)
        .await?
        .ok_or_else(|| ClawxError::NotFound {
            entity: "skill".into(),
            id: id.to_string(),
        })?;

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE skills SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status.to_string())
        .bind(&now)
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("set skill status: {}", e)))?;

    get_skill(pool, id)
        .await?
        .ok_or_else(|| ClawxError::Internal("skill not found after status update".into()))
}

/// Uninstall (delete) a skill by ID.
pub async fn uninstall_skill(pool: &SqlitePool, id: &SkillId) -> Result<()> {
    let result = sqlx::query("DELETE FROM skills WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("uninstall skill: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ClawxError::NotFound {
            entity: "skill".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use clawx_types::skill::CapabilityDeclaration;

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

    #[tokio::test]
    async fn install_and_get_skill() {
        let db = Database::in_memory().await.unwrap();
        let manifest = make_manifest("greeting");
        let wasm = b"fake-wasm-bytes";

        let installed = install_skill(&db.main, &manifest, wasm, None).await.unwrap();
        assert_eq!(installed.name, "greeting");
        assert_eq!(installed.version, "1.0.0");
        assert_eq!(installed.status, SkillStatus::Enabled);
        assert!(!installed.hash.is_empty());
        assert!(installed.signature.is_none());

        let fetched = get_skill(&db.main, &installed.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, installed.id);
        assert_eq!(fetched.manifest.entrypoint, "main.wasm");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let db = Database::in_memory().await.unwrap();
        let result = get_skill(&db.main, &SkillId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_skills_returns_all() {
        let db = Database::in_memory().await.unwrap();
        install_skill(&db.main, &make_manifest("a"), b"a", None).await.unwrap();
        install_skill(&db.main, &make_manifest("b"), b"b", None).await.unwrap();

        let skills = list_skills(&db.main).await.unwrap();
        assert_eq!(skills.len(), 2);
    }

    #[tokio::test]
    async fn disable_and_enable_skill() {
        let db = Database::in_memory().await.unwrap();
        let installed = install_skill(&db.main, &make_manifest("toggle"), b"wasm", None)
            .await
            .unwrap();
        assert_eq!(installed.status, SkillStatus::Enabled);

        let disabled = disable_skill(&db.main, &installed.id).await.unwrap();
        assert_eq!(disabled.status, SkillStatus::Disabled);

        let enabled = enable_skill(&db.main, &installed.id).await.unwrap();
        assert_eq!(enabled.status, SkillStatus::Enabled);
    }

    #[tokio::test]
    async fn uninstall_removes_skill() {
        let db = Database::in_memory().await.unwrap();
        let installed = install_skill(&db.main, &make_manifest("remove-me"), b"wasm", None)
            .await
            .unwrap();

        uninstall_skill(&db.main, &installed.id).await.unwrap();
        let fetched = get_skill(&db.main, &installed.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn uninstall_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = uninstall_skill(&db.main, &SkillId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn install_with_signature() {
        let db = Database::in_memory().await.unwrap();
        let manifest = make_manifest("signed-skill");
        let sig = "sha256:abcdef1234567890";

        let installed = install_skill(&db.main, &manifest, b"wasm", Some(sig))
            .await
            .unwrap();
        assert_eq!(installed.signature.as_deref(), Some(sig));
    }

    #[tokio::test]
    async fn hash_is_deterministic_for_same_bytes() {
        let db = Database::in_memory().await.unwrap();
        let wasm = b"deterministic-content";

        let s1 = install_skill(&db.main, &make_manifest("s1"), wasm, None)
            .await
            .unwrap();
        let s2 = install_skill(&db.main, &make_manifest("s2"), wasm, None)
            .await
            .unwrap();

        assert_eq!(s1.hash, s2.hash);
        assert!(!s1.hash.is_empty());
    }

    #[tokio::test]
    async fn manifest_roundtrips_as_json() {
        let db = Database::in_memory().await.unwrap();
        let mut manifest = make_manifest("caps-test");
        manifest.capabilities.net_http = vec!["https://api.example.com/*".to_string()];
        manifest.capabilities.secrets = vec!["API_KEY".to_string()];

        let installed = install_skill(&db.main, &manifest, b"wasm", None)
            .await
            .unwrap();

        let fetched = get_skill(&db.main, &installed.id).await.unwrap().unwrap();
        assert_eq!(fetched.manifest.capabilities.net_http, vec!["https://api.example.com/*"]);
        assert_eq!(fetched.manifest.capabilities.secrets, vec!["API_KEY"]);
    }

    #[tokio::test]
    async fn enable_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = enable_skill(&db.main, &SkillId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }
}
