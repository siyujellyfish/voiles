#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.5-T4／V7-A2：整批 shutdown 真終態雙帳預檢與零部分取消 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_shutdown_terminal_evidence_tests::"
CASES = (
    "t4_shutdown_已完成工作保留真結果且待取消工作逆登錄順序安全關閉",
    "t4_shutdown_正常已取消Prepared與已執行Suspended仍可重複安全預檢",
    "t4_shutdown_某已取消真框架刪Unwind時拒絕並保留其他Prepared",
    "t4_shutdown_某已完成工作事件帳被竄改必先拒絕而不取消其他工作",
    "t4_shutdown_從未驅動卻假標Succeeded不能阻止後續來源修復",
    "t4_shutdown_原Checker真預算耗盡Failed有完整R05帳可安全關閉其餘工作",
    "t4_shutdown_終結Succeeded被移除原結果即使無checkpoint也不得報安全關閉",
    "t4_shutdown_已真執行取消工作被篡成無清理證據仍須整批零更動",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 shutdown 八筆測試漂移：缺 " + str(sorted(expected-actual))
                + "；多 " + str(sorted(actual-expected))
            )
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"T4 真 shutdown {index}/8 失敗：{name}\n"+output[-5000:])
            print(f"T4 V7 整批關閉終態責任 {index}/8：{name}（通過）")
        print("T4 V7 整批關閉雙帳八項通過；T4/A2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as e:
        print("T4 shutdown Gate 失敗："+str(e), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
