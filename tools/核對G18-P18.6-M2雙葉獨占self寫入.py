#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：原 Checker 同步 self if/else 雙葉獨占寫入及逐框架歸票八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_else_exclusive_write_tests::"
CASES = (
    "m2_else_write_true寫入19但不執行else寫入23",
    "m2_else_write_false只寫入23並從最新self欄位讀取返回",
    "m2_else_write_僅else寫入仍由Checker要求方法入口獨占",
    "m2_else_write_true時不執行未選分支也不求值末尾溢位",
    "m2_else_write_false時不執行未選真葉且不求值末尾溢位",
    "m2_else_write_方法入口故障不得假造兩葉寫入仍真歸還獨占票據",
    "m2_else_write_已核准else值及Return位置遭篡改執行前拒絕",
    "m2_else_write_原Checker拒絕const與不合法else型別且未證控制保守拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1600:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 雙葉真獨占來源漂移：缺 "
                + str(sorted(expected-actual))
                + "；未登錄 " + str(sorted(actual-expected)))
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"M2 雙葉 {i}/8 失敗：{case}\n" + output[-3800:])
            print(f"M2 同步 self 雙葉真獨占 {i}/8：{case}（通過）")
        print("G18 M2 真 if/else 獨占雙葉八項原來源 Gate 成功；M2 整體仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("M2 真雙葉獨占安全 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
