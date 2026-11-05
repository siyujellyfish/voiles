#!/usr/bin/env python3
"""G18 P18.1～P18.6 原始六項真來源稽核：--audit 核實子集，--check 嚴格正式驗收。

--audit 不等於 G18 Passed。一般遞迴／中止 D18-01 與方法／async D18-02
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
        "execution::tests::p18_尚未建立遞迴退出契約時安全拒絕_不爆堆疊",
        "struct_stage::g18_private_source_loop_tests::g18_c_第二次迭代真資源故障必回滾當次及外層持有者",
        "struct_stage::g18_private_call_borrow_tests::g18_c_被呼叫函式真借用存活時故障必先歸還票據再釋放擁有者",
        "struct_stage::p17_private_str_execution_tests::p18_c_三層巢狀返回跳過外層結果且真資源逐層退出",
    ),
    "P18.4": (
        "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id",
        "struct_stage::g18_private_call_borrow_tests::g18_d_來源借用別名再作函式實參須由_checker_保守拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據",
    ),
    "P18.5": (
        "execution::tests::p18_執行期錯誤保留真實函式退出且不冒稱_return_成功",
        "execution::tests::p18_巢狀真函式的進出順序與返回值一致",
        "struct_stage::g18_private_source_loop_tests::g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確",
        "struct_stage::g18_private_source_loop_tests::g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕",
    ),
    # 18.6 需要真 method/self／多載／async 的核准簽章與生命週期。
    # 沒有這些真執行正反時不得填入名稱冒充已完成。
    "P18.6": (),
}

# 原始必需項目實際驗收狀態，並非設計時理想值。
# 18.1／18.2／18.4／18.5 已有合法動態子集，但不能代表完整原項；
# 18.3／18.6 的未裁決安全行為不能僅靠「拒絕」宣稱 Passed。
狀態 = {
    "P18.1": ("SubsetVerified", "普通函式已核准部分真呼叫／返回；更廣參數與前向相依仍需完整矩陣"),
    "P18.2": ("SubsetVerified", "真 if／else-if 與可證明的有限 for/while 子集；一般動態迴圈尚未驗收"),
    "P18.3": ("BlockedByDecision", "D18-01：遞迴深度／中止與執行耗盡之資源清理政策尚未核准"),
    "P18.4": ("SubsetVerified", "單一 str 參數真正 BorrowTicket；一般參數、方法 self／逃逸仍缺完整正反"),
    "P18.5": ("SubsetVerified", "已有控制、資源與故障真回歸，但尚非原全矩陣結案"),
    "P18.6": ("BlockedByDecision", "D18-02：方法多載與 async／跨 await／例外生命週期缺明確語意與真執行證據"),
}

真執行群組 = (
    "execution::tests::p18_",
    "struct_stage::p17_private_str_execution_tests::p18_",
    "struct_stage::g18_private_source_loop_tests::g18_",
    "struct_stage::g18_private_call_borrow_tests::g18_",
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
