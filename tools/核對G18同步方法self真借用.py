#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6：原私有 Checker 同步方法共享／獨占 self 真 BorrowTicket 九十九項 Rust 正反 Gate。"""

from __future__ import annotations
import argparse
import re
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_r05_真方法局部初始化_return_離域與還票須同一共用時間序",
    "g18_r05_偽造同步方法在真正_return_之後才初始化局部必拒絕",
    "g18_r05_偽造同步方法退出之後才清理局部雖總數相等仍拒絕",
    "g18_r05_方法局部另一張事件帳漏一筆即使單帳平衡仍禁止交付",
    "g18_r05_錯誤回滾起點前假清理方法局部不得冒充合法離域",
    "g18_r05_方法回滾後新建int局部即使補做離域亦不可通過",
    "g18_r04_同步self方法int局部以真參數初始化後正確返回",
    "g18_r04_同步self連續兩個int局部依原checker宣告先後傳值及逆序離域",
    "g18_r04_方法局部Copyable同一來源多次讀取不當成Move或重借",
    "g18_r05_方法第二int局部初始化溢位只離域已初始化第一槽",
    "g18_r05_方法返回算式溢位先逆序清真int局部再歸還self票據",
    "g18_r05_方法存在int局部且方法故障注入前不假造初始化",
    "g18_r04_篡改方法int局部編譯TypedHIR樹及真宣告身分一律執行前拒絕",
    "g18_r04_方法局部未知名稱或bool混入初始化交由原Checker拒絕",
    "g18_r05_偽造方法局部逆序離域或重複初始化皆拒絕",
    "g18_r05_共用帳簿拒絕同步self在真返回之前提前還票",
    "g18_r05_共用帳簿拒絕同步self成功退出後才補還真票據",
    "g18_r05_同步self真共享借用及成功返回逐筆由共用進退帳驗證",
    "g18_r05_同步self故障須真回滾後歸還同張票據再退出",
    "g18_r05_方法引數尚未求值成功不得假造self借用或進入退出",
    "g18_r05_偽造同步self歸還票據缺失或重複均由共用帳簿拒絕",
    "g18_r05_同步self刪真返回改退出及竄改票據來源均不可交付",
    "g18_r02_r04_main整數複合初始化使用同一_typedhir_再傳同步方法",
    "g18_r02_r04_巢狀main局部初始化依原整數減法左右次序傳入方法",
    "g18_r02_r04_已計算main局部可再被Copyable名稱引用並與_self_欄位混算",
    "g18_r02_r05_main局部動態加法溢位在方法借用前受控失敗",
    "g18_r02_r05_中途初始化溢位不可清理未進入的後續局部也不能假造返回",
    "g18_r02_r04_篡改已編譯main整數初始化工作樹必在真執行前拒絕",
    "g18_r02_r04_main複合初始化未知或錯型別來源由原_checker_先拒絕",
    "g18_r02_r04_同步方法實參共用原_typedhir_具名加法括號與返回",
    "g18_r02_r04_巢狀括號與左右加減依原_checker_樹求值",
    "g18_r02_r04_已核准複合整數實參與_self_欄位回傳使用同一真框架",
    "g18_r02_r05_第二個整數複合實參溢位不可提前取得_self_借用或進入方法",
    "g18_r02_r05_第一個複合實參溢位不得執行後續方法本體",
    "g18_r02_r04_偽造已編譯方法實參算術樹或來源位置於執行前拒絕",
    "g18_r02_r04_非法布林混用與未宣告名稱在方法複合引數前先被_checker_拒絕",
    "g18_r02_r04_同步方法讀取main具名_const_int_真局部框架",
    "g18_r02_r04_雙具名_copyable_int_按main宣告及原方法參數次序做減法",
    "g18_r02_r04_main整數局部由前一已初始化名稱複製再傳入self方法",
    "g18_r02_r04_同一具名_copyable_int_可作兩個方法實參不消耗來源",
    "g18_r02_r05_具名main實參與_self_欄位加法溢位仍清除局部及真票據",
    "g18_r02_r05_同步方法具名實參故障注入仍先歸還借用後清空main局部",
    "g18_r02_r04_篡改具名來源局部值身分或方法實參來源皆須零執行拒絕",
    "g18_r02_r04_尚未宣告或未初始化main實參不得進入同步self方法",
    "g18_f_共享_self_欄位加參數讀取真_runtime_與宣告身分",
    "g18_f_共享_self_形式參數減欄位維持左減右次序",
    "g18_f_獨占_self_真寫入後再同方法與_int_形式參數混算",
    "g18_f_共享_self_欄位加巨大_int_實參溢位仍歸還真借用",
    "g18_f_雙_int_方法只讀第二參數與_self_欄位不混用第一個",
    "g18_f_篡改_self_混算方向與參數身分須在動態配置前拒絕",
    "g18_f_偽造_other_欄位與錯誤接收者不得冒充真正_self_來源",
    "g18_f_雙_int_方法真依原多載簽章選取並計算兩個獨立參數",
    "g18_f_雙_int_方法可只返回第二參數而不錯讀第一或_self_",
    "g18_f_雙_int_方法依實參順序正確執行減法",
    "g18_f_雙_int_方法加法溢位仍歸還唯一真_self_票據",
    "g18_f_多_int_實參方法故障不殘留形參且重新呼叫不受污染",
    "g18_f_竄改第二方法實參值或宣告身分均於真資源前拒絕",
    "g18_f_多_int_方法來源錯型別與不存在參數須原_checker_或安全準備拒絕",
    "g18_f_三個_int_方法參數按原宣告位置獨立綁定返回第三個",
    "g18_r05_同步共享與獨占_self_正常及故障均歸還真票據且零假資源",
    "g18_r05_同步_self_偽造未歸票據或無來源資源事件必被守門拒絕",
    "g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位",
    "g18_f_方法真參數返回使用所選_DeclarationId_的框架值",
    "g18_f_方法參數真加法在框架內求值不誤讀_self_欄位",
    "g18_f_方法參數真減法與重複呼叫保留同一來源參數值",
    "g18_f_方法參數真加法溢位受控錯誤且先歸還_self_票據",
    "g18_f_方法參數真讀取之前故障不得報告成功返回",
    "g18_f_篡改已驗證的方法參數返回身分或算術方案必在配置前拒絕",
    "g18_f_方法未定義的參數來源及錯誤型別須先由原_checker_拒絕",
    "g18_f_同名方法單_int_實參依原_checker_型別選真共享多載",
    "g18_f_具_int_實參的多載宣告順序不影響真方法選擇",
    "g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據",
    "g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活",
    "g18_f_竄改已驗證_int_方法實參值或宣告身分須在動態執行前拒絕",
    "g18_f_已核准_int_參數與_self_欄位可真混算但非_int_實參仍拒絕",
    "g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選",
    "g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行",
    "g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選",
    "g18_f_零參數多載方法中途故障仍先歸還原_self_票據",
    "g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕",
    "g18_f_同一_int_參數左右加法仍取同一真框架值",
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
    # 僅 --list 並不能證明真正執行；逐項要求同一 Rust binary
    # 的真 Parser／Checker→self BorrowTicket 正反均實際成功且零忽略。
    run = subprocess.run(
        [str(args.test_bin.resolve()), "--test-threads=2",
         "struct_stage::g18_private_method_self_tests::"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    match = re.search(
        r"test result: ok\.\s+(\d+) passed;\s+0 failed;\s+0 ignored;",
        run.stdout + run.stderr,
    )
    if run.returncode or not match or int(match.group(1)) != len(required):
        print("G18 同步方法 self 真執行測試未全數通過："
              + run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 真同步方法 self BorrowTicket：{len(required)}/{len(required)} 真執行通過；"
          "多參數 int 已接通受限子集，複雜型別及 async 尚未完成，原 P18.6 不得正式 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
