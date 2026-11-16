#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：main int／bool直線工作圖真原Parser順序十項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_scalar_ordered_steps_tests::"
CASES = (
    "b4_ordered_main_原Checker普通helper先於bool更新真退出且方法不進分支",
    "b4_ordered_main_bool更新後宣告int再改回true選中真else",
    "b4_ordered_main_具名bool來源更新在普通函式退出後按DeclId讀取",
    "b4_ordered_main_普通helper後宣告bool再重賦已有值槽",
    "b4_ordered_main_先helper再bool參數反向調整不誤用int值槽",
    "b4_ordered_main_兩個main_helper夾bool更新與方法helper真先後順序",
    "b4_ordered_main_前序int溢位不可執行後續bool更新與self借票",
    "b4_ordered_main_後序int溢位不會啟動本次方法或製造假Returned",
    "b4_ordered_main_步驟來源交換或重播即使值一樣也必執行前拒絕",
    "b4_ordered_main_非法const與不安全bool型別來源仍由原Checker先拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"],
            text=True, encoding="utf-8", capture_output=True, check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError("跨型別順序Gate缺少=" + str(sorted(expected-actual))
                             + "；多出=" + str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"真main順序 {index}/10 {name} 失敗\n" + output[-7000:])
            print(f"原Parser順序 {index}/10：{name}（通過）")
        print("十項Copyable int/bool真順序、helper溢位與零假票據；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main跨型別順序必要Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
