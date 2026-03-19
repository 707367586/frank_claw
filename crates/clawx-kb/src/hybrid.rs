//! Reciprocal Rank Fusion (RRF) for combining search results from multiple sources.

use std::collections::HashMap;

/// Combine two ranked result lists using Reciprocal Rank Fusion.
///
/// Each input is a list of `(chunk_id, score)` pairs ordered by descending score.
/// The RRF formula: `rrf_score = sum(1 / (k + rank_i))` where `rank_i` is the
/// 1-based rank in each result list.
///
/// Returns `(chunk_id, rrf_score)` pairs sorted by descending RRF score, limited to `top_n`.
pub fn rrf_fusion(
    results_a: Vec<(String, f64)>,
    results_b: Vec<(String, f64)>,
    k: f64,
    top_n: usize,
) -> Vec<(String, f64)> {
    let mut scores: HashMap<String, f64> = HashMap::new();

    for (rank_0, (chunk_id, _score)) in results_a.into_iter().enumerate() {
        let rank = (rank_0 + 1) as f64; // 1-based
        *scores.entry(chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    for (rank_0, (chunk_id, _score)) in results_b.into_iter().enumerate() {
        let rank = (rank_0 + 1) as f64;
        *scores.entry(chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    let mut fused: Vec<(String, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused.truncate(top_n);
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_fusion_basic() {
        // Two lists with overlapping chunk_ids
        let a = vec![
            ("c1".to_string(), 10.0),
            ("c2".to_string(), 8.0),
            ("c3".to_string(), 5.0),
        ];
        let b = vec![
            ("c2".to_string(), 12.0),
            ("c1".to_string(), 7.0),
            ("c4".to_string(), 3.0),
        ];

        let fused = rrf_fusion(a, b, 60.0, 10);

        // c1 appears at rank 1 in A and rank 2 in B => 1/61 + 1/62
        // c2 appears at rank 2 in A and rank 1 in B => 1/62 + 1/61
        // So c1 and c2 should have the same score
        assert!(fused.len() == 4);

        let c1_score = fused.iter().find(|(id, _)| id == "c1").unwrap().1;
        let c2_score = fused.iter().find(|(id, _)| id == "c2").unwrap().1;
        assert!((c1_score - c2_score).abs() < 1e-10, "c1 and c2 should have equal RRF scores");

        // c3 only in A at rank 3 => 1/63
        let c3_score = fused.iter().find(|(id, _)| id == "c3").unwrap().1;
        assert!((c3_score - 1.0 / 63.0).abs() < 1e-10);

        // c4 only in B at rank 3 => 1/63
        let c4_score = fused.iter().find(|(id, _)| id == "c4").unwrap().1;
        assert!((c4_score - 1.0 / 63.0).abs() < 1e-10);
    }

    #[test]
    fn test_rrf_fusion_disjoint() {
        let a = vec![("c1".to_string(), 5.0), ("c2".to_string(), 3.0)];
        let b = vec![("c3".to_string(), 9.0), ("c4".to_string(), 1.0)];

        let fused = rrf_fusion(a, b, 60.0, 10);
        assert_eq!(fused.len(), 4);

        // All items appear in exactly one list at either rank 1 or rank 2
        // rank 1 items: c1 (1/61) and c3 (1/61) — same score
        // rank 2 items: c2 (1/62) and c4 (1/62) — same score
        let c1 = fused.iter().find(|(id, _)| id == "c1").unwrap().1;
        let c3 = fused.iter().find(|(id, _)| id == "c3").unwrap().1;
        assert!((c1 - c3).abs() < 1e-10);

        let c2 = fused.iter().find(|(id, _)| id == "c2").unwrap().1;
        let c4 = fused.iter().find(|(id, _)| id == "c4").unwrap().1;
        assert!((c2 - c4).abs() < 1e-10);
        assert!(c1 > c2);
    }

    #[test]
    fn test_rrf_fusion_empty() {
        let fused = rrf_fusion(vec![], vec![], 60.0, 10);
        assert!(fused.is_empty());

        // One empty, one non-empty
        let a = vec![("c1".to_string(), 5.0)];
        let fused = rrf_fusion(a, vec![], 60.0, 10);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].0, "c1");
    }
}
