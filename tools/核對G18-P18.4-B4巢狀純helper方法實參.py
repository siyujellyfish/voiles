#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4/M3：有限層純 int 巢狀 helper／同步 self 真退出八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_nested_pure_helper_method_argument_tests::"
CASES = (
    "b4_nested_兩個不同普通函式由內而外真完成後才借self",
    "b4_nested_反向函式身分不可誤用另一helper的計算圖",
    "b4_nested_三層有限原Parser工作圖逐一真進退且不重借self",
    "b4_nested_同一父函式左右兩個巢狀實參先各自完成",
    "b4_nested_第一子函式完成但第二實參溢位父函式不得假進入",
    "b4_nested_子函式本體溢位只退出真子框架不會進父函式",
    "b4_nested_父函式本體溢位須保留子函式成功退出帳",
    "b4_nested_含巢狀函式算術包裝資源副作用及偽造原計畫均拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("B4/M3 巢狀測試不一致：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected)))
        for index, case in enumerate(CASES,1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"B4/M3 {index}/8 {case} 失敗\n" + output[-4000:])
            print(f"B4/M3 巢狀真退出 {index}/8：{case}（通過）")
        print("G18 B4/M3 原 Checker 純 int 巢狀方法引數八項通過；B4/M3 未全閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4/M3 巢狀 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
