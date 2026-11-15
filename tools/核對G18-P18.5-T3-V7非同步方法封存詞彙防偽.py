#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／V7：真Checker async self來源AST/Lexer封存十一項防偽。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t3_async_self_token_provenance_recheck_tests::"
CASES = (
    "t3_async_token_原Checker六相鄰詞彙及同步舊來源均可真執行",
    "t3_async_token_偷改await前綴Name不得當成合法等待來源",
    "t3_async_token_偷改接收者Token雖原AST未改也須拒絕",
    "t3_async_token_偷改點號Token不得越過方法安全Checker",
    "t3_async_token_偷改方法名Token但TypedHIR不變仍須拒絕",
    "t3_async_token_偷改左括號Token不得認領真位置參數",
    "t3_async_token_偷改尾括號Token但返回跨度相同仍須拒絕",
    "t3_async_token_偷改方法async宣告Token也不能忽略原來源",
    "t3_async_token_偷換AST接收者但原Lexer文字不變必拒",
    "t3_async_token_偷換AST方法名但位置與票據不變必拒",
    "t3_async_token_偷抹await_AST身分改同步MethodCall必拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1700:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 11 or actual != expected:
            raise ValueError("T3/V7 封存詞彙 Gate 缺 "
                + str(sorted(expected-actual)) + "；多 "
                + str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T3/V7 async self 身分 {index}/11 {name} 失敗\n" + output[-4500:]
                )
            print(f"T3/V7 封存 AST/Lexer {index}/11：{name}（通過）")
        print("T3/V7 async self 原Checker封存詞彙十一項通過；T3、A1/A2原義務未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("T3/V7 async self 封存詞彙 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
