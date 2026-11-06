#!/usr/bin/env python3
"""G18／P18.2～18.3 有限 for／while 真迭代與 UTF-8 資源清理正反測試門檻。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_b_真_for_三次每迭代重新建立唯一_utf8_並離域釋放",
    "g18_b_真_for_iterator_返回目前迭代整數而非固定常值",
    "g18_b_真_for_iterator_多輪實際值不含右端點",
    "g18_b_巢狀_for_iterator_返回真內層宣告且逐層清理",
    "g18_b_巢狀同名_for_iterator_以真宣告身份遮蔽外層而非名稱猜測",
    "g18_b_真_for_iterator_continue_迭代每輪獨立且沒有逃逸",
    "g18_b_空半開_for_不建立迭代器值且不執行_return",
    "g18_b_返回已離域_for_iterator_須原_checker_先拒絕",
    "g18_b_竄改_for_iterator_身分或_return_來源須零資源拒絕",
    "g18_b_具名_var_bool_while_真重讀條件且本體改_false_後停止",
    "g18_b_具名_false_while_零迭代且僅建外部資源",
    "g18_b_具名_bool_重新賦值_false_到_true_選擇一次_break",
    "g18_b_已核准_copyable_bool_名稱複製後與來源變更不構成別名",
    "g18_b_具名_while_中的_early_return_僅清理實際選中本體與根資源",
    "g18_c_具名_while_動態故障真清理且不補做未到達賦值",
    "g18_b_具名_while_條件_DeclarationId_偷換必須零資源拒絕",
    "g18_b_具名_while_無須靜態終止改由執行期預算安全中止",
    "g18_b_具名_while_continue_每輪先清理真資源再回到條件受控耗盡",
    "g18_b_具名_const_bool_重賦值須在原_checker_先拒絕",
    "g18_b_半開空區間不建立任何假迭代資源",
    "g18_b_真_continue_每次先離域釋放再開始下一次",
    "g18_b_真_break_只執行首輪且離域釋放",
    "g18_b_真_while_false_不執行未走過的字串建構",
    "g18_b_while_true_只在直接已驗證_break_出口下執行一次",
    "g18_c_真_for_內_return_提前退出並按動態完成順序清理",
    "g18_c_第二次迭代真資源故障必回滾當次及外層持有者",
    "g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確",
    "g18_b_非法隱含_str_複製由原_checker_在第一筆資源前拒絕",
    "g18_b_六十五輪合法半開_for_不再遭私有固定上界拒絕",
    "g18_b_超過六十四輪的合法_for_依真實步數預算成功或受控耗盡",
    "g18_b_巨大合法_i64_for_上界不應在_prepare_被拒絕或試圖全量迭代",
    "g18_b_空步數預算必須在任一真_utf8_建立前拒絕",
    "g18_c_巢狀_for_執行預算耗盡必清理全部已走到的迭代資源",
    "g18_b_while_條件零迭代仍依真檢查消耗預算而非靜態略過",
    "g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕",
    "g18_b_刪除原_checker_控制轉移或改寫來源不可沿用舊執行許可",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    check = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if check.returncode:
        print("G18 真迴圈：無法讀取原 Rust 測試列表", file=sys.stderr)
        print(check.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in check.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::g18_private_source_loop_tests::" + item for item in 案例
    }
    missing = sorted(expected - names)
    if missing:
        print("G18 有限迴圈真來源正反缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    # 驗收期真跑迴圈執行器，不只是靜態測試名稱存在。
    executed = subprocess.run(
        [str(args.test_bin.resolve()), "--test-threads=2",
         "struct_stage::g18_private_source_loop_tests::"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if executed.returncode or (
        f"{len(expected)} passed; 0 failed;" not in executed.stdout
    ):
        print("G18／P18.2 迴圈資源 Gate 未完整真執行：" +
              executed.stdout + executed.stderr, file=sys.stderr)
        return 1
    print(f"G18／P18.2 真 for／while 與執行預算："
          f"{len(expected)}/{len(expected)} 項真 Rust 動態回歸全部通過。")
    print("注意：這是已核准的受限內部執行子批，不代表 P18.1～P18.6 G18 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
