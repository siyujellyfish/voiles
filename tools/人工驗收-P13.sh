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
echo "【4/7】未核准 .move() 必須仍是 MVP-A 語法錯誤"
cat >target/p13-accept/move_not_supported.voil <<'VOILES'
fn main()->int{
var n:int = 1
return n.move()
}
VOILES
if ./target/p13-accept/voiles target/p13-accept/move_not_supported.voil >target/p13-accept/move.out 2>target/p13-accept/move.err; then
  echo "MVP-A 不應接受 .move()" >&2; exit 1
fi
grep -q "語法錯誤" target/p13-accept/move.err
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
./target/p13-accept/internal_tests struct_stage:: \
  >target/p13-accept/p13_struct_tests.log
./target/p13-accept/internal_tests borrow_safety::tests:: \
  >target/p13-accept/p13_borrow_tests.log
./target/p13-accept/internal_tests ownership_policy::tests:: \
  >target/p13-accept/p13_copy_tests.log
grep -q "test result: ok." target/p13-accept/p13_struct_tests.log
grep -q "test result: ok." target/p13-accept/p13_borrow_tests.log
grep -q "test result: ok." target/p13-accept/p13_copy_tests.log
echo "【7/7】49 項 P01 語意案例與 1 項素材追溯"
python3 tools/產生規格測試.py
python3 tools/檢查P03測試.py
rustup run stable rustc --edition=2021 --test --crate-name voiles_p01_spec \
  --extern voiles_core=target/p13-accept/libvoiles_core.rlib \
  -L dependency=target/p13-accept target/p02/p03_spec_generated.rs -o target/p13-accept/spec_tests
./target/p13-accept/spec_tests
echo "人工驗收測試完成。已保存 P13 結構來源與借用回歸日誌；P13～P16 完整 Phase 仍待獨立驗收。"
