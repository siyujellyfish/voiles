#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7：for／while 本體 await 真 Checker 動態暫停、資源與取消正反驗收。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_loop_body_await_checkpoint_tests::"
案例 = (
    "g18_v7_e2_原Checker_for迴圈本體三次真await逐輪checkpoint而不重複建立資源",
    "g18_v7_e3_for真第二輪await取消只清理當前局部而不重建前輪",
    "g18_v7_e2_原Checker_while本體await後continue真局部資源不重建",
    "g18_v7_e3_原Checker_while本體await被取消後不執行後續continue或再迭代",
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
        print("G18 V7 迴圈本體真測試清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    available = {line.split(": test", 1)[0]
                 for line in listed.stdout.splitlines()
                 if line.endswith(": test")}
    required = [前綴 + case for case in 案例]
    missing = [name for name in required if name not in available]
    if missing:
        print("G18 V7 迴圈本體真來源缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        result = subprocess.run(
            [binary, "--exact", name], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if result.returncode or "1 passed; 0 failed;" not in result.stdout:
            print("G18 V7 迴圈本體真來源執行失敗：" + name
                  + "\n" + result.stdout + result.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 真 for／while 本體 await 協作續行、P16 資源與取消："
          f"{len(required)}/{len(required)} 逐項 --exact 通過；"
          "P18 原六項尚未正式結案。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
