#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.5-T3/B4/M3：原 Checker 借 self 前普通函式真進退時序八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_t3_self_preborrow_call_order_tests::"
CASES=(
    "t3_preborrow_原Checker唯一main_helper真先Returned再借self",
    "t3_preborrow_兩份main同步helper與方法內純helper各自原位進退",
    "t3_preborrow_main第二helper真溢位不得要求其後方法已借票",
    "t3_preborrow_self入口注入失敗仍必保留main_helper真正退出與借票歸還",
    "t3_preborrow_移動完整main_helper三事件至方法退出後泛用帳仍平衡須拒絕",
    "t3_preborrow_移動main_helper三事件到self持票方法中仍不得歸錯框架",
    "t3_preborrow_兩個main_helper只移第二個到方法內仍須核對原全數",
    "t3_preborrow_方法真溢位且借票無變仍不得將main_helper移入方法",
)


def main()->int:
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
            raise ValueError("Rust --list 失敗："+listing.stderr[-1600:])
        found={line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+case for case in CASES}
        if len(expected)!=8 or found!=expected:
            raise ValueError("T3 借票前函式時序清單不符：缺 "
                +str(sorted(expected-found))+"；多 "+str(sorted(found-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"T3 借票前來源 {index}/8 {case} 失敗：\n"+output[-4000:])
            print(f"T3 借票前原 helper 時序 {index}/8：{case}（通過）")
        print("G18 T3/B4/M3 原 main→helper→self 借票前真正時序八項通過；原義務仍未全閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("G18 T3 借票前函式時序 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
