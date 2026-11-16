#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／P18.6-M2/M3：方法原Checker bool Group 十項真執行。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_m3_grouped_bool_method_tests::"
CASES = (
    "m2_group_bool_兩個原Checker括號實參真選中內層helper",
    "m2_group_bool_內層false括號實參只跑真else函式",
    "m2_group_bool_外層false括號實參不進入任何子函式",
    "m2_group_bool_外層Group條件包裹真Checker布林形參",
    "m2_group_bool_內層Group條件仍使用原Checker獨立DeclId",
    "m2_group_bool_內外層Group常值條件與混合Group實參共同真執行",
    "m2_group_bool_重排bool_int形參及Group條件仍維持正確原Lexer位置",
    "m2_group_bool_async_self直接await原FIFO保留唯一真借票",
    "m2_group_bool_錯型別Group引數與未證名詞不得因解殼而放行",
    "m2_group_bool_準備後原Lexer括號遭改寫執行前不得進框架",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError(
                "Group bool 方法清冊缺少 " + str(sorted(expected-actual))
                + "；多出 " + str(sorted(actual-expected))
            )
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"Group bool 原Checker方法 {index}/10 {name} 失敗\n" + output[-7500:]
                )
            print(f"Group bool 原Checker方法 {index}/10：{name}（通過）")
        print("十項合法值葉、真借票、拒絕與恢復案例通過；B4/M2/M3/A1/A2仍未Closed。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 Group bool方法必要Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
