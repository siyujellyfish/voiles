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
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_已核准_int_可寫分支後仍執行真正尾端返回",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_false_未選中分支不得寫入而仍由原值返回",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_變更_bool_使後續獨立_if_重新讀取真動態值",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_混合布林整數與巢狀提前返回動態事件帳簿必平衡",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_已核准_bool_true_函式實參與_int_返回共用真框架",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_已核准_bool_false_int_混合參數保持宣告順序和型別",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_巢狀普通函式真_bool_參數不被誤讀成父整數槽",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_main_變動具名_bool_正值先更新再真正當實參",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_具名_bool_true_提前_return_不可再執行未達返回路徑",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_具名_bool_false_不走提前_return_而正常執行後段",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_巢狀子函式提前返回只退出該函式不跳過父框架返回",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_函式內多筆可寫純值賦值必依原_checker_語句次序",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_賦值右值真呼叫先完成後才寫入持有者",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_main_具名_copyable_int_多次更新後才進入普通函式",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_真巢狀函式可寫宣告不能跨動態框架共用",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_動態整數初始化應真執行後再傳入普通函式",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_函式結果可先存入區域再用純值算術返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_前向零參數結果可作為初始化再跨局部複製",
        "struct_stage::g18_private_scalar_call_tests::g18_a_兩筆真區域初始化與逆序離域事件保持同源宣告身分",
        "struct_stage::g18_private_scalar_call_tests::g18_a_函式參數與多個純值區域初始化依_checker_順序真返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_巢狀函式各自區域宣告不與父框架參數混用",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真動態_main_實參進入帶區域宣告函式仍唯一返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_普通函式返回值與常值相加由同一真動態工作堆疊完成",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_兩個兄弟函式合成返回必照原_checker_左右順序執行",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_多層括號包含函式複合返回保持原_parser_int_來源",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_巢狀實參先返回後可合法復用唯一框架額度",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_動態實參的來源框架不占用唯一使用者函式預算",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_原_checker_算術實參於真動態框架依順序求值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_具名_copyable_算術與巢狀普通函式依真來源求值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_零參數葉函式作為複合實參須先真返回再綁定目標",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_已核准_copyable_int_名稱初始化串接真傳參",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號名稱初始化與未使用副本不消耗來源",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_具名_copyable_int_多層括號初始化仍真傳入函式",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_未使用的合法_group_int_初始化不應阻擋零參數函式",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號返回帶參數普通函式依原_checker_真求值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_多層括號零參數前向函式返回不虛構參數",
        "struct_stage::g18_private_scalar_call_tests::g18_a_數個未使用純值宣告與具名實參各自保留來源身分",
        "struct_stage::g18_private_scalar_call_tests::g18_a_未使用純值宣告遭竄改仍須執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_多層括號具名_copyable_來源仍維持原宣告身分",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_原_checker_括號常值引數真正進入雙參數框架",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_零參數呼叫真正建立空參數框架並返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_零參數前向函式串接巢狀真返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_巢狀兄弟呼叫每個實參求值完成立即紀錄且不重複",
        "struct_stage::g18_private_scalar_call_tests::g18_a_零參數前向葉函式與_copyable_int_父框架真返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_零參數函式重複呼叫不重用動態框架且保留真求值順序",
        "execution::tests::p18_多層非遞迴函式框架按實際呼叫返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出",
        "struct_stage::g18_private_scalar_call_tests::g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_同一巢狀葉函式重複真呼叫各自返回且不共用參數槽",
        "struct_stage::g18_private_scalar_call_tests::g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回",
        "struct_stage::g18_private_scalar_call_tests::g18_c_父框架故障時不可進入尚未求值的巢狀子函式",
        "struct_stage::g18_private_scalar_call_tests::g18_c_巢狀函式來源或參數宣告身份被篡改零動態框架拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出",
        "struct_stage::g18_private_scalar_call_tests::g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_雙_copyable_int_參數依原_checker_宣告順序逐一綁定",
        "struct_stage::g18_private_scalar_call_tests::g18_a_三個_int_參數在同一真框架內逐項參與原_hir_算式",
        "struct_stage::g18_private_scalar_call_tests::g18_a_同一個_const_int_持有者作兩個實參屬_copyable_不消耗",
        "struct_stage::g18_private_scalar_call_tests::g18_a_多個具名_var_const_int_和常值實參按照原宣告身分傳遞",
        "struct_stage::g18_private_scalar_call_tests::g18_c_多參數整數溢位不可回傳成功且所有動態參數清空",
        "struct_stage::g18_private_scalar_call_tests::g18_c_多參數真框架測試故障不殘留參數也不假冒_return",
        "struct_stage::g18_private_scalar_call_tests::g18_a_擅改第二實參值或第二_parameter_declarationid_先驗後拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_多參數錯誤型別引數數量拒絕且未使用純值宣告合法",
        "struct_stage::g18_private_scalar_call_tests::g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位",
        "struct_stage::g18_private_scalar_call_tests::g18_a_合法前向六層以上_copyable_int_函式真逐層返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_五個_copyable_int_真參數不應被內部三參數上界拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出",
        "struct_stage::g18_private_scalar_call_tests::g18_c_零框架或零步數預算不得偽造任何真函式進入",
        "struct_stage::g18_private_scalar_call_tests::g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_c_有界步數的兩個巢狀兄弟呼叫僅在實際出現時建立獨立框架",
        "struct_stage::g18_private_scalar_call_tests::g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
        "struct_stage::g18_private_scalar_call_tests::g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算",
        "struct_stage::g18_private_scalar_call_tests::g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數",
        "execution::tests::p18_同一已驗證函式可重複呼叫且每次只執行實際路徑",
        "struct_stage::g18_private_call_borrow_tests::g18_d_唯一_str_借用與四個_copyable_int_同框架真算術返回",
        "struct_stage::g18_private_call_borrow_tests::g18_d_參數宣告順序為_int_str_int_int_仍以原_checker_身分真借用",
        "struct_stage::g18_private_call_borrow_tests::g18_d_同一具名_copyable_int_重複使用不消耗_str_借用持有者",
        "struct_stage::g18_private_call_borrow_tests::g18_c_多個_int_算術溢位須於真_str_borrowticket_歸還前中止成功返回",
        "struct_stage::g18_private_call_borrow_tests::g18_c_混合四個_int_與_str_中途故障不得殘留任一動態參數或票據",
        "struct_stage::g18_private_call_borrow_tests::g18_d_多個_copyable_int_後續參數被篡改不得建立任何_utf8_資源",
        "struct_stage::g18_private_call_borrow_tests::g18_d_多參數型別錯誤與未使用具名_int_保持原_checker_拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_const_str_參數共享借用真票據與返回值",
        "struct_stage::g18_private_call_borrow_tests::g18_d_錯誤實參型別及缺少參數在正式_checker_先拒絕",
    ),
    "P18.2": (
        "execution::tests::p18_真控制邊紀錄只包含實際檢查的條件與返回",
        "struct_stage::g18_private_source_loop_tests::g18_b_六十五輪合法半開_for_不再遭私有固定上界拒絕",
        "struct_stage::g18_private_source_loop_tests::g18_b_超過六十四輪的合法_for_依真實步數預算成功或受控耗盡",
        "struct_stage::g18_private_source_loop_tests::g18_b_巨大合法_i64_for_上界不應在_prepare_被拒絕或試圖全量迭代",
        "struct_stage::g18_private_source_loop_tests::g18_b_空步數預算必須在任一真_utf8_建立前拒絕",
        "struct_stage::g18_private_source_loop_tests::g18_c_巢狀_for_執行預算耗盡必清理全部已走到的迭代資源",
        "struct_stage::g18_private_source_loop_tests::g18_b_while_條件零迭代仍依真檢查消耗預算而非靜態略過",
        "struct_stage::g18_private_source_loop_tests::g18_b_具名_while_無須靜態終止改由執行期預算安全中止",
        "struct_stage::g18_private_source_loop_tests::g18_b_具名_while_continue_每輪先清理真資源再回到條件受控耗盡",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_for_三次每迭代重新建立唯一_utf8_並離域釋放",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_continue_每次先離域釋放再開始下一次",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_while_false_不執行未走過的字串建構",
        "struct_stage::g18_private_source_loop_tests::g18_b_真_break_只執行首輪且離域釋放",
    ),
    "P18.3": (
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_分支呼叫溢位不允許後續真賦值或假返回",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真直接遞迴_utf8_借用與返回框架交付前全數一致",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真互相遞迴多_resourceid_跨四框架逆序退出必一致",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真資源遞迴預算耗盡與局部建立故障均須完整退出",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真零預算尚未進入主框架不允許假資源事件",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態資源返回後偽造退出名字與成功標記必拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真資源釋放或票據空集合遭竄改必拒絕交付",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態框架重複進入與漏退出不得因資源表平衡而放行",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_動態整數溢位與真失敗退出仍保留合法平衡事件",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_遞迴重入相同靜態宣告身分不應誤判為重複清理",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_偽造未建立區域的離域與錯順序均遭事件帳簿拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_虛構函式返回和錯誤退出標記不得通過一致性稽核",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_bool_混合參數進入後故障只退出本次真框架",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_提前返回分支溢位只退出真的已初始化作用域",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_賦值右值溢位不得補記後續賦值或假返回",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_動態初始化中後筆溢位不得跳過前筆真離域",
        "struct_stage::g18_private_scalar_call_tests::g18_c_第二局部初始化運算溢位只釋放已完成宣告",
        "struct_stage::g18_private_scalar_call_tests::g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域",
        "struct_stage::g18_private_scalar_call_tests::g18_c_函式區域算術溢位必退出而不得虛構返回",
        "struct_stage::g18_private_scalar_call_tests::g18_c_局部整數宣告跨多個遞迴框架不得共用儲存位置",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_子函式已真返回後最外層算術溢位不得冒充成功",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_多兄弟複合返回不得假造單一根函式故障注入",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_同名巢狀實參故障只能中斷真正外層函式框架",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_不同名子函式先成功後故障只注入根函式綁定",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_第二算術實參溢位保留前一完成事件且不可進入目標",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_算術實參步數預算耗盡不會補寫未完成參數事件",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_動態實參綁定後故障須先退出真根框架",
        "struct_stage::g18_private_scalar_call_tests::g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出",
        "struct_stage::g18_private_scalar_call_tests::g18_c_零參數自遞迴由真框架預算受控中止不虛構返回",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_獨占_var_str_借用與另一獨立來源整筆_move_可同次合法存在",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合借用與_move_在每個原引數及函式內故障點仍清兩資源",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合轉交函式未再移動內部參數仍由被呼叫者離域釋放",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_多函式逐層_move_同一_resourceid_末端真正釋放且返回逐框架退出",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_合法自我遞迴每層轉交唯一_utf8_受框架預算安全耗盡",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_跨函式_move_步數預算耗盡不得假成功或雙重釋放",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_真移動之前及連續轉交之後注入故障都由目前所有者清理一次",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_零函式框架或零步數預算在配置真_utf8_前受控拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位",
        "struct_stage::g18_private_scalar_call_tests::g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出",
        "struct_stage::g18_private_scalar_call_tests::g18_c_零框架或零步數預算不得偽造任何真函式進入",
        "struct_stage::g18_private_scalar_call_tests::g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕",
        "execution::tests::p18_無限遞迴耗盡預算安全拒絕_不爆堆疊",
        "execution::tests::p18_已核准直接遞迴_可由全域_bool_終止並逐框架返回",
        "execution::tests::p18_已核准互相遞迴_使用原函式宣告與真動態框架",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個獨立_var_str_加雙_int_跨遞迴真票據全序列及逆序清理",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同函式兩個_const_str_共享借用同一來源須用兩張真票據歸還",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個_str_與_int_不同比例順序之互相遞迴仍真借用並返回",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據存活時框架預算耗盡仍全部原子歸還",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據已簽發後動態故障不留任何資源",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩張借用中第二張_source_或_kind_被竄改必在配置前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同個可寫_str_來源同次傳兩個獨占參數須原_checker_拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_雙_str_呼叫漏參數及錯誤型別仍由原_checker_拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴兩個_copyable_int_互換父子參數槽並保護真_str_票據",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_單張_str_borrowticket_和三個_int_跨遞迴位置交換實際返回",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_相同_copyable_int_父實參可給兩個子參數但不可複製真_str",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_五個互相呼叫函式與六層混合資源_不得被舊兩函式上限拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_多函式混合資源框架預算耗盡須逆序釋放每個已配置_resourceid",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合多個_int_底層溢位不得略過真借用歸還及_utf8_釋放",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴第二_int_及多函式來源篡改必在建立資源前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合多_int_錯誤參數型別與個數須由原_checker_拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴_str_真票據與_copyable_int_逐框架傳遞並由_hir_算術返回",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_互相遞迴_參數順序不同仍保留原_checker_int_與_str_借用身份",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_無_str_實參的_bool_int_遞迴仍真建立每框架_utf8_並返回動態參數",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_子遞迴_int_常值覆寫父框架值_底層不得錯讀父參數",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合_int_底層算術溢位須失敗並先歸還借用票據再逐層釋放",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合_str_int_跨框架呼叫預算耗盡仍保留實際參數值與全部清理",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴第二層_int_已綁定後故障仍清理所有_utf8_與票據",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_竄改遞迴_int_實參來源或返回_hir_必須在真資源建立前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴錯誤_int_型別及漏參數必先由原_checker_拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_深層重入同一宣告來源須各有動態_borrow_tracker_且受控耗盡",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_跨框架相同原宣告位置的唯讀和獨占票據不能誤互斥",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_深層重入步數耗盡各框架票據與所有者全部逆序歸還",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_重入框架內故障不以共用靜態宣告身份錯誤解鎖",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴真_str_參數兩層_borrowticket_先歸還再清理_utf8",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_互相遞迴每層真_str_借用票據不重覆也不洩漏",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_跨框架借用存活時預算耗盡須先還真票據再釋放來源",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_第二層借用來源完成配置後注入錯誤仍歸還全部票據",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_竄改已核准呼叫_borrowfact_來源與_kind_在配置前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_c_原_checker_缺失呼叫借用票據或非法_str_來源不可真執行",
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
        "struct_stage::g18_private_scalar_call_tests::g18_r04_bool_參數不是_int_隱含轉型_原_checker_型別優先拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一真函式先_move_後共享借用_兩資源身分分離且各釋放一次",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一真函式先_shared_borrow_後_move_不交換原_parser_求值次序",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_獨占_var_str_借用與另一獨立來源整筆_move_可同次合法存在",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合借用與_move_在每個原引數及函式內故障點仍清兩資源",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合轉交函式未再移動內部參數仍由被呼叫者離域釋放",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一來源先借用再移動或先移動再借用必由原_checker_拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_借用別名不能冒充所有者移動_錯型別及漏參數仍拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_竄改借用_kind_或_move_來源參數身份都必須真資源建立前拒絕",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_原_checker_明確_move_普通函式實參轉交同一真_utf8_不複製",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_被呼叫者真的再次_move_參數至局部唯一所有者退出才釋放一次",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_移動後原來源再次使用與漏掉明確_move_必由原_checker_拒絕",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_非法借用別名不能以_move_變成真正所有者",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_更改已核准轉交函式或字串原值必須零執行前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個獨立_var_str_加雙_int_跨遞迴真票據全序列及逆序清理",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同函式兩個_const_str_共享借用同一來源須用兩張真票據歸還",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個_str_與_int_不同比例順序之互相遞迴仍真借用並返回",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩張借用中第二張_source_或_kind_被竄改必在配置前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同個可寫_str_來源同次傳兩個獨占參數須原_checker_拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_d_雙_str_呼叫漏參數及錯誤型別仍由原_checker_拒絕",

        "struct_stage::g18_private_call_borrow_tests::g18_d_具名_const_int_與_str_借用同框架按原_checker_引用身份返回",
        "struct_stage::g18_private_call_borrow_tests::g18_d_具名_var_int_在前_var_str_在後_真獨占借用及複製求值",
        "struct_stage::g18_private_call_borrow_tests::g18_c_具名_copyable_與_str_借用並存故障仍清空參數及歸還票據",
        "struct_stage::g18_private_call_borrow_tests::g18_d_偽造具名_copyable_所有者_decl_id_或初始值須零資源拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_具名_int_未初始化或實參型別不符由原_checker_先拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_混合_const_str_與_copyable_int_同框架真借用及_hir_加法返回",
        "struct_stage::g18_private_call_borrow_tests::g18_d_混合_int_在前_var_str_在後_仍以真獨占票據與參數身分執行",
        "struct_stage::g18_private_call_borrow_tests::g18_d_混合函式重複呼叫不消耗_str_持有者也不重用上次_int_參數框架",
        "struct_stage::g18_private_call_borrow_tests::g18_c_混合借用期間_int_加法溢位須歸還票據及真_utf8_資源",
        "struct_stage::g18_private_call_borrow_tests::g18_c_混合函式真借用與_int_參數已綁定後故障仍原子歸還清理",
        "struct_stage::g18_private_call_borrow_tests::g18_d_篡改混合函式_int_值與_DeclarationId_或_hir_返回必須配置前拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_混合函式刪除原_checker_真借用表不得使用舊參數票據",
        "struct_stage::g18_private_call_borrow_tests::g18_d_混合呼叫錯誤型別個數與多餘未執行宣告必保守拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id",
        "struct_stage::g18_private_scalar_call_tests::g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者",
        "struct_stage::g18_private_call_borrow_tests::g18_d_來源借用別名再作函式實參須由_checker_保守拒絕",
        "struct_stage::g18_private_call_borrow_tests::g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據",
    ),
    "P18.5": (
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_分支不能更改_const_或偷換型別_原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_if_分支原_checker_賦值工作計畫篡改於執行前拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態資源返回後偽造退出名字與成功標記必拒絕",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真資源釋放或票據空集合遭竄改必拒絕交付",
        "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態框架重複進入與漏退出不得因資源表平衡而放行",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_布林與整數反向離域跨型別錯序不可冒充真清理",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_純值事件帳簿不得將錯誤參數綁定或布林寫入當作合法框架",
        "struct_stage::g18_private_scalar_call_tests::g18_r04_bool_參數綁定靜態型別被竄改前必拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_未宣告_bool_及_const_bool_重綁由_checker_靜態拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_篡改已編譯分支條件前須拒絕任何真框架",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_const_重賦值與錯型別必由真_checker_先拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_r02_已編譯賦值的_checker_身分遭竄改在執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_真動態初始化計畫被篡改必在執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_非法動態初始化型別及未宣告來源仍交由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_函式區域宣告工作計畫遭偽造必在執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_函式區域初始化錯誤型別與未知參照由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_複合返回工作計畫被竄改必在任何函式執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_複合函式返回與非法型別或未知名稱仍由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_動態算式已驗證工作計畫遭竄改必在執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_動態實參非法型別或未知名稱仍須由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_初始化來源錯型別或未宣告及尚未初始化仍靜態拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_被複製初始化持有者來源身分竄改必先拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號初始化為布林或缺失名稱仍由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號初始化持有者遭竄改不得建立動態框架",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號返回的真內層呼叫跨度不得竄改",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號返回錯型別與不存在函式由原_checker_拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_括號布林與未知名稱不可充當合法_int_實參",
        "struct_stage::g18_private_scalar_call_tests::g18_a_main_零參數來源被偽造一個參數必執行前拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_c_第一實參完成後第二實參溢位仍保留真事件而不進入父呼叫",
        "struct_stage::g18_private_scalar_call_tests::g18_a_零參數子函式原宣告來源遭竄改必在動態執行前拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合借用與_move_在每個原引數及函式內故障點仍清兩資源",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合轉交函式未再移動內部參數仍由被呼叫者離域釋放",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一來源先借用再移動或先移動再借用必由原_checker_拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_借用別名不能冒充所有者移動_錯型別及漏參數仍拒絕",
        "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_竄改借用_kind_或_move_來源參數身份都必須真資源建立前拒絕",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_跨函式_move_步數預算耗盡不得假成功或雙重釋放",
        "struct_stage::g18_private_owned_move_call_tests::g18_c_真移動之前及連續轉交之後注入故障都由目前所有者清理一次",
        "struct_stage::g18_private_owned_move_call_tests::g18_d_非法借用別名不能以_move_變成真正所有者",
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
    "P18.1": ("SubsetVerified", "原 Checker Copyable int 已接通多參數及非固定層數函式圖、重複葉函式、遞迴顯式框架／可配置預算；一般控制語句與其他型別混合呼叫圖仍需完整驗收"),
    "P18.2": ("SubsetVerified", "真 if／else-if、原 Checker 合法半開 for 與已驗證 bool while 現依可配置步數預算多輪重讀、continue／break 真清理；複雜動態控制及全矩陣仍待驗收"),
    "P18.3": ("Incomplete", "已接通多資源遞迴真 BorrowTicket、str.move 跨函式與直接遞迴唯一 ResourceId，另同 Call move＋借用在各階段故障逆序清理；任意控制流／複合資源參數仍待完整正反"),
    "P18.4": ("SubsetVerified", "原 Checker 多 str BorrowTicket、明確 str.move 及同一 Call 對獨立 str 來源的 move＋共享／獨占借用皆可真執行與原子清理；多個 move、逃逸及 self 完整矩陣仍缺"),
    "P18.5": ("SubsetVerified", "已有控制、資源與故障真回歸，但尚非原全矩陣結案"),
    "P18.6": ("BlockedByDecision", "D18-02-A～D 原則已核准，同步共享／獨占 self 真票據與靜態唯一零參數及單 int 常值實參多載、真參數直接返回與加減已可執行；多參數／複雜算式及 async 調度取消細則、正向執行仍待裁決及驗收"),
}

真執行群組 = (
    "execution::tests::p18_",
    "struct_stage::p17_private_str_execution_tests::p18_",
    "struct_stage::g18_private_source_loop_tests::g18_",
    "struct_stage::g18_private_call_borrow_tests::g18_",
    "struct_stage::g18_private_owned_move_call_tests::g18_",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_",
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
