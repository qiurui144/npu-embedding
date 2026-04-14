// npu-vault/crates/vault-core/src/search.rs

use std::collections::HashMap;

use crate::vectors::VectorIndex;

/// RRF 参数
pub const RRF_K: f32 = 60.0;
pub const RERANK_VECTOR_WEIGHT: f32 = 0.7;
pub const RERANK_RRF_WEIGHT: f32 = 0.3;
pub const RERANK_TOP_K_THRESHOLD: usize = 20;
pub const DEFAULT_VECTOR_WEIGHT: f32 = 0.6;
pub const DEFAULT_FULLTEXT_WEIGHT: f32 = 0.4;
pub const INJECTION_BUDGET: usize = 2000;

/// 搜索结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub item_id: String,
    pub score: f32,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub inject_content: Option<String>,
}

/// RRF 融合两组排名结果
pub fn rrf_fuse(
    vector_results: &[(String, f32)],
    fulltext_results: &[(String, f32)],
    vector_weight: f32,
    fulltext_weight: f32,
    top_k: usize,
) -> Vec<(String, f32)> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for (rank, (id, _score)) in vector_results.iter().enumerate() {
        let rrf = vector_weight / (RRF_K + rank as f32 + 1.0);
        *scores.entry(id.clone()).or_default() += rrf;
    }
    for (rank, (id, _score)) in fulltext_results.iter().enumerate() {
        let rrf = fulltext_weight / (RRF_K + rank as f32 + 1.0);
        *scores.entry(id.clone()).or_default() += rrf;
    }

    let mut sorted: Vec<(String, f32)> = scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(top_k);
    sorted
}

/// 动态注入预算分配
pub fn allocate_budget(results: &mut [SearchResult], budget: usize) {
    let total_score: f32 = results.iter().map(|r| r.score).sum();
    if total_score <= 0.0 || results.is_empty() {
        let per_item = budget / results.len().max(1);
        for r in results.iter_mut() {
            let content = &r.content;
            let end = content.char_indices()
                .nth(per_item)
                .map(|(i, _)| i)
                .unwrap_or(content.len());
            r.inject_content = Some(content[..end].to_string());
        }
        return;
    }
    for r in results.iter_mut() {
        let share = r.score / total_score;
        let alloc = (budget as f32 * share).max(100.0) as usize;
        let content = &r.content;
        let end = content.char_indices()
            .nth(alloc)
            .map(|(i, _)| i)
            .unwrap_or(content.len());
        r.inject_content = Some(content[..end].to_string());
    }
}

