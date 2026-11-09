#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E2：真正私有 async fn 經 Checker 核准，在動態 await 安全點讓出並返回五項真來源。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_v7_e2_真私有await在內層函式中讓出且父框架資源仍存活",
    "g18_v7_e2_雙真正await只按動態左右順序發出協作切換",
    "g18_v7_e2_未達等待點的框架預算耗盡絕不執行排程者",
    "g18_v7_e2_普通同步呼叫不得觸發等待排程",
    "g18_v7_e2_真await讓出後原執行溢位仍逐框架清理且不虛構返回",
)
前綴 = "struct_stage::g18_private_scalar_call_tests::"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    result = subprocess.run(
        [binary, "--list"], capture_output=True, text=True, encoding="utf-8",
        check=False,
    )
    if result.returncode:
        print("G18 V7 E2 讀取真 Rust 測試清單失敗："
              + result.stdout + result.stderr, file=sys.stderr)
        return 1
    names = {line.split(": test", 1)[0]
             for line in result.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in names]
    if missing:
        print("G18 V7 E2 真來源缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run(
            [binary, "--exact", name], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 E2 協作讓出真執行失敗：" + name
                  + "\n" + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 E2 真私有 Checker async 協作讓出：{len(required)}/"
          f"{len(required)} 逐項 --exact 成功；完整可持久協程仍未完成。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
