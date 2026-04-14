//! Tests for memory vector index (Phase 9.4).
//!
//! Written test-first following TDD Red-Green-Refactor.

use clawx_kb::qdrant::{QdrantStore, StubEmbeddingService};
use clawx_memory::vector_index::VectorMemoryIndex;

fn make_index() -> VectorMemoryIndex {
    let store = QdrantStore::new("memories", Box::new(StubEmbeddingService::new(64)));
    VectorMemoryIndex::new(store)
}

// ---------------------------------------------------------------------------
// 1. Index 3 memories, search finds most relevant
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_index_and_search_memory() {
    let index = make_index();

    // Index three memories with distinct content
    index
        .index_memory("mem-001", "Rust is a systems programming language")
        .await
        .unwrap();
    index
        .index_memory("mem-002", "Python is great for data science")
        .await
        .unwrap();
    index
        .index_memory("mem-003", "Cooking Italian pasta recipes")
        .await
        .unwrap();

    // Search for something closely related to mem-001
    let results = index
        .search_memories("Rust systems programming", 3)
        .await
        .unwrap();

    assert_eq!(results.len(), 3, "should return top_k results");
    // The top result should be the most semantically similar
    assert_eq!(
        results[0].memory_id, "mem-001",
        "Rust memory should rank first for Rust query"
    );
    // Scores should be in descending order
    for w in results.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "results must be sorted by score descending"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Hybrid recall combines FTS and vector results via RRF
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_hybrid_recall_combines_fts_and_vector() {
    let index = make_index();

    // Index several memories
    index.index_memory("mem-a", "machine learning algorithms").await.unwrap();
    index.index_memory("mem-b", "deep learning neural networks").await.unwrap();
    index.index_memory("mem-c", "gardening tips for spring").await.unwrap();
    index.index_memory("mem-d", "reinforcement learning agents").await.unwrap();

    // Simulate FTS5 results: mem-a ranked 1st, mem-d ranked 2nd (keyword match on "learning")
    let fts_results = vec![
        ("mem-a".to_string(), 0.9_f32),
        ("mem-d".to_string(), 0.7),
    ];

    let hybrid = index
        .hybrid_recall(&fts_results, "deep learning neural network model", 4)
        .await
        .unwrap();

    // hybrid_recall should return results that appear in either FTS or vector
    assert!(!hybrid.is_empty(), "hybrid recall must not be empty");
    // mem-b should appear since it's highly relevant to the vector query
    let ids: Vec<&str> = hybrid.iter().map(|r| r.0.as_str()).collect();
    assert!(
        ids.contains(&"mem-b"),
        "vector-similar mem-b should appear in hybrid results"
    );
    // FTS results should also be present
    assert!(
        ids.contains(&"mem-a"),
        "FTS result mem-a should appear in hybrid results"
    );
}

// ---------------------------------------------------------------------------
// 3. Delete memory from index
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_delete_memory_from_index() {
    let index = make_index();

    index.index_memory("mem-x", "temporary memory content").await.unwrap();
    assert_eq!(index.count().await, 1);

    let deleted = index.delete_memory("mem-x").await.unwrap();
    assert!(deleted, "delete should return true for existing memory");
    assert_eq!(index.count().await, 0);

    // Searching after delete should return empty
    let results = index.search_memories("temporary", 5).await.unwrap();
    assert!(results.is_empty(), "no results after deletion");

    // Deleting again should return false
    let deleted_again = index.delete_memory("mem-x").await.unwrap();
    assert!(!deleted_again, "second delete should return false");
}

// ---------------------------------------------------------------------------
// 4. Search on empty index returns empty
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_search_empty_index() {
    let index = make_index();

    let results = index.search_memories("anything at all", 10).await.unwrap();
    assert!(results.is_empty(), "empty index should return no results");
}

// ---------------------------------------------------------------------------
// 5. Indexing duplicate ID updates (upserts) rather than duplicating
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_index_duplicate_updates() {
    let index = make_index();

    index
        .index_memory("mem-dup", "original content about cats")
        .await
        .unwrap();
    assert_eq!(index.count().await, 1);

    // Re-index with updated content
    index
        .index_memory("mem-dup", "updated content about dogs")
        .await
        .unwrap();
    assert_eq!(index.count().await, 1, "upsert should not create duplicate");

    // Search should find the updated content
    let results = index.search_memories("dogs", 1).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].memory_id, "mem-dup");
    assert_eq!(results[0].content, "updated content about dogs");
}