/// 计算两个向量的余弦相似度，任一范数为 0 时返回 0.0
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "cosine_similarity: dimension mismatch");
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-8 || norm_b < 1e-8 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// 对 RRF 一阶结果进行余弦相似度二次排序。
///
/// 当 query 向量可用且结果集不超过 `RERANK_TOP_K_THRESHOLD` 时调用。
/// 原地修改 `results` 的 `score` 字段并重新排序。
pub fn rerank(
    query_vec: &[f32],
    results: &mut Vec<SearchResult>,
    vector_index: &VectorIndex,
) {
    for result in results.iter_mut() {
        let rrf_score = result.score;
        let rerank_score = vector_index
            .get_vector(&result.item_id)
            .map(|item_vec| cosine_similarity(query_vec, &item_vec))
            .unwrap_or(0.0);
        result.score = RERANK_VECTOR_WEIGHT * rerank_score + RERANK_RRF_WEIGHT * rrf_score;
    }
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_fuse_basic() {
        let vec_results = vec![
            ("a".into(), 0.9), ("b".into(), 0.7), ("c".into(), 0.5),
        ];
        let ft_results = vec![
            ("b".into(), 10.0), ("a".into(), 8.0), ("d".into(), 5.0),
        ];

        let fused = rrf_fuse(&vec_results, &ft_results, 0.6, 0.4, 10);
        assert!(!fused.is_empty());
        // "a" 和 "b" 在两个列表中都出现，应该排名靠前
        let top_ids: Vec<&str> = fused.iter().map(|(id, _)| id.as_str()).collect();
        assert!(top_ids.contains(&"a"));
        assert!(top_ids.contains(&"b"));
    }

    #[test]
    fn rrf_fuse_empty() {
        let fused = rrf_fuse(&[], &[], 0.6, 0.4, 10);
        assert!(fused.is_empty());
    }

    #[test]
    fn rrf_fuse_single_source() {
        let vec_results = vec![("a".into(), 0.9)];
        let fused = rrf_fuse(&vec_results, &[], 0.6, 0.4, 10);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].0, "a");
    }

    #[test]
    fn allocate_budget_proportional() {
        let mut results = vec![
            SearchResult {
                item_id: "a".into(), score: 0.8, title: "A".into(),
                content: "A".repeat(3000), source_type: "note".into(), inject_content: None,
            },
            SearchResult {
                item_id: "b".into(), score: 0.2, title: "B".into(),
                content: "B".repeat(3000), source_type: "note".into(), inject_content: None,
            },
        ];
        allocate_budget(&mut results, 2000);

        let a_len = results[0].inject_content.as_ref().unwrap().chars().count();
        let b_len = results[1].inject_content.as_ref().unwrap().chars().count();
        // "a" has 80% score, should get ~1600 chars; "b" has 20%, should get ~400 (min 100)
        assert!(a_len > b_len, "Higher score should get more budget: a={a_len} b={b_len}");
        assert!(b_len >= 100, "Minimum budget should be 100: got {b_len}");
    }

    #[test]
    fn allocate_budget_zero_scores() {
        let mut results = vec![
            SearchResult {
                item_id: "a".into(), score: 0.0, title: "A".into(),
                content: "A".repeat(3000), source_type: "note".into(), inject_content: None,
            },
            SearchResult {
                item_id: "b".into(), score: 0.0, title: "B".into(),
                content: "B".repeat(3000), source_type: "note".into(), inject_content: None,
            },
        ];
        allocate_budget(&mut results, 2000);
        // Equal distribution when scores are 0
        let a_len = results[0].inject_content.as_ref().unwrap().chars().count();
        let b_len = results[1].inject_content.as_ref().unwrap().chars().count();
        assert_eq!(a_len, b_len, "Equal scores should get equal budget");
    }

    #[test]
    fn cosine_similarity_basic() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-5);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-5);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn rerank_orders_by_cosine() {
        use crate::vectors::{VectorIndex, VectorMeta};

        let mut idx = VectorIndex::new(2).unwrap();
        idx.add(&[1.0, 0.0], VectorMeta { item_id: "close".into(), chunk_idx: 0, level: 2, section_idx: 0 }).unwrap();
        idx.add(&[0.0, 1.0], VectorMeta { item_id: "far".into(), chunk_idx: 0, level: 2, section_idx: 0 }).unwrap();

        let mut results = vec![
            SearchResult { item_id: "far".into(),   score: 0.9, title: "Far".into(),   content: "c".into(), source_type: "note".into(), inject_content: None },
            SearchResult { item_id: "close".into(), score: 0.5, title: "Close".into(), content: "c".into(), source_type: "note".into(), inject_content: None },
        ];

        rerank(&[1.0, 0.0], &mut results, &idx);
        assert_eq!(results[0].item_id, "close", "Reranker should elevate closer vector");
    }

    #[test]
    fn rerank_fallback_when_no_vector() {
        use crate::vectors::VectorIndex;

        let idx = VectorIndex::new(2).unwrap();
        let mut results = vec![
            SearchResult { item_id: "a".into(), score: 0.8, title: "A".into(), content: "c".into(), source_type: "note".into(), inject_content: None },
            SearchResult { item_id: "b".into(), score: 0.3, title: "B".into(), content: "c".into(), source_type: "note".into(), inject_content: None },
        ];
        rerank(&[1.0, 0.0], &mut results, &idx);
        assert!(results[0].score >= results[1].score);
    }
}
