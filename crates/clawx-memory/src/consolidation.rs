//! Memory consolidation — deduplication and merging of similar memories.
//!
//! Identifies memories with similar summaries and merges them to avoid
//! redundancy. Uses simple token overlap (Jaccard similarity) for v0.1.

use clawx_types::error::Result;
use clawx_types::ids::MemoryId;
use clawx_types::memory::{ConsolidationReport, MemoryEntry, MemoryUpdate};
use clawx_types::traits::MemoryService;

/// Configuration for consolidation.
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Minimum Jaccard similarity (0.0-1.0) to consider two memories as duplicates.
    pub similarity_threshold: f64,
    /// Maximum number of memories to process per run.
    pub batch_size: usize,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.6,
            batch_size: 100,
        }
    }
}

/// Compute Jaccard similarity between two strings based on word overlap.
pub fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let lower_a = a.to_lowercase();
    let lower_b = b.to_lowercase();

    let words_a: std::collections::HashSet<&str> = lower_a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = lower_b.split_whitespace().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection = words_a.intersection(&words_b).count() as f64;
    let union = words_a.union(&words_b).count() as f64;

    intersection / union
}

/// Find pairs of similar memories from a list.
pub fn find_similar_pairs(
    memories: &[MemoryEntry],
    threshold: f64,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();

    for i in 0..memories.len() {
        for j in (i + 1)..memories.len() {
            // Only compare memories of the same scope and kind
            if memories[i].scope != memories[j].scope || memories[i].kind != memories[j].kind {
                continue;
            }
            // Skip already superseded memories
            if memories[i].superseded_by.is_some() || memories[j].superseded_by.is_some() {
                continue;
            }

            let sim = jaccard_similarity(&memories[i].summary, &memories[j].summary);
            if sim >= threshold {
                pairs.push((i, j));
            }
        }
    }

    pairs
}

/// Choose which memory to keep in a pair (the one with higher importance * freshness).
fn pick_survivor(a: &MemoryEntry, b: &MemoryEntry) -> SurvivorChoice {
    let score_a = a.importance * a.freshness;
    let score_b = b.importance * b.freshness;

    if score_a >= score_b {
        SurvivorChoice::KeepFirst
    } else {
        SurvivorChoice::KeepSecond
    }
}

enum SurvivorChoice {
    KeepFirst,
    KeepSecond,
}

