#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B4：方法 var int RHS 真 helper 呼叫、原子更新、溢位退票。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_helper_assignment_unwind_tests::"
CASES = (
    "b4_helper_assign_原Checker真helper算術作var更新返回12並還票",
    "b4_helper_assign_兩次var重賦各走真helper框架最後結果11",
    "b4_helper_assign_原Checker初始化與重賦兩次真呼叫不混用框架",
    "b4_helper_assign_前序純整數更新先執行後才進入真helper",
    "b4_helper_assign_真helper溢位不能提交var而須內後外退出還票",
    "b4_helper_assign_已有前序更新後右值溢位不得偽造第二次LocalAssigned",
    "b4_helper_assign_篡改已核准Call計畫或偽造呼叫次數必拒絕",
    "b4_helper_assign_原Checker拒絕const目標與將helper嵌入算式及錯型別",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1400:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "B4 helper var 賦值來源漂移：缺 "
                + str(sorted(expected - actual)) + "；多 " + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(
                    f"B4 var RHS 真 helper {index}/8 失敗：{name}\n" + out[-3900:]
                )
            print(f"B4 var int RHS 真 helper {index}/8：{name}（通過）")
        print("G18 B4 原方法 var RHS helper 八項已完成；B4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 var RHS helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
