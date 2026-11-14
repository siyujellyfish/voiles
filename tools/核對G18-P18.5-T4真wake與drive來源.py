#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker FIFO wake／drive_one 真來源完整性八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_verified_wake_drive_history_tests::"
CASES = (
    "t4_drive_合法Prepared第一次drive只真執行至await並保留checkpoint",
    "t4_drive_真Suspended喚醒後才重新排隊並完成同一工作",
    "t4_drive_偽Completed歷史混入真Suspended後wake須拒絕且零入列",
    "t4_drive_有真checkpoint但假取消歷史不得以wake重取執行權",
    "t4_drive_真暫停後被偽標Prepared不得重新消耗第一個poll",
    "t4_drive_同一工作重複入列不能讓drive_one消耗兩個token",
    "t4_drive_未執行Prepared被塞入假Unwind不得污染FIFO後一份工作",
    "t4_drive_真完成工作偽標Prepared及重入列不得二次執行原結果",
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
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 真醒來／驅動來源測試不一致：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T4 真 wake／drive {index}/8 失敗：{case}\n" + output[-4000:]
                )
            print(f"T4 FIFO 真 wake／drive {index}/8：{case}（通過）")
        print("G18 T4/V7-E2 真 wake／drive 完整來源八項通過；T4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 T4 wake／drive 真來源 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
