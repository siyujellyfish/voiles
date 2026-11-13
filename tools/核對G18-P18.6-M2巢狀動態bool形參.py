#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6 M2：原 Checker 巢狀 bool 參數真動態兩葉及同一 self 借用退出八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_nested_dynamic_bool_tests::"
CASES = (
    "m2_nested_dynamic_外層常true內層真bool參數true回傳加法12",
    "m2_nested_dynamic_外層常true內層真bool參數false回傳self欄位7",
    "m2_nested_dynamic_外層也用相同bool形參false時內層根本未進入",
    "m2_nested_dynamic_外層與內層同參數true時完成唯一正常返回",
    "m2_nested_dynamic_內層參數false不執行可能溢位的true返回葉",
    "m2_nested_dynamic_內層參數true已到達溢位必真Unwind並還票",
    "m2_nested_dynamic_竄改內層真參數DeclarationId或靜態預期值均拒絕",
    "m2_nested_dynamic_原Checker拒絕內層int條件及未宣告bool名稱",
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
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1400:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 巢狀動態 bool 真來源漂移：缺 "
                             + str(sorted(expected - actual))
                             + "；未登錄 " + str(sorted(actual - expected)))
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output,
            ):
                raise ValueError(f"M2 巢狀動態 bool {i}/8 失敗：{case}\n" + output[-3900:])
            print(f"M2 巢狀動態 bool {i}/8：{case}（通過）")
        print("G18 M2 巢狀 bool 形參真動態選葉八項正反完成；整項 M2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M2 巢狀動態 bool Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
