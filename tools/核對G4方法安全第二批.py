#!/usr/bin/env python3
"""R03 G4 第二批：受限方法 self 借用模式及逐 return 真 Checker 安全來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g4_唯讀方法Checker直接簽出共享self與精確return來源",
    "g4_合法可寫方法Checker應獨立記錄獨占self借用",
    "g4_方法提前return與最終return必須各有真Checker位置",
    "g4_假造方法唯讀self或把來源改成別的Span都拒絕",
    "g4_刪除方法最後return或增加虛構退出證據必定拒絕",
)

def run(cmd: list[str]) -> str:
    process = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if process.returncode:
        raise AssertionError(
            f"Rust 真來源失敗 {process.returncode}: {' '.join(cmd)}\n"
            + process.stdout[-1700:] + process.stderr[-1700:]
        )
    return process.stdout + process.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須指定 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for source, marker in (
        (core, "struct VerifiedMethodSafety {"),
        (core, "method_safety: BTreeMap<usize, VerifiedMethodSafety>"),
        (core, "fn method_safety_is_consistent("),
        (core, "fn method_requires_write("),
        (core, "plan.method_safety = checker.recorded_method_safety.into_inner()"),
        (checker, "recorded_method_safety.borrow_mut()"),
        (checker, "checked_returns.insert(statement.node_span())"),
        (checker, "checked_returns.insert(method.body.last()"),
    ):
        if source.count(marker) != 1:
            raise AssertionError(f"方法安全來源不存在或重複可寫：{marker}")
    if core.count("&& self.method_safety_is_consistent(structs, text)") != 1:
        raise AssertionError("VerifiedSafetyPlan 未消費唯一受限方法來源")
    listed = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    for suffix in CASES:
        matches = [name for name in names if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in used:
            raise AssertionError(f"真 Rust 方法案例缺少或重複：{suffix}")
        name = matches[0]
        used.add(name)
        response = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in response:
            raise AssertionError(f"Rust --exact 並未通過：{name}")
    print(f"G4 方法安全第二批：{len(used)} 個互異真 Rust 正反 --exact 成功")
    print("本批不代表 G4 全 CFG 逐出口清理或 G5/G6 已驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G4 方法第二批失敗：{error}", file=sys.stderr)
        sys.exit(1)
