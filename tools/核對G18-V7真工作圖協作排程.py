#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7：真私有 Checker 動態工作圖 FIFO 排程、喚醒與取消八筆獨立正反。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_cooperative_graph_queue_tests::"
案例 = (
    "g18_v7_e2_真動態工作三份FIFO交錯後依明確喚醒次序完成",
    "g18_v7_e2_多await工作在不同真指令位置反覆yield且其他工作可插入",
    "g18_v7_e3_已醒來尚待執行的真父框架取消會移除FIFO並真釋放資源",
    "g18_v7_e5_喚醒前及已排隊後新增真祖先票據都不得偷消耗checkpoint",
    "g18_v7_e3_取消前真祖先票據未還需保留已喚醒FIFO與原ResourceId",
    "g18_v7_e2_普通同步工作不得醒來且重複喚醒不得重複入FIFO",
    "g18_v7_e3_未啟動工作取消必零真動態事件且無未知工作副作用",
    "g18_v7_e2_工作身分耗盡必拒絕且不污染原待執行FIFO",
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
        print("G18 V7 真排程器 Rust 清單失敗：" + listed.stdout + listed.stderr,
              file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("G18 V7 真排程器來源缺項：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 真工作圖排程失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 真 Checker 動態工作圖 FIFO／取消／借用："
          f"{len(required)}/{len(required)} 逐筆 --exact Rust 成功；"
          "完全持有式排程及 async self 尚未完成。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
