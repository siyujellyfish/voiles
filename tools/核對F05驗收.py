#!/usr/bin/env python3
"""A5/F05-01～05：真公私 Checker、重複可寫權威清除與 CI 模式驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "F05-01": [
        "f05_公私正式_checker_相同已核准純值來源產生相同型別與半開位置證據",
        "公開與私有正式運算式真正共用節點外殼與來源位置",
    ],
    "F05-02": [
        "f05_全部已核准純值交集_公私_checker_完整_hir_節點逐一相等",
        "f05_交集矩陣不可擅自擴大公開_mvp_a_初始化及賦值運算式",
        "相同來源跨度的讀寫由節點角色隔離但共用真正已解析表",
    ],
    "F05-03": [
        "f03_03_多層_else_if_else_真鏈與原私有合流",
        "f04_公私正式_checker_具名讀取及賦值目標的_typed_use_證據逐筆一致",
        "真實_checker_移動_返回_覆寫_都具有原始_ast_逐筆來源",
    ],
    "F05-04": [
        "f03_04_不完整區塊先經原_lexer_再由雙_parser_拒絕",
        "同型別但沒有_move_的目標不得冒充另一筆已核准移動",
        "非_return_move_以及非欄位寫入來源不得偽造資源責任",
    ],
    "F05-05": [
        "f01_f04_真正公私_checker_簽出不可跨模式冒用的完整_typed_hir",
        "f01_f04_欄位及方法與函式參數的型別證據來自唯一_checker_且不能授權資源",
        "f04_公開_p11_可達_hir_不將不可達型別事實冒充已執行且拒絕私有簽章",
    ],
}
MARKERS = {
    "src/common_ir.rs": [
        "macro_rules! define_shared_expression_kind",
        "pub(crate) struct VerifiedScalarHir",
        "pub(crate) enum ScalarType",
        "pub(crate) fn syntax_covers_verified_scalar_hir(",
        "pub(crate) struct ModeNodeIdentity",
    ],
    "src/syntax_shared.rs": [
        "pub(crate) fn parse_function_decl<",
        "pub(crate) fn parse_declaration<",
        "pub(crate) fn parse_named_assignment<",
        "pub(crate) fn parse_return<",
        "pub(crate) fn parse_if_chain<",
        "pub(crate) fn parse_block<",
        "pub(crate) fn parse_named_call<",
        "pub(crate) fn parse_scalar_atom<",
        "pub(crate) fn statement_head(",
    ],
    "src/parser.rs": [
        "FrontendMode::PublicMvpA",
        "parse_function_decl(self)",
        "parse_declaration(self)",
        "parse_named_assignment(self)",
        "parse_return(self)",
        "parse_if_chain(self)",
        "parse_block(self)",
    ],
    "src/struct_stage/syntax.rs": [
        "FrontendMode::PrivateVerified",
        "parse_function_decl(self)",
        "parse_declaration(self)",
        "parse_named_assignment(self)",
        "parse_return(self)",
        "parse_if_chain(self)",
        "parse_block(self)",
        "ParsedElse::ElseIf",
        "ParsedElse::Block",
    ],
    "src/struct_stage/safety_checker.rs": [
        "fn resolve_empty_call(",
        "ScalarType::from_name(ty).is_some()",
        "fn checked_binding_use<",
        "fn check_direct(",
        "fn authorize_borrow(",
        "fn check_borrow_conflicts(",
    ],
    "src/struct_stage.rs": [
        "fn operations_match_verified_ast(",
        "fn member_sources_are_consistent(",
        "fn is_consistent(",
    ],
    "src/struct_stage/ir_projection.rs": [
        "types::check_with_names_and_hir(",
        "verified_private_source(",
    ],
    "src/flow.rs": [
        "struct ReachableScalarHirView",
    ],
}
PARSER_AUTHORITY = MARKERS["src/syntax_shared.rs"]
OLD_AST_TYPES = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|type)\s+"
    r"(?:SharedAst|ScalarExpr|ScalarStmt)\b"
)


def authority_check() -> None:
    source_map = {}
    for path, markers in MARKERS.items():
        contents = (ROOT / path).read_text(encoding="utf-8")
        source_map[path] = contents
        for marker in markers:
            if marker not in contents:
                raise AssertionError(f"{path} 缺少原始真路徑／唯一權威：{marker}")
        if OLD_AST_TYPES.search(contents):
            raise AssertionError(f"{path} 仍定義測試期第二棵語意樹權威")
    shared = source_map["src/syntax_shared.rs"]
    for marker in PARSER_AUTHORITY:
        if shared.count(marker) != 1:
            raise AssertionError(f"共同 AST 正式生產者並非唯一：{marker}")
    common = source_map["src/common_ir.rs"]
    if common.count("macro_rules! define_shared_expression_kind") != 1:
        raise AssertionError("共用正式 Expression enum 建構巨集有多份")
    if common.count("pub(crate) enum ScalarType") != 1:
        raise AssertionError("共用 int/bool 名稱與型別枚舉不是一份")
    if source_map["src/struct_stage/safety_checker.rs"].count("fn resolve_empty_call(") != 1:
        raise AssertionError("private Struct()／Call() 消歧仍存在第二份可寫 Checker")
    if "no: Option<Vec<Stmt>>" in source_map["src/struct_stage.rs"]:
        raise AssertionError("正式私有 else-if 又退回無來源的 Vec AST")
    if "StatementHead::ModeSpecific" in source_map["src/struct_stage/syntax.rs"]:
        raise AssertionError("私有迴圈仍在原分類之外二次猜語句頭")
    if 'matches!(ty, "int" | "bool" | "float" | "str")' in source_map["src/struct_stage/safety_checker.rs"]:
        raise AssertionError("私有 Checker 重建了 int/bool 第二份型別白名單")
    flow = (ROOT / ".github/workflows/前端建置.yml").read_text(encoding="utf-8")
    for marker in [
        "  push:\n    branches:\n      - main",
        "tools/核對F01驗收.py",
        "tools/核對F02驗收.py",
        "tools/核對F03驗收.py",
        "tools/核對F04驗收.py",
        "tools/核對F05驗收.py",
        "tools/核對G1G2驗收.py",
        "tools/核對F00總驗收.py",
    ]:
        if marker not in flow:
            raise AssertionError(f"缺少同 SHA 主線 Rust 或五個獨立驗收入口：{marker}")
    print("F05 可寫權威：共通9項 parser 分別唯一；正式公私消費與私有專用消歧閉合，舊投影型別無可寫定義")
    print("F05 CI：main push 與 F00/F01～F05/G1/G2 獨立真工作掛載")


def run(command: list[str]) -> str:
    result = subprocess.run(
        command, check=False, text=True, capture_output=True,
        encoding="utf-8", errors="replace",
    )
    if result.returncode:
        raise AssertionError(
            f"真測試執行失敗（{result.returncode}）：{' '.join(command)}\n"
            + result.stdout[-2500:] + "\n" + result.stderr[-2500:]
        )
    return result.stdout + result.stderr


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式驗收必須指定 --check")
    authority_check()
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for subitem, cases in CASES.items():
        for suffix in cases:
            found = [test for test in names if test.endswith("::" + suffix)]
            if len(found) != 1:
                raise AssertionError(f"{subitem} 非唯一或空的真 Rust 測試：{suffix}，找到 {len(found)}")
            name = found[0]
            if name in seen:
                raise AssertionError(f"跨子項重複借用同一真實測試：{name}")
            seen.add(name)
            result = run([args.test_bin, "--exact", name, "--test-threads=1"])
            if "1 passed; 0 failed" not in result:
                raise AssertionError(f"{subitem} 未執行真正 --exact：{name}\n{result[-1800:]}")
        print(f"{subitem}：{len(cases)} 個互異真 Parser/Checker Rust --exact 成功")
    if len(CASES) != 5 or len(seen) != sum(len(x) for x in CASES.values()):
        raise AssertionError("F05 五子項或總唯一案例數錯誤")
    print(f"F05 候選：5/5 子項，{len(seen)} 個互異真 Rust 測試；"
          "G1/G2 與 G0 仍須真 CI 自行重跑並比對最新 main SHA")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F05/G2 未驗收：{exc}", file=sys.stderr)
        sys.exit(1)
