#!/usr/bin/env bash
# P14～P16 工程候選證據：對實際 Rust 內部測試執行檔逐階段
# 核對非零執行測試且失敗／忽略均為零；不是 Phase 正式驗收。
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$#" -ne 2 ]; then
  echo "用法：bash tools/核對-P14-P16-工程證據.sh <Rust內部測試執行檔> <輸出目錄>" >&2
  exit 2
fi
test_bin="$1"
output="$2"
if [ ! -x "$test_bin" ]; then
  echo "找不到 Rust 內部測試執行檔：$test_bin" >&2
  exit 2
fi
mkdir -p "$output"
"$test_bin" --list > "$output/p14_p16_已編譯測試清單.log"
check_phase() {
  local phase="$1"
  local filter="$2"
  local log="$output/${phase}_候選工程測試.log"
  local count
  count=$(grep -F -c "$filter" "$output/p14_p16_已編譯測試清單.log" || true)
  if [ "$count" -eq 0 ]; then
    echo "${phase} 未列名任何可執行的私有安全測試" >&2
    exit 1
  fi
  "$test_bin" "$filter" --test-threads=1 > "$log"
  if ! grep -Eq 'test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored' "$log"; then
    echo "${phase} 沒有取得非零通過及零失敗／忽略的測試證據" >&2
    exit 1
  fi
  echo "${phase} 私有工程候選測試：$count 項列名且執行成功"
}
check_phase P14 borrow_ownership_gate::tests::
check_phase P15 borrow_lifetime_probe::tests::
check_phase P16 resource_obligation_audit::tests::
check_phase P14_P16 resource_borrow_pipeline::tests::
echo "P14～P16 工程候選證據可重現，正式 Phase 依原規格及相依階段另行驗收。"
