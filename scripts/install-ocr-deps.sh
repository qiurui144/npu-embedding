#!/usr/bin/env bash
# 安装 OCR 依赖（tesseract + poppler-utils），中英文训练数据
#
# 用法: ./scripts/install-ocr-deps.sh
# 目标平台: Linux (apt/dnf/pacman) / macOS (brew) / 不支持 Windows（请手动）
#
# 安装后，attune 在 ingest 扫描版 PDF 时自动检测并调用 OCR；不装则降级
# 为纯文字层提取（扫描版 PDF 会 ingest 空内容）。

set -euo pipefail

if [ "$(uname)" = "Darwin" ]; then
  echo "[macOS] brew install tesseract tesseract-lang poppler"
  brew install tesseract tesseract-lang poppler
  echo "[ok] tesseract + poppler-utils installed via Homebrew"
  tesseract --list-langs 2>&1 | tail -10
  exit 0
fi

if ! [ -f /etc/os-release ]; then
  echo "error: unknown OS. Please install tesseract + tesseract-ocr-chi-sim + poppler-utils manually." >&2
  exit 1
fi
. /etc/os-release

case "$ID" in
  ubuntu|debian|linuxmint)
    echo "[apt] 安装 tesseract + poppler-utils + 中英训练数据"
    echo 123123 | sudo -S apt update -qq
    echo 123123 | sudo -S apt install -y \
      tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim poppler-utils
    ;;
  fedora|rhel|centos|rocky)
    echo "[dnf] 安装 tesseract + poppler-utils"
    sudo dnf install -y tesseract tesseract-langpack-chi_sim tesseract-langpack-eng poppler-utils
    ;;
  arch|manjaro)
    echo "[pacman] 安装 tesseract + poppler"
    sudo pacman -S --noconfirm tesseract tesseract-data-chi_sim tesseract-data-eng poppler
    ;;
  *)
    echo "error: 未知发行版 $ID。手动装 tesseract + chi_sim + eng 训练数据 + poppler-utils。" >&2
    exit 1
    ;;
esac

echo
echo "=== 验证安装 ==="
which tesseract pdftoppm || { echo "error: binary 未找到" >&2; exit 1; }
echo
echo "可用 OCR 语言："
tesseract --list-langs 2>&1 | tail -20
echo
echo "[完成] Attune 启动时会自动探测并启用 OCR fallback。"
