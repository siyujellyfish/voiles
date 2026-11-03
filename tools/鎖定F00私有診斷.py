"""F00：從 P13～P16 真正私有 Checker 的歷史驗收測試採集診斷。

完整記錄每個已實際送入 verify_source_with_ability 的 UTF-8 來源、
核准／拒絕、錯誤類別、完整繁體中文文字與起終位置。原本不會
呼叫私有 Checker 的純模型測試明確標示，不能偽裝成原始來源。
只以 CI --print 的真實資料人工核對後凍結；--check 不得自行更新。
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PHASE_INDEX = ROOT / "tests" / "fixtures" / "F00-原始24項測試證據快照.json"
FROZEN = ROOT / "tests" / "fixtures" / "F00-私有來源診斷快照.json"
BASE_HEAD = "caea4b5b3de423a16d5640d7bd3a2487bf40081a"
PREFIX = "F00_PRIVATE_SOURCE="


def 解碼來源(record: str, phase: str) -> dict[str, object]:
    fields = record.split("\t")
    if len(fields) != 8:
        raise ValueError(f"{phase}：真正 Checker 來源紀錄欄位數量異常")
    source_hex, status, kind, message_hex, *locations = fields
    raw = bytes.fromhex(source_hex)
    source = raw.decode("utf-8", errors="strict")
    if not source:
        raise ValueError(f"{phase}：不可將空來源冒充正式驗收事實")
    message = bytes.fromhex(message_hex).decode("utf-8", errors="strict")
    if status == "Passed":
        if kind != "-" or message or any(pos != "-" for pos in locations):
            raise ValueError(f"{phase}：成功結果卻攜帶失敗診斷")
        diagnostic = None
    elif status == "Failed":
        if kind not in {"Lexical", "Syntax", "Static"} or not message:
            raise ValueError(f"{phase}：失敗結果沒有合法診斷類別與文字")
        if all(pos == "-" for pos in locations):
            span = None
        elif all(pos.isdigit() and int(pos) >= 1 for pos in locations):
            span = {"start": [int(locations[0]), int(locations[1])],
                    "end": [int(locations[2]), int(locations[3])]}
        else:
            raise ValueError(f"{phase}：診斷半開位置缺少部分起訖欄位")
        diagnostic = {"kind": kind, "message": message, "span": span}
    else:
        raise ValueError(f"{phase}：未核准的 Checker 結果 {status}")
    return {
        "source": source,
        "source_sha256": hashlib.sha256(raw).hexdigest(),
        "status": status,
        "diagnostic": diagnostic,
    }


def 取得實際24項診斷(test_bin: Path, emit: bool) -> dict[str, object]:
    indexed = json.loads(PHASE_INDEX.read_text(encoding="utf-8"))
    if len(indexed["cases"]) != 24:
        raise ValueError("F00：原始 24 項具名基準缺失")
    results = []
    for item in indexed["cases"]:
        phase, test_filter = item["id"], item["test_filter"]
        env = dict(os.environ)
        env["VOILES_F00_PRIVATE_CAPTURE"] = "1"
        executed = subprocess.run(
            [str(test_bin), "--nocapture", "--test-threads=1", test_filter],
            cwd=ROOT, env=env, capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if executed.returncode != 0:
            raise RuntimeError(f"{phase}：實際 Rust 驗收執行失敗：\n"
                               + executed.stdout[-1500:] + executed.stderr[-1500:])
        match = re.search(
            r"test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored",
            executed.stdout,
        )
        if match is None or int(match.group(1)) != item["passed"]:
            raise RuntimeError(f"{phase}：執行數量或真實成功結果與凍結 24 項矩陣不同")
        distinct: dict[str, dict[str, object]] = {}
        for line in executed.stdout.splitlines():
            if PREFIX not in line:
                continue
            body = line[line.index(PREFIX) + len(PREFIX):].rstrip("\r\n")
            case = 解碼來源(body, phase)
            key = json.dumps(case, ensure_ascii=False, sort_keys=True)
            distinct[key] = case
        sources = [distinct[key] for key in sorted(distinct)]
        results.append({
            "id": phase,
            "test_filter": test_filter,
            "compiled_tests_passed": item["passed"],
            "private_source_coverage": "captured" if sources else "no_private_entry_in_selected_tests",
            "sources": sources,
        })
        if emit:
            print("F00_PRIVATE_PHASE_JSON=" + json.dumps(
                results[-1], ensure_ascii=False, sort_keys=True, separators=(",", ":")
            ))
    if not any(item["sources"] for item in results):
        raise RuntimeError("F00：全部 24 項均未擷取到任何真實私有 Checker 來源")
    return {
        "format_version": 1,
        "source_baseline_head": BASE_HEAD,
        "category": "由現有 24 項真正 Rust 驗收來源產生的私有 Checker 精確診斷",
        "cases": results,
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
        parser.error(f"找不到已編譯 Rust 核心測試：{test_bin}")
    current = 取得實際24項診斷(test_bin, args.print)
    if args.print:
        total = sum(len(item["sources"]) for item in current["cases"])
        print(f"F00 私有 Checker 候選來源擷取：24 項、{total} 筆去重後的來源／診斷。")
        return
    frozen = json.loads(FROZEN.read_text(encoding="utf-8"))
    if current != frozen:
        older = json.dumps(frozen, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        newer = json.dumps(current, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        print("\n".join(difflib.unified_diff(
            older, newer, fromfile="F00 凍結私有 Checker",
            tofile="目前真正私有 Checker", lineterm="",
        )))
        raise SystemExit("F00：P13～P16 原始來源的精確安全診斷發生變化")
    total = sum(len(item["sources"]) for item in current["cases"])
    print(f"F00 私有 Checker 精確診斷快照通過：24 項、{total} 個已擷取來源。")


if __name__ == "__main__":
    主程式()
