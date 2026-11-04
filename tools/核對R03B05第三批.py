#!/usr/bin/env python3
"""R03 B05／F08 第三批：真實 Checker 單一可寫 SafetyState 物理來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f08_b05_真正Binding值與借用活性只有SafetyState一份可寫儲存",
    "f08_b05_不同Cfg快照的SafetyState寫時複製不回寫入口來源",
    "f08_b05_未初始化整筆移動失敗時SafetyState的欄位與借用均不變",
)

def run(command: list[str]) -> str:
    proc = subprocess.run(
        command, text=True, capture_output=True,
        encoding="utf-8", errors="replace", check=False,
    )
    if proc.returncode:
        raise AssertionError(
            f"真 Rust 執行失敗（{proc.returncode}）\n"
            + proc.stdout[-1800:] + proc.stderr[-1800:]
        )
    return proc.stdout + proc.stderr

def verify_single_storage() -> None:
    src = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    patterns = (
        "struct SafetyState {",
        "struct Binding {",
        "impl std::ops::Deref for Binding {",
        "impl std::ops::DerefMut for Binding {",
        "fn new(\n        declaration_start: DeclarationId",
        "safety: SafetyState { value, pending_fields, loan }",
    )
    for marker in patterns:
        if src.count(marker) != 1:
            raise AssertionError(f"單一靜態狀態物理權威不存在或重複：{marker}")
    m = re.search(r"struct Binding\s*\{([^{}]+)\}", src)
    if m is None:
        raise AssertionError("缺少真正式 Binding")
    for old_field in ("value:", "pending_fields:", "loan:"):
        if old_field in m.group(1):
            raise AssertionError(f"Binding 仍另行持有第二份可寫安全欄位：{old_field}")
    if "safety: SafetyState" not in m.group(1):
        raise AssertionError("真正 Binding 不是只有一份 SafetyState")
    for path in (
        "src/struct_stage/safety_checker.rs",
        "src/struct_stage/tests_ownership.rs",
        "src/struct_stage/tests_lifetime_resources.rs",
    ):
        txt = (ROOT / path).read_text(encoding="utf-8")
        if txt.count("Binding::new(") < 3:
            raise AssertionError(f"正式 Checker／測試仍有舊寫入構造：{path}")
    print("F08 單一物理狀態：ValueSlot、pending_fields、loan 均由唯一 SafetyState 實例維護")

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    verify_single_storage()
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        found = [name for name in names if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in seen:
            raise AssertionError(f"真正 Rust 案例缺失、重複或非唯一：{suffix}")
        name = found[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"Rust --exact 未實際通過：{name}")
    print(f"R03 B05/F08 第三批：{len(seen)} 筆真 Rust --exact 與狀態物理單一權威已驗證")
    print("F08 整包／G3 尚待 F07/F09/F10 全操作安全轉移與 CFG 整合，不能提前簽核")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F08 第三批失敗：{exc}", file=sys.stderr)
        sys.exit(1)
