#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.3 四個原始遞迴義務 R1～R4 全 Closed、62 筆真來源與共通退出完整重跑。"""
from __future__ import annotations
import argparse
import ast
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
REGISTER = ROOT/"AI-build/G18-P18-V0-原六項義務清冊-20261108.json"
GATES = (
    ("R1", "tools/核對G18-P18.3-R1原遞迴真返回.py", 16),
    ("R2", "tools/核對G18-P18.3-R2動態預算.py", 14),
    ("R3", "tools/核對G18-P18.3-R3跨框架身分.py", 16),
    ("R4", "tools/核對G18-P18.3-R4失敗回滾.py", 16),
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        data = json.loads(REGISTER.read_text(encoding="utf-8"))
        if data["current_status"]["P18.3"] != "Passed":
            raise ValueError("P18.3 未標記正式 Passed")
        items = [v for v in data["obligations"] if v["phase"] == "P18.3"]
        if [v["id"] for v in items] != [v[0] for v in GATES]:
            raise ValueError("R1～R4 原契約身分及順序變更")
        seen = set()
        for item, (name, path, expected_count) in zip(items, GATES):
            if item["status"] != "Closed" or item.get("closure_proof",{}).get("gate") != path:
                raise ValueError(f"{name} 無獨立 closure_proof，禁止正式驗收")
            script = ROOT/path
            tree = ast.parse(script.read_text(encoding="utf-8"), filename=str(script))
            assignments = [a.value for a in tree.body if isinstance(a, ast.Assign)
                           and any(isinstance(t, ast.Name) and t.id=="CASES"
                                   for t in a.targets)]
            if len(assignments) != 1:
                raise ValueError(f"{name} 測試名清冊無單一字面權威")
            names = ast.literal_eval(assignments[0])
            if len(names) != expected_count or len(set(names)) != expected_count:
                raise ValueError(f"{name} 必須有固定且唯一的原始測試")
            if seen.intersection(names):
                raise ValueError(f"{name} 與其他 R 義務重複計數")
            seen.update(names)
            result = subprocess.run([sys.executable, str(script), "--test-bin",
                str(opts.test_bin), "--check"], capture_output=True, text=True,
                encoding="utf-8", check=False)
            if result.returncode:
                raise ValueError(f"{name} 真子 Gate 失敗：\n"+
                                 (result.stdout+result.stderr)[-3700:])
            print(f"P18.3 {name} 正式 Gate 重跑 {expected_count}/{expected_count} Passed")
        if len(seen) != 62:
            raise ValueError("P18.3 四義務總來源不足 62")
        print("P18.3 原 R1～R4 全 Closed，共 62/62 不重複 Rust 正反真執行，禁止提前結案其他 Phase")
        return 0
    except (OSError, ValueError, SyntaxError, KeyError, TypeError) as exc:
        print("P18.3 正式四義務閉合拒絕："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
