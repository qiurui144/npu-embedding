# W3 Batch C Design (K2 Parse Golden Set Baseline)

**Date**: 2026-04-27
**Roadmap**: 12-week strategy v4 Phase 1 W3 F-P0c
**Depends on**: W3 batch A (commit `28bd691`) + W3 batch B (commit `674cf55`)
**Depended by**: G3 page extraction (W5-6 will reuse parse golden set), J6 W4 benchmark

[English](2026-04-27-w3-batch-c-design.md) · [简体中文](2026-04-27-w3-batch-c-design.zh.md)

---

## 1. Why this batch

per memory `reference_attune_open_source_landscape.md` K2：**Readwise 工程实践 — 200 篇真实页面 parsing benchmark + CI 回归 < 95% 准确率不准发版**。

attune 现有 golden set (`rust/tests/golden/queries.json`) 测的是**检索质量**（query → expected hits），但 chunker / parser 链路本身没有质量门槛。如果哪天 chunker 或 parser 改坏了（提取的 title 错、章节切碎、UTF-8 边界错位），现有 golden set 不会发现。

W3 batch C = **K2 Parse Golden Set MVP**：5 篇 inline HTML fixture + manifest 描述 expected 输出 + CI 回归 harness。设计可扩到 200 篇，本会话只交付框架 + 5 篇 baseline。

## 2. Design

### Corpus structure

```
rust/crates/attune-core/tests/fixtures/parse_corpus/
├── manifest.yaml          # 描述每个 fixture 的 expected 输出
├── 001-rust-ownership-doc.html       # rust-lang/book ch4
├── 002-china-civil-code.html         # 民法典节选
├── 003-tech-blog-post.html            # 技术博客
├── 004-news-article.html              # 新闻
└── 005-academic-paper-section.html    # 论文片段
```

每个 fixture 是 **版本固定的真实 HTML 截取**，存为 attune 仓库内 inline file（避免下载 + 网络依赖）。manifest 定义：

```yaml
fixtures:
  - id: "001-rust-ownership-doc"
    file: "001-rust-ownership-doc.html"
    source: "rust-lang/book ch4"
    pinned_version: "trpl-v0.3.0"
    license: "MIT/Apache-2.0"
    expected:
      title_contains: ["Ownership", "What Is Ownership"]
      min_text_chars: 500
      must_contain_phrases:
        - "ownership"
        - "scope"
      section_count_min: 2
      section_paths_must_include:
        - ["What Is Ownership?"]
```

### Test harness

`rust/crates/attune-core/tests/parse_golden_set_regression.rs`:

1. 读 manifest.yaml
2. 对每个 fixture：load HTML → run attune parser (parser::parse_bytes 或 chunker) → 拿到 title / chunks / sections
3. 对照 expected 字段：
   - `title_contains`: title 含全部短语
   - `min_text_chars`: 总字符数 ≥
   - `must_contain_phrases`: 抽取文本含全部
   - `section_count_min`: extract_sections_with_path 至少 N 段
   - `section_paths_must_include`: 必须出现某些 path
4. 任何 fixture fail → CI 红

### Regression gate

```rust
const MIN_PASS_RATE: f32 = 0.95;  // <95% 阻塞合并 (per Readwise 范例)
```

5 篇 baseline 中 **必须全过**（100%）才能发版。框架支持未来扩到 200 篇时降到 95% 容许 5% 边界 case fail。

## 3. Out of Scope (defer)

- ❌ 200 篇真实页面采集 — 需要 1-2 天 corpus 工作 (W4)
- ❌ CI 集成（GitHub Actions yaml）— W4 与 J6 benchmark 一起接入
- ❌ Per-language fixture 矩阵（zh / en / mixed）— 当前 5 篇含 1 zh + 4 en，足够 baseline
- ❌ PDF parsing fixture — pdf-extract 走 attune 的另一路径，独立 golden set 留 W5-6
- ❌ 真正的 Readability.js style content extraction（阻塞 G3）— 本批次仅测现有 parser

## 4. Acknowledgments

per `ACKNOWLEDGMENTS.md` policy:
- **K2 Parse Golden Set methodology**: [Readwise Reader engineering blog](https://blog.readwise.io/the-next-chapter-of-reader-public-beta/) — "200 page benchmark + CI regression < 95% blocks release" 直接抄
- **Fixture content**: rust-lang/book (MIT/Apache-2.0)、民法典（公开法律文本）、其余技术博客 / 新闻片段（fair use 截取，注明来源）

## 5. Acceptance

- [ ] `manifest.yaml` 含 5 fixtures
- [ ] `tests/fixtures/parse_corpus/*.html` 5 个文件
- [ ] `parse_golden_set_regression.rs` 5 测试（每个 fixture 一个 test fn）
- [ ] 全部 5 测试通过 → MIN_PASS_RATE 100%
- [ ] `docs/TESTING.md` 加 K2 章节
- [ ] R1 review pass
- [ ] ACKNOWLEDGMENTS update
- [ ] commit + push develop
