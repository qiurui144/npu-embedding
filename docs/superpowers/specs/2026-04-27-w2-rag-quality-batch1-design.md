# W2 RAG Quality Batch 1 (J1 + J3 + J5 + B1 backend) Design

**Date**: 2026-04-27
**Roadmap**: 12-week strategy v4, Phase 1 W2 F-P0b
**Depends on**: H1 governor (commit `2bc558c`), A1 memory (`71a714f`)
**Depended by**: J6 public benchmark (W4), B1 frontend highlight (next session), J2 dynamic window (W5-6)

[English](2026-04-27-w2-rag-quality-batch1-design.md) · [简体中文](2026-04-27-w2-rag-quality-batch1-design.zh.md)

---

## 1. Why This Batch

W1 closed the foundation (H1 governor + A1 memory). W2 is the first **user-facing RAG quality** push — closing the gap between "Demo runs" and "production-grade good". Per 吴师兄 article (see References §A) the four most leveraged hardening points are:

| Lever | attune Status | This batch |
|-------|---------------|-----------|
| Chunk breadcrumb path | extract_sections splits but **no path injection** | **J1** |
| Explicit recall threshold | RRF fusion, no cosine cutoff | **J3** |
| Strict prompt + confidence | temperate "answer if you can" prompt | **J5** |
| Citation deep-link with source coordinates | citations have no offsets | **B1 backend** |

Frontend (B1 highlight UI, H2/H3 settings UI, D1 toggle) is deferred — those need Tauri + i18n framework which is W5 work.

## 2. Module-by-Module Spec

### J1 — Chunk Breadcrumb Path Prefix

**File**: `rust/crates/attune-core/src/chunker.rs`

**Current**: `extract_sections(content: &str) -> Vec<(usize, String)>` returns `(section_idx, raw_section_text)`.

**New**: `extract_sections_with_path(content: &str) -> Vec<SectionWithPath>` where:

```rust
pub struct SectionWithPath {
    pub section_idx: usize,
    /// Heading hierarchy from document root, e.g. ["Title", "Chapter 3", "3.2 Waiting Period"].
    /// Empty Vec for content before any heading.
    pub path: Vec<String>,
    pub content: String,
}
```

The original `extract_sections` is **retained** for backwards compatibility (existing callers in test code and elsewhere). New callers use `extract_sections_with_path`.

**Heading detection**: Markdown `#`/`##`/`###` (depth = count of `#`); code `def`/`class`/`fn`/`pub fn`/`impl` treated as same-depth peers (depth=1) since code structure rarely nests deeply.

**Path stack maintenance**: track current depth, pop deeper-or-equal entries before pushing.

**Where the breadcrumb is injected into chunk text**: Caller (e.g. indexer pipeline) prepends `> ` lines:

```
> Title > Chapter 3 > 3.2 Waiting Period

[original section content]
```

The `>` prefix uses Markdown blockquote so LLM prompts read naturally. This pattern is per 吴师兄 §1.

### J3 — Explicit Cosine Threshold

**File**: `rust/crates/attune-core/src/search.rs`

**Current**: `SearchParams` has `top_k`, `vector_weight`, `fulltext_weight` but **no min cosine score** for vector results. RRF fusion is unconditional.

**New**: Add `min_score: Option<f32>` to `SearchParams`. Default `Some(0.65)` — the conservative end of 吴师兄's 0.65/0.72/0.78 curve, balancing recall vs precision. Vector results below threshold are filtered **before** RRF. BM25 results are not affected (BM25 score is not normalized to [0,1]; filtering would need separate calibration).

Settings exposes `cosine_threshold` field; default 0.65; UI control in W5+ (this batch only adds the backend).

**Snapshot test**: Insert 3 vector results with scores [0.50, 0.70, 0.85]. With threshold 0.65, expect 2 results (0.70 + 0.85). With 0.78, expect 1 (0.85). With 0.55, expect 3.

### J5 — Strict Prompt + Confidence + Secondary Retrieval

**File**: `rust/crates/attune-core/src/chat.rs`

**Current**: `build_rag_system_prompt` is permissive: "answer if knowledge present, else don't fabricate". No anti-fabrication rules, no confidence ask, no secondary retrieval.

**New** — three sub-changes:

#### J5.a Strict prompt (per 吴师兄 §4 + Self-RAG token concept)

Replace the permissive intro with explicit constraints (Chinese, since prompt is Chinese):

```text
你是用户的个人知识助手。请严格基于以下文档回答用户问题。

【硬性规则】
1. 只用文档中的信息，不要补充推理
2. 文档无明确答案 → 回复"知识库中暂无相关信息"
3. 禁用模糊措辞："可能" "大概" "建议咨询" "或许" "应该"
4. 引用必带来源：[文档标题 > 路径]
5. 回答末尾必须输出【置信度: N/5】（5=完全确定，1=高度不确定）

文档内容：
[1] 《标题》(来源: file, 路径: > A > B)
...
```

The `> A > B` path comes from J1 breadcrumb.

#### J5.b Confidence parsing

After LLM response, parse `【置信度: N/5】` (or English fallback `[Confidence: N/5]`) with regex. If absent, default to 3 (neutral). Stripped from final response shown to user (returned as `confidence: u8` in `ChatResponse` struct).

#### J5.c Secondary retrieval (per CRAG §3.2)

If `confidence < 3`, ChatEngine triggers ONE secondary retrieval with `min_score` lowered to `0.55` (broader recall). Re-runs LLM once with the expanded context. Marks `secondary_retrieval_used: true` in response. **Hard cap one retry** — no infinite loops.

### B1 backend — Citation char offset + breadcrumb

**File**: `rust/crates/attune-core/src/chat.rs` + `search.rs`

