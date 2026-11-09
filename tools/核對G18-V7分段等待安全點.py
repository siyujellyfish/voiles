#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 A2：原 CheckedTask 分段安全點、單執行緒喚醒與取消五項真 Rust 正反。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

模組 = "execution::async_task::v7_a2_safe_point_tests::"
案例 = (
    "v7_a2_分段安全點允許其他已核准工作先執行再喚醒原工作",
    "v7_a2_已暫停工作取消先阻止啟動再真實反序清理",
    "v7_a2_不同會話相同工作編號不能冒用喚醒或取消",
    "v7_a2_暫停後會話新增真借用需歸還才能喚醒",
    "v7_a2_工作在暫停期間被提前取消仍可清理等待框架",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listing = subprocess.run(
        [binary, "--list"], capture_output=True, text=True,
        encoding="utf-8", check=False,
    )
    if listing.returncode:
        print("G18 V7 讀取 Rust 真測試失敗：" + listing.stderr, file=sys.stderr)
        return 1
    existing = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines() if line.endswith(": test")
    }
    expected = [模組 + name for name in 案例]
    missing = [name for name in expected if name not in existing]
    if missing:
        print("G18 V7 A2 真來源缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in expected:
        result = subprocess.run(
            [binary, "--exact", name],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if result.returncode or "1 passed; 0 failed;" not in result.stdout:
            print("G18 V7 A2 真執行失敗：" + name + "\n" +
                  result.stdout + result.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 A2 已核准工作分段安全點正反：{len(expected)}/{len(expected)}"
          " 已逐項 --exact 真執行；P18.6 完整協程仍未驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
