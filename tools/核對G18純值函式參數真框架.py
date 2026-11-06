#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1、P18.4 正式私有 Checker Copyable int 參數真框架的五十二項 Rust 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_a_main_原_checker_括號常值引數真正進入雙參數框架",
    "g18_a_main_多層括號具名_copyable_來源仍維持原宣告身分",
    "g18_a_main_括號布林與未知名稱不可充當合法_int_實參",
    "g18_a_main_零參數呼叫真正建立空參數框架並返回",
    "g18_a_main_零參數前向函式串接巢狀真返回",
    "g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出",
    "g18_a_main_零參數來源被偽造一個參數必執行前拒絕",
    "g18_a_巢狀兄弟呼叫每個實參求值完成立即紀錄且不重複",
    "g18_c_第一實參完成後第二實參溢位仍保留真事件而不進入父呼叫",
    "g18_a_零參數前向葉函式與_copyable_int_父框架真返回",
    "g18_a_零參數函式重複呼叫不重用動態框架且保留真求值順序",
    "g18_c_零參數自遞迴由真框架預算受控中止不虛構返回",
    "g18_a_零參數子函式原宣告來源遭竄改必在動態執行前拒絕",
    "g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位",
    "g18_a_合法前向六層以上_copyable_int_函式真逐層返回",
    "g18_a_五個_copyable_int_真參數不應被內部三參數上界拒絕",
    "g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出",
    "g18_c_零框架或零步數預算不得偽造任何真函式進入",
    "g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕",
    "g18_c_有界步數的兩個巢狀兄弟呼叫僅在實際出現時建立獨立框架",
    "g18_a_雙_copyable_int_參數依原_checker_宣告順序逐一綁定",
    "g18_a_三個_int_參數在同一真框架內逐項參與原_hir_算式",
    "g18_a_同一個_const_int_持有者作兩個實參屬_copyable_不消耗",
    "g18_a_多個具名_var_const_int_和常值實參按照原宣告身分傳遞",
    "g18_c_多參數整數溢位不可回傳成功且所有動態參數清空",
    "g18_c_多參數真框架測試故障不殘留參數也不假冒_return",
    "g18_a_擅改第二實參值或第二_parameter_declarationid_先驗後拒絕",
    "g18_a_多參數錯誤型別缺少引數與多餘未執行語句一律拒絕",
    "g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出",
    "g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值",
    "g18_a_同一巢狀葉函式重複真呼叫各自返回且不共用參數槽",
    "g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回",
    "g18_c_父框架故障時不可進入尚未求值的巢狀子函式",
    "g18_c_巢狀函式來源或參數宣告身份被篡改零動態框架拒絕",
    "g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出",
    "g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕",
    "g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序",
    "g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
    "g18_a_真函式_copyable_int_參數多次引用每次都取同一實際值",
    "g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算",
    "g18_a_具名_copyable_int_實參傳入函式後參與真減法",
    "g18_c_函式參數真加法溢位不能假造_return_且確實離域",
    "g18_c_函式參數真減法溢位亦不可留下框架或偽造成功事件",
    "g18_a_竄改編譯後_true_arithmetic_運算樹必在執行前拒絕",
    "g18_a_非法_bool_參與_int_加法與字面值不足均由原_checker_拒絕",
    "g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數",
    "g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者",
    "g18_c_真正參數框架中途故障必清空且不冒充成功返回",
    "g18_a_錯誤_copyable_int_實參型別及引數個數由原_checker_拒絕",
    "g18_a_具名未初始化_int_無法假裝有函式實參值",
    "g18_a_竄改已驗證參數_declarationid_和返回值必須零執行拒絕",
    "g18_a_竄改主函式呼叫跨度或原來源位元組不得建立任何參數框架",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if result.returncode:
        print("G18 純值函式框架無法取得 Rust 測試清單：" + result.stderr,
              file=sys.stderr)
        return 1
    present = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::g18_private_scalar_call_tests::" + name for name in 案例
    }
    missing = sorted(required - present)
    if missing:
        print("G18 Copyable int 函式真來源正反缺漏：" + "、".join(missing),
              file=sys.stderr)
        return 1
    # 不僅核對清單；多參數必須真正經原 Checker -> runtime 重跑。
    executed = subprocess.run(
        [str(args.test_bin.resolve()), "--test-threads=2",
         "struct_stage::g18_private_scalar_call_tests::"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    expected = f"{len(required)} passed; 0 failed;"
    if executed.returncode or expected not in executed.stdout:
        print("G18 Copyable int 真執行 Gate 未全過：" +
              executed.stdout + executed.stderr, file=sys.stderr)
        return 1
    print(f"G18 真 Copyable int 已核准一般數量 Copyable int 與遞迴真工作堆疊框架：{len(required)}/{len(required)}"
          " 真實重跑；一般函式完整矩陣仍待 P18 正式驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
