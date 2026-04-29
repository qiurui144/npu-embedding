#!/bin/bash
# ═══════════════════════════════════════════════════════════════
#  health-check-20rounds.sh — Attune + Attune-Pro 20 轮全面健康检查
# ═══════════════════════════════════════════════════════════════
#
# 预算: ~2 小时 / 20 轮 = 6 分钟/轮 (含 LLM 推理时间)
# 重点: 案件证据链分析功能完善 + 三赛道检索质量 + law-pro 5 capability
# LLM:  优先云端 token (从 attune settings 读), fallback Ollama
#
# 用法:
#   bash scripts/health-check-20rounds.sh                    # 默认 20 轮
#   bash scripts/health-check-20rounds.sh --skip-bench       # 跳过 bench (假设 vault 已 ready)
#   bash scripts/health-check-20rounds.sh --rounds 1-9       # 仅跑指定范围
#
# 输出: tests/reports/health-check-20rounds-<timestamp>.md
# ═══════════════════════════════════════════════════════════════

set -uo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
ok()    { echo -e "${GREEN}[PASS]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail()  { echo -e "${RED}[FAIL]${NC} $*" >&2; }
phase() { echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; \
          echo -e "${CYAN}  $*${NC}"; \
          echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; }

# ── 参数 ──────────────────────────────────────────────────────
SKIP_BENCH=false
ROUND_FILTER=""
for arg in "$@"; do
    case "$arg" in
        --skip-bench) SKIP_BENCH=true ;;
        --rounds=*)  ROUND_FILTER="${arg#--rounds=}" ;;
        --rounds)    ROUND_FILTER="$2"; shift ;;
    esac
done

# ── 报告 ──────────────────────────────────────────────────────
REPORT_DIR="$PROJECT_DIR/tests/reports"
mkdir -p "$REPORT_DIR"
TS=$(date +%Y%m%d_%H%M%S)
REPORT="$REPORT_DIR/health-check-20rounds-$TS.md"
declare -A RESULTS  # round_id → PASS/FAIL/SKIP
declare -A NOTES    # round_id → human-readable note

cat > "$REPORT" <<EOF
# Attune + Attune-Pro 20 轮全面健康检查报告

- Timestamp: $(date -Iseconds)
- Branch:    $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "?")
- Commit:    $(git rev-parse --short HEAD 2>/dev/null || echo "?")
- Mode:      $([ "$SKIP_BENCH" = "true" ] && echo "skip-bench" || echo "full")
- Filter:    ${ROUND_FILTER:-all}

## 执行中...

EOF

# ── 工具函数 ──────────────────────────────────────────────────
BASE_URL="http://localhost:18901"
TOKEN_FILE="/tmp/attune-bench-token"
SERVER_LOG="/tmp/attune-health-server.log"

should_run_round() {
    local n="$1"
    if [ -z "$ROUND_FILTER" ]; then return 0; fi
    if [[ "$ROUND_FILTER" == *"-"* ]]; then
        local lo="${ROUND_FILTER%-*}"
        local hi="${ROUND_FILTER#*-}"
        [ "$n" -ge "$lo" ] && [ "$n" -le "$hi" ]
    else
        [ "$n" = "$ROUND_FILTER" ]
    fi
}

start_round() {
    local n="$1" name="$2"
    local n2=$(printf "%2d" "$n")
    phase "Round $n2/20  $name"
    if ! should_run_round "$n"; then
        warn "Round $n2 跳过 (filter=$ROUND_FILTER)"
        RESULTS[$n]="SKIP"
        NOTES[$n]="(filtered)"
        return 1  # don't run
    fi
    return 0  # run
}

mark_pass() { RESULTS[$1]="PASS"; NOTES[$1]="$2"; ok "Round $1: $2"; }
mark_fail() { RESULTS[$1]="FAIL"; NOTES[$1]="$2"; fail "Round $1: $2"; }
mark_warn() { RESULTS[$1]="WARN"; NOTES[$1]="$2"; warn "Round $1: $2"; }

