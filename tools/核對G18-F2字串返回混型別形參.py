#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／F2 原 Checked Source 帶 int/bool/str 的 str-return 真 OwnedUtf8 八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_return_args_tests::"
CASES = (
    "f2_str_args_整數布林混型別正確求值才交付真utf8",
    "f2_str_args_先真借用外部字串後再接收返回資源且兩者各自釋放",
    "f2_str_args_移動參數再從原函式返回同一resourceid不許二次建立",
    "f2_str_args_整數子函式先真返回後作為str返回實參",
    "f2_str_args_借用後續整數溢位不進callee不假str返回仍還票",
    "f2_str_args_移動意圖後續溢位不得提前提交資源轉交",
    "f2_str_args_原checker拒絕整數字串實參錯置及不合法複製",
    "f2_str_args_偽造返回接受resourceid即使進退筆數平衡也必拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(exe), "--list"], text=True,
            encoding="utf-8", capture_output=True, check=False)
        if listed.returncode:
            raise ValueError("原 Rust --list 失敗：" + listed.stderr[-1800:])
        observed = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                    if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if observed != expected:
            raise ValueError("來源身分不符：缺少 " + str(sorted(expected - observed))
                + "；多餘 " + str(sorted(observed - expected)))
        for n, case in enumerate(CASES, 1):
            run = subprocess.run([str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"真 Rust {n}/{len(CASES)} 失敗：{case}\n" + output[-2200:])
            print(f"F2 str-return 多形參 {n}/{len(CASES)}：{case}（通過）")
        print("F2 str-return 混合形參八項真測試通過；原 F2 全部義務未必 Closed")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 str-return 真 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
