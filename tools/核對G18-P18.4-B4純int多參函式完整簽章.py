#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4：不另設純 int helper 形參上限，沿用原 Checker 完整簽章。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_verified_int_helper_arity_tests::"
CASES = (
    "b4_arity_四個int原Checker形參在main獨立綁定並回傳13",
    "b4_arity_四個int動態方法形參在self存活期內真呼叫結果9",
    "b4_arity_四個int方法var_rhs與先後原子更新使用相同真框架",
    "b4_arity_五個形參與兩次四參helper重入均使用唯一宣告身分",
    "b4_arity_六個形參與混合正負實參保持原Parser位置序列",
    "b4_arity_首筆或最後一筆實參溢位都不可記錄未達helper進入",
    "b4_arity_四形參全綁定後本體溢位先退普通框架再還唯一self票",
    "b4_arity_錯誤實參數型別未知參數與篡改身分不得進Runtime",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "B4 多 int 原簽章測試漂移：缺 "
                + str(sorted(expected - actual)) + "；多 " + str(sorted(actual - expected))
            )
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
                    f"B4 多參純 int 真來源 {index}/8 失敗：{name}\n" + output[-4000:]
                )
            print(f"B4 多 int 原簽章及錯誤歸票 {index}/8：{name}（通過）")
        print("G18 B4 原 Checker 任意數純 int 形參子集八項完成；B4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 原 int 參數簽章 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
