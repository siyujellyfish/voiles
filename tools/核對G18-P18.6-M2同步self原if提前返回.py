#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-M2：同步 self 真 if/早退七個原 Parser／Checker 真動態來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_self_if_return_tests::"
CASES = (
    "m2_self_if_真true葉返回self最新欄位且不執行最後return",
    "m2_self_if_真false跳過提前return而執行已核准最後return",
    "m2_self_if_未命中算式溢位葉不應在true提前return後執行",
    "m2_self_if_false葉後續溢位必受控故障且先歸還同張self票據",
    "m2_self_if_方法入口注入故障不得假造未到達if條件或回傳",
    "m2_self_if_來源bool值或提前return位置遭篡改執行前必拒絕",
    "m2_self_if_條件錯型別與非法self欄位仍由原Checker先拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("真 Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if actual != expected:
            raise ValueError("M2 七項真來源漂移：缺 "
                + str(sorted(expected-actual)) + " 多 " + str(sorted(actual-expected)))
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(f"M2 真 if {i}/7 失敗：{case}\n" + out[-3600:])
            print(f"M2 同步self真分支 {i}/7：{case}（成功）")
        print("G18 P18.6 M2 原 Checker 真 if/早退七項完成；M2 全義務仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 M2 真方法分支 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