**Current**: `Citation { item_id, title, relevance }`.

**New**:

```rust
pub struct Citation {
    pub item_id: String,
    pub title: String,
    pub relevance: f32,
    /// Char-level offset into source item content (inclusive start, exclusive end).
    /// None for web-search results (no source item).
    pub chunk_offset_start: Option<usize>,
    pub chunk_offset_end: Option<usize>,
    /// Breadcrumb path from J1; empty for sources without sectioning (e.g. plain notes).
    pub breadcrumb: Vec<String>,
}
```

`SearchResult` already carries `item_id` and `chunk_idx`; we extend `VectorMeta` to include `(offset_start, offset_end)` so chat can compute citations without re-tokenizing.

**Frontend not in this batch**: Reader modal highlight + scroll-to-offset is a separate Tauri/Preact PR.

## 3. Test Plan

Per CLAUDE.md: deterministic inputs, real Store + tempfile, no random.

### J1 unit (chunker.rs)
- Markdown nested headings → path `[H1, H2, H3]` correct
- Code with `fn` siblings → all path length 1
- Empty content → empty Vec
- Path stack pop on dedent: `# A\n## B\n# C` → C's path is `[C]` not `[A, B, C]`

### J3 unit (search.rs)
- min_score 0.65 filters [0.50, 0.70, 0.85] → 2 results
- min_score 0.78 → 1 result
- min_score None → no filter (backwards compat)

### J5 unit (chat.rs)
- prompt contains "禁用模糊措辞"
- confidence regex matches "【置信度: 4/5】" → 4
- confidence regex matches "[Confidence: 2/5]" → 2 (English fallback)
- absent confidence → default 3
- mock LLM returns confidence 2 → secondary retrieval triggered
- mock LLM returns confidence 4 → no secondary retrieval
- secondary retrieval failure (LLM error) → return original response, `secondary_retrieval_used = true` but `confidence_after = confidence_before`

### B1 backend unit (chat.rs)
- Citation has Some(start) Some(end) when SearchResult has known offsets
- offsets satisfy `start < end <= content.len()`
- web search results have None offsets
- breadcrumb propagated from VectorMeta path

### Integration
Single integration test that runs full chat cycle against in-memory store with seeded chunks (3 docs, mixed-confidence mock LLM), verifies:
- Strict prompt was issued
- Confidence parsed
- Secondary retrieval branch exercised at least once across cycle
- Citations include offsets + breadcrumb

## 4. Backwards Compatibility

| Change | Risk | Mitigation |
|--------|------|------------|
| `extract_sections` → keep + add new fn | None | Old fn untouched |
| `SearchParams.min_score: Option<f32>` | Default `Some(0.65)` may filter results that previously surfaced | Settings UI exposes; users can set None to restore old behavior; integration tests verify golden recall doesn't drop > 5% |
| `Citation` struct adds 3 new fields | API consumers (server routes) may need recompile | All new fields are `Option` or default-empty `Vec`; serde-friendly |
| Strict prompt | Existing chat sessions get new system prompt mid-conversation? | System prompt is per-message, not stored; safe |

## 5. Acknowledgments (References)

Per attune `ACKNOWLEDGMENTS.md` policy. Sources for this batch:

- **吴师兄. "鹅厂面试官追问：你的 RAG 能跑通 Demo？那让它在 5000 份文档里稳定答对，试试看"**, 公众号 "吴师兄学大模型", 2026-04-27. https://mp.weixin.qq.com/s/YNcfSN0uv1c1LsLPzgB0jw — entire J series origin
- **CRAG paper**, Yan et al., 2024. arXiv:2401.15884 — J5.c three-class gating + lowered-threshold secondary retrieval
- **Self-RAG paper**, Asai et al., 2023. arXiv:2310.11511 — J5.b confidence as token-level signal (we use 1-5 scale instead of generation tokens)
- **explodinggradients/ragas** (Apache-2.0). https://github.com/explodinggradients/ragas — metric naming convention for J6 (next batch)

Each new function/struct in code carries a `// per <Source> §<Section>` inline comment so future maintainers can trace design intent.

## 6. Acceptance Checklist

- [ ] J1: `cargo test -p attune-core chunker::tests::extract_sections_with_path*` green
- [ ] J3: `cargo test -p attune-core search::tests::min_score*` green
- [ ] J5: `cargo test -p attune-core chat::tests::strict_prompt*` + `confidence_parse*` + `secondary_retrieval*` all green
- [ ] B1 backend: `cargo test -p attune-core chat::tests::citation_offsets*` green
- [ ] Integration: `cargo test -p attune-core --test rag_w2_batch1_integration` green
- [ ] Full lib regression: `cargo test --workspace --lib` ≥ previous 397 passing, 0 failures
- [ ] `ACKNOWLEDGMENTS.md` + `.zh.md` entries for J1/J3/J5/B1 (already in framework, will refine)
- [ ] `rust/RELEASE.md` + `.zh.md`(future) changelog entries with citations
- [ ] `rust/DEVELOP.md` J series section
- [ ] `tests/MANUAL_TEST_CHECKLIST.md` W2 batch 1 verification block
- [ ] git commit with `Inspired-by:` lines + push to develop

## 7. Out of Scope (Explicit)

- ❌ B1 frontend (Reader modal scroll/highlight) — next session
- ❌ H2 settings tier picker UI — needs Tauri i18n framework (W5)
- ❌ H3 topbar Pause button frontend — same constraint
- ❌ D1 No-telemetry toggle UI — same
- ❌ J6 public benchmark numbers — W4 after J1/J3/J5 stabilize
- ❌ J2 dynamic window (return ±1 chunks) — W5-6 (depends on chunk_idx adjacency lookups; will need indexer change)
- ❌ J4 query intent ML routing — W5-6
