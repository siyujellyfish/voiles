#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：原 Checker 同步 self if/else 真分支、提前返回與歸票八例。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_if_else_tests::"
CASES = (
    "m2_else_bool_true走真self早退未執行else",
    "m2_else_bool_false走原else整數返回不落至最後return",
    "m2_else_false已返回不提前求值不可能到達之溢位",
    "m2_else_true分支不受未選中合法最大整數葉影響",
    "m2_else_bool與常值if共用真分支判斷器但不混淆宣告身分",
    "m2_else_方法入口故障保留唯一真票據歸還且不可假造分支",
    "m2_else_修改已驗證else返回值或跨度須執行前拒絕",
    "m2_else_原Checker拒絕else回傳bool與錯誤bool實參",
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
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if actual != expected or len(expected) != 8:
            raise ValueError(
                "M2 if/else 真來源遺失或重複：缺 " + str(sorted(expected - actual))
                + " 多 " + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(
                    f"M2 if/else 第 {index}/8 筆失敗：{case}\n" + out[-3600:]
                )
            print(f"M2 真 if/else {index}/8：{case}（通過）")
        print("M2 原 Parser/Checker 同步 self if/else 八項真執行正反通過；完整 M2 不提前 Closed")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 M2 真 if/else Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
