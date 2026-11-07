#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／R07：嚴格驗收結果必須與原六項真實狀態一致，不把 --audit 當 --check。"""

from __future__ import annotations

import argparse
import ast
import subprocess
import sys
from pathlib import Path

稽核腳本 = Path(__file__).resolve().parent / "核對G18原六項驗收差距.py"


def 原六項狀態() -> dict[str, tuple[str, str]]:
    # AST 只讀取原稽核腳本字面值，不執行或重建任何 voiles 語意。
    source = 稽核腳本.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(稽核腳本))
    states = []
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "狀態"
            for target in node.targets
        ):
            states.append(ast.literal_eval(node.value))
    if len(states) != 1:
        raise ValueError("G18 必須只有一份原六項的正式狀態表")
    result = states[0]
    expected = {f"P18.{i}" for i in range(1, 7)}
    if not isinstance(result, dict) or set(result) != expected:
        raise ValueError("G18 原六項狀態身分不一致")
    for key, item in result.items():
        if (
            not isinstance(item, tuple) or len(item) != 2
            or item[0] not in {"Passed", "SubsetVerified", "Incomplete", "BlockedByDecision"}
            or not isinstance(item[1], str) or not item[1].strip()
        ):
            raise ValueError(f"{key} 狀態或剩餘驗收責任不完整")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()

    try:
        states = 原六項狀態()
    except (ValueError, SyntaxError) as err:
        print(f"G18 R07 狀態檢查失敗：{err}", file=sys.stderr)
        return 1

    # 唯有原始 --check 能驗證該次真 Rust 測試清單及執行結果。
    result = subprocess.run(
        [sys.executable, str(稽核腳本), "--test-bin", str(args.test_bin),
         "--check"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    printed = result.stdout + result.stderr
    passed = sum(1 for status, _ in states.values() if status == "Passed")
    complete = passed == 6
    expected_status = (
        "Passed" if complete
        else "BlockedByDecision" if any(
            state == "BlockedByDecision" for state, _ in states.values()
        ) else "Incomplete"
    )
    stamp = f"G18 正式驗收狀態＝{expected_status}；原項全 Passed {passed}/6"
    if stamp not in printed:
        print("G18 R07 真執行稽核結果與原六項狀態不一致：\n" + printed[-6000:],
              file=sys.stderr)
        return 1

    if complete:
        if result.returncode != 0:
            print("G18 原六項已標記全 Passed 但嚴格 --check 未成功：\n"
                  + printed[-6000:], file=sys.stderr)
            return 1
        print("G18 R07 嚴格 --check 已真正 Passed 6/6；須另核對同 SHA 主線驗收。")
        return 0

    # 尚未 Passed 時，拒絕是唯一合法的 --check 結果；
    # 其餘錯誤（測試缺漏、來源異動、腳本故障）不能假裝成預期拒絕。
    if result.returncode != 1 or (
        "G18 --check：原六項尚未全部 Passed，嚴禁標記正式結案。" not in printed
    ):
        print("G18 R07 未完成卻未由指定正式阻塞理由拒絕：\n"
              + printed[-6000:], file=sys.stderr)
        return 1
    print(f"G18 R07 安全阻塞狀態正確：{passed}/6 Passed；"
          "--audit 可核實真子集，正式 --check 仍正確拒絕；不提前結案。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
