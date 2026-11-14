#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker FIFO submit／wake 憑據八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_fifo_issued_token_order_tests::"
CASES = (
    "t4_issue_兩份合法Prepared交換ready仍須拒絕不執行",
    "t4_issue_兩份合法Suspended依wake先後入列不可偷換順序",
    "t4_issue_先存在Prepared後真wake須保持原混合入列順序",
    "t4_issue_偽增第二份wake入列紀錄不得創造新token",
    "t4_issue_偽造重複submit授權即使ready未變也拒絕",
    "t4_issue_刪除真提交來源即使留單張ready仍不可動",
    "t4_issue_合法cancel刪除佇列中間token仍可保持剩餘FIFO",
    "t4_issue_錯序時shutdown與退休須全零副作用修復後正常",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"],
            capture_output=True, text=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("真 FIFO token 測試清單不一致，缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected)))
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"FIFO 授權 {index}/8 {case} 失敗：\n" + output[-4000:])
            print(f"T4 FIFO 授權 {index}/8：{case}（通過）")
        print("G18 T4/V7-E2 原 Checker 真 FIFO 入列順序八項通過；原 T4 仍未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 真 FIFO 入列憑據 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
