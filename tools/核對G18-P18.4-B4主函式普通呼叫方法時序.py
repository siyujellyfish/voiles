#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4：main 普通函式後進同步 self 的七條真正反動態橋接 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_main_call_method_tests::"
CASES = (
    "b4_main_原普通函式先返回後同步self取得票據且結果正確",
    "b4_main_兄弟兩次零參數函式先各自返回再將兩值傳給self",
    "b4_main_同一普通fn先在main後在方法局部各自真執行並歸還self",
    "b4_main_ordinary函式成功後方法中途故障必保留已完成返回並真還票",
    "b4_main_ordinary返回後main後序算術溢位不得提前借用self或進方法",
    "b4_main_主函式已編譯Call來源被改成常數須在執行前拒絕",
    "b4_main_普通函式未宣告或型別不相容必由原Checker拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1600:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError("B4 main→方法測試模組不完整：缺 "
                + str(sorted(expected-actual)) + " 多 " + str(sorted(actual-expected)))
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"B4 main 真呼叫 {i}/7 失敗：{case}\n" + output[-3300:])
            print(f"B4 main fn→同步 self {i}/7：{case}（真通過）")
        print("B4 第二批原普通函式／方法安全時序 Gate 通過，完整 B4 尚 OpenEngineering")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 B4 第二批安全 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
