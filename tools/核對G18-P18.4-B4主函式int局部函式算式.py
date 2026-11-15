#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.4-B4/M3：原 main int 局部算術與同源普通函式八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_b4_m3_main_local_int_call_arithmetic_tests::"
CASES=(
    "b4_main_math_原main局部函式結果再加一後傳入self等於12",
    "b4_main_math_兩個不同純函式分居算術左右先後真完成",
    "b4_main_math_內層函式完整退出後外層加法才交付main局部",
    "b4_main_math_第二main局部以第一局部真值呼叫並交付兩值加總",
    "b4_main_math_左函式真完成右邊溢位不得憑空借self",
    "b4_main_math_左算術先溢位右側純函式零Enter",
    "b4_main_math_原Checker零參純int函式結果參與main局部算術",
    "b4_main_math_未知函式資源非法型別及原計畫變造必先拒絕",
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
            raise ValueError("Rust --list 失敗："+listing.stderr[-1500:])
        actual={line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("main int 局部 Gate 清單不一致：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False)
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"main int 算式 {index}/8 {case} 失敗\n"+output[-4000:])
            print(f"main 真 Checker int 算式 {index}/8：{case}（通過）")
        print("G18 B4/M3 main int 局部函式算術八項通過；B4/M3 尚未全項閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("G18 B4/M3 main int 算式 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
