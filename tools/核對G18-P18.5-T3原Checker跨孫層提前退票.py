#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／R05：真 Checker 同步 self 孫層 helper 過早退票六項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_r05_t3_outer_ticket_grandchild_tests::"
CASES = (
    "t3_grandchild_原Checker唯讀self方法真helper完成後才歸唯一票",
    "t3_grandchild_正常helper已Returned但尚未Exited不得先還外self票",
    "t3_grandchild_真內層溢位已Unwind但尚未Exited不能預退self票",
    "t3_grandchild_雙個真純helper第二個Returned後仍不得提前還self票",
    "t3_grandchild_獨占self欄位寫入純helper結果也不能提前退票",
    "t3_grandchild_沒有子helper的真self方法在自身Returned後歸票合法",
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
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 6 or actual != expected:
            raise ValueError("T3/R05 外票深度 Gate 缺 "
                + str(sorted(expected-actual)) + "，多 "
                + str(sorted(actual-expected)))
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
                    f"T3/R05 外票深度 {index}/6 {case} 失敗\n" + output[-4500:]
                )
            print(f"T3/R05 外票真歷史 {index}/6：{case}（通過）")
        print("T3/R05 外層 self 票據在孫層仍活躍時不可提早歸還六項 Gate 通過，T3 未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("T3/R05 孫層退票 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