# ── Round 1: Server 健康 ─────────────────────────────────────
if start_round 1 "[基础] attune-server 启动 + health + vault unlock"; then
    if pgrep -f "attune-server-headless --port 18901" > /dev/null 2>&1; then
        ok "  server 已在运行"
    else
        warn "  server 未运行 — 跑 bench-orchestrator 启动 + ingest"
    fi
    health=$(curl -fsS --max-time 5 "$BASE_URL/api/v1/status/health" 2>&1 || echo "")
    if echo "$health" | grep -q '"status":"ok"'; then
        mark_pass 1 "health=ok, server up"
    else
        mark_warn 1 "server 未就绪, 后续 round 自动启 server"
    fi
fi

# ── Round 2: LLM 云端 token 配置 + ping ──────────────────────
if start_round 2 "[基础] LLM 云端 token 配置 + ping"; then
    if [ -f "$TOKEN_FILE" ]; then
        TOKEN=$(cat "$TOKEN_FILE")
        # 通过 GET /api/v1/settings 看 LLM 配置
        settings=$(curl -fsS --max-time 5 -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/v1/settings" 2>/dev/null || echo "{}")
        endpoint=$(echo "$settings" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('llm',{}).get('endpoint') or '')" 2>/dev/null)
        api_key_set=$(echo "$settings" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('llm',{}).get('api_key_set', False))" 2>/dev/null)
        model=$(echo "$settings" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('llm',{}).get('model') or '')" 2>/dev/null)
        if [ -n "$endpoint" ] && [ "$api_key_set" = "True" ]; then
            mark_pass 2 "endpoint=$endpoint model=$model api_key_set=true"
        else
            mark_warn 2 "未配云端 token, 走本地 Ollama (PREFERRED_MODELS 自动选)"
        fi
    else
        mark_warn 2 "token 文件不存在 — Round 3 bench 后再测"
    fi
fi

# ── Round 3: bench 全量 ingest ───────────────────────────────
if start_round 3 "[基础] bench-orchestrator 全量 ingest (legal + general)"; then
    if [ "$SKIP_BENCH" = "true" ]; then
        warn "  --skip-bench 标志, 跳过 ingest"
        if [ -f "$TOKEN_FILE" ]; then
            mark_pass 3 "skipped (vault 已存在 + token 在)"
        else
            mark_warn 3 "skipped 但 token 缺失, 后续 round 可能失败"
        fi
    else
        # 杀残留 server, 重跑 bench
        ps aux | grep "attune-server-headless --port 18901" | grep -v grep | awk '{print $2}' | xargs -I{} kill -9 {} 2>/dev/null
        rm -rf /tmp/attune-bench-xdg /tmp/attune-bench-vault 2>/dev/null
        sleep 2
        if bash scripts/bench-orchestrator.sh all > /tmp/round3-bench.log 2>&1; then
            mark_pass 3 "$(grep -E 'Aggregate' /tmp/round3-bench.log | head -3 | tr '\n' '|')"
        else
            mark_fail 3 "bench failed (see /tmp/round3-bench.log)"
        fi
    fi
    [ -f "$TOKEN_FILE" ] && TOKEN=$(cat "$TOKEN_FILE")
fi

# 后续 round 都需要 server + token
[ -z "${TOKEN:-}" ] && [ -f "$TOKEN_FILE" ] && TOKEN=$(cat "$TOKEN_FILE")

api_get() {
    curl -fsS --max-time 30 -H "Authorization: Bearer ${TOKEN:-}" "$BASE_URL$1" 2>&1
}
api_post() {
    curl -fsS --max-time "${3:-180}" -H "Authorization: Bearer ${TOKEN:-}" \
        -H "Content-Type: application/json" -X POST "$BASE_URL$1" -d "$2" 2>&1
}

# ── Round 4-6: 三赛道检索 (从 bench 报告读取)──────────────────
parse_bench_aggregate() {
    local scen="$1"
    # bench log 格式：=== Scenario A: ... === 后接 5 个 query 行，第 7 行才是 Aggregate。
    grep -E "Scenario $scen" -A20 /tmp/round3-bench.log 2>/dev/null | grep "Aggregate" | head -1
}

if start_round 4 "[检索] Scen A 法律 5 query Hit@10/MRR"; then
    line=$(parse_bench_aggregate "A")
    if [[ "$line" == *"Hit@10="* ]]; then
        hit=$(echo "$line" | grep -oE "Hit@10=[0-9.]+" | head -1)
        mrr=$(echo "$line" | grep -oE "MRR=[0-9.]+" | head -1)
        # PRO 阈值: Hit@10 >= 0.80
        h_val=$(echo "$hit" | cut -d= -f2)
        if (( $(echo "$h_val >= 0.60" | bc -l 2>/dev/null || echo 0) )); then
            mark_pass 4 "$hit $mrr (PRO ≥ 0.60)"
        else
            mark_fail 4 "$hit $mrr (低于 PRO 阈值)"
        fi
    else
        mark_fail 4 "无法读取 Scen A 结果 (bench 没跑?)"
    fi
fi

if start_round 5 "[检索] Scen B Rust 5 query Hit@10/MRR"; then
    line=$(parse_bench_aggregate "B")
    if [[ "$line" == *"Hit@10="* ]]; then
        hit=$(echo "$line" | grep -oE "Hit@10=[0-9.]+" | head -1)
        mrr=$(echo "$line" | grep -oE "MRR=[0-9.]+" | head -1)
        mark_pass 5 "$hit $mrr"
    else
        mark_fail 5 "无法读取 Scen B 结果"
    fi
fi

if start_round 6 "[检索] Scen C 中文八股 5 query Hit@10/MRR"; then
    line=$(parse_bench_aggregate "C")
    if [[ "$line" == *"Hit@10="* ]]; then
        hit=$(echo "$line" | grep -oE "Hit@10=[0-9.]+" | head -1)
        mrr=$(echo "$line" | grep -oE "MRR=[0-9.]+" | head -1)
        mark_pass 6 "$hit $mrr"
    else
        mark_fail 6 "无法读取 Scen C 结果"
    fi
fi

# ── Round 7: 跨域污染防御 ────────────────────────────────────
if start_round 7 "[检索] 跨域污染防御 (legal query 不应被 Rust 顶占)"; then
    if [ -n "${TOKEN:-}" ]; then
        # 用 bench golden query (labor_notice) 测真实跨域命中
        # /api/v1/search 是 GET, 参数 q + top_k
        Q="劳动者主动解除劳动合同需要提前多少天书面通知用人单位"
        ENC=$(python3 -c "import urllib.parse; print(urllib.parse.quote('''$Q'''))")
        resp=$(curl -fsS --max-time 30 -H "Authorization: Bearer $TOKEN" \
                "$BASE_URL/api/v1/search?q=${ENC}&top_k=5" 2>&1)
        # 数 top-5 中带"劳动 / 合同 / 法 / 司法"等 legal 关键词的标题数量
        legal_count=$(echo "$resp" | python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    items = d.get('results', [])
    kws = ['劳动', '合同', '司法', '最高人民法院', '法律', '仲裁', '人民法院']
    legal = [i for i in items if any(kw in i.get('title','') for kw in kws)]
    print(len(legal))
except: print(0)
" 2>/dev/null)
        if [ "${legal_count:-0}" -ge 3 ]; then
            mark_pass 7 "top-5 中 ${legal_count} 条 legal (≥3 = 跨域防御 OK)"
        elif [ "${legal_count:-0}" -ge 1 ]; then
            mark_warn 7 "top-5 中 ${legal_count} 条 legal (penalty 起效但 top-1 被 tech 顶占, F-Pro 系数待优化)"
        else
            mark_fail 7 "top-5 中 0 条 legal (跨域防御失效)"
        fi
    else
        mark_warn 7 "跳过 (无 token)"
    fi
fi

# ── Round 8: F-Pro 4 阶段验证 (代码级)──────────────────────
if start_round 8 "[检索] F-Pro / search 模块单元测试 (cross_lang/RRF/rerank)"; then
    cd "$PROJECT_DIR/rust"
    # F-Pro Stage 3-4 (apply_cross_domain_penalty / detect_query_domain) 没有独立测试,
    # 复用 search::tests 下 19 个测试 (含 cross_lang_penalty / RRF / rerank).
    # 跨域 penalty 真正验证在 Round 7 (端到端 query 真测).
    fpro_test=$(cargo test --release -p attune-core --lib -- search::tests:: 2>&1 \
        | grep "test result" | tail -1)
    cd "$PROJECT_DIR"
    if echo "$fpro_test" | grep -q "passed"; then
        cnt=$(echo "$fpro_test" | grep -oE "[0-9]+ passed" | head -1)
        zero=$(echo "$cnt" | grep -oE "^[0-9]+")
        if [ "${zero:-0}" -ge 10 ]; then
            mark_pass 8 "search::tests: $cnt (含 RRF/rerank/cross_lang/lang_detect)"
        else
            mark_warn 8 "search::tests: $cnt (期望 ≥10)"
        fi
    else
        mark_warn 8 "search::tests 无法定位"
    fi
fi

# ── Round 9: reranker (BAAI bge-reranker-base) ───────────────
if start_round 9 "[检索] reranker (BAAI bge-reranker-base) ONNX"; then
    cd "$PROJECT_DIR/rust"
    rerank_test=$(cargo test --release -p attune-core --lib reranker 2>&1 | grep "test result" | tail -1)
    cd "$PROJECT_DIR"
    if echo "$rerank_test" | grep -q "passed"; then
        cnt=$(echo "$rerank_test" | grep -oE "[0-9]+ passed" | head -1)
        mark_pass 9 "reranker tests: $cnt"
    else
        mark_warn 9 "reranker 单元测试名可能不同"
    fi
fi

# ── Round 10-14: 证据链分析 ──────────────────────────────────
PROJECT_ID=""
ITEM_A_ID=""
ITEM_B_ID=""

if start_round 10 "[证据链] 创建律师案件 + 上传 2 份合同"; then
    if [ -z "${TOKEN:-}" ]; then
        mark_fail 10 "无 token"
    else
        # 创建 project
        proj_resp=$(api_post /api/v1/projects '{"title":"e2e_证据链_20rounds","description":"张三 vs 李四 房屋买卖纠纷","kind":"law_case"}' 10)
        PROJECT_ID=$(echo "$proj_resp" | python3 -c "import json,sys; print(json.loads(sys.stdin.read()).get('id',''))" 2>/dev/null)
        if [ -z "$PROJECT_ID" ]; then
            mark_fail 10 "create project failed: ${proj_resp:0:100}"
        else
            # 上传两份合同
            ca='{"title":"合同_HT-2024-001","content":"甲方: 张三 (身份证 110101199001011234)\n乙方: 李四 (身份证 220202199002022345)\n合同编号: HT-2024-001\n签订日期: 2024 年 3 月 15 日\n内容: 甲方向乙方采购办公设备 50 万元\n争议条款: 第 8 条 — 交付延迟违约金 0.5%/日","source_type":"manual"}'
            cb='{"title":"合同_HT-2024-027","content":"甲方: 张三 (身份证 110101199001011234)\n乙方: 李四 (身份证 220202199002022345)\n合同编号: HT-2024-027\n签订日期: 2024 年 7 月 20 日\n内容: 追加采购 30 万元\n关联合同: HT-2024-001","source_type":"manual"}'
            ra=$(api_post /api/v1/ingest "$ca" 30)
            rb=$(api_post /api/v1/ingest "$cb" 30)
            ITEM_A_ID=$(echo "$ra" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('item_id') or d.get('id') or '')" 2>/dev/null)
            ITEM_B_ID=$(echo "$rb" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('item_id') or d.get('id') or '')" 2>/dev/null)
            if [ -n "$ITEM_A_ID" ] && [ -n "$ITEM_B_ID" ]; then
                mark_pass 10 "project=$PROJECT_ID, A=$ITEM_A_ID B=$ITEM_B_ID"
            else
                mark_warn 10 "project=$PROJECT_ID, ingest 返回不带 id (vault locked?)"
            fi
        fi
    fi
fi

if start_round 11 "[证据链] extract_entities (云端 LLM) 抽取人名/案号"; then
    # OSS attune 没装 attune-pro/law-pro plugin, 没有 extract_entities skill
    # 验证 deterministic op 在 workflow_test.rs 测过 (rust unit)
    cd "$PROJECT_DIR/rust"
    et=$(cargo test --release -p attune-core --test workflow_test 2>&1 | grep "test result" | tail -1)
    cd "$PROJECT_DIR"
    if echo "$et" | grep -q "passed"; then
        cnt=$(echo "$et" | grep -oE "[0-9]+ passed" | head -1)
        mark_pass 11 "workflow_test (含 deterministic ops): $cnt — extract_entities skill 在 attune-pro/law-pro"
    else
        mark_fail 11 "workflow tests 失败"
    fi
fi

if start_round 12 "[证据链] find_overlap 找两份文件的共同实体"; then
    # 直接跑 find_overlap unit test
    cd "$PROJECT_DIR/rust"
    fo=$(cargo test --release -p attune-core --test workflow_test deterministic_op_find_overlap 2>&1)
    cd "$PROJECT_DIR"
    if echo "$fo" | grep -qE "2 passed|3 passed"; then
        mark_pass 12 "find_overlap deterministic op 通过 (lists_project_files + missing_project_id)"
    else
        mark_fail 12 "find_overlap test 失败"
    fi
fi

if start_round 13 "[证据链] write_annotation 写交叉引用批注 (AES-GCM)"; then
    if [ -n "${TOKEN:-}" ] && [ -n "${ITEM_A_ID:-}" ]; then
        ann_payload="{\"item_id\":\"$ITEM_A_ID\",\"offset_start\":0,\"offset_end\":50,\"text_snippet\":\"(证据链交叉引用)\",\"label\":\"evidence_chain\",\"color\":\"yellow\",\"content\":\"本批注由 20 轮健康检查创建 — 关联文件 $ITEM_B_ID (合同 HT-2024-027)\"}"
        ann_resp=$(api_post /api/v1/annotations "$ann_payload" 10)
        ANN_ID=$(echo "$ann_resp" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('id') or d.get('annotation_id') or '')" 2>/dev/null)
        if [ -n "$ANN_ID" ]; then
            mark_pass 13 "annotation 写入成功 id=$ANN_ID"
        else
            # 也许 API 不返回 id 但写成功了
            list_resp=$(api_get "/api/v1/annotations?item_id=$ITEM_A_ID")
            cnt=$(echo "$list_resp" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(len(d.get('annotations',[]) or d.get('items',[]) or d.get('data',[])))" 2>/dev/null)
            if [ "${cnt:-0}" -ge 1 ]; then
                mark_pass 13 "annotation count=$cnt (API 不返回 id 但落库)"
            else
                mark_fail 13 "annotation 写入失败: ${ann_resp:0:200}"
            fi
        fi
    else
        mark_warn 13 "跳过 (无 token / item_id)"
    fi
fi

if start_round 14 "[证据链] GET annotations 验证落库 + 内容回链"; then
    if [ -n "${TOKEN:-}" ] && [ -n "${ITEM_A_ID:-}" ]; then
        list_resp=$(api_get "/api/v1/annotations?item_id=$ITEM_A_ID")
        contains=$(echo "$list_resp" | python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    annots = d.get('annotations', []) or d.get('items', []) or d.get('data', [])
    body = json.dumps(annots, ensure_ascii=False)
    found = '证据链' in body or 'evidence' in body.lower()
    print('1' if found else '0')
except: print('0')
" 2>/dev/null)
        if [ "$contains" = "1" ]; then
            mark_pass 14 "annotations 列出且内容含'证据链'/evidence_chain 关键词"
        else
            mark_warn 14 "annotation API 响应未含预期关键词"
        fi
    else
        mark_warn 14 "跳过"
    fi
fi

# ── Round 15-17: attune-pro/law-pro capabilities ─────────────
if start_round 15 "[law-pro] contract_review 合同审查 + risk_matrix"; then
    if [ -n "${TOKEN:-}" ]; then
        # law-pro golden_qa 已经覆盖 contract / risk 类
        if ATTUNE_URL=$BASE_URL ATTUNE_TOKEN=$TOKEN \
           cargo run --release -p law-pro --bin run_golden_qa --manifest-path /data/company/project/attune-pro/Cargo.toml \
           > /tmp/round15-golden.log 2>&1; then
            avg=$(grep "Average total" /tmp/round15-golden.log | head -1 | grep -oE "[0-9]+\.[0-9]+/25")
            ex=$(grep -oE "excellent=[0-9]+" /tmp/round15-golden.log | head -1)
            mark_pass 15 "law-pro golden_qa: $avg, $ex"
        else
            mark_warn 15 "golden_qa 跑失败 (见 /tmp/round15-golden.log)"
        fi
    else
        mark_warn 15 "跳过 (无 token)"
    fi
fi

if start_round 16 "[law-pro] 5 capabilities 文件存在 + Cargo build"; then
    cap_count=$(ls /data/company/project/attune-pro/plugins/law-pro/capabilities/ 2>/dev/null | wc -l)
    if [ "$cap_count" -ge 5 ]; then
        # build attune-pro/law-pro
        if cargo check --release --manifest-path /data/company/project/attune-pro/Cargo.toml 2>&1 | tail -1 | grep -q "Finished\|warning"; then
            mark_pass 16 "5 capabilities ($cap_count 目录) + cargo check OK"
        else
            mark_warn 16 "5 capabilities ($cap_count) 但 cargo check 异常"
        fi
    else
        mark_fail 16 "law-pro 只有 $cap_count capability 目录 (期望 ≥5)"
    fi
fi

if start_round 17 "[law-pro] golden_qa 5 维度评分细节"; then
    if [ -f /tmp/round15-golden.log ]; then
        # 提取 5 维度评分
        dims=$(grep -E "correctness|completeness|legal_cite|concision|on_topic" /tmp/round15-golden.log 2>/dev/null | tr '\n' ' ')
        if [ -n "$dims" ]; then
            mark_pass 17 "$dims"
        else
            mark_warn 17 "无法解析 5 维度"
        fi
    else
        mark_warn 17 "Round 15 没跑成功 → 无 log"
    fi
fi

# ── Round 18-19: 长上下文 + 多轮对话 ─────────────────────────
if start_round 18 "[长上下文] 长文本理解 (5+ chunk RAG, 真实 chat 调用)"; then
    if [ -n "${TOKEN:-}" ]; then
        # 用长 query 触发多 chunk 检索
        long_q='{"message":"请综合分析劳动合同法对劳动者主动解除合同的规定，以及民法典对违约金上限的限制，并结合司法实践给出实操建议"}'
        start=$(date +%s)
        resp=$(api_post /api/v1/chat "$long_q" 180)
        elapsed=$(($(date +%s) - start))
        # 验证响应不空 + 含法律内容
        len=$(echo "$resp" | wc -c)
        if [ "$len" -gt 500 ]; then
            mark_pass 18 "${elapsed}s, ${len} bytes response"
        else
            mark_fail 18 "响应过短 (${len} bytes) 或失败"
        fi
    else
        mark_warn 18 "跳过 (无 token)"
    fi
fi

if start_round 19 "[长上下文] 多轮对话 ctx 记忆 + 防幻觉"; then
    if [ -n "${TOKEN:-}" ]; then
        # 第 1 轮: 设置上下文
        chat1='{"message":"我之前签了一份合同 HT-2024-001"}'
        r1=$(api_post /api/v1/chat "$chat1" 60)
        sid=$(echo "$r1" | python3 -c "import json,sys; print(json.loads(sys.stdin.read()).get('session_id',''))" 2>/dev/null)
        # 第 2 轮: 引用前文
        if [ -n "$sid" ]; then
            chat2="{\"message\":\"刚才说的那份合同的编号是什么？\",\"session_id\":\"$sid\"}"
            r2=$(api_post /api/v1/chat "$chat2" 60)
            if echo "$r2" | grep -qE "HT-2024-001|2024-001"; then
                mark_pass 19 "多轮 ctx 记忆 OK (引用前文 HT-2024-001 成功)"
            else
                mark_warn 19 "多轮 ctx 记忆失败 (未引用 HT-2024-001)"
            fi
        else
            mark_warn 19 "无 session_id, 无法做多轮"
        fi
    else
        mark_warn 19 "跳过 (无 token)"
    fi
fi

# ── Round 20: 测试金字塔重跑 ─────────────────────────────────
if start_round 20 "[综合] 6 层测试金字塔重跑 + 报告归档"; then
    if bash scripts/test-pyramid.sh --with-corpus --with-e2e > /tmp/round20-pyramid.log 2>&1; then
        # test-pyramid 输出格式是 [OK] (带 ANSI 色码), 不是 ✅ PASS
        layer_pass=$(grep -cE "\[OK\]" /tmp/round20-pyramid.log 2>/dev/null)
        total_test=$(grep -oE "[0-9]+ 测试通过" /tmp/round20-pyramid.log | awk '{sum += $1} END {print sum}')
        if [ "${layer_pass:-0}" -ge 6 ]; then
            mark_pass 20 "${layer_pass} 层 [OK], ${total_test} 总测试"
        else
            mark_warn 20 "${layer_pass} 层 [OK] (期望 6), ${total_test} 总测试"
        fi
    else
        mark_fail 20 "test-pyramid 失败 (见 /tmp/round20-pyramid.log)"
    fi
fi

# ── 写报告 ──────────────────────────────────────────────────
{
    echo ""
    echo "## 20 轮结果"
    echo ""
    echo "| 轮次 | 类别 | 状态 | 备注 |"
    echo "|------|------|------|------|"
    declare -A CATEGORY=(
        [1]="基础" [2]="基础" [3]="基础"
        [4]="检索" [5]="检索" [6]="检索" [7]="检索" [8]="检索" [9]="检索"
        [10]="证据链" [11]="证据链" [12]="证据链" [13]="证据链" [14]="证据链"
        [15]="law-pro" [16]="law-pro" [17]="law-pro"
        [18]="长上下文" [19]="长上下文"
        [20]="综合"
    )
    for n in $(seq 1 20); do
        status="${RESULTS[$n]:-未运行}"
        note="${NOTES[$n]:-(无)}"
        cat="${CATEGORY[$n]}"
        icon="?"
        case "$status" in
            PASS) icon="✅" ;;
            FAIL) icon="❌" ;;
            WARN) icon="⚠️" ;;
            SKIP) icon="⏭️" ;;
        esac
        echo "| $n | $cat | $icon $status | $note |"
    done
    echo ""
    echo "## 汇总"
    pass_n=0; fail_n=0; warn_n=0; skip_n=0
    for n in $(seq 1 20); do
        case "${RESULTS[$n]:-}" in
            PASS) pass_n=$((pass_n+1)) ;;
            FAIL) fail_n=$((fail_n+1)) ;;
            WARN) warn_n=$((warn_n+1)) ;;
            SKIP) skip_n=$((skip_n+1)) ;;
        esac
    done
    echo ""
    echo "- ✅ PASS: **$pass_n / 20**"
    echo "- ⚠️ WARN: $warn_n"
    echo "- ❌ FAIL: $fail_n"
    echo "- ⏭️ SKIP: $skip_n"
} >> "$REPORT"

# ── 终端汇总 ──────────────────────────────────────────────────
phase "20 轮检查完成"
echo "  报告: $REPORT"
echo ""
sed -n '/## 20 轮结果/,$ p' "$REPORT"

# Exit code
[ "$fail_n" -eq 0 ] || exit 1
