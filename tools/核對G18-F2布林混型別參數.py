#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F2：原 Checker bool-return 的 int/bool 混合形參八項真 Rust 正反 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_mixed_tests::"
CASES = (
    "f2_bool_mixed_同步bool葉函式可綁定int與bool後正確返回",
    "f2_bool_mixed_多個混型別參數按真左到右順序綁定並用於if",
    "f2_bool_mixed_可寫bool局部兩次由帶整數參數函式完成返回後更新",
    "f2_bool_mixed_整數子函式可先真返回並作為布林函式的int實參",
    "f2_bool_mixed_布林葉函式返回另一帶int實參布林函式結果",
    "f2_bool_mixed_整數實參溢位不可進入布林函式或偽造布林返回",
    "f2_bool_mixed_非法int與bool位置互換由原checker先拒絕",
    "f2_bool_mixed_竄改已編譯形式參數型別即使返回bool仍在執行前拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 無法列出真測試：" + listing.stderr[-1400:])
        existing = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if existing != expected:
            raise ValueError("真測試來源失配：缺少 " + str(sorted(expected-existing))
                             + "；額外 " + str(sorted(existing-expected)))
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output,
            ):
                raise ValueError(
                    f"第 {index} 項真 Rust 正反失敗：{case}\n" + output[-2400:]
                )
            print(f"F2 混型別 bool-return {index}/{len(CASES)}：{case}（成功）")
        print("F2 混合 int/bool 形式參數八項真驗收成功；F2 原義務仍待完整 Closed。")
        return 0
    except (OSError, ValueError) as exc:
        print(f"G18 F2 bool 混型別 Gate 失敗：{exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
