#!/usr/bin/env python3
"""R03 B06/F09 第一批：真正 SafetyState 靜態安全轉移與失敗原子性。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f09_b06_錯誤首次欄位及移動都沒有半提交",
    "f09_b06_部分欄位單一轉移最後一欄才恢復完整可讀",
    "f09_b06_合法整筆移動與重新綁定恢復同一安全狀態",
    "f09_b06_非參數別名可以結束但參數自身借用不能提前結束",
    "f09_b06_靜態借用結束絕不可改變真資源或票據的所有權身分",
)

def run(cmd: list[str]) -> str:
    result = subprocess.run(cmd, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if result.returncode:
        raise AssertionError(
            f"真 Rust --exact 失敗：{' '.join(cmd)}\n"
            + result.stdout[-2000:] + result.stderr[-2000:]
        )
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("真來源驗收必須指定 --check")
    code = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "enum StaticTransition<",
        "impl SafetyState {",
        "fn transition(&mut self, action: StaticTransition<",
        "StaticTransition::MoveWhole =>",
        "StaticTransition::CommitRebindChecked =>",
        "StaticTransition::InitializeField(field) =>",
        "StaticTransition::ExpireAliasChecked =>",
        "self.safety.transition(StaticTransition::MoveWhole)",
        "self.safety.transition(StaticTransition::InitializeField(field))",
        "fn expire_alias_checked(&mut self)",
    ):
        if code.count(marker) != 1:
            raise AssertionError(f"F09 靜態轉移權威不是唯一：{marker}")
    if checker.count("binding.expire_alias_checked()") != 1:
        raise AssertionError("正式 Checker 最後使用點沒有使用唯一 Transition")
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen = set()
    for suffix in CASES:
        found = [name for name in names if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in seen:
            raise AssertionError(f"非空／唯一真 Rust 測試失敗：{suffix}")
        name = found[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"--exact 真實執行失敗：{name}")
    print(f"R03 B06/F09 靜態轉移第一批：{len(seen)} 個互異真 Rust --exact 與唯一來源通過")
    print("真實雙帳簿全操作及 CFG 固定點仍須 G3 獨立驗收；不能提前結案")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F09 第一批驗收失敗：{exc}", file=sys.stderr)
        sys.exit(1)
