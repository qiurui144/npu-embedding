#!/usr/bin/env bash
# 将指定目录下的 Markdown 文件批量 POST 到 attune-server /api/v1/ingest。
#
# 用法：
#   ./scripts/bulk-ingest.sh <source_dir> [base_url]
#
# source_dir: 含 .md 文件的目录（递归）
# base_url:   attune-server 地址，默认 http://localhost:18900

set -euo pipefail

SOURCE_DIR="${1:?usage: bulk-ingest.sh <source_dir> [base_url]}"
BASE_URL="${2:-http://localhost:18900}"

if [ ! -d "$SOURCE_DIR" ]; then
  echo "error: $SOURCE_DIR not a directory" >&2
  exit 1
fi

total=0
success=0
failed=0

while IFS= read -r -d '' file; do
  total=$((total + 1))
  title=$(basename "$file" .md)
  # Strip leading chNN-NN- for readability
  title=${title#ch*-*-}
  content=$(cat "$file")

  # jq for safe JSON encoding
  body=$(jq -Rn --arg t "$title" --arg c "$content" \
    '{title: $t, content: $c, source_type: "file"}')

  if curl -sSf -X POST -H 'Content-Type: application/json' \
       --data "$body" \
       "$BASE_URL/api/v1/ingest" > /dev/null 2>&1; then
    success=$((success + 1))
  else
    failed=$((failed + 1))
    echo "[FAIL] $file" >&2
  fi

  # 每 10 条打印进度
  if [ $((total % 10)) -eq 0 ]; then
    echo "[progress] $total processed ($success ok, $failed fail)"
  fi
done < <(find "$SOURCE_DIR" -maxdepth 1 -name "*.md" -print0)

echo
echo "=== bulk-ingest complete ==="
echo "Total:   $total"
echo "Success: $success"
echo "Failed:  $failed"
