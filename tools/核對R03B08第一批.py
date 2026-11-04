#!/usr/bin/env python3
"""R03 B08/F11 第一批：正式 Checker 完整函式 CFG 退出證據。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f11_b08_真正Checker為每個已驗證函式各自交付完整退出身份",
    "f11_b08_任一函式退出證據被移除必須拒絕整個已驗證計畫",
    "f11_b08_相同起點但錯誤函式終點不得冒充可達退出義務",
    "f11_b08_靜態未返回分類不可冒充完整函式已驗證退出",
    "f11_b08_非法函式不會交付任何成功安全退出計畫",
)

def run(command: list[str]) -> str:
    result = subprocess.run(command, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if result.returncode:
        raise AssertionError(f"Rust 真實執行失敗（{result.returncode}）: {' '.join(command)}\n"
            + result.stdout[-2200:] + result.stderr[-2200:])
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    source = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "struct VerifiedFunctionExit {",
        "recorded_function_exits: RefCell<BTreeMap<usize, VerifiedFunctionExit>>",
        "function_exits: BTreeMap<usize, VerifiedFunctionExit>",
        "plan.function_exits = checker.recorded_function_exits.into_inner()",
        "self.function_exits.len() == functions.len()",
        "exit.flow == FlowState::Returned",
    ):
        if source.count(marker) != 1:
            raise AssertionError(f"Checker 真實退出計畫單一權威缺失：{marker}")
    for marker in (
        "let flow = self.statements(&fun.body, &mut vars, &fun.result_ty, false)?;",
        "self.recorded_function_exits.borrow_mut()",
        "span: crate::common_ir::NodeSpan::new(fun.start, fun.end), flow,",
    ):
        if checker.count(marker) != 1:
            raise AssertionError(f"未能核對 Checker 來源安全退出：{marker}")
    listing = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen = set()
    for suffix in CASES:
        names = [name for name in available if name.endswith("::" + suffix)]
        if len(names) != 1 or names[0] in seen:
            raise AssertionError(f"真正 Rust 測試未唯一或被重複採計：{suffix}")
        name = names[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"Rust --exact 未真實成功：{name}")
    print(f"R03 B08/F11 第一批：{len(seen)} 筆互異 Rust --exact 通過；Checker 真函式退出身份閉合")
    print("F11 多路徑退出／清理義務、G4/G5/G6 仍待獨立驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"F11 第一批驗收失敗：{error}", file=sys.stderr)
        sys.exit(1)
