"""F00：核對原始 P13～P16 24 個已簽核項目的可執行 Rust 來源證據。

必須讀取現行權威 24 項矩陣及真正編譯的 Rust 核心測試執行檔；
禁止以歷史總數、空測試組或修改 JSON 期望冒充實際執行。
文字檔一律指定 UTF-8。
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MATRIX = ROOT / "AI-build" / "P13-P16-原始24項正式驗收矩陣.md"
FROZEN = ROOT / "tests" / "fixtures" / "F00-原始24項測試證據快照.json"
BASE_HEAD = "ce173af5810d008a40bc54f7624989acc6c0bbe2"

# 每一筆都來自 2026-10-30 現行已核准 24 項的真實來源或模型測試。
# 本表只決定重構回歸的必測來源，並不重寫原始 Phase 的驗收語意。
FILTERS = {
    "P13.1": "struct_stage::p13_1_正式來源閉包::",
    "P13.2": "struct_stage::p13_2_一般賦值正式來源閉包::",
    "P13.3": "struct_stage::p13_3_所有權移動來源閉包::",
    "P13.4": "struct_stage::p13_4_正式來源閉包::",
    "P13.5": "struct_stage::p13_5_資源與可複製值來源對照矩陣::",
    "P13.6": "struct_stage::p13_s19_s21_結構所有權來源矩陣::",
    "P14.1": "borrow_safety::tests::",
    "P14.2": "struct_stage::p14_1_p14_2_真實來源與衝突矩陣::",
    "P14.3": "struct_stage::va14_va15_borrow_source_tests::",
    "P14.4": "struct_stage::p14_4_借用別名複製能力來源::",
    "P14.5": "struct_stage::p14_5_p14_6_巢狀呼叫及_self_權限::",
    "P14.6": "struct_stage::automatic_self_borrow_tests::",
    "P15.1": "struct_stage::p15_1_父區塊後繼與內層宣告身分::",
    "P15.2": "struct_stage::p15_2_借用回傳與逃逸正式來源::",
    "P15.3": "struct_stage::p15_3_函式預設借用跨分支迴圈交叉驗證::",
    "P15.4": "struct_stage::p15_4_欄位索引與容器借用界線::",
    "P15.5": "struct_stage::p15_5_懸空與合法重新借用來源::",
    "P15.6": "struct_stage::p15_6_最後使用點與_self_退出邊界來源::",
    "P16.1": "owned_str_runtime::tests::",
    "P16.2": "struct_initialization::p16_真實持續存活結構值資源責任::",
    "P16.3": "struct_stage::p16_6_r03_10_單層布林分支與作用域資源清理::",
    "P16.4": "owned_str_runtime::p16_4_資源事件身分逐筆稽核::",
    "P16.5": "resource_obligation_audit::tests::p16_未清理責任須明確偵測不可倚賴_rust_drop_或_gc",
    "P16.6": "struct_eval_plan::tests::vb16_",
}


def 讀取原驗收矩陣() -> dict[str, str]:
    rows = {}
    for line in MATRIX.read_text(encoding="utf-8").splitlines():
        matched = re.match(r"^\|\s*(P(?:13|14|15|16)\.[1-6])\s*\|", line)
        if not matched:
            continue
        key = matched.group(1)
        cols = [part.strip() for part in line.split("|")[1:-1]]
        if len(cols) < 5 or "已驗收" not in cols[-1] or key in rows:
            raise ValueError(f"原始 24 項矩陣有重複、缺少簽核或格式異常：{key}")
        rows[key] = cols[1]
    expected = {f"P{phase}.{index}" for phase in range(13, 17)
                for index in range(1, 7)}
    if set(rows) != expected or set(FILTERS) != expected:
        raise ValueError("原始 24 項驗收矩陣或正式來源測試索引不完整")
    return rows


def 執行已核准來源(test_bin: Path) -> dict[str, object]:
    rows = 讀取原驗收矩陣()
    listing = subprocess.run(
        [str(test_bin), "--list"], cwd=ROOT, capture_output=True,
        text=True, encoding="utf-8", check=False,
    )
    if listing.returncode:
        raise RuntimeError("F00 無法取得真正編譯的 Rust 測試列表")
    compiled = {
        line.removesuffix(": test") for line in listing.stdout.splitlines()
        if line.endswith(": test")
    }
    evidence = []
    for phase, filter_text in FILTERS.items():
        selected = sorted(name for name in compiled if filter_text in name)
        if not selected:
            raise ValueError(f"{phase}：沒有任何正式編譯的來源測試符合 {filter_text}")
        executed = subprocess.run(
            [str(test_bin), filter_text, "--test-threads=1"],
            cwd=ROOT, capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if executed.returncode != 0:
            raise RuntimeError(f"{phase}：原始來源驗收回歸失敗：\n"
                               + executed.stdout[-1400:] + executed.stderr[-1400:])
        matched = re.search(
            r"test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored",
            executed.stdout,
        )
        if matched is None or int(matched.group(1)) != len(selected):
            raise RuntimeError(f"{phase}：正式 Rust 執行數量不符或缺少零失敗證據")
        evidence.append({
            "id": phase,
            "original_requirement": rows[phase],
            "requirement_sha256": hashlib.sha256(rows[phase].encode("utf-8")).hexdigest(),
            "test_filter": filter_text,
            "tests": selected,
            "passed": len(selected),
            "failed": 0,
            "ignored": 0,
        })
    return {
        "format_version": 1,
        "source_baseline_head": BASE_HEAD,
        "category": "P13～P16 已核准私有受限原始 24 項各自非零來源測試",
        "cases": evidence,
    }


def 主程式() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--print", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    test_bin = args.test_bin.resolve()
    if not test_bin.is_file():
        parser.error(f"找不到 Rust 核心測試執行檔：{test_bin}")
    current = 執行已核准來源(test_bin)
    if args.print:
        print("F00_PHASE24_JSON=" + json.dumps(
            current, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ))
        return
    frozen = json.loads(FROZEN.read_text(encoding="utf-8"))
    if frozen != current:
        older = json.dumps(frozen, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        newer = json.dumps(current, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        print("\n".join(difflib.unified_diff(
            older, newer, fromfile="已凍結 24 項原始來源",
            tofile="最新 Rust 編譯並執行的真實來源", lineterm="",
        )))
        raise SystemExit("F00：原始 P13～P16 24 項驗收來源或結果變更")
    print(f"F00 原 24 項重構回歸通過：{len(current['cases'])} 項皆有非零測試。")


if __name__ == "__main__":
    主程式()
