#!/usr/bin/env python3
"""
Phase B 终极评估 — 跑通双赛道 benchmark，输出真实数字 + evidence flow 验证。

前提：
- attune-server-headless 跑在 :18901
- vault unlock 完成（token 在 /tmp/bench-run8.log）
- corpus 全 indexed (法律 200 + rust-book-full 112 + cs-notes 175)

运行：
  python3 scripts/run-final-eval.py
"""
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

BASE = "http://localhost:18901"


def get_token():
    log = Path("/tmp/bench-run8.log").read_text()
    m = re.search(r'token":"([^"]+)"', log)
    if not m:
        raise RuntimeError("token not found in /tmp/bench-run8.log")
    return m.group(1)


def search(q, token, k=10):
    url = f"{BASE}/api/v1/search?{urllib.parse.urlencode({'q': q, 'limit': k})}"
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token}"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def chat(query, token):
    url = f"{BASE}/api/v1/chat"
    body = json.dumps({"message": query, "history": []}).encode()
    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.loads(r.read())


def title_matches(title, acceptable):
    tl = title.lower()
    for a in acceptable:
        if a in title or a.lower() in tl:
            return a
        for p in a.replace("_", " ").split():
            if len(p) >= 2 and (p in title or p.lower() in tl):
                return a
    return None


def score_retrieval(spec, token):
    print("=" * 100)
    print("[Phase B-3] 检索 baseline (Hit@10 / MRR / Recall@10)")
    print("=" * 100)
    agg = {}
    for scen in spec["scenarios"]:
        sid, name = scen["id"], scen["name"]
        metrics = []
        for q in scen.get("queries", []):
            try:
                resp = search(q["query"], token)
                items = resp.get("results", [])
                titles = [i.get("item_title") or i.get("title") or "?" for i in items[:10]]
            except Exception as e:
                print(f"{sid}/{q['id']:25}  ERROR: {e}")
                continue
            acc = q.get("acceptable_hits", [])
            hits = [(r, m) for r, t in enumerate(titles, 1) if (m := title_matches(t, acc))]
            h10 = 1 if hits else 0
            rr = 1.0 / hits[0][0] if hits else 0.0
            rec = len({h[1] for h in hits}) / max(len(acc), 1)
            metrics.append((h10, rr, rec))
            top3 = " | ".join(t[:20] for t in titles[:3])
            print(f"{sid}/{q['id']:25}  Hit={h10}  MRR={rr:.2f}  Rec={rec:.2f}  Top-3: {top3}")
        if metrics:
            n = len(metrics)
            agg[sid] = {
                "name": name,
                "n": n,
                "hit_at_10": sum(m[0] for m in metrics) / n,
                "mrr": sum(m[1] for m in metrics) / n,
                "recall": sum(m[2] for m in metrics) / n,
            }
            print(
                f"  → Aggregate Scen {sid}: "
                f"Hit@10={agg[sid]['hit_at_10']:.2f}  "
                f"MRR={agg[sid]['mrr']:.2f}  "
                f"Recall@10={agg[sid]['recall']:.2f}"
            )
            print()
    return agg


def check_evidence_flow(spec, token):
    """对每 scenario 第一题跑 chat，验证 citation 流"""
    print("\n" + "=" * 100)
    print("[Phase B-evidence] Chat citation 流验证")
    print("=" * 100)
    flow_results = {}
    for scen in spec["scenarios"]:
        sid = scen["id"]
        queries = scen.get("queries", [])
        if not queries:
            continue
        q = queries[0]  # 每场景跑首题
        try:
            resp = chat(q["query"], token)
        except Exception as e:
            print(f"{sid}/{q['id']:25}  CHAT ERROR: {e}")
            continue

        content = resp.get("content", "")
        citations = resp.get("citations", [])
        confidence = resp.get("confidence", 0)
        secondary = resp.get("secondary_retrieval_used", False)
        knowledge_count = resp.get("knowledge_count", 0)

        # 检验 citation 流完整性
        has_inline_marker = bool(re.search(r"\[\d+\]|\[.+?>.+?\]|《.+?》", content))
        has_breadcrumb = any(c.get("breadcrumb") for c in citations)
        has_offset = any(
            c.get("chunk_offset_start") is not None or c.get("chunk_offset_end") is not None
            for c in citations
        )

        print(f"\n{sid}/{q['id']}: {q['query'][:60]}")
        print(f"  knowledge_count={knowledge_count}, citations={len(citations)}, conf={confidence}/5, 2nd={secondary}")
        print(f"  inline_marker_in_content={has_inline_marker} | breadcrumb={has_breadcrumb} | offset={has_offset}")
        if citations:
            for i, c in enumerate(citations[:3], 1):
                print(
                    f"  [{i}] {c.get('title','?')[:50]:50}  rel={c.get('relevance', 0):.2f}  "
                    f"breadcrumb={c.get('breadcrumb', [])[:2]}  "
                    f"offset=[{c.get('chunk_offset_start')},{c.get('chunk_offset_end')}]"
                )
        print(f"  answer (first 200 chars): {content[:200]!r}")
        flow_results[f"{sid}/{q['id']}"] = {
            "knowledge_count": knowledge_count,
            "citations_count": len(citations),
            "confidence": confidence,
            "secondary_retrieval": secondary,
            "has_inline_marker": has_inline_marker,
            "has_breadcrumb": has_breadcrumb,
            "has_offset": has_offset,
        }
    return flow_results


def main():
    token = get_token()
    print(f"token: {token[:30]}...")

    spec_path = Path("rust/tests/golden/queries.json")
    if not spec_path.exists():
        spec_path = Path("/data/company/project/attune") / spec_path
    spec = json.loads(spec_path.read_text())

    retrieval_agg = score_retrieval(spec, token)
    evidence_flow = check_evidence_flow(spec, token)

    out = {
        "retrieval": retrieval_agg,
        "evidence_flow": evidence_flow,
    }
    out_path = Path("docs/benchmarks/phase-b-final.json")
    if not out_path.parent.exists():
        out_path = Path("/data/company/project/attune") / out_path
    out_path.write_text(json.dumps(out, ensure_ascii=False, indent=2))
    print(f"\n📊 Saved: {out_path}")

    # Pro 阈值评估
    print("\n" + "=" * 100)
    print("Pro 级别评估")
    print("=" * 100)
    for sid, m in retrieval_agg.items():
        h, r = m["hit_at_10"], m["mrr"]
        verdict = "✅ PRO" if h >= 0.80 and r >= 0.50 else "⚠️  GAP"
        print(f"  Scen {sid} ({m['name'][:30]}): Hit@10={h:.2f}  MRR={r:.2f}  → {verdict}")


if __name__ == "__main__":
    main()
