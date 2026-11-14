#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／M3：無方法局部直接 Return helper 真退出八項 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_final_helper_without_locals_tests::"
CASES = (
    "b4_final_無方法局部直接Return雙參數helper真返回7",
    "b4_final_零參數原Checker純helper也能當方法末尾直接Return",
    "b4_final_四個獨立int形參可先求全實參再進普通函式",
    "b4_final_方法已獨占更新self欄位再Return純helper只借還同一票",
    "b4_final_if_true提前回self欄位不得執行未達最後helper",
    "b4_final_最後引數溢位未進helper但仍真還self票",
    "b4_final_helper本體溢位先退普通函式再歸還唯一借用票",
    "b4_final_原Checker錯型別巢狀函式與竄改方法Return計畫必拒絕",
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
                "方法無局部 return-helper 測試漂移：缺 "
                + str(sorted(expected-actual)) + "；多 " + str(sorted(actual-expected))
            )
        for i, name in enumerate(CASES, 1):
            tested = subprocess.run(
                [str(binary), "--exact", PREFIX+name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = tested.stdout + tested.stderr
            if tested.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"無局部 Return helper {i}/8 失敗：{name}\n" + output[-4000:])
            print(f"B4/M3 方法無局部真 Return {i}/8：{name}（通過）")
        print("B4/M3 方法無局部末尾直接純 helper Return 八項通過；B4/M3 未閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("B4/M3 最後 Return helper Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
