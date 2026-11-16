#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4／V7：真直接 await 完成結果／執行順序四項雙帳收據防偽。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "execution::async_task::tests::"
CASES = (
    "v7_t4_receipt_真已完成工作收據位置篡改不得發行下一工作",
    "v7_t4_receipt_完成順序互換或重播不得提前取消等待中的合法工作",
    "v7_t4_receipt_真成功失敗終態與原結果相矛盾不得清除來源",
    "v7_t4_receipt_未知完成工作與假取消收據拒絕但真取消不執行",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"],
            text=True, encoding="utf-8", capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX + "v7_t4_receipt_") and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 4 or actual != expected:
            raise ValueError("直接 await 雙帳收據清冊缺=" + str(sorted(expected-actual))
                             + "；多=" + str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"直接 await 雙帳收據 {index}/4 {name} 失敗\n" + output[-7000:])
            print(f"直接 await 雙帳收據 {index}/4：{name}（通過）")
        print("四項真正 P17 結果收據交叉驗證；T3/T4/A1/A2 仍 OpenEngineering。")
        return 0
    except (ValueError, OSError) as err:
        print("G18 V7 直接 await 結果收據 Gate 失敗：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
