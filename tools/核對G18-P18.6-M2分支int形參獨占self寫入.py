#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.6-M2：真 if／else 動態方法 int 形參獨占 self 欄位八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_m2_conditional_int_parameter_write_tests::"
CASES=(
    "m2_branch_param_原Checker_if_true動態int形參寫19後同欄位返回",
    "m2_branch_param_原Checker_false未選中真寫入必返回原else常值22",
    "m2_branch_param_只在else真正選中才以方法形參寫23",
    "m2_branch_param_true與else用不同DeclId分別寫值且另一葉零寫入",
    "m2_branch_param_真巢狀if兩葉不同方法形參均依原bool選中",
    "m2_branch_param_外層false完全不訪問巢狀形參寫入且main局部可傳值",
    "m2_branch_param_方法入口注入故障不得提前選葉或寫入且真退獨占票",
    "m2_branch_param_未授權bool欄位別名const與變造任一分支DeclId須拒絕",
)


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run(
            [str(binary),"--list"],capture_output=True,text=True,
            encoding="utf-8",check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗："+listing.stderr[-1800:])
        actual={line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("M2 分支 int 形參 Gate 清單不符：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"M2 選葉 int 形參 {index}/8 {case} 失敗：\n"+output[-4500:])
            print(f"M2 真選中分支 int 形參 {index}/8：{case}（通過）")
        print("G18 M2 真Checker 選葉 int 形參獨占自借用八項通過；M2 原義務尚未全閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("G18 M2 分支 int 形參 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
