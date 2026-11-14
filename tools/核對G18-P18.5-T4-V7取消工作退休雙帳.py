#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.5-T4／V7-A2：原 Checker async 取消退休完整 R05／HC14／P16 雙帳 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_cancel_retire_proof_tests::"
CASES = (
    "t4_cancel_未啟動Prepared取消可交付None但須有唯一真取消紀錄",
    "t4_cancel_已真暫停必交付原Unwind與P16唯一ResourceId退出",
    "t4_cancel_已開始的工作刪除取消證據不得假退成Prepared",
    "t4_cancel_從未啟動的Prepared不能偽塞一份已執行取消證據",
    "t4_cancel_取消原ResourceId仍平衡但刪掉唯一Unwind不得退休",
    "t4_cancel_取消真FunctionExited錯置到Unwind之前即使資源張數相同亦拒絕",
    "t4_cancel_偽造Cancelled但從未實際取消且無歷史必拒絕",
    "t4_cancel_不同工作成功結果不得移植給未啟動的同源ID退休",
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
        actual = {x.removesuffix(": test") for x in listing.stdout.splitlines()
                  if x.startswith(PREFIX) and x.endswith(": test")}
        expected = {PREFIX + x for x in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 取消退休測試來源漂移：缺 " + str(sorted(expected-actual))
                + "；多 " + str(sorted(actual-expected))
            )
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T4 取消退休真雙帳 {i}/8 失敗：{case}\n" + output[-5000:]
                )
            print(f"T4 V7 取消退休真 R05/P16 證據 {i}/8：{case}（通過）")
        print("T4 V7 取消退休真來源／雙帳八項通過；T4、A1／A2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("T4 V7 取消退休 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
