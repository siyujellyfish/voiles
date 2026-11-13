#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：原 Checker 條件 true 葉獨占 self 欄位寫入及 false 無副作用八項真正反。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_conditional_write_tests::"
CASES = (
    "m2_write_true葉選中才取得獨占self並真寫入再讀取",
    "m2_write_false即使方法取得獨占票據也不執行true葉寫入",
    "m2_write_false無else時繼續讀取原欄位而非幽靈寫入",
    "m2_write_true提前返回不得求值未到達之溢位算式",
    "m2_write_方法入口注入故障尚未選葉不許寫入但須真還票",
    "m2_write_真欄位值與Lexer跨度被竄改須在執行前拒絕",
    "m2_write_原Checker拒絕const欄位及不可寫接收者",
    "m2_write_未核准的else欄位寫入仍保守拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1700:])
        available = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        required = {PREFIX + case for case in CASES}
        if len(required) != 8 or available != required:
            raise ValueError("M2 分支獨占來源漂移：缺 "
                + str(sorted(required-available)) + "；未登錄 "
                + str(sorted(available-required)))
        for i, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"M2 true 葉獨占寫入 {i}/8 失敗：{name}\n"+output[-3600:])
            print(f"M2 真分支獨占 self {i}/8：{name}（通過）")
        print("M2 原 Checker 真條件寫入八項 Gate 完成；M2 整項仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as error:
        print("M2 分支獨占自借用 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
