//! Conversation and message CRUD repository — thin layer over SQLite.

use chrono::Utc;
use clawx_types::error::{ClawxError, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Create a new conversation for the given agent.
/// Returns the conversation ID.
pub async fn create_conversation(
    pool: &SqlitePool,
    agent_id: &str,
    title: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, agent_id, title, status, created_at, updated_at)
         VALUES (?, ?, ?, 'active', ?, ?)",
    )
    .bind(&id)
    .bind(agent_id)
    .bind(title)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("create conversation: {}", e)))?;

    Ok(id)
}

/// Find a conversation by agent ID and exact title match.
pub async fn find_conversation_by_title(
    pool: &SqlitePool,
    agent_id: &str,
    title: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM conversations WHERE agent_id = ? AND title = ? LIMIT 1",
    )
    .bind(agent_id)
    .bind(title)
    .fetch_optional(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("find conversation by title: {}", e)))?;

    Ok(row.map(|(id,)| id))
}

/// Shape a conversation row (as fetched by `list_conversations`) into JSON.
fn conversation_row_to_json(
    (id, agent_id, title, status, created_at, updated_at): (
        String,
        String,
        Option<String>,
        String,
        String,
        String,
    ),
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "agent_id": agent_id,
        "title": title,
        "status": status,
        "created_at": created_at,
        "updated_at": updated_at,
    })
}

/// List conversations ordered by most recent first.
///
/// When `agent_id` is `Some`, restrict to conversations belonging to that
/// agent. When `None`, return every conversation across all agents.
///
/// Ordering is stable via `(created_at DESC, id DESC)` — the `id` tiebreaker
/// ensures deterministic output when two rows share a timestamp.
pub async fn list_conversations(
    pool: &SqlitePool,
    agent_id: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    type Row = (String, String, Option<String>, String, String, String);

    let rows: Vec<Row> = match agent_id {
        Some(id) => sqlx::query_as(
            "SELECT id, agent_id, title, status, created_at, updated_at
             FROM conversations
             WHERE agent_id = ?
             ORDER BY created_at DESC, id DESC",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("list conversations: {}", e)))?,
        None => sqlx::query_as(
            "SELECT id, agent_id, title, status, created_at, updated_at
             FROM conversations
             ORDER BY created_at DESC, id DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("list conversations: {}", e)))?,
    };

    Ok(rows.into_iter().map(conversation_row_to_json).collect())
}

/// Get a single conversation by ID.
pub async fn get_conversation(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<serde_json::Value>> {
    let row: Option<(String, String, Option<String>, String, String, String)> = sqlx::query_as(
        "SELECT id, agent_id, title, status, created_at, updated_at
         FROM conversations WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("get conversation: {}", e)))?;

    Ok(row.map(|(id, agent_id, title, status, created_at, updated_at)| {
        serde_json::json!({
            "id": id,
            "agent_id": agent_id,
            "title": title,
            "status": status,
            "created_at": created_at,
            "updated_at": updated_at,
        })
    }))
}

