#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.3｜使用原 P01 Checker AW06-A 的直接／相互遞迴真框架與受控耗盡七項正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "p18_無限遞迴耗盡預算安全拒絕_不爆堆疊",
    "p18_已核准直接遞迴_可由全域_bool_終止並逐框架返回",
    "p18_已核准互相遞迴_使用原函式宣告與真動態框架",
    "p18_遞迴框架預算小於合法真呼叫層數也須原子報錯",
    "p18_步數預算耗盡須沿真函式逐層退出不偽造成功",
    "p18_同一已核准遞迴來源使用不同預算不改變語言語意",
    "p18_零預算與遭竄改來源不得產生假進入函式事件",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    output = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if output.returncode:
        print("G18 遞迴真來源 Rust 清單取得失敗：" + output.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in output.stdout.splitlines() if line.endswith(": test")
    }
    expected = {"execution::tests::" + name for name in 案例}
    if expected - names:
        print("G18 AW06 遞迴正反案例缺漏：" + "、".join(sorted(expected - names)),
              file=sys.stderr)
        return 1
    print(f"G18 AW06 直接／相互遞迴及可配置耗盡真框架：{len(expected)}/{len(expected)}；"
          "原 P18.3 尚待 str／借用多框架清理完整驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
