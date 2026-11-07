#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1、P18.4 正式私有 Checker Copyable int 參數真框架的一百五十項 Rust 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_r03_真巢狀_if_葉提前返回保留雙層條件順序",
    "g18_r03_外層先更新_bool_後內層_if_須重讀最新值再寫入_int",
    "g18_r03_外層_false_路徑內的_else_巢狀_if_只執行選中的葉",
    "g18_r03_巢狀提前_return_只退出內函式不截斷父函式算術",
    "g18_r03_巢狀分支第二次_int_更新溢位真框架不補寫後續結果",
    "g18_r03_巢狀條件非法名稱與工作計畫遭竄改必先拒絕",
    "g18_r03_同一真_if_分支兩次_var_int_更新須照原_parser_先後返回",
    "g18_r03_else_if_選中分支多筆更新不可執行其他兄弟分支",
    "g18_r03_分支先更新_int_bool_後下個_if_必讀動態最新值",
    "g18_r03_分支內先變更再提前_return_不得執行不可達尾端",
    "g18_r03_同分支第二更新溢位只保存第一個真更新並失敗退出",
    "g18_r03_分支內常值錯誤賦值與真工作計畫竄改均須零額外執行",
    "g18_r03_if_已核准_int_可寫分支後仍執行真正尾端返回",
    "g18_r03_if_false_未選中分支不得寫入而仍由原值返回",
    "g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫",
    "g18_r03_if_變更_bool_使後續獨立_if_重新讀取真動態值",
    "g18_r03_if_分支呼叫溢位不允許後續真賦值或假返回",
    "g18_r03_if_分支不能更改_const_或偷換型別_原_checker_拒絕",
    "g18_r03_if_分支原_checker_賦值工作計畫篡改於執行前拒絕",
    "g18_r05_混合布林整數與巢狀提前返回動態事件帳簿必平衡",
    "g18_r05_動態整數溢位與真失敗退出仍保留合法平衡事件",
    "g18_r05_遞迴重入相同靜態宣告身分不應誤判為重複清理",
    "g18_r05_偽造未建立區域的離域與錯順序均遭事件帳簿拒絕",
    "g18_r05_虛構函式返回和錯誤退出標記不得通過一致性稽核",
    "g18_r05_布林與整數反向離域跨型別錯序不可冒充真清理",
    "g18_r05_純值事件帳簿不得將錯誤參數綁定或布林寫入當作合法框架",
    "g18_r04_已核准_bool_true_函式實參與_int_返回共用真框架",
    "g18_r04_已核准_bool_false_int_混合參數保持宣告順序和型別",
    "g18_r04_巢狀普通函式真_bool_參數不被誤讀成父整數槽",
    "g18_r04_main_變動具名_bool_正值先更新再真正當實參",
    "g18_r04_bool_混合參數進入後故障只退出本次真框架",
    "g18_r04_bool_參數不是_int_隱含轉型_原_checker_型別優先拒絕",
    "g18_r04_bool_參數綁定靜態型別被竄改前必拒絕",
    "g18_r03_具名_bool_true_提前_return_不可再執行未達返回路徑",
    "g18_r03_具名_bool_false_不走提前_return_而正常執行後段",
    "g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值",
    "g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回",
    "g18_r03_巢狀子函式提前返回只退出該函式不跳過父框架返回",
    "g18_r03_提前返回分支溢位只退出真的已初始化作用域",
    "g18_r03_未宣告_bool_及_const_bool_重綁由_checker_靜態拒絕",
    "g18_r03_篡改已編譯分支條件前須拒絕任何真框架",
    "g18_r01_r02_函式內多筆可寫純值賦值必依原_checker_語句次序",
    "g18_r01_r02_賦值右值真呼叫先完成後才寫入持有者",
    "g18_r01_r02_main_具名_copyable_int_多次更新後才進入普通函式",
    "g18_r01_r02_真巢狀函式可寫宣告不能跨動態框架共用",
    "g18_r01_r02_賦值右值溢位不得補記後續賦值或假返回",
    "g18_r01_r02_const_重賦值與錯型別必由真_checker_先拒絕",
    "g18_r01_r02_已編譯賦值的_checker_身分遭竄改在執行前拒絕",
    "g18_a_main_動態整數初始化應真執行後再傳入普通函式",
    "g18_a_main_函式結果可先存入區域再用純值算術返回",
    "g18_a_main_前向零參數結果可作為初始化再跨局部複製",
    "g18_c_main_動態初始化中後筆溢位不得跳過前筆真離域",
    "g18_a_main_真動態初始化計畫被篡改必在執行前拒絕",
    "g18_a_main_非法動態初始化型別及未宣告來源仍交由原_checker_拒絕",
    "g18_a_兩筆真區域初始化與逆序離域事件保持同源宣告身分",
    "g18_c_第二局部初始化運算溢位只釋放已完成宣告",
    "g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域",
    "g18_a_函式參數與多個純值區域初始化依_checker_順序真返回",
    "g18_a_巢狀函式各自區域宣告不與父框架參數混用",
    "g18_a_真動態_main_實參進入帶區域宣告函式仍唯一返回",
    "g18_c_函式區域算術溢位必退出而不得虛構返回",
    "g18_c_局部整數宣告跨多個遞迴框架不得共用儲存位置",
    "g18_a_函式區域宣告工作計畫遭偽造必在執行前拒絕",
    "g18_a_函式區域初始化錯誤型別與未知參照由原_checker_拒絕",
    "g18_a_main_普通函式返回值與常值相加由同一真動態工作堆疊完成",
    "g18_a_main_兩個兄弟函式合成返回必照原_checker_左右順序執行",
    "g18_a_main_多層括號包含函式複合返回保持原_parser_int_來源",
    "g18_c_main_子函式已真返回後最外層算術溢位不得冒充成功",
    "g18_a_main_複合返回工作計畫被竄改必在任何函式執行前拒絕",
    "g18_a_main_複合函式返回與非法型別或未知名稱仍由原_checker_拒絕",
    "g18_c_main_多兄弟複合返回不得假造單一根函式故障注入",
    "g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入",
    "g18_c_main_同名巢狀實參故障只能中斷真正外層函式框架",
    "g18_c_main_不同名子函式先成功後故障只注入根函式綁定",
    "g18_a_main_動態實參的來源框架不占用唯一使用者函式預算",
    "g18_a_main_巢狀實參先返回後可合法復用唯一框架額度",
    "g18_a_main_原_checker_算術實參於真動態框架依順序求值",
    "g18_a_main_具名_copyable_算術與巢狀普通函式依真來源求值",
    "g18_a_main_零參數葉函式作為複合實參須先真返回再綁定目標",
    "g18_c_main_第二算術實參溢位保留前一完成事件且不可進入目標",
    "g18_c_main_算術實參步數預算耗盡不會補寫未完成參數事件",
    "g18_c_main_動態實參綁定後故障須先退出真根框架",
    "g18_a_main_動態算式已驗證工作計畫遭竄改必在執行前拒絕",
    "g18_a_main_動態實參非法型別或未知名稱仍須由原_checker_拒絕",
    "g18_a_main_已核准_copyable_int_名稱初始化串接真傳參",
    "g18_a_main_括號名稱初始化與未使用副本不消耗來源",
    "g18_a_main_初始化來源錯型別或未宣告及尚未初始化仍靜態拒絕",
    "g18_a_main_被複製初始化持有者來源身分竄改必先拒絕",
    "g18_a_main_具名_copyable_int_多層括號初始化仍真傳入函式",
    "g18_a_main_未使用的合法_group_int_初始化不應阻擋零參數函式",
    "g18_a_main_括號初始化為布林或缺失名稱仍由原_checker_拒絕",
    "g18_a_main_括號初始化持有者遭竄改不得建立動態框架",
    "g18_a_main_括號返回帶參數普通函式依原_checker_真求值",
    "g18_a_main_多層括號零參數前向函式返回不虛構參數",
    "g18_a_main_括號返回的真內層呼叫跨度不得竄改",
    "g18_a_main_括號返回錯型別與不存在函式由原_checker_拒絕",
    "g18_a_數個未使用純值宣告與具名實參各自保留來源身分",
    "g18_a_未使用純值宣告遭竄改仍須執行前拒絕",
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
    "g18_a_多參數錯誤型別引數數量拒絕且未使用純值宣告合法",
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
