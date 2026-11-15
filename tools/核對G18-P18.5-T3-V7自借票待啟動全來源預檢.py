#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3/V7：async self Pending來源全復驗及取消前安全點十項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t3_v7_self_deferred_preflight_tests::"
CASES = (
    "t3_v7_self_preflight_與真execute共用同一完整來源權威",
    "t3_v7_self_preflight_Prepared回傳計畫被改不得先取消消證",
    "t3_v7_self_preflight_Suspended純helper子計畫被改不得假喚醒",
    "t3_v7_self_preflight_原Lexer的await詞彙遭替換整批關閉須先拒絕",
    "t3_v7_self_preflight_受損準備計畫不得替下一ID註冊合法外觀",
    "t3_v7_self_preflight_獨立真方法局部HIR受損不得進入Ready",
    "t3_v7_self_preflight_其它合法Ready不得掩蓋污染工作先被執行",
    "t3_v7_self_preflight_真已完工可正常退休但另一污染計畫必拒絕移交",
    "t3_v7_self_preflight_多工作全取消必先證明所有來源一致而不部分提交",
    "t3_v7_self_preflight_成功退休收據不持AST仍可與新合法工作並存",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        executable = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(executable), "--list"],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("真Rust測試清冊失敗：" + listing.stderr[-2000:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 10 or actual != expected:
            raise ValueError(
                "T3/V7預檢真清冊不一致。缺少 "
                + str(sorted(expected-actual)) + "；多出 " + str(sorted(actual-expected))
            )
        for number, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(executable), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            result = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", result
            ):
                raise ValueError(f"T3/V7 {number}/10：{case} 失敗\n" + result[-6500:])
            print(f"T3/V7 原Checker待啟動完整來源 {number}/10：{case} 通過")
        print("十項V7真正封存Lexer/Checker預檢與零部分取消皆通過；T3/T4/A1/A2尚未Closed。")
        return 0
    except (OSError, ValueError) as exc:
        print("T3/V7待啟動來源一致性Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