/// Delete a conversation and its messages by ID.
pub async fn delete_conversation(pool: &SqlitePool, id: &str) -> Result<()> {
    // Delete messages first (FK constraint)
    sqlx::query("DELETE FROM messages WHERE conversation_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("delete messages for conversation: {}", e)))?;

    let result = sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("delete conversation: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ClawxError::NotFound {
            entity: "conversation".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Add a message to a conversation.
/// Returns the message ID.
pub async fn add_message(
    pool: &SqlitePool,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(role)
    .bind(content)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("add message: {}", e)))?;

    // Update conversation updated_at
    sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(conversation_id)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update conversation timestamp: {}", e)))?;

    Ok(id)
}

/// List all messages in a conversation, ordered by creation time.
pub async fn list_messages(
    pool: &SqlitePool,
    conversation_id: &str,
) -> Result<Vec<serde_json::Value>> {
    let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, conversation_id, role, content, metadata, created_at
         FROM messages
         WHERE conversation_id = ?
         ORDER BY created_at ASC",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("list messages: {}", e)))?;

    Ok(rows
        .into_iter()
        .map(
            |(id, conversation_id, role, content, metadata, created_at)| {
                let meta = metadata
                    .and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok())
                    .unwrap_or(serde_json::Value::Null);
                serde_json::json!({
                    "id": id,
                    "conversation_id": conversation_id,
                    "role": role,
                    "content": content,
                    "metadata": meta,
                    "created_at": created_at,
                })
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Helper: create a test agent (conversations FK → agents).
    async fn seed_agent(pool: &SqlitePool, agent_id: &str) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, 'Test Agent', 'assistant', 'default', 'idle', '[]', ?, ?)",
        )
        .bind(agent_id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn create_and_get_conversation() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Hello World"))
            .await
            .unwrap();
        assert!(!conv_id.is_empty());

        let conv = get_conversation(&db.main, &conv_id).await.unwrap().unwrap();
        assert_eq!(conv["id"], conv_id);
        assert_eq!(conv["agent_id"], agent_id);
        assert_eq!(conv["title"], "Hello World");
        assert_eq!(conv["status"], "active");
    }

    #[tokio::test]
    async fn create_conversation_without_title() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, None).await.unwrap();
        let conv = get_conversation(&db.main, &conv_id).await.unwrap().unwrap();
        assert!(conv["title"].is_null());
    }

    #[tokio::test]
    async fn list_conversations_by_agent() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        create_conversation(&db.main, &agent_id, Some("First")).await.unwrap();
        create_conversation(&db.main, &agent_id, Some("Second")).await.unwrap();

        let convs = list_conversations(&db.main, Some(&agent_id)).await.unwrap();
        assert_eq!(convs.len(), 2);
    }

    #[tokio::test]
    async fn list_conversations_filters_by_agent() {
        let db = Database::in_memory().await.unwrap();
        let agent_a = Uuid::new_v4().to_string();
        let agent_b = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_a).await;
        seed_agent(&db.main, &agent_b).await;

        create_conversation(&db.main, &agent_a, Some("A's conv")).await.unwrap();
        create_conversation(&db.main, &agent_b, Some("B's conv")).await.unwrap();

        let convs_a = list_conversations(&db.main, Some(&agent_a)).await.unwrap();
        assert_eq!(convs_a.len(), 1);
        assert_eq!(convs_a[0]["title"], "A's conv");
    }

    #[tokio::test]
    async fn list_conversations_all_agents_returns_unfiltered() {
        let db = Database::in_memory().await.unwrap();
        let agent_a = Uuid::new_v4().to_string();
        let agent_b = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_a).await;
        seed_agent(&db.main, &agent_b).await;

        let older = create_conversation(&db.main, &agent_a, Some("A's conv"))
            .await
            .unwrap();
        // Small delay so the second row's created_at is strictly greater.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let newer = create_conversation(&db.main, &agent_b, Some("B's conv"))
            .await
            .unwrap();

        let convs = list_conversations(&db.main, None).await.unwrap();
        assert_eq!(convs.len(), 2, "expected both conversations, got {:?}", convs);

        // Newest-first: B's conv should come before A's conv.
        assert_eq!(convs[0]["id"], newer);
        assert_eq!(convs[1]["id"], older);

        // Both agents are represented.
        let titles: Vec<&str> = convs
            .iter()
            .map(|c| c["title"].as_str().unwrap_or_default())
            .collect();
        assert!(titles.contains(&"A's conv"));
        assert!(titles.contains(&"B's conv"));
    }

    #[tokio::test]
    async fn list_conversations_stable_tiebreak() {
        // When two rows share created_at, `ORDER BY created_at DESC, id DESC`
        // must still produce a deterministic ordering.
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        // Insert rows with IDENTICAL created_at so the timestamp cannot
        // distinguish them. The id tiebreaker must take over.
        let now = Utc::now().to_rfc3339();
        for _ in 0..5 {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO conversations
                 (id, agent_id, title, status, created_at, updated_at)
                 VALUES (?, ?, 'dup', 'active', ?, ?)",
            )
            .bind(&id)
            .bind(&agent_id)
            .bind(&now)
            .bind(&now)
            .execute(&db.main)
            .await
            .unwrap();
        }

        let first = list_conversations(&db.main, Some(&agent_id)).await.unwrap();
        let second = list_conversations(&db.main, Some(&agent_id)).await.unwrap();
        let third = list_conversations(&db.main, None).await.unwrap();

        // Same query twice must yield byte-identical ordering.
        let ids_first: Vec<&str> = first.iter().map(|c| c["id"].as_str().unwrap()).collect();
        let ids_second: Vec<&str> = second.iter().map(|c| c["id"].as_str().unwrap()).collect();
        let ids_third: Vec<&str> = third.iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert_eq!(ids_first, ids_second, "filtered ordering must be stable");
        assert_eq!(ids_first, ids_third, "filtered and unfiltered orderings must agree when only one agent exists");

        // Verify the explicit `id DESC` tiebreaker actually took effect.
        let mut sorted = ids_first.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(ids_first, sorted, "tiebreaker must be id DESC");
    }

    #[tokio::test]
    async fn get_nonexistent_conversation_returns_none() {
        let db = Database::in_memory().await.unwrap();
        let result = get_conversation(&db.main, "nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_conversation_removes_it() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("To Delete"))
            .await
            .unwrap();
        delete_conversation(&db.main, &conv_id).await.unwrap();

        let result = get_conversation(&db.main, &conv_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_conversation_also_removes_messages() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Chat"))
            .await
            .unwrap();
        add_message(&db.main, &conv_id, "user", "hello").await.unwrap();
        add_message(&db.main, &conv_id, "assistant", "hi").await.unwrap();

        delete_conversation(&db.main, &conv_id).await.unwrap();

        let msgs = list_messages(&db.main, &conv_id).await.unwrap();
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_conversation_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = delete_conversation(&db.main, "nonexistent").await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn add_and_list_messages() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Chat"))
            .await
            .unwrap();

        let msg1_id = add_message(&db.main, &conv_id, "user", "Hello!").await.unwrap();
        let msg2_id = add_message(&db.main, &conv_id, "assistant", "Hi there!")
            .await
            .unwrap();

        assert!(!msg1_id.is_empty());
        assert_ne!(msg1_id, msg2_id);

        let msgs = list_messages(&db.main, &conv_id).await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello!");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"], "Hi there!");
    }

    #[tokio::test]
    async fn list_messages_empty_conversation() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Empty"))
            .await
            .unwrap();

        let msgs = list_messages(&db.main, &conv_id).await.unwrap();
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn list_messages_nonexistent_conversation() {
        let db = Database::in_memory().await.unwrap();
        let msgs = list_messages(&db.main, "nonexistent").await.unwrap();
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn add_message_updates_conversation_timestamp() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Chat"))
            .await
            .unwrap();

        let before = get_conversation(&db.main, &conv_id).await.unwrap().unwrap();
        let before_ts = before["updated_at"].as_str().unwrap().to_string();

        // Small delay to ensure timestamp differs
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        add_message(&db.main, &conv_id, "user", "hello").await.unwrap();

        let after = get_conversation(&db.main, &conv_id).await.unwrap().unwrap();
        let after_ts = after["updated_at"].as_str().unwrap().to_string();

        assert!(after_ts >= before_ts);
    }

    #[tokio::test]
    async fn messages_ordered_by_created_at() {
        let db = Database::in_memory().await.unwrap();
        let agent_id = Uuid::new_v4().to_string();
        seed_agent(&db.main, &agent_id).await;

        let conv_id = create_conversation(&db.main, &agent_id, Some("Order test"))
            .await
            .unwrap();

        add_message(&db.main, &conv_id, "user", "first").await.unwrap();
        add_message(&db.main, &conv_id, "assistant", "second").await.unwrap();
        add_message(&db.main, &conv_id, "user", "third").await.unwrap();

        let msgs = list_messages(&db.main, &conv_id).await.unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["content"], "first");
        assert_eq!(msgs[1]["content"], "second");
        assert_eq!(msgs[2]["content"], "third");
    }
}
