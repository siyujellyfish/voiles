#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E3：終態 CheckedTask 回收須保留 R05／P16 證據、拒絕未完成與無證工作。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_terminal_task_retirement_tests::"
案例 = (
    "g18_v7_e3_完成與預算故障退休必移交R05原結果且工作身分不回繞",
    "g18_v7_e3_準備與暫停不得退休取消後真ResourceId證據隨工作移交",
    "g18_v7_e3_未有原R05驗證的Failed不得被退休以掩蓋失敗",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run(
        [binary, "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listed.returncode:
        print("G18 V7 終態工作 Rust 清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("G18 V7 終態工作驗收來源缺項：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 終態工作回收正反失敗：" + name
                  + "\n" + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 E3 終態 CheckedTask 安全證據移交："
          f"{len(required)}/{len(required)} --exact Rust 成功；"
          "只處理已證實終態，不提前標記 P18.6 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
