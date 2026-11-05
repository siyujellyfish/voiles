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
        "execution::tests::p18_巢狀真函式的進出順序與返回值一致",
        "struct_stage::g18_private_source_loop_tests::g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確",
        "struct_stage::g18_private_source_loop_tests::g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕",
    ),
    # P18.6 同步 self 共享借用有真執行正反；多載／async 仍需
    # 原 Checker 唯一解析與正向執行，不能憑本子集宣稱 Passed。
    "P18.6": (
        "struct_stage::g18_private_method_self_tests::g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位",
        "struct_stage::g18_private_method_overload_runtime_tests::g18_f_真多載_int_參數由已選獨占方法實際寫入欄位並返回",
        "struct_stage::g18_private_method_overload_runtime_tests::g18_f_真多載_bool_參數由原_checker_唯一選中共享方法不寫入",
        "struct_stage::g18_private_method_overload_runtime_tests::g18_f_真多載借用存活中失敗必歸還票據和局部參數",
        "struct_stage::g18_private_method_self_tests::g18_f_獨占_self_真寫入_var_int_並以同張票據返回更新後值",
        "struct_stage::g18_private_method_self_tests::g18_f_共享_self_真欄位加法按原_checker_hir_求值",
        "struct_stage::g18_private_method_self_tests::g18_f_共享_self_真欄位加法溢位受控錯誤且歸還票據",
        "struct_stage::g18_private_method_self_tests::g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域",
        "struct_stage::g18_private_method_self_tests::g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕",
    ),
}

# 原始必需項目實際驗收狀態，並非設計時理想值。
# 18.1／18.2／18.4／18.5 已有合法動態子集，但不能代表完整原項；
# 18.3／18.6 的未裁決安全行為不能僅靠「拒絕」宣稱 Passed。
狀態 = {
    "P18.1": ("SubsetVerified", "無參數函式、str 借用與前向 Copyable int 真傳參／群組加減已接通；一般參數及呼叫圖仍缺完整矩陣"),
    "P18.2": ("SubsetVerified", "真 if／else-if 與可證明的有限 for/while 子集；一般動態迴圈尚未驗收"),
    "P18.3": ("Incomplete", "D18-01-A～D 的 AW06 零參數直接／相互遞迴、執行預算／故障逐框架退出已有真正正反；str／借用遞迴資源完整責任與一般簽章仍待驗收"),
    "P18.4": ("SubsetVerified", "單一 str 參數 BorrowTicket 實際受控讀取及 Copyable int 真傳遞；多型別、self／逃逸仍缺完整正反"),
    "P18.5": ("SubsetVerified", "已有控制、資源與故障真回歸，但尚非原全矩陣結案"),
    "P18.6": ("BlockedByDecision", "D18-02-A～D 已核准，S20 同名 int/bool 真參數與 self BorrowTicket 已有動態子集；一般多載／async 調度、取消與正向真執行仍缺完整裁決驗收"),
}

真執行群組 = (
    "execution::tests::p18_",
    "struct_stage::p17_private_str_execution_tests::p18_",
    "struct_stage::g18_private_source_loop_tests::g18_",
    "struct_stage::g18_private_call_borrow_tests::g18_",
    "struct_stage::g18_private_scalar_call_tests::g18_",
    "struct_stage::g18_private_method_self_tests::g18_",
    "struct_stage::g18_private_method_overload_runtime_tests::g18_",
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
