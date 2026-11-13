#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V0：原六項 26 條義務、現有真來源及工程／規格阻塞的可執行盤點。

本關卡核對並逐條真正執行 24 個互不重複的主要 Rust 證據來源。
這是 V0 來源盤點，不是原 P18.1～P18.6 的正式 --check 驗收。
"""

from __future__ import annotations

import argparse
import ast
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import sys

根目錄 = Path(__file__).resolve().parent.parent
原六項腳本 = 根目錄 / "tools" / "核對G18原六項驗收差距.py"
義務清冊 = 根目錄 / "AI-build" / "G18-P18-V0-原六項義務清冊-20261108.json"
原項 = tuple(f"P18.{i}" for i in range(1, 7))
義務身分 = {
    "P18.1": ("F1", "F2", "F3", "F4"),
    "P18.2": ("C1", "C2", "C3", "C4"),
    "P18.3": ("R1", "R2", "R3", "R4"),
    "P18.4": ("B1", "B2", "B3", "B4", "B5"),
    "P18.5": ("T1", "T2", "T3", "T4"),
    "P18.6": ("M1", "M2", "M3", "A1", "A2"),
}
證據種類 = {
    "RuntimeSuccess", "RuntimeFailure", "CheckerRejection",
    "TraceRejection", "DecisionBlocked", "EngineeringPending",
}


def 取得靜態字面值(目標: str) -> dict:
    """只讀 AST 字面值，不執行、複製或重建原 Checker。"""
    原始碼 = 原六項腳本.read_text(encoding="utf-8")
    語法樹 = ast.parse(原始碼, filename=str(原六項腳本))
    符合 = [
        ast.literal_eval(statement.value)
        for statement in 語法樹.body
        if isinstance(statement, ast.Assign)
        and any(
            isinstance(name, ast.Name) and name.id == 目標
            for name in statement.targets
        )
    ]
    if len(符合) != 1 or not isinstance(符合[0], dict):
        raise ValueError(f"原 G18 正式 {目標} 必須只有一份字面權威表")
    return 符合[0]


def 核對清冊(清冊: dict, 原證據: dict, 正式狀態: dict) -> tuple[list[dict], dict]:
    if 清冊.get("schema") != "G18-P18-V0/2":
        raise ValueError("V0 機讀清冊格式識別不符合")
    if set(原證據) != set(原項) or set(正式狀態) != set(原項):
        raise ValueError("原六項證據或正式狀態不完整")
    if set(清冊.get("original_status", {})) != set(原項):
        raise ValueError("V0 六項原始狀態快照身分缺漏")
    if 清冊["original_status"] != {
        "P18.1": "SubsetVerified", "P18.2": "SubsetVerified",
        "P18.3": "Incomplete", "P18.4": "SubsetVerified",
        "P18.5": "SubsetVerified", "P18.6": "BlockedByDecision",
    }:
        raise ValueError("V0 2026-11-08 原始快照不得被新核准覆寫")
    if set(清冊.get("current_status", {})) != set(原項):
        raise ValueError("G18 現行六項狀態不完整")
    if 清冊.get("approved_async_semantics") != [
        "D18-02-E1-A: 延遲啟動且只限直接 await",
        "D18-02-E2-A: 單執行緒協作式排程",
        "D18-02-E3-A: 安全點取消與逐框架退出",
        "D18-02-E4-A: 現行 RuntimeFailure 傳遞",
        "D18-02-E5: 維持 VB15-01-A 有效借用不得跨 await",
    ]:
        raise ValueError("D18-02-E1～E5 已核准語意記錄不完整，不能解除決策阻塞")

    for 項目 in 原項:
        if not isinstance(原證據[項目], (tuple, list)):
            raise ValueError(f"{項目} 真來源證據非序列")
        if not 原證據[項目] or len(原證據[項目]) != len(set(原證據[項目])):
            raise ValueError(f"{項目} 真來源缺少或重複")
        現狀 = 正式狀態[項目]
        if not isinstance(現狀, tuple) or len(現狀) != 2:
            raise ValueError(f"{項目} 正式狀態格式不正確")
        if 清冊["current_status"][項目] != 現狀[0]:
            raise ValueError(f"{項目} 現行正式狀態未與核准語意及機讀義務同步")
        if 項目 not in {"P18.1", "P18.2", "P18.3", "P18.6"} and 清冊["original_status"][項目] != 現狀[0]:
            raise ValueError(f"{項目} 非本次核准範圍卻變更了原始驗收狀態")
        if 項目 == "P18.1" and 現狀[0] != "Passed":
            raise ValueError("P18.1 必須與 F1～F4 正式四義務同步為 Passed")
        if 項目 == "P18.2" and 現狀[0] != "Passed":
            raise ValueError("P18.2 只有四項原控制義務均 Closed 才能 Passed")
        if 項目 == "P18.3" and 現狀[0] != "Passed":
            raise ValueError("P18.3 必須完整閉合 R1～R4 才能 Passed")
        if 項目 == "P18.6" and 現狀[0] != "Incomplete":
            raise ValueError("D18-02 已核准但 A1/A2 尚無完整真任務證據，P18.6 只能 Incomplete")
    所有來源 = {來源 for 來源群 in 原證據.values() for 來源 in 來源群}
    if not 所有來源:
        raise ValueError("原六項真來源不可為空")

    清冊義務 = 清冊.get("obligations")
    if not isinstance(清冊義務, list) or len(清冊義務) != 26:
        raise ValueError("V0 必須列出完整的 26 條原始義務")
    可用主證據: set[str] = set()
    已見義務: set[str] = set()
    每項義務 = Counter()
    統計 = Counter()
    for 義務 in 清冊義務:
        if not isinstance(義務, dict):
            raise ValueError("V0 義務格式不正確")
        名稱 = 義務.get("id")
        階段 = 義務.get("phase")
        狀態 = 義務.get("status")
        種類 = 義務.get("evidence_kind")
        來源 = 義務.get("primary_case")
        if 名稱 in 已見義務 or 名稱 not in 義務身分.get(階段, ()):
            raise ValueError(f"V0 義務重複或來源階段不一致：{名稱}")
        if 義務.get("batch") not in {f"V{i}" for i in range(1, 9)}:
            raise ValueError(f"V0 {名稱} 沒有指向實際後續工程批次")
        路徑 = 義務.get("work_file")
        if not isinstance(路徑, str) or not (根目錄 / 路徑).is_file():
            raise ValueError(f"V0 {名稱} 未指向實際存在的原始碼／稽核檔案")
        if not isinstance(義務.get("remaining"), str) or not 義務["remaining"].strip():
            raise ValueError(f"V0 {名稱} 未列出尚須收口的工程責任")
        if 種類 not in 證據種類 or 狀態 not in {
            "Closed", "OpenEngineering", "BlockedDecision", "OutOfScope",
        }:
            raise ValueError(f"V0 {名稱} 狀態或證據種類非法")

        if 名稱 in {"A1", "A2"}:
            if (
                階段 != "P18.6" or 狀態 != "OpenEngineering"
                or 種類 != "EngineeringPending" or 來源 is not None
            ):
                raise ValueError(f"V0 {名稱} 決策已核准但真正 async 完整驗收仍不得偽造主證據")
        else:
            if 名稱 == "F1":
                # 只有實際補入原 Checker→Runtime 真正反七項獨立 Gate、
                # 並在本次 PR 最新 HEAD CI 實際執行，才可將 F1 單條 Closed。
                # 不以此將 P18.1 或任一相鄰義務標成 Passed。
                必要證明 = {
                    "contract": "F1-C01..F1-C07",
                    "tests": "src/struct_stage/g18_f1_obligation_tests.rs",
                    "gate": "tools/核對G18-F1真函式圖閉合.py",
                    "report": "AI-build/G18-P18-F1-函式圖原始義務閉合證據-20261110.md",
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError("F1 不得在缺少七項來源／真 Gate 的情況下標 Closed")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError("F1 正式驗收來源、Gate 或工程報告不存在")
            elif 名稱 == "F2":
                必要證明 = {
                          "contract": "F2-C01..F2-C15",
                          "tests": "src/struct_stage/g18_f2_obligation_tests.rs",
                          "gate": "tools/核對G18-F2原始義務正式閉合.py",
                          "report": "AI-build/G18-P18-F2-原始義務閉合證據-20261113.md"
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError("F2 必須有十五等價類及三十項真來源閉合證明才可 Closed")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError("F2 正式來源、Gate 或工程報告不存在")
            elif 名稱 == "F3":
                必要證明 = {
                    "contract": "F3-C01..F3-C15",
                    "tests": "src/struct_stage/g18_f3_call_site_tests.rs",
                    "gate": "tools/核對G18-F3原始義務正式閉合.py",
                    "report": "AI-build/G18-P18-F3-原始義務閉合證據-20261113.md",
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError("F3 必須有十五等價類三十真來源證據才准 Closed")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError("F3 原始證據路徑不存在")
            elif 名稱 == "F4":
                必要證明 = {
                    "contract": "F4-C01..F4-C13",
                    "tests": "src/struct_stage/execute_call_scalar.rs",
                    "gate": "tools/核對G18-F4可達函式圖原始義務閉合.py",
                    "report": "AI-build/G18-P18-F4-可達函式圖原始義務閉合證據-20261113.md",
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError("F4 原 Checker 靜態圖十三類閉合證明不完整")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError("F4 原始來源／專用 Gate 或報告不存在")
            elif 名稱 in {"C1", "C2", "C3", "C4"}:
                預期數量 = {"C1": 10, "C2": 10, "C3": 12, "C4": 12}[名稱]
                必要證明 = {
                    "contract": f"{名稱}-01..{名稱}-{預期數量:02d}",
                    "tests": "src/struct_stage/execute_call_scalar.rs",
                    "gate": {
                        "C1": "tools/核對G18-P18.2-C1分支選葉.py",
                        "C2": "tools/核對G18-P18.2-C2迴圈回邊.py",
                        "C3": "tools/核對G18-P18.2-C3巢狀控制轉移.py",
                        "C4": "tools/核對G18-P18.2-C4局部資源離域.py",
                    }[名稱],
                    "report": "AI-build/G18-P18.2-原始四義務正式驗收證據-20261113.md",
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError(f"P18.2 {名稱} 需獨立原控制與資源真證據")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError(f"P18.2 {名稱} 真來源／Gate／報告不存在")
            elif 名稱 in {"R1", "R2", "R3", "R4"}:
                數量 = {"R1":16,"R2":14,"R3":16,"R4":16}[名稱]
                必要證明 = {
                    "contract": f"{名稱}-01..{名稱}-{數量:02d}",
                    "tests": "src/struct_stage/execute_recursive_str.rs",
                    "gate": {
                        "R1": "tools/核對G18-P18.3-R1原遞迴真返回.py",
                        "R2": "tools/核對G18-P18.3-R2動態預算.py",
                        "R3": "tools/核對G18-P18.3-R3跨框架身分.py",
                        "R4": "tools/核對G18-P18.3-R4失敗回滾.py",
                    }[名稱],
                    "report": "AI-build/G18-P18.3-原始四義務正式驗收證據-20261113.md",
                }
                if 狀態 != "Closed" or 義務.get("closure_proof") != 必要證明:
                    raise ValueError(f"P18.3 {名稱} 沒有原 Checker／真動態遞迴必要證據")
                if any(not (根目錄 / 路徑).is_file() for 欄位, 路徑 in 必要證明.items() if 欄位 != "contract"):
                    raise ValueError(f"P18.3 {名稱} 的 Gate／Rust 來源／報告不存在")
            elif 狀態 != "OpenEngineering" or 義務.get("closure_proof") is not None:
                raise ValueError(f"V0 {名稱} 尚未達完整契約，不得提前 Closed")
            if 種類 in {"DecisionBlocked", "EngineeringPending"}:
                raise ValueError(f"V0 {名稱} 不得使用舊決策阻塞或偽造證據分類")
            if not isinstance(來源, str) or 來源 not in 原證據[階段]:
                raise ValueError(f"V0 {名稱} 主要真來源未列在相應原始六項證據")
            if 來源 in 可用主證據:
                raise ValueError(f"V0 {名稱} 重複使用同一主要證據充當新覆蓋")
            可用主證據.add(來源)
        已見義務.add(名稱)
        每項義務[階段] += 1
        統計[狀態] += 1

    if 清冊["current_status"]["P18.1"] != "Passed" or any(
        item["phase"] == "P18.1" and item["status"] != "Closed"
        for item in 清冊義務
    ):
        raise ValueError("P18.1 只有 F1～F4 四義務全 Closed 才可 Passed")

    if 清冊["current_status"]["P18.2"] != "Passed" or any(
        item["phase"] == "P18.2" and item["status"] != "Closed"
        for item in 清冊義務
    ):
        raise ValueError("P18.2 C1～C4 必須全部 Closed 才可 Passed")

    if 清冊["current_status"]["P18.3"] != "Passed" or any(
        item["phase"] == "P18.3" and item["status"] != "Closed"
        for item in 清冊義務
    ):
        raise ValueError("P18.3 R1～R4 必須 Closed 才可 Passed")

    for 階段, 預期 in 義務身分.items():
        if 每項義務[階段] != len(預期):
            raise ValueError(f"V0 {階段} 的原始義務不足或數量錯誤")
    if 已見義務 != {名稱 for 群 in 義務身分.values() for 名稱 in 群}:
        raise ValueError("V0 義務身分完整性不符")
    if 統計["Closed"] != 12 or 統計["OpenEngineering"] != 14 or 統計["BlockedDecision"] != 0:
        raise ValueError("F1～F4、C1～C4、R1～R4 閉合；其餘 14 條 OpenEngineering；不得提前 P18 Passed")

    待重現缺口 = 清冊.get("proven_gap", {})
    if 待重現缺口.get("id") != "F4" or (
        待重現缺口.get("case") not in 原證據["P18.1"]
    ) or 待重現缺口.get("category") != "HistoricalCheckerAcceptedRuntimeUnsupported" or (
        待重現缺口.get("resolution") != "ResolvedInV1DefaultVarStr"
    ) or not isinstance(待重現缺口.get("resolution_details"), str):
        raise ValueError("V0 歷史真錯拒來源與 V1 真修復測試的追溯不一致")

    return 清冊義務, {
        "original_phase_references": sum(len(群) for 群 in 原證據.values()),
        "unique_original_tests": len(所有來源),
        "independent_primary_tests": len(可用主證據),
        "engineering_open": 統計["OpenEngineering"],
        "decision_blocked": 統計["BlockedDecision"],
        "closed": 統計["Closed"],
        "per_phase": {
            階段: {
                "original_status": 清冊["original_status"][階段],
                "current_status": 正式狀態[階段][0],
                "evidence_references": len(原證據[階段]),
                "obligations": 每項義務[階段],
            }
            for 階段 in 原項
        },
    }


def 真重跑(執行檔: Path, 義務清單: list[dict], 補充來源: str) -> int:
    清單 = subprocess.run(
        [str(執行檔), "--list"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if 清單.returncode != 0:
        raise ValueError("Rust 真測試清單無法取得：" + 清單.stderr[-1200:])
    現有名稱 = {
        行.split(": test", 1)[0]
        for 行 in 清單.stdout.splitlines() if 行.endswith(": test")
    }
    正反來源 = [項["primary_case"] for 項 in 義務清單 if 項["primary_case"] is not None]
    正反來源.append(補充來源)
    遺失 = sorted(set(正反來源) - 現有名稱)
    if 遺失:
        raise ValueError("V0 來源只有文字而沒有真正 Rust 測試：" + "、".join(遺失))
    for index, 名稱 in enumerate(正反來源, 1):
        結果 = subprocess.run(
            [str(執行檔), "--exact", 名稱, "--test-threads=1"],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        輸出 = 結果.stdout + 結果.stderr
        if 結果.returncode or not re.search(
            r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
            輸出,
        ):
            raise ValueError(
                f"V0 第 {index} 筆真來源未實際執行成功：{名稱}\n" + 輸出[-1400:]
            )
        print(f"V0 真 Rust 證據 {index}/{len(正反來源)}：{名稱}（成功）")
    return len(正反來源)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path,
                        help="輸出 UTF-8 JSON 基線盤點，可供下一批機器比對")
    args = parser.parse_args()

    try:
        清冊 = json.loads(義務清冊.read_text(encoding="utf-8"))
        原證據 = 取得靜態字面值("證據")
        正式狀態 = 取得靜態字面值("狀態")
        義務清單, 報告 = 核對清冊(清冊, 原證據, 正式狀態)
        執行檔 = args.test_bin.resolve(strict=True)
        報告["executed_evidence_tests"] = 真重跑(
            執行檔, 義務清單, 清冊["proven_gap"]["case"],
        )
        報告["baseline_main_sha"] = 清冊["baseline_main_sha"]
        報告["obligation_results"] = [
            {
                "id": 項["id"], "phase": 項["phase"], "batch": 項["batch"],
                "status": 項["status"], "evidence_kind": 項["evidence_kind"],
                "primary_case": 項["primary_case"],
                "remaining": 項["remaining"], "work_file": 項["work_file"],
            }
            for 項 in 義務清單
        ]
        if args.report is not None:
            報告路徑 = args.report.resolve()
            報告路徑.parent.mkdir(parents=True, exist_ok=True)
            報告路徑.write_text(
                json.dumps(報告, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
    except (OSError, ValueError, json.JSONDecodeError, SyntaxError) as err:
        print(f"G18 V0 正式施工盤點失敗：{err}", file=sys.stderr)
        return 1

    print(
        f"G18 V0 盤點：原六項 {報告['unique_original_tests']} 個不同 Rust 名稱、"
        f"{報告['original_phase_references']} 次跨原項引用；"
        f"26 條義務／{報告['closed']} 條已閉合／"
        f"{報告['engineering_open']} 條工程未閉合／0 條待決 async 決策；"
        f"25 筆獨立真 Rust 測試均已實跑；"
        "P18 原六項 P18.1～P18.3 Passed（3/6）；其餘三原項未 Passed。"
    )
    print("V0 完成只代表來源／義務清冊可信；R07 正式 --check 未完成仍必須非零。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
