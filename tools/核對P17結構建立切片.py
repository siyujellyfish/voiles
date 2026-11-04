#!/usr/bin/env python3
"""P17-F 具名與預設欄位之私有真結構建立式十一項 Rust 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "p17_f_真私有_checker_空建立式求值預設_utf8_並真正釋放",
    "p17_f_真具名引數先依來源序後評估未覆蓋預設且逆序清理",
    "p17_f_真具名欄位覆蓋預設時不分配多餘_utf8_資源",
    "p17_f_真結構預設建構中途失敗回滾並逆序釋放已建資源",
    "p17_f_不合法未知具名欄位在真私有_checker_階段拒絕",
    "p17_f_巢狀結構或來源替換仍不會執行未驗證資源",
    "p17_f_結構真原始計畫合法時可重複執行且逐次釋放資源",
    "p17_f_已驗證欄位預設值內容改寫不得使用原_checker_授權資源",
    "p17_f_已驗證具名欄位值與欄位名稱若被偷換必須事前拒絕",
    "p17_f_欄位種類與不可變權限竄改不得產生結構資源事件",
    "p17_f_入口返回整數與實例名稱若被竄改不能沿用原來源",
)


def main() -> int:
    command = argparse.ArgumentParser(description=__doc__)
    command.add_argument("--test-bin", type=Path, required=True)
    args = command.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if result.returncode != 0:
        print(f"P17-F 取得 Rust 原測試列表失敗：{result.stderr}", file=sys.stderr)
        return 1
    actual = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::p17_private_struct_execution_tests::" + name for name in 案例
    }
    missing = sorted(expected - actual)
    if missing:
        print("P17-F 真 Rust 正反測試缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"P17-F：{len(expected)}/{len(expected)} 真結構來源測試已納入現有完整 Rust 執行。")
    print("本檢查不是 G17 17.6 正式驗收；不重跑 Rust 核心全量測試。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
