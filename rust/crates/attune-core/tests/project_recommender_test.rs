//! ProjectRecommender 集成测试 — 用 in-memory Store 模拟现实推荐。

use attune_core::project_recommender::{recommend_for_chat, recommend_for_file};
use attune_core::store::Store;

/// 工具：在 in-memory store 上建一个 project 并放入若干文件归属（mock：file 内容由调用方传 entities）。
fn setup_store_with_project(project_title: &str, file_ids: Vec<&str>) -> (Store, String) {
    let store = Store::open_memory().expect("open memory store");
    let p = store
        .create_project(project_title, "generic")
        .expect("create project");
    for file_id in &file_ids {
        store
            .add_file_to_project(&p.id, file_id, "reference")
            .expect("add file");
    }
    (store, p.id)
}

#[test]
fn recommend_for_file_match_high_overlap() {
    // 用通用实体（人名 + 金额 + 日期 + 机构）测试，避免行业专属 (case_no)
    let project_entities: Vec<attune_core::entities::Entity> =
        attune_core::entities::extract_entities(
            "张三 与 北京云麓科技有限公司 签约 ¥10000 2024-03-15",
        );
    let new_file_entities = attune_core::entities::extract_entities(
        "张三 履行 ¥10000 2024-03-15 北京云麓科技有限公司 续约",
    );

    let (store, pid) = setup_store_with_project("协作项目", vec!["ev-1"]);
    let cand = recommend_for_file(
        &store,
        "new-file-1",
        &new_file_entities,
        Some(vec![(&pid, project_entities)]),
    )
    .expect("recommend");

    assert!(!cand.is_empty(), "高重叠应推荐至少 1 个 project");
    assert_eq!(cand[0].project_id, pid);
    assert!(cand[0].score >= 0.6, "应过 0.6 阈值，got {}", cand[0].score);
}

#[test]
fn recommend_for_file_no_match_low_overlap() {
    let new_entities = attune_core::entities::extract_entities("李四 签约 ¥50000");
    let other_entities = attune_core::entities::extract_entities("张三 借款 ¥10000");

    let (store, pid) = setup_store_with_project("无关项目", vec!["ev-2"]);
    let cand = recommend_for_file(
        &store,
        "new-file-2",
        &new_entities,
        Some(vec![(&pid, other_entities)]),
    )
    .expect("recommend");

    assert!(
        cand.iter().all(|c| c.score < 0.6),
        "无重叠不应推荐过阈值，got {:?}",
        cand
    );
}

#[test]
fn recommend_for_chat_with_user_keywords() {
    // 调用方（attune-server）从 PluginRegistry 聚合 keywords 后传入
    let keywords = ["项目", "客户", "topic"];
    let hit = recommend_for_chat("我现在的项目，王某 vs 李某，有几个文件要整理。", &keywords);
    assert!(hit.is_some(), "含'项目'应触发 hint");
    let h = hit.unwrap();
    assert!(h.matched_keywords.contains(&"项目".to_string()));
}

#[test]
fn recommend_for_chat_no_keyword() {
    let hit = recommend_for_chat("今天天气真好啊", &["项目", "客户"]);
    assert!(hit.is_none(), "无关键词不应触发 hint");
}

#[test]
fn recommend_for_chat_empty_keywords() {
    // OSS 裸装无 vertical plugin → keywords = [] → 永不触发
    let hit = recommend_for_chat("我的项目和客户都很重要", &[]);
    assert!(hit.is_none(), "空 keywords 列表永不触发（OSS 默认行为）");
}
