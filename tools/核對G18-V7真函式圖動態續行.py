#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E2：原私有 Checker 顯式動態工作棧跨呼叫真暫停與恢復六項驗收。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_real_graph_checkpoint_tests::"
案例 = (
    "g18_v7_e2_原私有函式真UTF8資源與顯式工作棧跨Rust呼叫暫停後原點恢復",
    "g18_v7_e2_原Checker兩個真await可跨三次呼叫從不同指令位置接續",
    "g18_v7_e2_私有真框架暫停後能先執行另一工作再喚醒原工作",
    "g18_v7_e2_真私有await暫停後溢位仍只清理原P16資源不虛構返回",
    "g18_v7_e2_同步原Checker工作不產生虛假的可暫停await",
    "g18_v7_e2_原Checker未授權跨await借用不能建立動態續行",
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
        print("G18 V7 動態函式圖真測試清單失敗：" + listed.stderr,
              file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("G18 V7 動態函式圖測試缺失：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 原私有動態續行真來源失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 原 Checker 真動態工作棧跨呼叫保存／恢復："
          f"{len(required)}/{len(required)} 逐筆 --exact 成功。"
          "已執行幀取消與 async self 尚未驗收。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
