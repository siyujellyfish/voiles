#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B5：原 Checker／真 BorrowSafety 有效期、無證再借、逃逸 42 項真來源正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::const_來源兩個共享別名可以同時讀取且歸還後使用原來源",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::不同持有者可各自保有獨占借用而不互相衝突",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::同一持有者的兩個獨占借用不得重疊",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::有效獨占借用期間不得直接讀取原來源",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::const_來源不能冒充可變來源建立獨占別名",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::已移動資源不允許借用或憑別名建立第二所有者",
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::已存在借用別名不得在缺乏父子證明時再次借用",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::const_共享別名透過巢狀函式與唯讀方法可取得獨立值",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::巢狀函式不得在共享別名仍使用時直接操作原持有者",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::var_獨占別名可以呼叫寫入方法並於最後使用後歸還",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::const_共享別名不能呼叫會寫入_self_的方法",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::var_借用期間不得以嵌套方法直接再次使用原持有者",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::非擁有別名永遠不能藉_move_成為完整資源持有者",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::copyable_借用值轉成獨立回傳值仍安全合法",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::非_copyable_來源的借用別名不得跨函式回傳",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::借用運算式不可直接當函式回傳值",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::借用別名不可透過_move_偷換成跨作用域所有者",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::巢狀區塊局部資源借用不得存入外層擁有者",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::活躍借用跨移動原來源必須拒絕",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::巢狀區塊純值回傳不應誤判為借用逃逸",
    "struct_stage::p15_5_懸空與合法重新借用來源::前一筆借用最後使用後可重新借用同一原持有者",
    "struct_stage::p15_5_懸空與合法重新借用來源::第二筆借用不可與仍有未來使用的獨占借用重疊",
    "struct_stage::p15_5_懸空與合法重新借用來源::局部借用不能逃逸作用域成為懸空名稱",
    "struct_stage::p15_5_懸空與合法重新借用來源::未來仍會使用的借用不得允許原持有者先移動",
    "struct_stage::p15_5_懸空與合法重新借用來源::借用別名再次借用仍保持_vb14_階段性拒絕",
    "struct_stage::p15_5_懸空與合法重新借用來源::巢狀區塊的借用歸還不應污染外層再次借用",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::一般借用不必等函式區塊出口即可歸還",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::self_借用於唯讀方法返回後才歸還並允許接收者整筆移動",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::self_方法不可在仍活躍的原來源借用期間偷拿新接收者",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::函式接受已核准_copyable_借用值的獨立複製不造成參數逃逸",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::尚未建立_await_執行期證明前不得放行有效別名跨越語法邊界",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::尚未建立_例外_歸還證明前不得將錯誤邊界當成合法來源",
    "borrow_safety::r03_f06_b05_別名身分獨立證據::f06_b05_不同別名共享同來源保留重數並對帳真票據",
    "borrow_safety::r03_f06_b05_別名身分獨立證據::f06_b05_重複靜態別名不允許冒充兩張合法共享票據",
    "borrow_safety::r03_f06_b05_別名身分獨立證據::f06_b05_失效別名不被算作活躍借用或虛構票據",
    "borrow_safety::r03_f06_b05_別名身分獨立證據::f06_b05_遮蔽別名與參數自身借用使用真宣告身分",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::一般別名最後使用點可單獨歸還而方法_self_須等作用域結束",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::方法作用域退出失敗不可提前解鎖接收者",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::g18_d_真共享票據只准借用唯讀而不授權原持有者存取",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::g18_d_獨占票據確實能授權借用讀寫但離域後失效",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::g18_d_不同追蹤器票據不能作另一次函式的參數存取能力",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::g18_d_多個獨立共享票據其中一個歸還後不得憑過期序號讀取",
)
EXACT_MODULE_PREFIXES = (
    "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::",
    "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::",
    "struct_stage::p15_2_借用回傳與逃逸正式來源::",
    "struct_stage::p15_5_懸空與合法重新借用來源::",
    "struct_stage::p15_6_最後使用點與_self_退出邊界來源::",
    "borrow_safety::r03_f06_b05_別名身分獨立證據::",
    "borrow_safety::p15_6_一般最後使用點與方法作用域歸還區分::",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1800:])
        available = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.endswith(": test")
        }
        expected = set(CASES)
        if len(expected) != 42:
            raise ValueError("B5 正反來源缺少或重複，不能重複採計")
        missing = sorted(expected - available)
        if missing:
            raise ValueError("B5 真 Rust 測試遺失：" + str(missing))
        for prefix in EXACT_MODULE_PREFIXES:
            actual = {name for name in available if name.startswith(prefix)}
            planned = {name for name in expected if name.startswith(prefix)}
            if actual != planned:
                raise ValueError(
                    f"B5 來源模組 {prefix} 漂移：缺 {sorted(planned-actual)}；"
                    f"新增未登錄 {sorted(actual-planned)}"
                )
        for index, case in enumerate(CASES, 1):
            executed = subprocess.run(
                [str(binary), "--exact", case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = executed.stdout + executed.stderr
            if executed.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B5 第 {index}/42 筆原真來源未通過：{case}\n" + output[-3200:]
                )
            print(f"B5 借用權限正反 {index}/42：{case}（真通過）")
        print("G18 P18.4-B5 42 項 Checker／BorrowSafety 正反驗收通過；B4／P18.4 仍不得提前 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print(f"G18 B5 安全 Gate 失敗：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
