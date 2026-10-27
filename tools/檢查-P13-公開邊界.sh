#!/usr/bin/env bash
# P13 僅供內部使用：公開 MVP-A CLI 不得接受私有結構、移動或未開放容器型別。
# 由 CI 沿用已編譯 CLI，人工驗收也共用此檢查，不重複建置。
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$#" -ne 2 ]; then
  echo "用法：bash tools/檢查-P13-公開邊界.sh <CLI路徑> <日誌目錄>" >&2
  exit 2
fi
cli="$1"
outdir="$2"
if [ ! -x "$cli" ]; then
  echo "無法執行 voiles CLI：$cli" >&2
  exit 2
fi
mkdir -p "$outdir"

check_rejected() {
  local source="$1"
  local label="$2"
  if "$cli" "$source" >"$outdir/$label.out" 2>"$outdir/$label.err"; then
    echo "P13 入口 B 遭破壞：公開 CLI 意外接受 $source" >&2
    exit 1
  fi
  test ! -s "$outdir/$label.out"
  grep -q "語法錯誤" "$outdir/$label.err"
}

cat >"$outdir/move_not_supported.voil" <<'VOILES'
fn main()->int{
var n:int = 1
return n.move()
}
VOILES
check_rejected "$outdir/move_not_supported.voil" move

check_rejected "tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil" struct

for kind in list dict; do
  case "$kind" in
    list) ty='list[int]' ;;
    dict) ty='dict{str,int}' ;;
  esac
  cat >"$outdir/d01_${kind}_not_public.voil" <<VOILES
fn main()->int{
var item:$ty
return 0
}
VOILES
  check_rejected "$outdir/d01_${kind}_not_public.voil" "d01_$kind"
done
echo "P13 公開入口 B：.move()、struct、list、dict 均維持語法拒絕。"
