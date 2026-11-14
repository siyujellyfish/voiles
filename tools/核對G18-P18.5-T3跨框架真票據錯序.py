#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／R05：真 HC14 BorrowTicket 跨框架錯序反向驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_r05_t3_nested_ticket_order_tests::"
CASES = (
    "t3_ticket_真兩層成功返回必先內票後外票",
    "t3_ticket_成功時交換內外票還序即使張數相同仍必拒絕",
    "t3_ticket_原HC14真兩層故障唯一Unwind必內後外還票",
    "t3_ticket_故障中交換內外票只改順序仍不得偽造平衡",
    "t3_ticket_同一詞彙框架兩本共享借用可依原最後使用點正序歸還",
    "t3_ticket_不同真Tracker序號不能冒充同一票據歸還",
    "t3_ticket_重複歸還或未歸還真票都不能假成功",
    "t3_ticket_三層真借用與局部逆序清理成功但錯序竄改必拒絕",
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
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T3 原 HC14 真票據測試名稱漂移：缺 "
                + str(sorted(expected - actual)) + "；多 " + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T3 真票據錯序對帳 {index}/8 失敗：{case}\n" + output[-4000:]
                )
            print(f"T3 R05 真票據退出因果 {index}/8：{case}（通過）")
        print("T3／R05 真跨框架借用票據錯序八項通過；P18.5-T3 仍須全部原證據。")
        return 0
    except (OSError, ValueError) as error:
        print("T3 真 BorrowTicket 錯序 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
