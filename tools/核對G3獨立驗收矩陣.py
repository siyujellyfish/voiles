#!/usr/bin/env python3
"""R03 G3 非空獨立驗收矩陣：F06／F07／F08／F09 子證據真 Rust 逐筆核對。
本工具刻意不宣告 G3 整包 Passed，因完整 CFG/動靜安全責任仍待簽核。
"""
from __future__ import annotations

import argparse
import ast
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
GATES = {
    "F06": ("核對R03B05第一批.py", "核對G3跨層票據第一批.py"),
    "F07": ("核對R03B06第一批.py", "核對G3參數來源第二批.py", "核對G3共同use-def第三批.py"),
    "F08": ("核對R03B05第二批.py", "核對R03B05第三批.py"),
    "F09": ("核對R03B06第二批.py",),
}
MINIMUM = {"F06": 12, "F07": 13, "F08": 7, "F09": 5}
REMAINING = [
    "G3：所有已核准 CFG 路徑的 HIR use-def 與退出責任尚待完整稽核",
    "G3：Owner、真 BorrowTicket、ResourceId 全操作 Transition 與錯誤回滾尚待整包簽核",
]

def run(cmd: list[str]) -> str:
    process = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if process.returncode:
        raise AssertionError(
            f"真實執行失敗（{process.returncode}）：{' '.join(cmd)}\n"
            + process.stdout[-2500:] + process.stderr[-2500:]
        )
    return process.stdout + process.stderr

def cases_for(script: Path) -> tuple[str, ...]:
    source = script.read_text(encoding="utf-8")
    parsed = ast.parse(source, filename=str(script))
    for node in parsed.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "CASES"
            for target in node.targets
        ):
            cases = ast.literal_eval(node.value)
            if not isinstance(cases, tuple) or not cases or not all(
                isinstance(name, str) and name for name in cases
            ):
                raise AssertionError(f"專屬測試 CASES 為空或非固定清單：{script.name}")
            return cases
    raise AssertionError(f"缺少正式獨立真 Rust 測試清單：{script.name}")

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須指定 --check 才能核對真 Rust")
    testbin = (ROOT / args.test_bin).resolve()
    if not testbin.is_file():
        raise AssertionError(f"缺少真正已編譯的 Rust 測試執行檔：{testbin}")
    listing = run([str(testbin), "--list"])
    available = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    per_group: dict[str, int] = {}
    for group, scripts in GATES.items():
        count = 0
        for script_name in scripts:
            script = ROOT / "tools" / script_name
            cases = cases_for(script)
            for suffix in cases:
                found = [name for name in available if name.endswith("::" + suffix)]
                if len(found) != 1 or found[0] in used:
                    raise AssertionError(f"{group} CASES 缺失、重複或非唯一：{suffix}")
                used.add(found[0])
                count += 1
            # 必須實際重新執行每個 gate 的所有 --exact，不可只讀 Python CASES。
            result = run([sys.executable, str(script), "--test-bin", str(testbin), "--check"])
            if "通過" not in result and "成功" not in result and "已驗證" not in result:
                raise AssertionError(f"未取得真執行證據：{script.name}")
        if count < MINIMUM[group]:
            raise AssertionError(f"{group} 真 Rust 獨立測試量不足：{count}")
        per_group[group] = count
    head = run(["git", "rev-parse", "HEAD"]).strip()
    ci_sha = os.environ.get("GITHUB_SHA", "")
    ref = os.environ.get("GITHUB_REF", "")
    if ci_sha and head != ci_sha:
        raise AssertionError("當前 checkout 與 GitHub SHA 不相符，不得寫入任何驗收證據")
    result = {
        "gate": "P16.6-R03-G3-SUBSET",
        "result": "VerifiedPartial",
        "github_sha": head,
        "github_ref": ref,
        "main_verified": ref == "refs/heads/main" and ci_sha == head,
        "groups": per_group,
        "unique_rust_tests": len(used),
        "outstanding": REMAINING,
        "r02_out_of_scope": True,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError, SyntaxError, ValueError) as exc:
        print(f"G3 獨立矩陣拒絕：{exc}", file=sys.stderr)
        sys.exit(1)
