#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4／M3：主函式var int真ScalarHIR重賦與helper中斷十項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_int_assignment_tests::"
CASES = (
    "b4_int_assign_原Checker_var_int常值覆寫後方法使用最後值",
    "b4_int_assign_原Checker_具名int_declid覆寫不依相同數值猜身分",
    "b4_int_assign_原Checker普通helper作RHS真執行先於self借票",
    "b4_int_assign_原Checker_多層Group加減自來源真求值",
    "b4_int_assign_與bool變更交錯仍嚴格按原Parser結果選葉",
    "b4_int_assign_先普通helper再bool宣告仍有唯一self真票",
    "b4_int_assign_真動態溢位停止後續bool與方法不假進入",
    "b4_int_assign_真helper成功而外部加法溢位不啟動self",
    "b4_int_assign_準備後變造int賦值目標或重播步驟SourceChanged",
    "b4_int_assign_原Checker_const未知與bool錯型別賦值仍安全拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError("main int重賦Gate缺少="+str(sorted(expected-actual))
                             +"；多出="+str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"main int重賦 {index}/10 {name} 失敗\n" + output[-7500:])
            print(f"原Checker var int重賦 {index}/10：{name}（通過）")
        print("十項真ScalarHIR、BorrowTicket與溢位中斷證據；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as err:
        print("G18 main int重賦必要Gate失敗：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
