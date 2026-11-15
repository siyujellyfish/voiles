#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2／P18.4-B4：零參數純 int helper 直接／選葉寫 self 十項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_zero_arg_helper_self_write_tests::"
CASES = (
    "m2_zero_arg_直接self寫入真seed零參數回傳14並唯一歸票",
    "m2_zero_arg_直接零參數helper與加法真ScalarHIR組合寫入19",
    "m2_zero_arg_零參數helper作另一helper實參先內後外真退出",
    "m2_zero_arg_真if選葉啟動seed寫入14再歸唯一票",
    "m2_zero_arg_未選if_true不可以提前進入會溢位的seed",
    "m2_zero_arg_原if_else零參數寫入只在false葉執行",
    "m2_zero_arg_直接seed本體溢位先Unwind子框架再歸self票",
    "m2_zero_arg_選葉seed本體溢位不可提交欄位且須真歸票",
    "m2_zero_arg_不合法零引數型別async或資源helper皆不得準備",
    "m2_zero_arg_已封存真Call與原Checker工作圖變造先拒絕",
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
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 10 or actual != expected:
            raise ValueError("M2 零參數 helper Gate 缺 "
                + str(sorted(expected-actual)) + "，多 "
                + str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
                output,
            ):
                raise ValueError(
                    f"M2 零參數 helper {index}/10 {name} 失敗\n" + output[-4500:]
                )
            print(f"M2 零參數真呼叫 {index}/10：{name}（通過）")
        print("M2／B4 零參數 helper 真來源十項通過；原義務尚未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("M2／B4 零參數 helper Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
