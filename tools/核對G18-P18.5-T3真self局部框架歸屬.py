#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.5-T3：原同步 self 真方法局部跨框架雙帳來源防偽八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_t3_method_local_frame_provenance_tests::"
CASES=(
    "t3_self_frame_原main_helper先真返回再借self內局部成功可驗",
    "t3_self_frame_同main兩次真helper後仍只有唯一self局部歸屬",
    "t3_self_frame_方法進入即故障時不得偽造未執行局部但票據須真退",
    "t3_self_frame_方法局部真初始化後Return溢位仍保留逆序清理",
    "t3_self_frame_移到main普通helper同數量局部與有效self票仍必拒絕",
    "t3_self_frame_移到self內巢狀普通helper同數量局部仍必拒絕",
    "t3_self_frame_初始化重賦與退域三事件移到main_helper不得冒充",
    "t3_self_frame_真失敗Unwind與票序完全不改但清理被移至main亦拒絕",
)


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run(
            [str(binary),"--list"],capture_output=True,
            text=True,encoding="utf-8",check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗："+listing.stderr[-1600:])
        actual={line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("T3 自借用框架歸屬 Gate 清單不符：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for number,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output):
                raise ValueError(f"T3 真方法框架歸屬 {number}/8 {case} 失敗\n"+output[-4500:])
            print(f"T3 真方法框架歸屬 {number}/8：{case}（通過）")
        print("G18 T3/B4/M3 self 方法局部跨普通函式歸屬八項通過；原義務仍未全項閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 T3 self 方法局部框架防偽 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
