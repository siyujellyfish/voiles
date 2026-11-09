#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E3：真 FIFO 安全關閉、跨工作借用與遞迴／迴圈 P16 退場八項 Rust。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_cooperative_shutdown_tests::"
案例 = (
    "g18_v7_e3_三工作安全關閉須逆登錄順序取消且各自真資源歸零",
    "g18_v7_e3_任何待關閉工作仍持真BorrowTicket整批預檢必零狀態變更",
    "g18_v7_e3_真遞迴兩層UTF8框架停在await後關閉須內層優先釋放",
    "g18_v7_e2_三次for真UTF8迭代已清理後才允許await暫停並安全關閉",
    "g18_v7_e2_while真動態迴圈清理後等待由同一工作恢復不重跑迭代",
    "g18_v7_e3_關閉保留已完成工作真Outcome只取消其餘Ready或Suspended",
    "g18_v7_e3_尚有Running工作關閉保守拒絕且保持整批FIFO",
    "g18_v7_e3_尚未開始的三份來源關閉不得產生任何真Runtime事件",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run([binary, "--list"], text=True,
                            encoding="utf-8", capture_output=True, check=False)
    if listed.returncode:
        print("G18 V7 批次關閉真測試清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + case for case in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("G18 V7 批次關閉來源缺項：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 批次安全點真清理失敗：" + name
                  + "\n" + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 E3 安全關閉／真遞迴與迴圈／HC14 P16："
          f"{len(required)}/{len(required)} 逐筆 --exact Rust 成功；"
          "不提前將 P18.6 標記 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
