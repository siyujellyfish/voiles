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
    "p18_真實_checker_來源的控制事件可通過_ast_角色核對",
    "p18_相同函式內的合法跨度不能冒充另一個_return_角色",
    "p18_同函式內非法_else_事件不得借用正常_span_冒充來源",
    "p18_另一函式的真跨度不得冒用現行函式身分",
    "p18_竄改非原始來源的_函式_ast_或文字不得授權執行事件",
    "p18_e_真正_else_if_多段決策依原_ast_順序通過",
    "p18_e_合法條件跨度也不可跳過前一段_else_if_直接進入後段",
    "p18_e_同一_if_已真成立後不可偽報再檢查_else_if",
    "p18_e_真假_else_必須由全_false_決策鏈觸發",
    "p18_e_不能刪去最後_false_條件後仍假稱已完成正常返回",
    "p18_e_同一函式多次呼叫應分別重設真正分支進度",
    "p18_e_真_if_else_返回僅可由實際選中的路徑取得",
    "p18_e_真_if_已成立時不可把合法_else_return_偽造成實際返回",
    "p18_e_選中的_else_return_不可假冒未成立的第一分支返回",
    "p18_e_不成立分支的_return_不能取代外層確實執行的_return",
    "p18_e_巢狀_if_else_只有實際完整祖先路徑可授權返回",
)

來源詞彙防偽測試名稱 = (
    "p17_真實標量來源與巢狀算術_群組_函式正常求值",
    "p17_相同跨度換整數常值不可沿用舊型別_hir_執行",
    "p17_布林條件內容改寫不能憑原跨度取得真控制邊",
    "p17_具名讀取的節點文字改寫不能偷用原宣告身分",
    "p17_同型別函式目標換名不能借用已驗證呼叫型別",
    "p17_賦值目標偷換同型別名稱須在動態效果前拒絕",
    "p17_來源原始字串被改寫不會發出任何真執行事件",
)

檔案測試名稱 = (
    "p17_真實中文檔名與_utf8_來源先驗證再計算",
    "p17_真實檔案_io_缺檔不偽造語言診斷",
    "p17_真實非法_utf8_禁止進入執行器且保留詞彙行列",
    "p17_真實未知名稱與未初始化屬靜態錯誤而非動態",
    "p17_真實運算溢位獨立回報來源行與檔案身分",
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
    必要.update({"execution::source_audit::tests::" + 名稱 for 名稱 in 來源詞彙防偽測試名稱})
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
