#!/usr/bin/env python3
"""R03 G3 第二批：參數 move 不得由同名字串掃描冒充來源；每項真 Rust --exact。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g3_同名內層字串移動不得冒充外層參數已真正消耗",
    "g3_真正參數來源的整筆移動仍必須保持已消耗",
    "g3_確定零次迴圈內的來源移動不能當成已執行參數移交",
    "g3_同一函式中先遮蔽再解除遮蔽後的外層移動來源必須正確",
)

def run(cmd: list[str]) -> str:
    proc = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if proc.returncode:
        raise AssertionError(
            f"Rust 真測試回傳 {proc.returncode}: {' '.join(cmd)}\n"
            + proc.stdout[-1800:] + proc.stderr[-1800:]
        )
    return proc.stdout + proc.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須使用 --check")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    checks = (
        "enum ResolvedUseKind {",
        "fn scan_resolved_term_uses(",
        "fn scan_resolved_block_uses(",
        "fn parameter_is_consumed(",
        "Self::scan_resolved_block_uses(",
        "ResolvedUseKind::WholeMove",
        "visible.remove(&declaration.name)",
        "Self::known_range_iterations(lower, upper) != Some(0)",
    )
    for marker in checks:
        if checker.count(marker) < 1:
            raise AssertionError(f"G3 唯一詞彙來源 use-def 守衛缺失：{marker}")
    for old in (
        "fn statements_move_name(", "fn term_moves_name(",
        "fn term_moves_source(", "fn block_moves_source(",
        "fn scan_future_term_uses(",
    ):
        if old in checker:
            raise AssertionError(f"正式 Checker 仍存在獨立同名 move/使用點掃描權威：{old}")
    listing = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        matches = [name for name in available if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in seen:
            raise AssertionError(f"Rust 正反來源非空、唯一檢查失敗：{suffix}")
        name = matches[0]
        seen.add(name)
        observed = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in observed:
            raise AssertionError(f"真實 --exact 未通過：{name}")
    print(f"G3 參數詞彙來源第二批：{len(seen)} 個互異真 Rust --exact 全通過")
    print("仍不得視為完整 G3／R03 總驗收；需其餘 CFG/票據及安全計畫證據")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as err:
        print(f"G3 詞彙參數來源失敗：{err}", file=sys.stderr)
        sys.exit(1)
