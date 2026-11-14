#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4-M3：多個原 Checker 純 int helper 與同步 self 八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_multiple_approved_helpers_tests::"
CASES = (
    "b4_multi_兩個不同原Checker純函式依名稱分別返回並借入self",
    "b4_multi_主函式各具名局部先真執行不同helper再借self",
    "b4_multi_方法真局部與末尾Return各呼叫不同helper共用歸票",
    "b4_multi_原Checker布林選枝各使用不同普通helper且未選零事件",
    "b4_multi_前一不同函式完成後第二求參溢位不得提前借self",
    "b4_multi_第二函式本體溢位不得把第一函式成功回滾掉",
    "b4_multi_不存在的函式與巢狀helper及非純值helper仍拒絕",
    "b4_multi_竄改任一助手原DeclId或返回計畫必在真執行前拒絕",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1200:])
        actual = {
            x.removesuffix(": test") for x in listing.stdout.splitlines()
            if x.startswith(PREFIX) and x.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "多純值 helper 原 Checker 清單不一致：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"{index}/8 {case} 失敗\n" + output[-4000:])
            print(f"B4/M3 同源多 helper {index}/8：{case}（通過）")
        print("G18 B4/M3 同源多純整數 helper 八項 Gate 通過；B4／M3 尚未全面閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4/M3 多 helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
