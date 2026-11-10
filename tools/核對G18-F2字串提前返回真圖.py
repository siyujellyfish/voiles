#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／F2：原 Checker 字面／Move 字串提前返回與真 P16 資源八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_early_return_tests::"
CASES = (
    "f2_str_early_真if提前字面返回與未選中尾端不同資源",
    "f2_str_early_完整if_else兩葉返回尾端不可達不得配置第三字串",
    "f2_str_early_提前返回前已配置兩字串須依逆序釋放旁觀資源",
    "f2_str_early_具名move提前轉交同一resourceid沒有重新建構",
    "f2_str_early_先真借用str實參再提前返回借用票據必歸還",
    "f2_str_early_分支前整數溢位只能清理已建資源不得假返回",
    "f2_str_early_原checker拒絕提前返回錯誤型別及非法str複製",
    "f2_str_early_竄改提前返回接受resourceid即使平衡也不能冒領",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        executable = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(executable), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-2000:])
        found = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if found != expected:
            raise ValueError(
                f"來源 Rust 測試不符：缺少 {sorted(expected - found)}、"
                f"多餘 {sorted(found - expected)}"
            )
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(executable), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            out = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(f"Rust {index}/{len(CASES)} 失敗：{case}\n" + out[-2500:])
            print(f"F2 str-return 提前返回 {index}/{len(CASES)}：{case}（通過）")
        print("F2 字串提前返回八項真 Gate 成功；完整 F2 仍依原 R07 結案規則")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 字串提前返回 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
