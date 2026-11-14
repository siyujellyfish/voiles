#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker FIFO 真完成／取消終態完整 poll 歷史防偽 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_fifo_terminal_poll_history_tests::"
CASES = (
    "t4_history_真Suspended後唯一Completed仍可正常退休並移交原P16結果",
    "t4_history_真已完成工作後再寫入Completed必拒絕且不可丟棄結果",
    "t4_history_真Completed後偽造Suspended即使最後不是完成也必拒絕",
    "t4_history_真受控CallBudget錯誤Completed只一次仍可合法退休",
    "t4_history_已Failed後重複Completed不能掩蓋原受控錯誤",
    "t4_history_合法已暫停後取消的歷史只有Suspended可安全退休",
    "t4_history_已真取消工作若歷史插入Completed不得用後續Suspended掩飾",
    "t4_history_shutdown先查所有終態完整歷史失敗不得部分取消他人",
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
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 FIFO 真 poll 歷史測試來源不一致：缺 "
                + str(sorted(expected - actual))
                + "；多 " + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            tested = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = tested.stdout + tested.stderr
            if tested.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T4 原 FIFO 終態歷史 {index}/8 失敗：{case}\n"
                    + output[-4000:]
                )
            print(f"T4 FIFO poll 真終態 {index}/8：{case}（通過）")
        print("G18 T4/V7-E2 完整 FIFO 驅動歷史八項完成；T4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 T4 FIFO 驅動歷史 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