/// Run consolidation on a set of memories, marking duplicates as superseded.
///
/// Returns the consolidation report with counts.
pub async fn consolidate(
    memory_service: &dyn MemoryService,
    memories: &[MemoryEntry],
    config: &ConsolidationConfig,
) -> Result<ConsolidationReport> {
    let pairs = find_similar_pairs(memories, config.similarity_threshold);

    let mut merged_count = 0u64;
    let mut superseded_ids: std::collections::HashSet<MemoryId> = std::collections::HashSet::new();

    for (i, j) in pairs {
        let a = &memories[i];
        let b = &memories[j];

        // Skip if either has already been superseded in this run
        if superseded_ids.contains(&a.id) || superseded_ids.contains(&b.id) {
            continue;
        }

        let (survivor, superseded) = match pick_survivor(a, b) {
            SurvivorChoice::KeepFirst => (a, b),
            SurvivorChoice::KeepSecond => (b, a),
        };

        // Update survivor: increase importance slightly for having confirmed info
        let new_importance = (survivor.importance + 0.5).min(10.0);
        memory_service
            .update(MemoryUpdate {
                id: survivor.id,
                summary: None,
                content: None,
                importance: Some(new_importance),
                kind: None,
            })
            .await?;

        // Mark the superseded memory by deleting it
        memory_service.delete(superseded.id).await?;

        superseded_ids.insert(superseded.id);
        merged_count += 1;
    }

    Ok(ConsolidationReport {
        merged_count,
        superseded_count: superseded_ids.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use clawx_types::ids::AgentId;
    use clawx_types::memory::*;

    fn make_memory(summary: &str, importance: f64, kind: MemoryKind) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: MemoryId::new(),
            scope: MemoryScope::Agent,
            agent_id: Some(AgentId::new()),
            kind,
            summary: summary.to_string(),
            content: serde_json::json!({"text": summary}),
            importance,
            freshness: 1.0,
            access_count: 0,
            is_pinned: false,
            source_agent_id: None,
            source_type: SourceType::Implicit,
            superseded_by: None,
            qdrant_point_id: None,
            created_at: now,
            last_accessed_at: now,
            updated_at: now,
        }
    }

    // -------------------------------------------------------------------
    // Jaccard similarity tests
    // -------------------------------------------------------------------

    #[test]
    fn jaccard_identical_strings() {
        assert_eq!(jaccard_similarity("hello world", "hello world"), 1.0);
    }

    #[test]
    fn jaccard_completely_different() {
        assert_eq!(jaccard_similarity("hello world", "foo bar baz"), 0.0);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let sim = jaccard_similarity("the user likes dark mode", "the user prefers dark mode");
        // overlap: "the", "user", "dark", "mode" = 4
        // union: "the", "user", "likes", "prefers", "dark", "mode" = 6
        // jaccard = 4/6 ≈ 0.667
        assert!(sim > 0.5 && sim < 0.8);
    }

    #[test]
    fn jaccard_empty_strings() {
        assert_eq!(jaccard_similarity("", ""), 1.0);
    }

    #[test]
    fn jaccard_one_empty() {
        assert_eq!(jaccard_similarity("hello", ""), 0.0);
    }

    #[test]
    fn jaccard_case_insensitive() {
        assert_eq!(jaccard_similarity("Hello World", "hello world"), 1.0);
    }

    // -------------------------------------------------------------------
    // find_similar_pairs tests
    // -------------------------------------------------------------------

    #[test]
    fn find_pairs_in_similar_memories() {
        let memories = vec![
            make_memory("user likes dark mode", 5.0, MemoryKind::Preference),
            make_memory("user prefers dark mode", 6.0, MemoryKind::Preference),
            make_memory("user works at Google", 7.0, MemoryKind::Fact),
        ];

        let pairs = find_similar_pairs(&memories, 0.5);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (0, 1));
    }

    #[test]
    fn find_pairs_no_matches() {
        let memories = vec![
            make_memory("user likes dark mode", 5.0, MemoryKind::Preference),
            make_memory("user works at Google", 7.0, MemoryKind::Fact),
        ];

        let pairs = find_similar_pairs(&memories, 0.5);
        assert!(pairs.is_empty());
    }

    #[test]
    fn find_pairs_different_kinds_not_matched() {
        let memories = vec![
            make_memory("user likes dark mode", 5.0, MemoryKind::Preference),
            make_memory("user likes dark mode", 5.0, MemoryKind::Fact),
        ];

        let pairs = find_similar_pairs(&memories, 0.5);
        assert!(pairs.is_empty());
    }

    #[test]
    fn find_pairs_skips_superseded() {
        let mut m1 = make_memory("user likes dark mode", 5.0, MemoryKind::Preference);
        m1.superseded_by = Some(MemoryId::new());
        let m2 = make_memory("user prefers dark mode", 6.0, MemoryKind::Preference);

        let pairs = find_similar_pairs(&[m1, m2], 0.5);
        assert!(pairs.is_empty());
    }

    // -------------------------------------------------------------------
    // consolidate tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn consolidate_merges_similar() {
        let svc = crate::StubMemoryService;
        let memories = vec![
            make_memory("user likes dark mode", 5.0, MemoryKind::Preference),
            make_memory("user prefers dark mode", 6.0, MemoryKind::Preference),
        ];

        let config = ConsolidationConfig {
            similarity_threshold: 0.5,
            batch_size: 100,
        };

        let report = consolidate(&svc, &memories, &config).await.unwrap();
        assert_eq!(report.merged_count, 1);
        assert_eq!(report.superseded_count, 1);
    }

    #[tokio::test]
    async fn consolidate_no_matches() {
        let svc = crate::StubMemoryService;
        let memories = vec![
            make_memory("user likes dark mode", 5.0, MemoryKind::Preference),
            make_memory("project uses Rust", 7.0, MemoryKind::Fact),
        ];

        let config = ConsolidationConfig::default();
        let report = consolidate(&svc, &memories, &config).await.unwrap();
        assert_eq!(report.merged_count, 0);
        assert_eq!(report.superseded_count, 0);
    }

    #[tokio::test]
    async fn consolidate_empty_list() {
        let svc = crate::StubMemoryService;
        let config = ConsolidationConfig::default();
        let report = consolidate(&svc, &[], &config).await.unwrap();
        assert_eq!(report.merged_count, 0);
    }
}
