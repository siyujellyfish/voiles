#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7：原 Checker 真來源擁有目錄與 FIFO 借用生命週期七項 Rust 正反。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_checked_program_catalog_tests::"
案例 = (
    "g18_v7_a1_原Checker來源由登錄持有且FIFO續行引用同一真計畫身分",
    "g18_v7_a1_登錄前靜態非法來源及不支援續行的真Checker形狀零身分消耗",
    "g18_v7_e2_未知來源登錄ID不得消耗FIFO工作身分或其他已核准佇列",
    "g18_v7_e2_同一份原Checker登錄來源可產生兩份相互獨立的真續行",
    "g18_v7_e3_登錄來源真溢位及取消分別沿原Runtime清理不偽造成功",
    "g18_v7_a1_來源登錄身分耗盡時原計畫不回繞也不污染先前工作",
    "g18_v7_a1_排程器完成後釋放借用原登錄仍能增加下一份真來源",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run(
        [binary, "--list"], capture_output=True, text=True,
        encoding="utf-8", check=False,
    )
    if listed.returncode:
        print("V7 真 Checker 來源登錄清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    present = {line.split(": test", 1)[0]
               for line in listed.stdout.splitlines()
               if line.endswith(": test")}
    names = [前綴 + name for name in 案例]
    missing = [name for name in names if name not in present]
    if missing:
        print("V7 真 Checker 來源登錄缺項：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in names:
        result = subprocess.run(
            [binary, "--exact", name], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if result.returncode or "1 passed; 0 failed;" not in result.stdout:
            print("V7 真 Checker 來源擁有登錄驗收失敗：" + name
                  + "\n" + result.stdout + result.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 原 Checker 計畫 Box 擁有目錄／FIFO 身分："
          f"{len(names)}/{len(names)} 逐項 --exact Rust 成功；"
          "持有式來源目錄與借用式排程器保持安全生命週期。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
