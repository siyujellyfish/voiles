#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／R06：核對五項原 Checker 真執行、資源／票據退出交叉矩陣；並非 G18 全項 Passed。"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_r06_混合布林整數與真借用成功返回各守門須同時成立",
    "g18_r06_同_call_整筆移動借用和同步_self_不混淆資源責任",
    "g18_r06_合法遞迴真資源與普通借用在預算耗盡時皆不得假返回",
    "g18_r06_不同執行器的正常失敗不得丟失已建資源或複製假返回",
    "g18_r06_跨模型偽造離域資源票據任一項必在交付稽核失敗",
)
前綴 = "struct_stage::g18_r06_cross_runtime_tests::"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    executable = str(args.test_bin.resolve())
    listing = subprocess.run(
        [executable, "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listing.returncode:
        print("R06 Rust 真來源清單無法取得：" + listing.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines() if line.endswith(": test")
    }
    missing = sorted({前綴 + name for name in 案例} - available)
    if missing:
        print("R06 真來源案例缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    run = subprocess.run(
        [executable, "--test-threads=2", 前綴], text=True,
        encoding="utf-8", capture_output=True, check=False,
    )
    match = re.search(
        r"test result: ok\.\s+(\d+) passed;\s+0 failed;\s+0 ignored;",
        run.stdout + run.stderr,
    )
    if run.returncode or not match or int(match.group(1)) != len(案例):
        print("R06 跨真執行器資源退出矩陣未全過："
              + run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 R06 真跨執行器正反：{len(案例)}/{len(案例)} 通過；"
          "這不代表原 P18.1～P18.6 已完整 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
