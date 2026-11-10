#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/F2：原 P16 OwnedStrRuntime 真 Return→Accept 的 const/var 五項 Rust Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "owned_str_runtime::g18_f2_str_return_permission_tests::"
CASES = (
    "f2_str_return_const接收保持原資源身分且不得賦值",
    "f2_str_return_var接收可覆寫且舊資源唯一釋放",
    "f2_str_return接收同名失敗要歸還真資源供呼叫端清理",
    "f2_str_return兩層跨框架交還同一實際utf8不得重配置",
    "f2_str_return尚未交付的資源需保持可追溯清理不能依賴宿主drop",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1400:])
        actual = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if actual != expected:
            raise ValueError("P16 Return 真測試身分不符；缺少 "
                + str(sorted(expected-actual)) + "；額外 " + str(sorted(actual-expected)))
        for n, name in enumerate(CASES, 1):
            run = subprocess.run([str(binary), "--exact", PREFIX+name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 OwnedStr return {n}/{len(CASES)} 失敗：{name}\n"+output[-2400:])
            print(f"F2 Return/Accept 真資源 {n}/{len(CASES)}：{name}（通過）")
        print("P16 Return/Accept 權限與失敗資源五項完成；仍非原 Checker→函式工作圖 F2 Closed")
        return 0
    except (OSError, ValueError) as error:
        print("F2 Return/Accept 核心 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
