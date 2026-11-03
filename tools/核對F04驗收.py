#!/usr/bin/env python3
"""A4/F04-01～05：真正雙 Checker、可達性及非純值能力驗收（UTF-8）。"""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "F04-01": [
        "f05_全部已核准純值交集_公私_checker_完整_hir_節點逐一相等",
        "f01_f04_真正公私_checker_簽出不可跨模式冒用的完整_typed_hir",
    ],
    "F04-02": [
        "f04_公開_p11_可達_hir_不將不可達型別事實冒充已執行且拒絕私有簽章",
        "f04_公私_checker_待對齊差異有具體來源與可重現的精確跨度",
    ],
    "F04-03": [
        "f01_f04_欄位及方法與函式參數的型別證據來自唯一_checker_且不能授權資源",
        "f11_正式計畫整體驗證禁止以另一份_hir_偷換_checker_來源證據",
    ],
    "F04-04": [
        "真實_checker_移動_返回_覆寫_都具有原始_ast_逐筆來源",
        "同型別但沒有_move_的目標不得冒充另一筆已核准移動",
        "非_return_move_以及非欄位寫入來源不得偽造資源責任",
        "f04_f08_私有實際讀取借用及移動必須消費同一_checker_具型別宣告權威",
        "str_已禁止_copyable_及含_str_欄位的_struct_都不可隱含複製",
        "同一持有者的兩個獨占借用不得重疊",
    ],
    "F04-05": [
        "f04_公私正式_checker_具名讀取及賦值目標的_typed_use_證據逐筆一致",
        "f05_交集矩陣不可擅自擴大公開_mvp_a_初始化及賦值運算式",
        "真正詞彙宣告同時核對來源型別及寫入權限",
        "int_bool_float_名稱複製與一般重新賦值均保持來源有效",
    ],
}
# 只檢驗正式程式實際使用的單一來源，不能拿名稱文字自行簽出能力。
SOURCES = {
    "src/common_ir.rs": [
        "pub(crate) enum ScalarType",
        "pub(crate) fn arithmetic_operand_is_ready(",
        "pub(crate) fn boolean_condition_is_ready(",
        "pub(crate) fn scalar_return_is_ready(",
        "pub(crate) struct VerifiedScalarHir",
        "pub(crate) struct VerifiedTypedMember",
        "pub(crate) fn syntax_covers_verified_scalar_hir(",
    ],
    "src/types.rs": [
        "VerifiedScalarHir::for_mode(",
        "FrontendMode::PublicMvpA",
    ],
    "src/flow.rs": [
        "struct ReachableScalarHirView",
        "check_with_typed_hir_and_reachability",
    ],
    "src/struct_stage/safety_checker.rs": [
        "ScalarType::from_name(ty).is_some()",
        "fn checked_binding_use<",
        "fn check_direct(",
        "fn authorize_borrow(",
        "fn check_borrow_conflicts(",
        "fn checked_return_type(",
    ],
    "src/struct_stage.rs": [
        "fn operations_match_verified_ast(",
        "&& self.declarations.operations_match_verified_ast(structs, functions)",
        "fn member_sources_are_consistent(",
        "fn is_consistent(",
    ],
    "src/ownership_policy.rs": [
        "approved_leaf_copy_permission",
    ],
}


def sources_checked() -> None:
    for path, markers in SOURCES.items():
        text = (ROOT / path).read_text(encoding="utf-8")
        for marker in markers:
            if marker not in text:
                raise AssertionError(f"{path} 缺少真正式消費／權威：{marker}")
    core = (ROOT / "src/common_ir.rs").read_text(encoding="utf-8")
    for marker in SOURCES["src/common_ir.rs"][:4]:
        if core.count(marker) != 1:
            raise AssertionError(f"共同權威重複：{marker}")
    private = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    if 'matches!(ty, "int" | "bool" | "float" | "str")' in private:
        raise AssertionError("私有仍有第二份 int/bool 原始名稱白名單")
    if "fn known_type(&self, ty: &str) -> bool" not in private:
        raise AssertionError("不能用刪除私有型別驗證來假裝已共享")
    print("F04 真 Parser/Checker、Typed HIR/可達性、所有權守衛與原 AST 型態均存在；int/bool 第二份白名單已刪除")


def run(command: list[str]) -> str:
    result = subprocess.run(
        command, check=False, capture_output=True, encoding="utf-8",
        errors="replace", text=True,
    )
    if result.returncode != 0:
        raise AssertionError(
            f"真測試指令失敗（{result.returncode}）：{' '.join(command)}\n"
            + result.stdout[-2500:] + "\n" + result.stderr[-2500:]
        )
    return result.stdout + result.stderr


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式出場必須指定 --check")
    sources_checked()
    listing = run([args.test_bin, "--list"])
    tests = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for stage, markers in CASES.items():
        for name_suffix in markers:
            matches = [name for name in tests if name.endswith("::" + name_suffix)]
            if len(matches) != 1:
                raise AssertionError(f"{stage} 真 Rust 測試非唯一非空：{name_suffix}，實際 {len(matches)}")
            name = matches[0]
            if name in seen:
                raise AssertionError(f"重複採計同一測試：{name}")
            seen.add(name)
            result = run([args.test_bin, "--exact", name, "--test-threads=1"])
            if "1 passed; 0 failed" not in result:
                raise AssertionError(f"{stage} --exact 未成功：{name}\n{result[-1800:]}")
        print(f"{stage}：{len(markers)} 個互異正式真 Rust --exact 成功")
    if len(seen) != sum(map(len, CASES.values())) or len(CASES) != 5:
        raise AssertionError("五項或互異測試計數不符")
    print(f"A4/F04 候選：5/5 子項、{len(seen)} 個真 Rust 互異案例；"
          "既有原始 24 Phase／八橋接 57 事件仍須由原完整 CI 獨立執行")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F04 驗收失敗：{exc}", file=sys.stderr)
        sys.exit(1)
