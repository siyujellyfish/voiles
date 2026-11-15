#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.6-M2：原 Checker if/else 選葉 int 算術 self 欄位寫入八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_m2_conditional_checked_int_self_write_tests::"
CASES=(
    "m2_branch_arith_原if_true取真int形參加3後獨占寫入22",
    "m2_branch_arith_原if_false未選中高風險算式零動態寫入",
    "m2_branch_arith_選中true後算術溢位不提交寫入且真還票",
    "m2_branch_arith_選中else後才求右運算式溢位不影響true合法值",
    "m2_branch_arith_true與else各用原Checker不同int參數純加減",
    "m2_branch_arith_巢狀原bool雙葉各執行獨立算術且唯一退票",
    "m2_branch_arith_外層false不可預先求巢狀已核准溢位算式",
    "m2_branch_arith_未授權型別const來源與變造原算式Span均拒絕",
)


def main()->int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run([str(binary),"--list"],
            capture_output=True,text=True,encoding="utf-8",check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗："+listing.stderr[-1800:])
        actual={line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+case for case in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("M2 真選葉算術 Gate 清單不一致，缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run([str(binary),"--exact",PREFIX+case,
                "--test-threads=1"],capture_output=True,
                text=True,encoding="utf-8",check=False)
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"M2 選葉算術 {index}/8 {case} 失敗\n"+output[-4000:])
            print(f"M2 選葉算術 {index}/8：{case}（通過）")
        print("M2 真 Checker if/else/巢狀選葉 int 算術寫入八項通過；M2 尚未全閉合。")
        return 0
    except (OSError,ValueError) as err:
        print("M2 選葉算術 Gate 失敗："+str(err),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
