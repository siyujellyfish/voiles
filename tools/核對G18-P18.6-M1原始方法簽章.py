#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M1 唯一方法簽章原始義務：9 靜態 S20＋18 真 Runtime，共 27 條嚴格逐項正反驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

STATIC_PREFIX = "struct_stage::g18_private_method_overload_tests::"
DYNAMIC_PREFIX = "struct_stage::g18_private_method_self_tests::"
CASES = (
    "struct_stage::g18_private_method_overload_tests::g18_f_原_parser_保留同名不同參數簽章全部方法候選順序",
    "struct_stage::g18_private_method_overload_tests::g18_f_同名_int_多載只用完整靜態參數型別取得真共享借用",
    "struct_stage::g18_private_method_overload_tests::g18_f_同名_bool_多載從原_checker_選獨占_self_不得誤選唯讀",
    "struct_stage::g18_private_method_overload_tests::g18_f_同名多載方法宣告顛倒不影響靜態唯一選擇或借用權限",
    "struct_stage::g18_private_method_overload_tests::g18_f_同名多載不可憑返回型別或參數名稱視為不同簽章",
    "struct_stage::g18_private_method_overload_tests::g18_f_const_結構僅允許與參數型別匹配的共享唯讀多載",
    "struct_stage::g18_private_method_overload_tests::g18_f_同名多載參數數量或型別無匹配必靜態拒絕",
    "struct_stage::g18_private_method_overload_tests::g18_f_欄位方法同名無論宣告順序仍由原_parser_拒絕",
    "struct_stage::g18_private_method_overload_tests::g18_f_原同名唯一方法索引與正常非多載不因新候選表退化",
    "struct_stage::g18_private_method_self_tests::g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選",
    "struct_stage::g18_private_method_self_tests::g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行",
    "struct_stage::g18_private_method_self_tests::g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選",
    "struct_stage::g18_private_method_self_tests::g18_f_零參數多載方法中途故障仍先歸還原_self_票據",
    "struct_stage::g18_private_method_self_tests::g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕",
    "struct_stage::g18_private_method_self_tests::g18_f_同名方法單_int_實參依原_checker_型別選真共享多載",
    "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參的多載宣告順序不影響真方法選擇",
    "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活",
    "struct_stage::g18_private_method_self_tests::g18_f_雙_int_方法真依原多載簽章選取並計算兩個獨立參數",
    "struct_stage::g18_private_method_self_tests::g18_f_雙_int_方法可只返回第二參數而不錯讀第一或_self_",
    "struct_stage::g18_private_method_self_tests::g18_f_雙_int_方法依實參順序正確執行減法",
    "struct_stage::g18_private_method_self_tests::g18_f_雙_int_方法加法溢位仍歸還唯一真_self_票據",
    "struct_stage::g18_private_method_self_tests::g18_f_多_int_實參方法故障不殘留形參且重新呼叫不受污染",
    "struct_stage::g18_private_method_self_tests::g18_f_三個_int_方法參數按原宣告位置獨立綁定返回第三個",
    "struct_stage::g18_private_method_self_tests::g18_f_多_int_方法來源錯型別與不存在參數須原_checker_或安全準備拒絕",
    "struct_stage::g18_private_method_self_tests::g18_f_雙_int_方法只讀第二參數與_self_欄位不混用第一個",
    "struct_stage::g18_private_method_self_tests::g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據",
    "struct_stage::g18_private_method_self_tests::g18_f_竄改第二方法實參值或宣告身分均於真資源前拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 無法列出原 Checker 真來源：" + listed.stderr[-1600:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.endswith(": test")
        }
        expected = set(CASES)
        if len(expected) != 27:
            raise ValueError("M1 測試重名或不滿 27，不能重複採計")
        static_list = {name for name in actual if name.startswith(STATIC_PREFIX)}
        expected_static = {name for name in expected if name.startswith(STATIC_PREFIX)}
        if len(expected_static) != 9 or static_list != expected_static:
            raise ValueError(
                "M1 靜態 S20 來源完整性不符：缺 "
                + str(sorted(expected_static-static_list))
                + "；多 " + str(sorted(static_list-expected_static))
            )
        if len({name for name in expected if name.startswith(DYNAMIC_PREFIX)}) != 18:
            raise ValueError("M1 原同步方法動態來源應為 18 筆")
        if expected - actual:
            raise ValueError("M1 原 Checker/真 Runtime Rust 測試不存在："
                + str(sorted(expected-actual)))
        for index, case in enumerate(CASES, 1):
            executed = subprocess.run(
                [str(binary), "--exact", case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = executed.stdout + executed.stderr
            if executed.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M1 第 {index}/27 條原簽章／真方法失敗：{case}\n" + output[-3000:]
                )
            print(f"M1 唯一簽章／安全方法 {index}/27：{case}（真正通過）")
        print("G18 P18.6-M1 9 靜態＋18 動態=27 筆原始義務逐項正反通過。")
        print("此 Gate 僅能關閉 M1；M2/M3/A1/A2、B4、T1～T4 尚未完成。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M1 真簽章驗收失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
