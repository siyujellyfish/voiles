#!/usr/bin/env python3
"""A3/F03-01～05：真 Rust 正反案例與公私 Parser 共用權威驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PREFIX = "a3_f03_五項正式驗收::"
CASES = {
    "F03-01": [
        "f03_01_合法函式與返回共用唯一真_ast",
        "f03_01_私有參數合法但公開拒絕_無表面語法擴張",
    ],
    "F03-02": [
        "f03_02_局部宣告與賦值_真解析後同一來源範圍",
        "f03_02_非法宣告與私有欄位的模式守衛",
    ],
    "F03-03": [
        "f03_03_多層_else_if_else_真鏈與原私有合流",
        "f03_03_缺少_else_區塊仍由兩真_parser_拒絕",
    ],
    "F03-04": [
        "f03_04_深層大括號區塊與真來源半開跨度",
        "f03_04_不完整區塊先經原_lexer_再由雙_parser_拒絕",
    ],
    "F03-05": [
        "f03_05_真私有迴圈跳轉及公開模式隔離",
        "f03_05_迴圈外_break_continue_真_checker_拒絕且診斷不變",
    ],
}
MARKERS = {
    "src/syntax_shared.rs": [
        "pub(crate) fn parse_function_decl<",
        "pub(crate) fn parse_return<",
        "pub(crate) fn parse_declaration<",
        "pub(crate) fn parse_named_assignment<",
        "pub(crate) fn parse_if_chain<",
        "pub(crate) fn parse_block<",
        "pub(crate) fn statement_head(",
        "ForRange,",
        "WhileLoop,",
        "Break,",
        "Continue,",
    ],
    "src/parser.rs": [
        "parse_function_decl(self)",
        "parse_declaration(self)",
        "parse_named_assignment(self)",
        "parse_if_chain(self)",
        "parse_return(self)",
        "parse_block(self)",
        "FrontendMode::PublicMvpA",
    ],
    "src/struct_stage/syntax.rs": [
        "parse_function_decl(self)",
        "parse_declaration(self)",
        "parse_named_assignment(self)",
        "parse_if_chain(self)",
        "parse_return(self)",
        "parse_block(self)",
        "FrontendMode::PrivateVerified",
        "ParsedElse::Block",
        "ParsedElse::ElseIf",
    ],
    "src/struct_stage.rs": [
        "no: Option<crate::syntax_shared::ParsedElse<Stmt>>",
        "body: crate::syntax_shared::ParsedBlock<Stmt>",
    ],
}


def authority_check() -> None:
    contents = {}
    for path, patterns in MARKERS.items():
        source = (ROOT / path).read_text(encoding="utf-8")
        contents[path] = source
        for pattern in patterns:
            if pattern not in source:
                raise AssertionError(f"來源缺失：{path} 的 {pattern}")
    shared = contents["src/syntax_shared.rs"]
    for pattern in MARKERS["src/syntax_shared.rs"][:7]:
        if shared.count(pattern) != 1:
            raise AssertionError(f"共同 Parser 可寫定義數不是一：{pattern}")
    if "StatementHead::ModeSpecific" in contents["src/struct_stage/syntax.rs"]:
        raise AssertionError("私有語句仍使用舊的二次詞彙分類")
    if "no: Option<Vec<Stmt>>" in contents["src/struct_stage.rs"]:
        raise AssertionError("私有 if/else 退回不可追溯的 Vec AST")
    print("A3/F03：7 個唯一共用來源、雙正式 Parser 與 CFG 來源形狀通過")


def run(command: list[str]) -> str:
    result = subprocess.run(
        command, capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    )
    if result.returncode:
        raise AssertionError(
            f"指令失敗（{result.returncode}）：{' '.join(command)}\n"
            + result.stdout[-3000:] + "\n" + result.stderr[-3000:]
        )
    return result.stdout + result.stderr


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    authority_check()
    listing = run([args.test_bin, "--list"])
    names = [row[:-6] for row in listing.splitlines() if row.endswith(": test")]
    seen: set[str] = set()
    for subitem, required in CASES.items():
        for marker in required:
            matches = [name for name in names if name.endswith(PREFIX + marker)]
            if len(matches) != 1:
                raise AssertionError(
                    f"{subitem} 真 Rust --list 非唯一非空：{marker}，找到 {len(matches)}"
                )
            name = matches[0]
            if name in seen:
                raise AssertionError(f"跨子項重複計數：{name}")
            seen.add(name)
            output = run([args.test_bin, "--exact", name, "--test-threads=1"])
            if "1 passed; 0 failed" not in output:
                raise AssertionError(f"{subitem} --exact 未完成：{name}\n{output[-2000:]}")
        print(f"{subitem}：{len(required)} 個獨立真 Rust 案例通過")
    if len(seen) != 10 or len(CASES) != 5:
        raise AssertionError("正式矩陣不可漏子項或重複計數")
    print("A3/F03 候選：5/5 子項、10/10 個互異真 Rust --exact。G0 由原 CI 另行重跑。")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F03 驗收失敗：{exc}", file=sys.stderr)
        sys.exit(1)
