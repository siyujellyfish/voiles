#!/usr/bin/env bash
# P13 正式驗收候選證據：六組候選門檻成功，不等於 P13 Phase 全面通過。
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$#" -ne 2 ]; then
  echo "用法：bash tools/核對-P13-驗收證據.sh <Rust內部測試執行檔> <輸出目錄>" >&2
  exit 2
fi
bin="$1"
outdir="$2"
if [ ! -x "$bin" ]; then
  echo "找不到可執行的 Rust 內部測試：$bin" >&2
  exit 2
fi
mkdir -p "$outdir"
"$bin" --list > "$outdir/p13_驗收候選清單.log"
verify_group() {
  local gate="$1"
  local label="$2"
  local filter="$3"
  local count
  count=$(grep -F -c "$filter" "$outdir/p13_驗收候選清單.log" || true)
  if [ "$count" -eq 0 ]; then
    echo "P13.$gate 缺少 $label 的必要測試：$filter" >&2
    exit 1
  fi
  "$bin" "$filter" --test-threads=1 > "$outdir/p13_驗收候選_${gate}_${label}.log"
  if ! grep -Eq 'test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored' \
    "$outdir/p13_驗收候選_${gate}_${label}.log"; then
    echo "P13.$gate 的 $label 測試未取得有效的非零通過證據" >&2
    exit 1
  fi
  echo "P13.$gate 候選證據 $label：$count 項已列名且非零測試成功"
}
verify_group 1 base ownership_policy::tests::
verify_group 1 container container_copy_policy::tests::
verify_group 2 copy ownership_model::tests::
verify_group 2 str struct_stage::str_noncopyable_policy_tests::
verify_group 2 move_matrix struct_stage::p13_formal_implicit_move_matrix_tests::
verify_group 3 move struct_ownership::tests::
verify_group 3 checker struct_stage::method_safety_tests::
verify_group 4 flow struct_stage::str_ownership_branch_tests::
verify_group 4 ledger ownership_ledger::tests::
verify_group 5 positive struct_stage::float_copyable_approved_tests::
verify_group 5 negative struct_stage::p13_d02_const_partial_safety_tests::
verify_group 6 partial_completion struct_stage::p13_formal_partial_field_completion_tests::
verify_group 6 field_write struct_stage::p13_formal_field_write_tests::
verify_group 6 struct struct_ownership::tests::
verify_group 6 method struct_stage::p13_deferred_borrowed_return_tests::
echo "P13.1～P13.6 私有測試候選門檻已取得證據；完整語意仍須正式驗收。"
