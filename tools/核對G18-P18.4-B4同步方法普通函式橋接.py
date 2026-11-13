#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4 首批：原 Checker 普通 fn 巢狀同步 self 的七條真執行正反。"""
from __future__ import annotations
import argparse
import re
import subprocess
import sys
from pathlib import Path

PREFIX = "struct_stage::g18_b4_mixed_self_call_tests::"
CASES = (
    "b4_真正普通helper在同步self借用存活時巢狀執行返回",
    "b4_同步方法注入失敗必在內層普通函式未進入前還self票據",
    "b4_原Checker普通函式返回工作計畫遭竄改不可偽造真結果",
    "b4_同步self兩次執行各自產生新真票據與巢狀返回",
    "b4_普通函式不存在或方法返回型別不符仍由原checker拒絕",
    "b4_尚未證明的普通函式參數與可溢位計畫仍保守拒絕",
    "b4_造假普通函式漏返回或重播來源即使數量相似亦應拒絕",
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
            raise ValueError("Rust --list 失敗：" + listed.stderr[-2000:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if actual != expected:
            raise ValueError("B4 測試模組清單不一致：缺 "
                + str(sorted(expected-actual)) + " 多 " + str(sorted(actual-expected)))
        for index, case in enumerate(CASES, 1):
            executed = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = executed.stdout + executed.stderr
            if executed.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B4 受限真動態橋接 {index}/7 失敗：{case}\n" + output[-3600:]
                )
            print(f"B4 真同步方法＋普通 fn {index}/7：{case}（通過）")
        print("B4 最小同步混合呼叫 Gate 成功；完整 B4 及 P18.4 仍不得提前 Closed/Passed")
        return 0
    except (OSError, ValueError) as error:
        print("B4 真方法混合橋接 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
