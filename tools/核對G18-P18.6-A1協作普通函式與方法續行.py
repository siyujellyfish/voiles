#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-A1 原 Checker→真 Runtime 協作普通函式與方法續行：4 項正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_async_method_graph_tests::a1_普通等待與兩種方法共用可喚醒框架",
    "struct_stage::g18_async_method_graph_tests::a1_兩工作在方法安全點按喚醒順序續行",
    "struct_stage::g18_async_method_graph_tests::a1_借用不能跨等待且續行不重設預算",
    "struct_stage::g18_async_method_graph_tests::a1_布林與字串等待結果回到原呼叫端",
)
EXACT_MODULE_PREFIXES = ("struct_stage::g18_async_method_graph_tests::a1_",)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        available = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.endswith(": test")
        }
        expected = set(CASES)
        if len(expected) != 4:
            raise ValueError("本契約測試名稱出現重複，不得重複採計")
        missing = sorted(expected - available)
        if missing:
            raise ValueError("缺少原 Checker 真 Rust 案例：" + str(missing))
        for prefix in EXACT_MODULE_PREFIXES:
            actual = {name for name in available if name.startswith(prefix)}
            expected_module = {name for name in expected if name.startswith(prefix)}
            if actual != expected_module:
                raise ValueError(
                    f"模組 {prefix} 正反矩陣漂移：缺 {sorted(expected_module-actual)} "
                    f"多 {sorted(actual-expected_module)}"
                )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"A1 第 {index}/4 條真來源失敗：{case}\n" + output[-3000:]
                )
            print(f"A1 原始義務 {index}/4：{case}（通過）")
        print("A1：原 Checker 真來源正反／方法工作圖獨立驗收通過；不代表 P18.6 全 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("A1 獨立 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
