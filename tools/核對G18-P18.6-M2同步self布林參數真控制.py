#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-M2：原 bool 形參在同步 self 中真選分支與歸票八例。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_bool_parameter_tests::"
CASES = (
    "m2_bool_真bool形參true驅動if選中並從self票據讀值",
    "m2_bool_真bool形參false不進早返葉並執行最後原Checker算式",
    "m2_bool_true時未選中溢位來源零效果",
    "m2_bool_false時已走到溢位必受控故障並真歸還self",
    "m2_bool_入口故障前已綁定原bool形參但不會假記條件",
    "m2_bool_兩次重跑原工作圖不共用BorrowTicket或bool值槽",
    "m2_bool_已核准形參動態值及DeclarationId遭篡改必在求值前拒絕",
    "m2_bool_原Checker拒絕int冒充bool及呼叫引數錯型別",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1800:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError("M2 bool 真來源測試名稱漂移：缺 "
                + str(sorted(expected-actual)) + " 多 " + str(sorted(actual-expected)))
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            out = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(
                    f"M2 bool 形參 {index}/8 真 Runtime 失敗：{case}\n" + out[-3600:]
                )
            print(f"M2 原 bool 真分支 {index}/8：{case}（真正通過）")
        print("G18 P18.6 M2 bool 形參真分支八項 Gate 成功；完整 M2 保留 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("M2 bool 真參數 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
