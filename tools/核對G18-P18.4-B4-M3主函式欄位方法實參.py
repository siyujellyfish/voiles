#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：main owner.int 方法實參真 Checker→P16／HC14 八項獨立 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_owner_method_argument_tests::"
CASES = (
    "b4_owner_method_arg_預設真int欄位直接作step並於自借前求值",
    "b4_owner_method_arg_先寫入十一後以同一真持有者欄位做實參",
    "b4_owner_method_arg_原Lexer雙層Group與欄位算術作實參",
    "b4_owner_method_arg_原P13禁止巢狀欄位helper作方法引數須靜態拒絕",
    "b4_owner_method_arg_前序helper寫入後欄位值再計算方法step",
    "b4_owner_method_arg_實參欄位加法溢位不可進方法或簽self票",
    "b4_owner_method_arg_封存Checker欄位引數遭改綁必拒絕副作用前",
    "b4_owner_method_arg_不存在欄位不能冒充原Checker已核准int實參",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        required = {PREFIX + case for case in CASES}
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 8 or found != required:
            raise ValueError(
                "方法欄位實參 Gate 缺失=" + str(sorted(required - found))
                + "；意外新增=" + str(sorted(found - required))
            )
        for i, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            printed = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", printed
            ):
                raise ValueError(f"真方法實參 {i}/8 {name} 失敗\n" + printed[-8000:])
            print(f"原Checker真owner.int方法實參 {i}/8：{name} 通過")
        print("B4/M3 八項真 Checker→P16→HC14 Gate 全部成功；原 B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4/M3 方法欄位實參 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
