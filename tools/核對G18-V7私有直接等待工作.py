#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 A1：原私有 Checker 直接 await 跨呼叫保存工作、真喚醒與取消十筆來源。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_private_deferred_await_tests::"
案例 = (
    "g18_v7_a1_私有Checker工作可跨呼叫保存直到真喚醒才執行",
    "g18_v7_a1_兩個私有Checked工作只依真喚醒順序執行",
    "g18_v7_a1_取消已暫停私有工作不執行任何函式事件",
    "g18_v7_a1_真正私有async局部UTF8只在喚醒後建構並唯一釋放",
    "g18_v7_a2_私有直接等待真運算溢位後錯誤與資源單次退出",
    "g18_v7_a1_有前置main資源不得假裝成已保存的等待框架",
    "g18_v7_a1_複合await與普通同步呼叫不能冒充直接等待",
    "g18_v7_a1_靜態非法來源不得建立工作或消耗工作身分",
    "g18_v7_a1_已封存私有來源竄改不能執行而可安全取消",
    "g18_v7_a1_真工作不允許重複暫停或重複喚醒",
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
        print("G18 V7 私有工作清單讀取失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    known = {
        line.split(": test", 1)[0] for line in listed.stdout.splitlines()
        if line.endswith(": test")
    }
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("G18 V7 私有來源缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 真執行失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 真私有 Checker 直接 await 工作跨呼叫保存："
          f"{len(required)}/{len(required)} 已逐筆 --exact 執行；"
          "不可替代執行中協程的正式驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
