#!/usr/bin/env python3
"""G18 P18.1～P18.6 原始六項真來源稽核：--audit 核實子集，--check 嚴格正式驗收。

--audit 不等於 G18 Passed。D18-01 已核准與部分真遞迴執行，方法／async D18-02
仍未核准時，必須明確回報 BlockedByDecision，不能以保守拒絕替代
原始六項的正反真執行驗收。
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

# 只收納確實有 Checker→真正 Runtime 執行的既有測試名；
# 不以靜態 CFG 或模型模擬結果冒充動態已經走過的路徑。
證據 = {
    "P18.1": (
        "execution::tests::p18_多層非遞迴函式框架按實際呼叫返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算",
        "struct_stage::g18_private_scalar_call_tests::g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數",
        "execution::tests::p18_同一已驗證函式可重複呼叫且每次只執行實際路徑",
        "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_const_str_參數共享借用真票據與返回值",
        "struct_stage::g18_private_call_borrow_tests::g18_d_錯誤實參型別及缺少參數在正式_checker_先拒絕",
    ),
    "P18.2": (
        "execution::tests::p18_真控制邊紀錄只包含實際檢查的條件與返回",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_for_三次每迭代重新建立唯一_utf8_並離域釋放",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_continue_每次先離域釋放再開始下一次",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_while_false_不執行未走過的字串建構",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_break_只執行首輪且離域釋放",
    ),
    "P18.3": (
        "execution::tests::p18_無限遞迴耗盡預算安全拒絕_不爆堆疊",
        "execution::tests::p18_已核准直接遞迴_可由全域_bool_終止並逐框架返回",
        "execution::tests::p18_已核准互相遞迴_使用原函式宣告與真動態框架",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真直接遞迴每層_utf8_所有者按呼叫及分支逆序清理",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真互相遞迴各層和分支資源跨框架清理不重覆",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_遞迴框架預算耗盡須釋放已建立_utf8_且不得假成功",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_遞迴步數預算耗盡即使分支開啟仍逐層逆序清理",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真第二層局部_utf8_建立後注入故障逐層回滾",
        "execution::tests::p18_步數預算耗盡須沿真函式逐層退出不偽造成功",
        "struct_stage::g18_private_source_loop_tests::g18_c_第二次迭代真資源故障必回滾當次及外層持有者",
        "struct_stage::g18_private_call_borrow_tests::g18_c_被呼叫函式真借用存活時故障必先歸還票據再釋放擁有者",
        "struct_stage::p17_private_str_execution_tests::p18_c_三層巢狀返回跳過外層結果且真資源逐層退出",
    ),
    "P18.4": (
        "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id",
        "struct_stage::g18_private_scalar_call_tests::g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者",
        "struct_stage::g18_private_call_borrow_tests::g18_d_來源借用別名再作函式實參須由_checker_保守拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據",
    ),
    "P18.5": (
        "execution::tests::p18_執行期錯誤保留真實函式退出且不冒稱_return_成功",
        "struct_stage::g18_private_scalar_call_tests::g18_c_真正參數框架中途故障必清空且不冒充成功返回",
        "struct_stage::g18_private_scalar_call_tests::g18_c_函式參數真加法溢位不能假造_return_且確實離域",
        "struct_stage::g18_private_scalar_call_tests::g18_c_函式參數真加法溢位不能假造_return_且確實離域",
        "execution::tests::p18_巢狀真函式的進出順序與返回值一致",
        "struct_stage::g18_private_source_loop_tests::g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確",
        "struct_stage::g18_private_source_loop_tests::g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕",
    ),
    # P18.6 同步 self 共享借用有真執行正反；多載／async 仍需
    # 原 Checker 唯一解析與正向執行，不能憑本子集宣稱 Passed。
    "P18.6": (
        "struct_stage::g18_private_method_self_tests::g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位",
        "struct_stage::g18_private_method_self_tests::g18_f_獨占_self_真寫入_var_int_並以同張票據返回更新後值",
        "struct_stage::g18_private_method_self_tests::g18_f_共享_self_真欄位加法按原_checker_hir_求值",
        "struct_stage::g18_private_method_self_tests::g18_f_共享_self_真欄位加法溢位受控錯誤且歸還票據",
        "struct_stage::g18_private_method_self_tests::g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域",
        "struct_stage::g18_private_method_self_tests::g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕",
        "struct_stage::g18_private_method_self_tests::g18_f_方法真參數返回使用所選_DeclarationId_的框架值",
        "struct_stage::g18_private_method_self_tests::g18_f_方法參數真加法在框架內求值不誤讀_self_欄位",
        "struct_stage::g18_private_method_self_tests::g18_f_方法參數真減法與重複呼叫保留同一來源參數值",
        "struct_stage::g18_private_method_self_tests::g18_f_方法參數真加法溢位受控錯誤且先歸還_self_票據",
        "struct_stage::g18_private_method_self_tests::g18_f_方法參數真讀取之前故障不得報告成功返回",
        "struct_stage::g18_private_method_self_tests::g18_f_篡改已驗證的方法參數返回身分或算術方案必在配置前拒絕",
        "struct_stage::g18_private_method_self_tests::g18_f_同名方法單_int_實參依原_checker_型別選真共享多載",
        "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參的多載宣告順序不影響真方法選擇",
        "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據",
        "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活",
        "struct_stage::g18_private_method_self_tests::g18_f_竄改已驗證_int_方法實參值或宣告身分須在動態執行前拒絕",
        "struct_stage::g18_private_method_self_tests::g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選",
        "struct_stage::g18_private_method_self_tests::g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行",
        "struct_stage::g18_private_method_self_tests::g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選",
        "struct_stage::g18_private_method_self_tests::g18_f_零參數多載方法中途故障仍先歸還原_self_票據",
        "struct_stage::g18_private_method_self_tests::g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕",
    ),
}

# 原始必需項目實際驗收狀態，並非設計時理想值。
# 18.1／18.2／18.4／18.5 已有合法動態子集，但不能代表完整原項；
# 18.3／18.6 的未裁決安全行為不能僅靠「拒絕」宣稱 Passed。
狀態 = {
    "P18.1": ("SubsetVerified", "無參數函式、str 借用與前向 Copyable int 真傳參／群組加減已接通；一般參數及呼叫圖仍缺完整矩陣"),
    "P18.2": ("SubsetVerified", "真 if／else-if 與可證明的有限 for/while 子集；一般動態迴圈尚未驗收"),
    "P18.3": ("Incomplete", "D18-01-A～D 的 AW06 無資源遞迴、私有 Checker bool 參數直接／互相遞迴與 str 真逐框架清理已接通；跨框架借用、一般簽章／遞迴資源完整矩陣仍待驗收"),
    "P18.4": ("SubsetVerified", "單一 str 參數 BorrowTicket 實際受控讀取及 Copyable int 真傳遞；多型別、self／逃逸仍缺完整正反"),
    "P18.5": ("SubsetVerified", "已有控制、資源與故障真回歸，但尚非原全矩陣結案"),
    "P18.6": ("BlockedByDecision", "D18-02-A～D 原則已核准，同步共享／獨占 self 真票據與靜態唯一零參數及單 int 常值實參多載、真參數直接返回與加減已可執行；多參數／複雜算式及 async 調度取消細則、正向執行仍待裁決及驗收"),
}

真執行群組 = (
    "execution::tests::p18_",
    "struct_stage::p17_private_str_execution_tests::p18_",
    "struct_stage::g18_private_source_loop_tests::g18_",
    "struct_stage::g18_private_call_borrow_tests::g18_",
    "struct_stage::g18_private_scalar_call_tests::g18_",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_",
    "struct_stage::g18_private_method_self_tests::g18_",
)


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, text=True, encoding="utf-8", capture_output=True, check=False)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--audit", action="store_true",
                      help="驗證既有真來源證據並明確回報未達 G18 的阻塞")
    mode.add_argument("--check", action="store_true",
                      help="嚴格正式驗收：六項真執行 Passed 才會成功")
    args = parser.parse_args()
    executable = str(args.test_bin.resolve())
    listing = run([executable, "--list"])
    if listing.returncode:
        print("G18 真來源測試清單無法取得：" + listing.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines()
        if line.endswith(": test")
    }
    if set(證據) != {f"P18.{i}" for i in range(1, 7)} or set(狀態) != set(證據):
        print("G18 原六項追溯數量／身分不完整", file=sys.stderr)
        return 1
    required = {case for group in 證據.values() for case in group}
    missing = sorted(required - available)
    if missing:
        print("G18 正反真來源缺失：" + "、".join(missing), file=sys.stderr)
        return 1

    total = 0
    for group in 真執行群組:
        execution = run([executable, "--test-threads=2", group])
        printed = execution.stdout + execution.stderr
        match = re.search(
            r"test result: ok\.\s+(\d+) passed;\s+0 failed;\s+0 ignored;",
            printed,
        )
        if execution.returncode != 0 or match is None or int(match.group(1)) <= 0:
            print(f"G18 真執行測試群 {group} 未完整成功", file=sys.stderr)
            print(printed[-6000:], file=sys.stderr)
            return 1
        count = int(match.group(1))
        total += count
        print(f"G18 來源群組真執行：{group}，{count} 通過")

    # PR merge ref 絕不能自稱 main 正式 Passed。
    github_ref = os.getenv("GITHUB_REF", "")
    github_sha = os.getenv("GITHUB_SHA", "")
    if github_ref == "refs/heads/main" and os.getenv("GITHUB_EVENT_NAME") == "push":
        checked_head = run(["git", "rev-parse", "HEAD"])
        if checked_head.returncode or checked_head.stdout.strip() != github_sha or not github_sha:
            print("G18 稽核同一真 main SHA 不一致", file=sys.stderr)
            return 1
    blocked = []
    subsets = []
    for item, (state, reason) in 狀態.items():
        print(f"{item}：{state}｜{reason}｜已追溯 {len(證據[item])} 筆真正 Rust 案例")
        if state == "BlockedByDecision":
            blocked.append(item)
        elif state != "Passed":
            subsets.append(item)
    complete = not blocked and not subsets and all(證據.values())
    result = "Passed" if complete else "BlockedByDecision" if blocked else "Incomplete"
    print(
        f"G18 正式驗收狀態＝{result}；原項全 Passed {6-len(blocked)-len(subsets)}/6；"
        f"保守阻塞 {','.join(blocked) or '無'}；待全覆蓋 {','.join(subsets) or '無'}；"
        f"實際重跑 {total} 項 Rust 真來源；追溯 {len(required)}/{len(required)}；"
        f"commit={github_sha or '非 GitHub CI'}"
    )
    if args.check and not complete:
        print("G18 --check：原六項尚未全部 Passed，嚴禁標記正式結案。", file=sys.stderr)
        return 1
    # --audit 回傳成功只表示「阻塞狀態被正確核實」，不表示 G18 Passed。
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
