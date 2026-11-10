#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.1-F2：原 Checked str-return 提前分支、巢狀分支與資源正反 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_early_return_tests::"
CASES = (
    "f2_str_early_if真分支只建立與返回選定的utf8",
    "f2_str_early_if假分支回到尾端真move維持唯一resourceid",
    "f2_str_early_else_if完整三分支僅各自真返回且逆序釋放",
    "f2_str_early局部move提前返回與旁觀資源各自唯一釋放",
    "f2_str_early內層條件真返回僅選擇原checker分支",
    "f2_str_early溢位不執行未達分支返回且真清理已建str",
    "f2_str_early原checker拒絕分支錯型別與隱含str複製",
    "f2_str_early篡改提前返回resourceid即使資源數相等仍拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(exe), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2000:])
        observed = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if observed != expected:
            raise ValueError(
                f"原 Rust 測試清冊不符：缺 {sorted(expected - observed)}、"
                f"多 {sorted(observed - expected)}"
            )
        for index, case in enumerate(CASES, 1):
            cmd = [str(exe), "--exact", PREFIX + case, "--test-threads=1"]
            run = subprocess.run(
                cmd, text=True, encoding="utf-8",
                capture_output=True, check=False,
            )
            result = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
                result,
            ):
                raise ValueError(
                    f"原 Rust {index}/{len(CASES)} 驗收失敗 {case}\n{result[-2500:]}"
                )
            print(f"F2 str-return 提前返回 {index}/{len(CASES)}：{case}（通過）")
        print("F2 str-return 提前返回八項已驗證；F2 全矩陣尚未閉合")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 str-return 提前返回 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
