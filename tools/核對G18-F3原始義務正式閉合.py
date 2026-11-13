#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F3：return／宣告／賦值／條件／迴圈之已核准呼叫位置，十五等價類三十真正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

# 同一原 Parser／Checker→TypedHIR→實際呼叫工作棧，不容重複測試冒充不同契約。
CONTRACT = {
    "F3-C01": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_巢狀實參呼叫圖需收錄內外兩個int目標而略過無關函式",
        "struct_stage::g18_f3_call_site_tests::f3_sites_主函式同一圖按初始化賦值分支與返回順序完成四呼叫",
    ),
    "F3-C02": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main前置真呼叫後尾端int常值return必執行呼叫",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main前置兩函式依序執行後才允許返回無關常值",
    ),
    "F3-C03": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main真賦值函式後尾端括號整數常值仍須執行",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main選中分支真字串借用嵌入第二層函式實參",
    ),
    "F3-C04": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_主函式兄弟分支只執行選中函式而收錄兩者並略過無關bool",
        "struct_stage::g18_f3_call_site_tests::f3_sites_未選if葉不呼叫而選中else借用完成先還票後離域",
    ),
    "F3-C05": (
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_if真函式返回true才執行選中分支",
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_if真函式返回false不執行未選分支",
    ),
    "F3-C06": (
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_else_if按原順序真執行前兩個條件",
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_else_if首分支已選中時不可多執行後續條件",
    ),
    "F3-C07": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f3_真結構bool欄位三輪for內呼叫函式有序累積",
        "struct_stage::g18_f3_call_site_tests::f3_sites_for三輪逐次呼叫並於每輪退出真utf8與借用",
    ),
    "F3-C08": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f3_真結構欄位零次for永不進入待呼叫函式",
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f3_真結構欄位for條件continue不執行後續呼叫",
    ),
    "F3-C09": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f3_真結構欄位while中呼叫後break不重複執行",
        "struct_stage::g18_f3_call_site_tests::f3_sites_while真首輪呼叫後break不得重放返回或第二輪",
    ),
    "F3-C10": (
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if提前return真move借用且跳過後續資源",
        "struct_stage::g18_f3_call_site_tests::f3_sites_選中if的被呼叫者溢位後未到達後續函式",
    ),
    "F3-C11": (
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main前置函式溢位不可被最後return常值掩蓋",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if借用首實參後第二實參溢位須回滾真票據與資源",
    ),
    "F3-C12": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main前置呼叫尾端常值計畫遭竄改必先拒絕",
        "struct_stage::g18_f3_call_site_tests::f3_sites_改寫已編譯主函式動態返回圖即使框架平衡仍先拒絕",
    ),
    "F3-C13": (
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_while僅准純值重讀_函式條件由原checker拒絕",
        "struct_stage::g18_f2_bool_condition_tests::f2_bool_cond_整數函式不能冒充bool條件由原checker拒絕",
    ),
    "F3-C14": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f1_f3_main結構欄位作為真函式實參依動態值求出",
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f1_f3_main結構欄位函式初始化賦值與返回有序執行",
    ),
    "F3-C15": (
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_false未選多層函式不造資源真else只借用一次",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main分支真str借用函式後返回常值應先歸還再釋放",
    ),
}


def main() -> int:
    arg = argparse.ArgumentParser(description=__doc__)
    arg.add_argument("--test-bin", type=Path, required=True)
    arg.add_argument("--check", action="store_true", required=True)
    opts = arg.parse_args()
    try:
        keys = {f"F3-C{i:02d}" for i in range(1, 16)}
        if set(CONTRACT) != keys or any(len(vals) != 2 for vals in CONTRACT.values()):
            raise ValueError("F3 必須十五類各兩個來源")
        all_names = [name for group in CONTRACT.values() for name in group]
        if len(set(all_names)) != 30:
            raise ValueError("F3 不可重複採計同一原始測試")
        binary = opts.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], capture_output=True,
                                 text=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("原 Rust --list 失敗：" + listing.stderr[-2200:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(all_names) - available)
        if missing:
            raise ValueError("F3 來源不存在：" + str(missing))
        for k, names in CONTRACT.items():
            for name in names:
                proc = subprocess.run([str(binary), "--exact", name, "--test-threads=1"],
                    capture_output=True, text=True, encoding="utf-8", check=False)
                output = proc.stdout + proc.stderr
                if proc.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                    raise ValueError(f"{k} 來源未真執行成功：{name}\n" + output[-2400:])
            print(f"F3 正式子契約 {k}：2/2 Checker→Runtime 正反成功")
        print("F3 正式義務 15/15、30/30 真來源通過；F4 與其餘原項仍需獨立驗收")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 F3 正式閉合失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
