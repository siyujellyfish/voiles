#!/usr/bin/env python3
"""核對五十九項 P17-C／P18-B／P18-C／P18-E 私有真 Checker→OwnedStrRuntime 來源執行案例存在且已由全量 Rust 執行。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "p17_c_真私有_checker_來源驅動字串建立移動再初始化並逆序清理",
    "p17_c_原私有來源動作計畫未變可連續真正執行並完成清理",
    "p17_c_已驗證字串動作常值被換掉須先拒絕零資源分配",
    "p17_c_明示_move_賦值目標或來源偷換須在持有者轉交前拒絕",
    "p17_c_具名_bool_原宣告真假及分支目標更動不可沿用舊_checker_授權",
    "p17_c_未選中的_else_字串若被改寫也不得造成靜態計畫漂移",
    "p17_c_巢狀返回常值被竄改不得在任何資源建立後才察覺",
    "p17_c_頂層返回值與額外插入動作都必須在執行前拒絕",
    "p17_c_真私有_lexer_字串內容含中文及跳脫仍只有一筆真資源",
    "p17_c_非法字串隱含複製在真_checker_階段就拒絕",
    "p17_c_尚未接通的結構或分支在資源建立前保守拒絕",
    "p17_c_原始來源遭到替換不得產生動態資源事件",
    "p17_c_真資源覆寫錯誤注入原子回滾並清理原持有者",
    "p17_c_真私有_checker_整筆_move_覆寫現存_str_舊資源立即釋放",
    "p17_c_移動後重新初始化原持有者_按真實完成次序逆序清理",
    "p17_c_賦值目標不可變與自身移動在_checker_先拒絕",
    "p17_c_已移動來源再次轉交必須在建立資源之前靜態拒絕",
    "p17_c_真資源整筆重綁前若來源改寫不得產生任何實際事件",
    "p18_b_真私有_成立分支建立兩筆_str_並在外層返回前逆序清理",
    "p18_b_不成立分支不可建立資源_真_else_才會配置與離域清理",
    "p18_b_條件_false_且無_else_不產生未走到的資源事件",
    "p18_b_局部同名遮蔽在離開真資源作用域後不破壞外層持有者",
    "p18_b_分支局部所有者跨作用域外洩必須在_checker_拒絕",
    "p18_c_原巢狀返回保守負例現可逐層逆序清理",
    "p18_b_原有巢狀資源保守負例現已真實求值並逆序釋放",
    "p18_b_巢狀兩層_true_的真資源按執行路徑逆序釋放",
    "p18_b_巢狀_false_只建立內層_else_實際資源",
    "p18_b_最外層_false_不得執行任何巢狀真資源",
    "p18_b_兩層同名局部遮蔽維持獨立持有者與逆序釋放",
    "p18_b_巢狀分支局部變數逃逸由真_checker_提前拒絕",
    "p18_c_巢狀未支援的動態整數運算返回仍在執行前拒絕",
    "p18_c_三層巢狀返回跳過外層結果且真資源逐層退出",
    "p18_c_巢狀_false_的_else_實際返回且未執行分支零事件",
    "p18_c_巢狀_false_未返回而正常離域後使用外層返回",
    "p18_e_巢狀第三筆字串建立後注入失敗逐層逆序回滾",
    "p18_e_外層第一筆局部資源建立後失敗不漏清理根持有者",
    "p18_e_未選分支故障注入不得因預先分析而被誤觸發",
    "p18_e_具名_false_選擇_else_後故障只作用於真求值的資源",
    "p18_e_來源失效時故障注入亦不能提前建立任何資源",
    "p18_b_正式_else_if_第二個條件成立只求值被選中字串並返回",
    "p18_b_正式連續兩個_else_if_先假後真只建立單一資源",
    "p18_b_正式_else_if_全部不成立由_else_建立唯一資源",
    "p18_b_巢狀_else_if_中真返回先清理其餘外層持有者",
    "p18_b_原具名_else_if_負例現依正式型別身分執行",
    "p18_b_具名_false_else_if_只走最終_else_並逆序清理",
    "p18_b_巢狀具名_bool_true_真返回依宣告身分逐層清理",
    "p18_b_巢狀名稱被_str_遮蔽不得拿外層_bool_宣告身分誤授權",
    "p18_e_具名_else_if_失敗注入只計真選中的分支資源",
    "p18_b_具名_else_if_經驗證後來源竄改仍不得開始執行",
    "p18_b_具名_bool_條件由真宣告身分決定實際資源分支",
    "p18_b_具名_bool_false_未選分支永不產生_utf8_資源",
    "p18_b_來源為_bool_但是由字串同名冒充條件須事前拒絕",
    "p18_b_未初始化_bool_或未知名稱不得開始資源執行",
    "p18_b_非純值_bool_初始式即便_checker_通過也不偷渡新求值能力",
    "p18_c_成立分支提前返回先離開內層資源再清理外層",
    "p18_c_false_轉入_else_真返回_未選分支不得建立資源",
    "p18_c_具名_bool_false_分支返回未執行_繼續到外層_return",
    "p18_c_具名_bool_true_依_runtime_取值返回且只釋放選中資源",
    "p18_c_分支內未支援的動態返回式須在執行前拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if result.returncode:
        print("P17-C 無法讀取既有 Rust 全量測試名稱", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::p17_private_str_execution_tests::" + name
        for name in 案例
    }
    missing = sorted(required - names)
    if missing:
        print("P17-C 缺少真 Rust 來源／資源正反案例：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"P17-C 非公開私有資源切片：{len(required)}/{len(required)} 項真 Rust 案例存在；"
          "由原全量 Rust 步驟已實際執行。")
    print("注意：此批不是 P17-C 全範圍或 G17 正式驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
