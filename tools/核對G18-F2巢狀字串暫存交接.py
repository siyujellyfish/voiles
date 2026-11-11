#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""F2 真巢狀 str-return 作為 str 實參：八項唯一 ResourceId 安全 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_nested_str_temporary_tests::"
CASES = (
    "f2_nested_str_單字串內層返回原id直接交付外層返回沒有複製",
    "f2_nested_str_原int返回混合str臨時與純值形參",
    "f2_nested_str_原bool返回接收同一真str臨時資源",
    "f2_nested_str_兩個獨立子返回以原形參順序交付唯一id",
    "f2_nested_str_內層已return但外層後序實參溢位零外層enter且釋放",
    "f2_nested_str_進入外層後故障逐框架只釋放唯一資源",
    "f2_nested_str_偽造暫存交付來源資源id被真trace拒絕",
    "f2_nested_str_原checker拒絕錯誤結果型別與非所有權實參",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(exe), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-2000:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(
                f"Rust 測試來源不符；缺少 {sorted(expected - actual)}，"
                f"多餘 {sorted(actual - expected)}"
            )
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"F2 巢狀 str {i}/{len(CASES)}：{case}\n" + output[-4000:]
                )
            print(f"F2 巢狀 str 真交接 {i}/{len(CASES)}：{case}（通過）")
        print("八項真 Gate 通過，F2 整體仍須逐項獨立正式驗收")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 巢狀 str Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
