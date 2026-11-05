#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6：原私有 Checker 同步方法共享／獨占 self 真 BorrowTicket 三十一項 Rust 正反 Gate。"""

from __future__ import annotations
import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位",
    "g18_f_同名方法單_int_實參依原_checker_型別選真共享多載",
    "g18_f_具_int_實參的多載宣告順序不影響真方法選擇",
    "g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據",
    "g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活",
    "g18_f_竄改已驗證_int_方法實參值或宣告身分須在動態執行前拒絕",
    "g18_f_非_int_方法實參與一般函式參數引用不冒充本次真執行子集",
    "g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選",
    "g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行",
    "g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選",
    "g18_f_零參數多載方法中途故障仍先歸還原_self_票據",
    "g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕",
    "g18_f_未接通的方法參數引用真執行保守拒絕_不假造動態值",
    "g18_f_共享_self_真欄位加法按原_checker_hir_求值",
    "g18_f_獨占_self_寫入後真欄位減法按最新值計算",
    "g18_f_共享_self_真欄位加法溢位受控錯誤且歸還票據",
    "g18_f_獨占_self_真欄位減法溢位亦不得保留借用",
    "g18_f_竄改已核准方法算術運算子或常值須在配置前拒絕",
    "g18_f_錯誤方法加法型別與未知欄位由原_checker_拒絕",
    "g18_f_獨占_self_真寫入_var_int_並以同張票據返回更新後值",
    "g18_f_獨占_self_真寫入後重跑同一已驗證來源不保留前次狀態",
    "g18_f_獨占_self_執行中故障必歸還票據且不報成功返回",
    "g18_f_const_接收者不能以獨占_self_修改原欄位",
    "g18_f_const_欄位即使接收者為_var_也不能由方法覆寫",
    "g18_f_篡改已驗證的獨占_self_票據或寫入值在動態配置前拒絕",
    "g18_f_獨占_self_寫入禁止非_int_值不藉執行器自行轉型",
    "g18_f_合法_const_結構接收者共享_self_不需可寫權限",
    "g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域",
    "g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕",
    "g18_f_移除真_checker_方法自動借用或方法證據不能執行",
    "g18_f_自動_self_錯誤返回型別與消耗整個_self_由原_checker_拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    listing = subprocess.run(
        [str(args.test_bin.resolve()), "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listing.returncode:
        print("G18 方法 self 真 Rust 清單失敗：" + listing.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::g18_private_method_self_tests::" + name for name in 案例
    }
    missing = sorted(required - available)
    if missing:
        print("G18 self 借用真來源正反缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"G18 真同步方法 self BorrowTicket：{len(required)}/{len(required)}；"
          "一般具參數多載與 async 正向執行尚未完成，原 P18.6 不得正式 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
