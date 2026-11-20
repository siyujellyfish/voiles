#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.4 五個原始借用義務 B1～B5 全 Closed、122 筆真來源與共通退出完整重跑。"""
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
    ("B1", "tools/核對G18-P18.4-B1原始Copyable責任.py", 11),
    ("B2", "tools/核對G18-P18.4-B2整筆Move借用共帳.py", 20),
    ("B3", "tools/核對G18-P18.4-B3真Borrow來源身分.py", 42),
    ("B4", "tools/核對G18-P18.4-B4同步方法借票責任.py", 7),
    ("B5", "tools/核對G18-P18.4-B5借用有效期與無證再借.py", 42),
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        data = json.loads(REGISTER.read_text(encoding="utf-8"))
        if data["current_status"]["P18.4"] != "Passed":
            raise ValueError("P18.4 未標記正式 Passed")
        items = [v for v in data["obligations"] if v["phase"] == "P18.4"]
        if [v["id"] for v in items] != [v[0] for v in GATES]:
            raise ValueError("B1～B5 原契約身分及順序變更")
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
                raise ValueError(f"{name} 與其他 B 義務重複計數")
            seen.update(names)
            result = subprocess.run([sys.executable, str(script), "--test-bin",
                str(opts.test_bin)], capture_output=True, text=True,
                encoding="utf-8", check=False)
            if result.returncode:
                raise ValueError(f"{name} 真子 Gate 失敗：\n"+
                                 (result.stdout+result.stderr)[-3700:])
            print(f"P18.4 {name} 正式 Gate 重跑 {expected_count}/{expected_count} Passed")
        if len(seen) != 122:
            raise ValueError("P18.4 五義務總來源不足 122")
        print("P18.4 原 B1～B5 全 Closed，共 122/122 不重複 Rust 正反真執行，禁止提前結案其他 Phase")
        return 0
    except (OSError, ValueError, SyntaxError, KeyError, TypeError) as exc:
        print("P18.4 正式五義務閉合拒絕："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
