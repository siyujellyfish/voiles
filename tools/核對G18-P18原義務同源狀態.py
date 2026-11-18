#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／S0：以原 26 項機讀義務核對 R07 六階段狀態；不代替 Rust 真執行 Gate。"""
from __future__ import annotations

import argparse
import ast
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "AI-build/G18-P18-V0-原六項義務清冊-20261108.json"
R07 = ROOT / "tools/核對G18原六項驗收差距.py"
PHASE_IDS = {
    "P18.1": ("F1", "F2", "F3", "F4"),
    "P18.2": ("C1", "C2", "C3", "C4"),
    "P18.3": ("R1", "R2", "R3", "R4"),
    "P18.4": ("B1", "B2", "B3", "B4", "B5"),
    "P18.5": ("T1", "T2", "T3", "T4"),
    "P18.6": ("M1", "M2", "M3", "A1", "A2"),
}
STATES = {"Passed", "SubsetVerified", "Incomplete", "BlockedByDecision"}
OBLIGATION_STATES = {"Closed", "OpenEngineering", "BlockedDecision", "OutOfScope"}


def read_r07_status() -> dict[str, tuple[str, str]]:
    # 不執行 R07 Python 模組；只解析其唯一字面值賦值，避免第二權威。
    root = ast.parse(R07.read_text(encoding="utf-8"), filename=str(R07))
    values = [
        ast.literal_eval(node.value)
        for node in root.body
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "狀態"
            for target in node.targets
        )
    ]
    if len(values) != 1 or not isinstance(values[0], dict):
        raise ValueError("R07 缺少唯一正式六項狀態表")
    data = values[0]
    if set(data) != set(PHASE_IDS) or any(
        not isinstance(value, tuple) or len(value) != 2
        or value[0] not in STATES or not isinstance(value[1], str)
        or not value[1].strip()
        for value in data.values()
    ):
        raise ValueError("R07 正式狀態身分或原因不完整")
    return data


def audit() -> tuple[int, int]:
    manifest = json.loads(MATRIX.read_text(encoding="utf-8"))
    if manifest.get("schema") != "G18-P18-V0/2":
        raise ValueError("原始機讀清冊版本不符，不可自行更換驗收權威")
    historical = manifest.get("original_status")
    if not isinstance(historical, dict) or set(historical) != set(PHASE_IDS) \
            or historical.get("P18.6") != "BlockedByDecision":
        raise ValueError("不得刪改 V0 歷史語意決策阻塞與六項身分")
    if not isinstance(manifest.get("approved_async_semantics"), list) \
            or len(manifest["approved_async_semantics"]) != 5:
        raise ValueError("已核准 V7 E1～E5 來源清冊不完整")
    phases = manifest.get("current_status")
    if not isinstance(phases, dict) or set(phases) != set(PHASE_IDS) \
            or any(value not in STATES for value in phases.values()):
        raise ValueError("最新原六項階段與可用正式狀態不一致")
    r07 = read_r07_status()
    for phase in PHASE_IDS:
        if phases[phase] != r07[phase][0]:
            raise ValueError(
                f"{phase} 機讀={phases[phase]}，R07={r07[phase][0]}，不得只改狀態文字"
            )
    rows = manifest.get("obligations")
    if not isinstance(rows, list) or len(rows) != 26:
        raise ValueError("原 26 項義務數量遭增刪")
    expected = {(phase, key) for phase, ids in PHASE_IDS.items() for key in ids}
    seen = set()
    all_gates = set()
    closed = 0
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("義務必須為可稽核機讀資料")
        identity = (row.get("phase"), row.get("id"))
        if identity not in expected or identity in seen:
            raise ValueError(f"原始義務身分重複或不合法：{identity}")
        seen.add(identity)
        state = row.get("status")
        if state not in OBLIGATION_STATES:
            raise ValueError(f"{identity} 缺少正式 Closed/OpenEngineering 分類")
        if state == "Closed":
            closed += 1
            proof = row.get("closure_proof")
            if not isinstance(proof, dict):
                raise ValueError(f"{identity} Closed 未綁定正式獨立證據")
            for key in ("contract", "tests", "gate", "report"):
                value = proof.get(key)
                if not isinstance(value, str) or not value.strip():
                    raise ValueError(f"{identity} Closed 缺少 {key}")
            if proof["gate"] in all_gates:
                raise ValueError(f"{identity} 重複採計其他原義務 Gate")
            all_gates.add(proof["gate"])
            for key in ("tests", "gate", "report"):
                artifact = ROOT / proof[key]
                if not artifact.is_file() or not artifact.resolve().is_relative_to(ROOT):
                    raise ValueError(f"{identity} 證據路徑不存在或離開專案：{proof[key]}")
    if seen != expected:
        raise ValueError(f"原義務缺失={sorted(expected - seen)}")
    by_phase = {
        phase: [row for row in rows if row["phase"] == phase]
        for phase in PHASE_IDS
    }
    for phase, group in by_phase.items():
        all_closed = all(row["status"] == "Closed" for row in group)
        if (phases[phase] == "Passed") != all_closed:
            raise ValueError(
                f"{phase} 狀態={phases[phase]} 與 {sum(row['status']=='Closed' for row in group)}"
                f"/{len(group)} 原始義務 Closed 不一致"
            )
        if phases[phase] == "BlockedByDecision" and not any(
            row["status"] == "BlockedDecision" for row in group
        ):
            raise ValueError(f"{phase} 沒有 BlockedDecision 不得宣稱尚待語意決策")
        if any(row["status"] == "BlockedDecision" for row in group) \
                and phases[phase] != "BlockedByDecision":
            raise ValueError(f"{phase} 遺漏真正待決語意阻塞")
    return closed, sum(state == "Passed" for state in phases.values())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.parse_args()
    try:
        closed, passed = audit()
        print(f"G18 S0 同源狀態稽核成功：{closed}/26 Closed；"
              f"{passed}/6 Passed；R07 與原機讀義務、獨立 Gate 路徑一致。")
        print("此檢查不執行 Rust；P18 正式完成仍必須 R07 --check 與主線 CI。")
        return 0
    except (ValueError, OSError, SyntaxError, json.JSONDecodeError) as error:
        print("G18 S0 原義務同源狀態錯誤：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
