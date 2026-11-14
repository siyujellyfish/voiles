#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker wake 全域 poll 時序八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_wake_chronology_tests::"
CASES = (
    "t4_chrono_首次真喚醒必在原Suspended後並於完成前",
    "t4_chrono_偽造時間點早於真Suspended必在drive前拒絕",
    "t4_chrono_偽造未來時間點不得讓已暫停工作執行",
    "t4_chrono_完成工作假喚醒晚於自身下一poll不得退休",
    "t4_chrono_取消工作偽造早於第一poll的wake不得退休",
    "t4_chrono_兩工作在同次全域poll刻度真喚醒皆可依序執行",
    "t4_chrono_兩工作跨poll真喚醒憑據保持全域非遞減時序",
    "t4_chrono_整批關閉遇跨工作wake倒序須全員零取消",
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
        listed = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or listed != expected:
            raise ValueError(
                "真 wake 時序測試不一致：缺 " + str(sorted(expected - listed))
                + "；多 " + str(sorted(listed - expected))
            )
        for number, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"時序 {number}/8 失敗：{case}\n" + output[-4000:]
                )
            print(f"T4 真時序 {number}/8：{case}（通過）")
        print("原 Checker wake 全域 poll 時序八項通過；原 T4 尚未全面閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 wake 時序 Gate 未通過：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
