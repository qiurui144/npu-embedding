//! 内置 workflow（Sprint 1 Phase C 范围仅含 evidence_chain_inference）
//!
//! 这些 workflow 编进 attune-core 二进制；Sprint 2 plugin loader 后可外提到 attune-law plugin。
//!
//! **当前限制**：write_annotation step 是 stub（Task 3 concern），AI 批注未真持久化。
//! Sprint 2 必须扩展 run_deterministic 签名传入 vault DEK。

use crate::workflow::schema::{parse_workflow_yaml, Workflow};

const EVIDENCE_CHAIN_YAML: &str = r#"
id: law-pro/evidence_chain_inference
type: workflow
trigger:
  on: file_added
  scope: project
steps:
  - id: extract_entities
    type: skill
    skill: law-pro/entity_extraction
    input:
      file_id: $event.file_id
    output: entities

  - id: cross_reference
    type: deterministic
    operation: find_overlap
    input:
      entities: $extract_entities.entities
      project_id: $event.project_id
    output: related_files

  - id: inference
    type: skill
    skill: law-pro/evidence_chain_skill
    input:
      new_file: $event.file_id
      related: $cross_reference.related_files
      project_id: $event.project_id
    output: inference

  - id: render
    type: deterministic
    operation: write_annotation
    input:
      item_id: $event.file_id
      project_id: $event.project_id
      body: 跨证据链推理结果（mock）
      source: ai
"#;

/// 加载内置 evidence_chain_inference workflow。
///
/// 编译期编进的字符串，**保证不 panic** — 如果 yaml 解析失败说明源代码有 bug。
pub fn evidence_chain_inference_workflow() -> Workflow {
    parse_workflow_yaml(EVIDENCE_CHAIN_YAML)
        .expect("BUILTIN evidence_chain_inference yaml must parse — fix attune-core source")
}

/// 列出全部内置 workflow（Sprint 1 仅 1 个）。
pub fn builtin_workflows() -> Vec<Workflow> {
    vec![evidence_chain_inference_workflow()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_chain_yaml_parses() {
        let wf = evidence_chain_inference_workflow();
        assert_eq!(wf.id, "law-pro/evidence_chain_inference");
        assert_eq!(wf.steps.len(), 4); // extract / cross_ref / inference / render
    }

    #[test]
    fn builtin_workflows_includes_evidence_chain() {
        let all = builtin_workflows();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "law-pro/evidence_chain_inference");
    }
}
