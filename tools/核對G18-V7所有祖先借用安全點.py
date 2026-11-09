#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E5：原私有 await 之所有祖先真 BorrowTicket／BorrowFact 安全點正反 Gate。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_ancestor_borrow_tests::"
案例 = (
    "g18_v7_e5_外層真BorrowTicket未還不能因內層無票據而進await",
    "g18_v7_e5_外層傳入仍有效的BorrowFact不能跨過內層await",
    "g18_v7_e5_空函式鏈不能被當成已驗證await安全點",
    "g18_v7_e5_真Checker三層函式皆無借用可正常直接await",
    "g18_v7_e5_原Checker仍須拒絕未證歸還的跨await字串借用",
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
        print("V7 E5 真 Rust 清單失敗：" + listed.stderr, file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("V7 E5 五項來源缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("V7 E5 真測試失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 E5 全祖先借用安全點：{len(required)}/{len(required)} "
          "逐項 --exact 已執行；P18.6 尚未完整結案。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
