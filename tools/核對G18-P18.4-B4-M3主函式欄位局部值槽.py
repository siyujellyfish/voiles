#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4 B4／P18.6 M3：原Checker main int局部真owner欄位八項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_owner_local_int_read_tests::"
CASES = (
    "b4_owner_local_原Checker預設欄位直接初始化const整數真值槽",
    "b4_owner_local_取值後原欄位再寫入不得污染已初始化const快照",
    "b4_owner_local_未初始化var於原Checker首次賦值讀取欄位加法",
    "b4_owner_local_前後欄位更新與var重賦必重讀同一真P16物件",
    "b4_owner_local_原Checker欄位值供main同步helper初始化再到方法",
    "b4_owner_local_欄位整數初始化溢位不得進方法或建立自借票",
    "b4_owner_local_原Checker非法欄位不得賦給main整數局部",
    "b4_owner_local_已封存Checker欄位Initializer身分遭偷換必先SourceChanged",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 無法列出真測試：" + listed.stderr[-3000:])
        required = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 8 or required != actual:
            raise ValueError(
                "main局部欄位讀取 Gate 缺失=" + str(sorted(required - actual))
                + "；多出=" + str(sorted(actual - required))
            )
        for i, case in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"main owner.int局部 {i}/8 {case} 失敗\n" + output[-7000:])
            print(f"G18 B4/M3 main局部真P16欄位 {i}/8：{case} 通過")
        print("main Copyable int 宣告／賦值真欄位交叉八項成功；B4/M3 仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main欄位局部值槽 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
