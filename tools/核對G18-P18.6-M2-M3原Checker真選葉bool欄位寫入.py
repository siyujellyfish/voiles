#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／M2/M3：唯一原Checker true葉 CFG 真self.bool P16獨占寫入十三項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_branch_bool_field_write_tests::"
CASES = (
    "m2_branch_bool_原Checker_true真選中才原子寫false",
    "m2_branch_bool_原Checker_false分支不寫即使靜態必須獨占票",
    "m2_branch_bool_寫true至原false欄位必反映真P16前後值",
    "m2_branch_bool_原Lexer_Group包裹bool常值仍使用原Checker真來源",
    "m2_branch_bool_原Checker_self_bool真條件與分支寫入共用唯一獨占票",
    "m2_branch_bool_原Checker_self_bool假條件不許誤執行其寫入",
    "m2_branch_bool_main前序真覆寫後方法按新bool值寫入",
    "m2_branch_bool_const首個int欄位與var_bool真選葉寫入不互相封鎖",
    "m2_branch_bool_故障在方法入口之前不曾真選葉或寫欄位",
    "m2_branch_bool_封存欄位身分或bool值竄改必在任何借票前拒絕",
    "m2_branch_bool_原Checker拒絕const欄位與型別錯誤及未知欄位",
    "m2_branch_bool_非獨立bool常值RHS需守住目前有界子集",
    "m2_branch_bool_V7無前置作用直接await_真wake後始寫入",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        required = {PREFIX + name for name in CASES}
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 13 or found != required:
            raise ValueError(
                "真self.bool選葉Gate缺失=" + str(sorted(required - found))
                + "；異常多出=" + str(sorted(found - required))
            )
        for i, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            result = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", result
            ):
                raise ValueError(f"真Checker bool選葉 {i}/13 {name}失敗\n" + result[-9000:])
            print(f"G18 M2/M3 真true葉 bool欄位 {i}/13：{name} 通過")
        print("十三項原Checker真選葉P16/HC14/V7獨立Gate成功；B4/M2/M3維持OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 bool true葉Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
