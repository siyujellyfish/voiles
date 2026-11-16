#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／V7：多幀 RuntimeFailure 全 BorrowTicket 預檢三項真證據。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "execution::async_task::v7_a2_safe_point_tests::"
CASES = (
    "v7_unwind_t3_多框架父票據事實污染錯誤退出前須零部分資源釋放",
    "v7_unwind_t3_跨框架重播同一真票據即使靜態重數相同亦不可先退出子框架",
    "v7_unwind_t3_退出前追蹤器多出無幀票據不得先釋放原框架資源",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"],
            capture_output=True, check=False, text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX + "v7_unwind_t3_") and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 3 or expected != actual:
            raise ValueError(
                "多幀借票原子預檢清冊差異：缺少 " + str(sorted(expected - actual))
                + "；多出 " + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            test = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = test.stdout + test.stderr
            if test.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"多幀退出票據 {index}/3 {name} 失敗\n" + output[-7000:])
            print(f"多幀退出票據 {index}/3：{name}（通過）")
        print("三項真HC14/P16多幀拒絕與回滾原子測試；T3/T4/A2仍未Closed。")
        return 0
    except (ValueError, OSError) as error:
        print("G18多幀錯誤退出票據Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
