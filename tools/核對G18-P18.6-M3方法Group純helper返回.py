#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M3／P18.4-B4：原 Checker Group 純 int helper Return 八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_grouped_helper_method_return_tests::"
CASES = (
    "m3_grouped_真Group包覆seed加2回傳16並只進一次helper",
    "m3_grouped_真雙層括號同一TypedHIR不重複執行函式",
    "m3_grouped_真純helper用Group先整數常值後呼叫返回16",
    "m3_grouped_if_true真執行而false不啟動封存helper",
    "m3_grouped_原else包覆純helper只在false葉真呼叫",
    "m3_grouped_巢狀bool兩Group選葉只能真執行一次",
    "m3_grouped_真Helper本體溢位須先退出子框架後歸票",
    "m3_grouped_不核准混合self欄位錯型別與篡改原圖須拒絕",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1600:])
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or found != expected:
            raise ValueError("M3/B4 Group Return Gate 缺 "
                + str(sorted(expected-found)) + "，多 "
                + str(sorted(found-expected)))
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
                    f"M3/B4 Group Return {index}/8 {case} 失敗\n" + output[-4500:]
                )
            print(f"M3/B4 Group Return {index}/8：{case}（通過）")
        print("M3/B4 原 Checker Group/Call 八項 Gate 通過；原義務尚未整項閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("M3/B4 Group helper Return Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
