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
    "p18_多層非遞迴函式框架按實際呼叫返回",
    "p18_提前回傳阻止同函式剩餘可執行路徑的溢位",
    "p18_函式框架的遮蔽不影響被呼叫函式的頂層宣告",
    "p18_同一已驗證函式可重複呼叫且每次只執行實際路徑",
    "p18_實際真分支與提前返回事件按路徑順序登錄",
    "p18_實際_false_分支只登錄_else_及被執行的返回",
    "p18_多層函式實際事件保持嚴格進出堆疊順序",
    "p18_動態溢位保留真實已進入函式且未成功退出的控制紀錄",
    "p18_來源或_hir_失效時事件簿必須完全為空",
    "p18_多重_else_if_僅登錄實際檢查至成立的分支",
    "p18_真控制邊紀錄只包含實際檢查的條件與返回",
    "p18_無任何成立分支才會紀錄真正_else_選擇",
    "p18_巢狀真函式的進出順序與返回值一致",
    "p18_執行期錯誤保留真實函式退出且不冒稱_return_成功",
    "p18_驗證來源被改寫時拒絕且不得產生任何控制事件",
)

事件稽核測試名稱 = (
    "p18_真實控制事件稽核接受巢狀函式成對成功返回",
    "p18_控制事件稽核拒絕無進入就退出與漏退出",
    "p18_控制事件稽核拒絕未執行返回卻虛報成功或錯報失敗",
    "p18_控制事件稽核拒絕假來源跨度與跨函式身分",
)

檔案測試名稱 = (
    "p17_真實中文檔名與_utf8_來源先驗證再計算",
    "p17_真實檔案_io_缺檔不偽造語言診斷",
    "p17_真實非法_utf8_禁止進入執行器且保留詞彙行列",
    "p17_真實未知名稱與未初始化屬靜態錯誤而非動態",
    "p17_真實運算溢位獨立回報來源行與檔案身分",
)

私有真資源測試名稱 = (
    "p17_c_真私有_checker_來源的字串實際建立移動逆向清理",
    "p17_c_真實字串跳脫只採用既有_lexer_已解碼結果",
    "p17_c_非_copyable_來源的隱含複製被正式_checker_拒絕",
    "p17_c_超出已證明範圍的合法語法在資源建立前保守阻擋",
    "p17_c_竄改真_checker_具名移動證據不得執行",
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
    必要.update({"execution::file::tests::" + 名稱 for 名稱 in 檔案測試名稱})
    必要.update({"execution::trace::tests::" + 名稱 for 名稱 in 事件稽核測試名稱})
    必要.update({"struct_stage::p17_checked_private_runtime_tests::" + 名稱
                for 名稱 in 私有真資源測試名稱})
    缺失 = sorted(必要 - 真正測試)
    if 缺失:
        print("P17 真正 Rust 正反案例缺失：" + "、".join(缺失), file=sys.stderr)
        return 1
    print(f"P17 內部切片證據存在：{len(必要)}／{len(必要)}；"
          "Rust 核心全量執行由既有步驟完成，不重複跑測試。")
    print("本關卡不是 G17/G18 正式驗收，只核對非公開內部真來源測試已由全量 Rust 執行。")
    return 0


if __name__ == "__main__":
    raise SystemExit(主程式())
