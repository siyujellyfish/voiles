#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.6 五個原始方法與協作義務 M1～M3、A1～A2 全 Closed、51 筆真來源與共通退出完整重跑。"""
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
    ("M1", "tools/核對G18-P18.6-M1原始方法簽章.py", 27),
    ("M2", "tools/核對G18-P18.6-M2同步方法借用與欄位.py", 8),
    ("M3", "tools/核對G18-P18.6-M3函式方法同一控制圖.py", 8),
    ("A1", "tools/核對G18-P18.6-A1協作普通函式與方法續行.py", 4),
    ("A2", "tools/核對G18-P18.6-A2取消與逐框架清理.py", 4),
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        data = json.loads(REGISTER.read_text(encoding="utf-8"))
        if data["current_status"]["P18.6"] != "Passed":
            raise ValueError("P18.6 未標記正式 Passed")
        items = [v for v in data["obligations"] if v["phase"] == "P18.6"]
        if [v["id"] for v in items] != [v[0] for v in GATES]:
            raise ValueError("M1～M3、A1～A2 原契約身分及順序變更")
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
                raise ValueError(f"{name} 與其他方法或等待 義務重複計數")
            seen.update(names)
            result = subprocess.run([sys.executable, str(script), "--test-bin",
                str(opts.test_bin)], capture_output=True, text=True,
                encoding="utf-8", check=False)
            if result.returncode:
                raise ValueError(f"{name} 真子 Gate 失敗：\n"+
                                 (result.stdout+result.stderr)[-3700:])
            print(f"P18.6 {name} 正式 Gate 重跑 {expected_count}/{expected_count} Passed")
        if len(seen) != 51:
            raise ValueError("P18.6 五義務總來源不足 51")
        print("P18.6 原 M1～M3、A1～A2 全 Closed，共 51/51 不重複 Rust 正反真執行，禁止提前結案其他 Phase")
        return 0
    except (OSError, ValueError, SyntaxError, KeyError, TypeError) as exc:
        print("P18.6 正式五義務閉合拒絕："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
