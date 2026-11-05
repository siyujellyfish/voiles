#!/usr/bin/env python3
"""G17 原始 P17.1～P17.6 同 SHA 正式內部執行總驗收，不涵蓋 P22 公開 CLI。"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

# 嚴格追溯原 P17～P22 施工流程所列六項；每項均需真正正向與拒絕證據。
# 此處的原始測試全在前面的 Rust 核心測試真正執行，另對精確範圍
# 逐個 Rust 測試分組再執行以避免 --list 空過濾器冒充驗收。
驗收 = {
    "P17.1／經 Checker 授權的來源與執行物件": {
        "正向": (
            "execution::tests::p17_a_同源正式_hir_與可達證據完整時可重複真求值",
        ),
        "拒絕": (
            "execution::tests::p17_a_已解析同型別名稱引用被轉接至其他宣告必須零事件拒絕",
            "execution::tests::p17_a_正式宣告可變性或型別事實被換掉不得沿用原授權",
            "execution::tests::p17_準備後來源更換或_hir_遭偽造_不得授權執行",
        ),
    },
    "P17.2／已驗證 int／bool 基本求值": {
        "正向": (
            "execution::tests::p17_標量讀寫_詞彙遮蔽與已驗證加減",
            "execution::tests::p17_布林分支只執行被選中的控制邊",
            "struct_stage::p17_private_struct_execution_tests::p17_f_已驗證結構具名覆蓋的整數欄位加法必須真求值",
        ),
        "拒絕": (
            "execution::tests::p17_動態溢位與來源位置分流_不冒充靜態診斷",
            "execution::tests::p17_靜態型別_未初始化_const_私有語法均在執行前拒絕",
        ),
    },
    "P17.3／所有權、借用與真資源": {
        "正向": (
            "struct_stage::p17_private_str_execution_tests::p17_c_真私有_checker_來源驅動字串建立移動再初始化並逆序清理",
            "struct_stage::p17_private_struct_execution_tests::p17_c_真私有混合結構整筆移動保留唯一_utf8_resource_id",
            "struct_stage::g17_private_source_borrow_tests::g17_c_合法_const_str_真共享借用取得和歸還動態票據不複製資源",
            "struct_stage::g17_private_source_borrow_tests::g17_c_合法_var_str_真獨占票據封鎖持有者直接存取後歸還",
        ),
        "拒絕": (
            "struct_stage::p17_private_str_execution_tests::p17_c_非法字串隱含複製在真_checker_階段就拒絕",
            "struct_stage::g17_private_source_borrow_tests::g17_c_偽造共享獨占權限或_source_id_在任何資源分配前拒絕",
            "struct_stage::g17_private_source_borrow_tests::g17_c_刪去原_checker_直線借用事實不可沿用舊票據授權",
        ),
    },
    "P17.4／內部錯誤、來源位置與失敗退出": {
        "正向": (
            "execution::file::tests::p17_真實運算溢位獨立回報來源行與檔案身分",
            "execution::tests::p18_執行期錯誤保留真實函式退出且不冒稱_return_成功",
        ),
        "拒絕": (
            "execution::file::tests::p17_真實非法_utf8_禁止進入執行器且保留詞彙行列",
            "execution::file::tests::p17_真實檔案_io_缺檔不偽造語言診斷",
        ),
    },
    "P17.5／原始檔至真計算端到端": {
        "正向": (
            "execution::file::tests::p17_真實中文檔名與_utf8_來源先驗證再計算",
            "execution::tests::p17_原始_voil_通過所有正式_checker_後真正計算_二加三",
        ),
        "拒絕": (
            "execution::file::tests::p17_真實未知名稱與未初始化屬靜態錯誤而非動態",
        ),
    },
    "P17.6／結構具名、預設、const、回滾順序": {
        "正向": (
            "struct_stage::p17_private_struct_execution_tests::p17_f_真具名引數先依來源序後評估未覆蓋預設且逆序清理",
            "struct_stage::p17_private_struct_execution_tests::p17_f_真具名欄位覆蓋預設時不分配多餘_utf8_資源",
            "struct_stage::p17_private_struct_execution_tests::p17_f_具名引數尾逗號沿用正式_parser_且只建立一次資源",
            "struct_stage::p17_private_struct_execution_tests::p17_f_已完整初始化_const_實例可以安全建立並離域釋放",
            "struct_stage::p17_private_struct_execution_tests::p17_f_具名整數來源算式必須按真_typed_hir_運算後才建構",
            "struct_stage::p17_private_struct_execution_tests::p17_f_被具名覆蓋的整數預設溢位式不能先行執行",
        ),
        "拒絕": (
            "struct_stage::p17_private_struct_execution_tests::p17_f_不合法未知具名欄位在真私有_checker_階段拒絕",
            "struct_stage::p17_private_struct_execution_tests::p17_f_重複具名引數須由真_checker_在建資源前拒絕",
            "struct_stage::p17_private_struct_execution_tests::p17_f_沒有預設的_const_欄位須在配置前拒絕",
            "struct_stage::p17_private_struct_execution_tests::p17_f_未初始化_const_實例必須由真_checker_拒絕",
            "struct_stage::p17_private_struct_execution_tests::p17_f_具名_utf8_先建立後預設_int_運算溢位須原子逆序清理",
            "struct_stage::p17_private_struct_execution_tests::p17_f_真整數欄位求值投影運算子被替換必須先於配置拒絕",
        ),
    },
}

# 測試全量已在 CI 同一工作執行一次；正式 Gate 再對六個真實
# Rust 測試模組分組執行，各組一定要實際執行至少一個案例。
測試模組 = (
    "execution::tests::p17_",
    "execution::source_audit::tests::p17_",
    "execution::file::tests::p17_",
    "struct_stage::p17_private_str_execution_tests::p17_",
    "struct_stage::p17_private_struct_execution_tests::p17_",
    "struct_stage::g17_private_source_borrow_tests::g17_",
)


def 執行(命令: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        命令, capture_output=True, text=True, encoding="utf-8", check=False
    )


def main() -> int:
    參數器 = argparse.ArgumentParser(description=__doc__)
    參數器.add_argument("--test-bin", type=Path, required=True)
    參數 = 參數器.parse_args()
    測試執行檔 = str(參數.test_bin.resolve())
    列表 = 執行([測試執行檔, "--list"])
    if 列表.returncode != 0:
        print("G17 正式 Gate：Rust 測試列表取得失敗", file=sys.stderr)
        print(列表.stderr, file=sys.stderr)
        return 1
    實際案例 = {
        行.split(": test", 1)[0]
        for 行 in 列表.stdout.splitlines()
        if 行.endswith(": test")
    }
    必需案例 = {
        名稱
        for 規格 in 驗收.values()
        for 類別 in ("正向", "拒絕")
        for 名稱 in 規格[類別]
    }
    if len(驗收) != 6 or any(
        not 規格["正向"] or not 規格["拒絕"]
        for 規格 in 驗收.values()
    ):
        print("G17 正式 Gate：六項中有空白的正反驗收", file=sys.stderr)
        return 1
    遺失 = sorted(必需案例 - 實際案例)
    if 遺失:
        print("G17 正式 Gate 真正測試缺漏：" + "、".join(遺失), file=sys.stderr)
        return 1

    總數 = 0
    for 模組 in 測試模組:
        結果 = 執行([測試執行檔, "--test-threads=2", 模組])
        原測試摘要 = 結果.stdout + 結果.stderr
        匹配 = re.search(
            r"test result: ok\.\s+(\d+) passed;\s+0 failed;\s+0 ignored;",
            原測試摘要,
        )
        if 結果.returncode != 0 or 匹配 is None or int(匹配.group(1)) == 0:
            print(f"G17 正式 Gate：模組 {模組} 未真正完整執行", file=sys.stderr)
            print(原測試摘要[-8000:], file=sys.stderr)
            return 1
        數目 = int(匹配.group(1))
        總數 += 數目
        print(f"已真正執行 {模組}：{數目} 通過")
    # 17.4 另核對真函式的動態失敗退出；其為 P18 相依的既有
    # 真實已執行測試，不在上面的 P17 前綴分組內。
    退出案例 = "execution::tests::p18_執行期錯誤保留真實函式退出且不冒稱_return_成功"
    退出 = 執行([測試執行檔, "--exact", 退出案例])
    if 退出.returncode or not re.search(
        r"test result: ok\.\s+1 passed;\s+0 failed;", 退出.stdout + 退出.stderr
    ):
        print("G17 17.4：真執行錯誤退出案例未成功", file=sys.stderr)
        print((退出.stdout + 退出.stderr)[-8000:], file=sys.stderr)
        return 1

    # 只有主線 push 的同 SHA 能宣稱正式 Passed；
    # PR 僅為 PassedCandidate，絕不因 CI 的 pull_request merge ref 混淆。
    github_ref = os.getenv("GITHUB_REF", "")
    event = os.getenv("GITHUB_EVENT_NAME", "")
    sha = os.getenv("GITHUB_SHA", "")
    if github_ref == "refs/heads/main" and event == "push":
        head = 執行(["git", "rev-parse", "HEAD"])
        if head.returncode != 0 or head.stdout.strip() != sha or not sha:
            print("G17 正式 Gate：main 推送與實際工作樹 SHA 不同", file=sys.stderr)
            return 1
        狀態 = "Passed"
    else:
        狀態 = "PassedCandidate"
    for 名稱, 規格 in 驗收.items():
        print(f"{名稱}：正向 {len(規格['正向'])}、拒絕 {len(規格['拒絕'])}，證據已真正執行")
    print(
        f"G17 六項＝6/6；正式分組真 Rust 測試 {總數}+1 例，"
        f"原始追溯案例 {len(必需案例)}／{len(必需案例)}；"
        f"驗收狀態＝{狀態}；commit={sha or '非 GitHub CI'}"
    )
    print(
        "此 Gate 僅驗收已核准非公開 P17 基本求值和真資源子集；"
        "不代表 P18～P22、一般巢狀方法／模組、未決共享語意或公開 run CLI。"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
