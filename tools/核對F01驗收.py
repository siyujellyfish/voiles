#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P16.6-R02／A1：依真正 Rust 測試來源嚴格驗證 F01-01～F01-06。

只讀取已編譯內部測試清單並重新執行每個具名來源。
不更新 G0 快照、不自行判定未核准語意、不以空測試視為成功。
"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

# 每個 F01 子項至少有獨立的成功／拒絕證據；跨項目可以引用同一
# 真實 Checker 測試，但只實際執行一次以免延長 CI 等待。
REQUIRED = {
    "F01-01": [
        "f01_公私正式共同函式與宣告維持相同跨度但模式身分互不混淆",
        "a1_f01_所有已核准角色由真_ast_token_與_checker_hir_生成且禁止假宣告",
    ],
    "F01-02": [
        "f01_正式區塊所有直接語句與私有方法來源皆保留共用模式身分",
        "f01_公私巢狀_ast_欄位_參數_所有節點皆由相同來源身分稽核",
        "a1_f01_遮蔽宣告的兩次具名引用必須分別對應真實_ast_身分",
        "a1_f01_原始語法錯誤須保留公私診斷先後種類與_utf8_位置",
    ],
    "F01-03": [
        "f01_欄位與方法參數型別必須逐_token_證明原始來源不可竄換",
        "a1_f01_私有成員與方法參數必須有相同_lexer_token_及正確模式",
    ],
    "F01-04": [
        "f01_f04_欄位及方法與函式參數的型別證據來自唯一_checker_且不能授權資源",
        "f11_完整_checker_hir_必須與真實宣告型別及_const_var_權限逐筆相符",
        "a1_f01_真_ast_來源與_具型別_hir_不得由合法字元範圍單獨偽造",
    ],
    "F01-05": [
        "a1_f01_真正公開_parser_函式與所有宣告名稱型別_token_正反驗證",
        "a1_f01_真正私有_parser_方法與巢狀宣告_token_來源不可竄改",
        "a1_f01_原始位元組與_lexer_token_一致性防止符號種類被偽造",
    ],
    "F01-06": [
        "f01_f04_真正公私_checker_簽出不可跨模式冒用的完整_typed_hir",
        "a1_f01_真_ast_來源與_具型別_hir_不得由合法字元範圍單獨偽造",
        "a1_f01_遮蔽宣告的兩次具名引用必須分別對應真實_ast_身分",
    ],
}


def run_command(binary: Path, arguments: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary.resolve()), *arguments],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="strict",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式 F01 驗收只能使用 --check；不支援修改快照")
    if not args.test_bin.is_file():
        parser.error(f"真正 Rust 內部測試執行檔不存在：{args.test_bin}")
    listing = run_command(args.test_bin, ["--list"])
    if listing.returncode != 0:
        print(listing.stdout, end="", file=sys.stderr)
        print(listing.stderr, end="", file=sys.stderr)
        raise RuntimeError("Rust --list 實際執行失敗")

    # Rust 測試輸出格式：模組路徑::具名測試: test
    names = [
        line.removesuffix(": test")
        for line in listing.stdout.splitlines()
        if line.endswith(": test")
    ]
    if not names:
        raise RuntimeError("Rust --list 沒有真實測試，拒絕空集合成功")

    resolved: dict[str, list[str]] = {}
    to_run: set[str] = set()
    for item_id, suffixes in REQUIRED.items():
        if not suffixes:
            raise RuntimeError(f"{item_id} 沒有驗收測試，不得以空表簽核")
        current = []
        for suffix in suffixes:
            matches = [name for name in names if name.endswith("::" + suffix)]
            if len(matches) != 1:
                raise RuntimeError(
                    f"{item_id} 具名測試 {suffix!r} 預期唯一，實際 {len(matches)}")
            current.append(matches[0])
            to_run.add(matches[0])
        resolved[item_id] = current

    for name in sorted(to_run):
        ran = run_command(args.test_bin, ["--exact", name, "--test-threads=1"])
        if ran.returncode != 0 or "1 passed; 0 failed" not in ran.stdout:
            print(ran.stdout, end="", file=sys.stderr)
            print(ran.stderr, end="", file=sys.stderr)
            raise RuntimeError(f"F01 具名原始 Rust 案例未實際成功：{name}")

    print(json.dumps({
        "gate": "P16.6-R02-A1-F01",
        "result": "Passed",
        "github_sha": os.environ.get("GITHUB_SHA", "local-unbound"),
        "subitems": {key: len(value) for key, value in resolved.items()},
        "unique_rust_tests_executed": len(to_run),
        "frozen_g0": "此工具不替代 G0 的四套 --check",
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
