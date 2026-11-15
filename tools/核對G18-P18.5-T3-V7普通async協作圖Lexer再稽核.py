#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／V7：普通 async 真圖與 FIFO 封存 Lexer 十項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t3_scalar_async_frozen_token_recheck_tests::"
CASES = (
    "t3_scalar_token_原Checker普通async直接await實執行與FIFO喚醒完整通過",
    "t3_scalar_token_封存Lexer的await前綴Name遭改即拒",
    "t3_scalar_token_封存Lexer的async函式被呼叫名稱遭改即拒",
    "t3_scalar_token_封存Lexer呼叫左括號遭改不可啟動",
    "t3_scalar_token_封存Lexer呼叫右括號遭改不可啟動",
    "t3_scalar_token_封存async函式Keyword身分遭改不可建立真圖",
    "t3_scalar_token_把原awaitCall_AST改成同步Call不得重播",
    "t3_scalar_token_偷換原AST呼叫函式名稱但Token未變必拒",
    "t3_scalar_token_偷換封存AST函式async宣告旗標不得偽造同步圖",
    "t3_scalar_token_工作提交時非法原Token不得消耗ID或ready授權",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1800:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 10 or actual != expected:
            raise ValueError("T3 普通async 原圖 Gate 缺 "
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
                    f"T3 async 原圖 {index}/10 {name} 失敗\n" + output[-4800:]
                )
            print(f"T3/V7 普通async 真詞彙 {index}/10：{name}（通過）")
        print("T3/V7 原Checker普通async圖與FIFO十項通過，原T3/A1/A2仍未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("T3/V7 普通async封存Lexer Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
