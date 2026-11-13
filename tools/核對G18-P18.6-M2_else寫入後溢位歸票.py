#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：else 已寫入之後真 i64 算術與 Unwind 歸票八項原 Checker 正反。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_else_post_write_overflow_tests::"
CASES = (
    "m2_else_overflow_false選中else先寫入23後求值加5",
    "m2_else_overflow_true選中原true葉不執行else算術",
    "m2_else_overflow_false真寫入後加法溢位必受控Unwind及真還票",
    "m2_else_overflow_true跳過else必溢位算式及假寫入",
    "m2_else_overflow_false從真更新欄位計算減法可返回負整數",
    "m2_else_overflow_即使只有else可寫也必取得唯一獨占self票據",
    "m2_else_overflow_竄改原Checker已選else計畫必在真資源前拒絕",
    "m2_else_overflow_不合法self欄位與bool算術仍由原Checker拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1800:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "M2 else 真返回安全八項來源漂移：缺 "
                + str(sorted(expected-actual))
                + "；多 " + str(sorted(actual-expected))
            )
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(f"M2 else 真算術 {i}/8 失敗：{case}\n"+out[-3900:])
            print(f"M2 else 真寫入後算術 {i}/8：{case}（通過）")
        print("G18 M2 else 寫入後加減／溢位安全八項真正反完成，M2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("M2 else 寫入後溢位 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
