#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""F2：同步 str-return 作為另一函式直接 return，唯一 ResourceId 八項真 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_forward_return_tests::"
CASES = (
    "f2_forward_str_兩層直接函式返回保留唯一utf8資源",
    "f2_forward_str_三層直接返回不複製資源id也不丟失動態框架",
    "f2_forward_str_直接返回結果再作下一函式str實參同id",
    "f2_forward_str_混合純值形參向內轉交不虛構返回資源",
    "f2_forward_str_提前if命中直接返回子函式不建未達字面",
    "f2_forward_str_內層建資源後溢位必逐框架回滾不捏造returned",
    "f2_forward_str_前置算式溢位零callee與零資源返回",
    "f2_forward_str_偽造第二次returned的另一資源及錯型別原checker拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(exe), "--list"], text=True, encoding="utf-8",
                                capture_output=True, check=False)
        if listed.returncode:
            raise ValueError("原 Rust --list 失敗：" + listed.stderr[-2000:])
        actual = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(f"來源名稱與數量不一致，缺少 {sorted(expected-actual)}，"
                             f"多餘 {sorted(actual-expected)}")
        for i, case in enumerate(CASES, 1):
            result = subprocess.run([str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                                    text=True, encoding="utf-8", capture_output=True, check=False)
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"直接 str 轉發 {i}/{len(CASES)} {case}\n" + output[-3500:])
            print(f"F2 str 轉發 {i}/{len(CASES)}：{case}（真通過）")
        print("F2 str 直接 return 八項真 Gate 通過，F2 整體仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 str 轉發 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
