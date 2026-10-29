#!/usr/bin/env bash
# P13 人工驗收輔助：P12 回歸、真正 voil 來源的私有 struct 安全子集及借用模型。
# 不包含完整對外 struct／move API 或 P15 借用回傳生命週期安全。
set -euo pipefail
cd "$(dirname "$0")/.."
command -v rustup >/dev/null || { echo "請先安裝 rustup" >&2; exit 2; }
command -v python3 >/dev/null || { echo "請先安裝 python3" >&2; exit 2; }
echo "【1/7】Rust stable"
rustup toolchain install stable --profile minimal
rustup run stable rustc --version
mkdir -p target/p13-accept target/p02
echo "【2/7】重新建置核心與 CLI"
rustup run stable rustc --edition=2021 --crate-name voiles_core --crate-type rlib src/lib.rs -o target/p13-accept/libvoiles_core.rlib
rustup run stable rustc --edition=2021 --crate-name voiles --extern voiles_core=target/p13-accept/libvoiles_core.rlib -L dependency=target/p13-accept src/main.rs -o target/p13-accept/voiles
echo "【3/7】首版正反例 CLI 退出碼與診斷"
./target/p13-accept/voiles tests/fixtures/voiles/P01-正01.voil >target/p13-accept/positive.out 2>target/p13-accept/positive.err
test ! -s target/p13-accept/positive.out
test ! -s target/p13-accept/positive.err
if ./target/p13-accept/voiles tests/fixtures/voiles/P01-反08.voil >target/p13-accept/negative.out 2>target/p13-accept/negative.err; then
  echo "P01 反例意外通過" >&2; exit 1
fi
test ! -s target/p13-accept/negative.out
grep -q "靜態錯誤" target/p13-accept/negative.err
echo "【4/7】方案 B：檢查公開 MVP-A 未開放的 P13／容器語法"
bash tools/檢查-P13-公開邊界.sh ./target/p13-accept/voiles target/p13-accept
echo "【5/7】Rust 核心與 P13 內部模型測試"
rustup run stable rustc --edition=2021 --test --crate-name voiles_internal src/lib.rs -o target/p13-accept/internal_tests
./target/p13-accept/internal_tests
echo "【6/7】P13 真實 struct／Copyable／借用回傳安全來源證據"
# 除全量回歸外，獨立保存 P13 受限來源與 HC14 借用證據，
# 以免人工驗收只看到總通過數而無法核對相關安全負例。
./target/p13-accept/internal_tests --list >target/p13-accept/internal_tests.list
grep -q "p13_deferred_borrowed_return_tests::p13_a_非_copyable_self_" \
  target/p13-accept/internal_tests.list
grep -q "nested_default_initialization_tests::s07_有預設運算式" \
  target/p13-accept/internal_tests.list
grep -q "automatic_self_borrow_tests::方案_a_" \
  target/p13-accept/internal_tests.list
grep -q "str_noncopyable_policy_tests::str_明確禁止名稱隱含複製" \
  target/p13-accept/internal_tests.list
grep -q "float_copyable_approved_tests::float_與遞迴全部_copyable_" \
  target/p13-accept/internal_tests.list
grep -q "p13_d02_const_partial_safety_tests::d02_a_const_" \
  target/p13-accept/internal_tests.list
grep -q "float_copyable_method_tests::float_已准_copyable_" \
  target/p13-accept/internal_tests.list
grep -q "container_copy_policy::tests::d01_後續_list_a_dict_a_保留兩種類獨立未決" \
  target/p13-accept/internal_tests.list
grep -q "container_copy_policy::tests::d01_後續_list_a_dict_a_拒絕隱含複製" \
  target/p13-accept/internal_tests.list
# P13 C／D：確保新增跨函式資源正反來源真的編譯並執行，不能只靠全量數字。
for case_name in \
  "p13_跨函式資源移交::p13_資源參數明確移動並由函式回傳" \
  "p13_跨函式資源移交::p13_移交後原來源不得再用" \
  "p13_跨函式資源移交::p13_非_copyable_具名引數不能隱含複製" \
  "p13_跨函式資源移交::p13_資源函式呼叫結果可再移交" \
  "p13_跨函式資源分支合流::p13_單側移交後合流不得再次使用來源" \
  "p13_跨函式資源分支合流::p13_雙分支皆移交後合流不得再次使用來源" \
  "p13_跨函式資源分支合流::p13_分支內遮蔽資源移交不影響外層來源" \
  "p13_跨函式資源分支合流::p13_雙分支重新初始化後可再次移交"
do
  grep -Fq "$case_name" target/p13-accept/internal_tests.list || {
    echo "P13 跨函式來源案例缺失：$case_name" >&2
    exit 1
  }
done
./target/p13-accept/internal_tests p13_跨函式資源移交:: \
  >target/p13-accept/p13_function_transfer_tests.log
./target/p13-accept/internal_tests p13_跨函式資源分支合流:: \
  >target/p13-accept/p13_function_branch_tests.log
grep -q "test result: ok. 4 passed; 0 failed" target/p13-accept/p13_function_transfer_tests.log
grep -q "test result: ok. 4 passed; 0 failed" target/p13-accept/p13_function_branch_tests.log
./target/p13-accept/internal_tests struct_stage:: \
  >target/p13-accept/p13_struct_tests.log
./target/p13-accept/internal_tests p13_d02_const_partial_safety_tests:: \
  >target/p13-accept/p13_d02_tests.log
./target/p13-accept/internal_tests borrow_safety::tests:: \
  >target/p13-accept/p13_borrow_tests.log
./target/p13-accept/internal_tests ownership_policy::tests:: \
  >target/p13-accept/p13_copy_tests.log
./target/p13-accept/internal_tests container_copy_policy::tests:: \
  >target/p13-accept/p13_container_copy_tests.log
./target/p13-accept/internal_tests str_noncopyable_policy_tests:: \
  >target/p13-accept/p13_str_tests.log
./target/p13-accept/internal_tests float_copyable_approved_tests:: \
  >target/p13-accept/p13_float_tests.log
./target/p13-accept/internal_tests float_copyable_method_tests:: \
  >>target/p13-accept/p13_float_tests.log
grep -q "test result: ok." target/p13-accept/p13_struct_tests.log
grep -q "test result: ok." target/p13-accept/p13_d02_tests.log
grep -q "test result: ok." target/p13-accept/p13_borrow_tests.log
grep -q "test result: ok." target/p13-accept/p13_copy_tests.log
grep -q "test result: ok." target/p13-accept/p13_container_copy_tests.log
grep -q "test result: ok." target/p13-accept/p13_str_tests.log
grep -q "test result: ok." target/p13-accept/p13_float_tests.log
echo "【7/7】49 項 P01 語意案例與 1 項素材追溯"
python3 tools/產生規格測試.py
python3 tools/檢查P03測試.py
rustup run stable rustc --edition=2021 --test --crate-name voiles_p01_spec \
  --extern voiles_core=target/p13-accept/libvoiles_core.rlib \
  -L dependency=target/p13-accept target/p02/p03_spec_generated.rs -o target/p13-accept/spec_tests
./target/p13-accept/spec_tests
echo "人工驗收測試完成。已保存 P13 結構來源與借用回歸日誌；P13～P16 完整 Phase 仍待獨立驗收。"
