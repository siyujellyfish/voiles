#!/usr/bin/env python3
"""P17 首批內部真實執行回歸入口，只驗證既有全量核心測試確實包含必需正反案例。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

測試名稱 = (
    "p17_原始_voil_通過所有正式_checker_後真正計算_二加三",
    "p17_標量讀寫_詞彙遮蔽與已驗證加減",
    "p17_布林分支只執行被選中的控制邊",
    "p17_無參數前向函式回傳值可作運算元",
    "p17_靜態型別_未初始化_const_私有語法均在執行前拒絕",
    "p17_動態溢位與來源位置分流_不冒充靜態診斷",
    "p17_準備後來源更換或_hir_遭偽造_不得授權執行",
    "p18_尚未建立遞迴退出契約時安全拒絕_不爆堆疊",
)


def 主程式() -> int:
    命令 = argparse.ArgumentParser(description=__doc__)
    命令.add_argument("--test-bin", type=Path, required=True)
    參數 = 命令.parse_args()
    結果 = subprocess.run(
        [str(參數.test_bin.resolve()), "--list"],
        text=True,
        encoding="utf-8",
        capture_output=True,
        check=False,
    )
    if 結果.returncode:
        print("P17 內部測試清單取得失敗", file=sys.stderr)
        print(結果.stderr, file=sys.stderr)
        return 1
    真正測試 = {
        一行.split(": test", 1)[0]
        for 一行 in 結果.stdout.splitlines()
        if 一行.endswith(": test")
    }
    必要 = {"execution::tests::" + 名稱 for 名稱 in 測試名稱}
    缺失 = sorted(必要 - 真正測試)
    if 缺失:
        print("P17 真正 Rust 正反案例缺失：" + "、".join(缺失), file=sys.stderr)
        return 1
    print(f"P17 內部切片證據存在：{len(必要)}／{len(必要)}；"
          "Rust 核心全量執行由既有步驟完成，不重複跑測試。")
    print("本關卡不是 G17 正式驗收，僅是 D00／P17 首批內部回歸。")
    return 0


if __name__ == "__main__":
    raise SystemExit(主程式())
