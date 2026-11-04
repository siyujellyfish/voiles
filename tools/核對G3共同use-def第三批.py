#!/usr/bin/env python3
"""R03 G3 第三批：後繼全引用與參數整筆移動使用相同詞彙 DeclarationId 掃描。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g3_正常參數純讀僅算後繼使用不算整筆移動",
    "g3_真正參數移動同時出現在唯一全引用與移動來源集合",
    "g3_內層同名遮蔽移動不可出現在外層參數的任何來源集合",
    "g3_確定零次for之不可達整筆移動不算任何後繼參數來源",
)

def run(cmd: list[str]) -> str:
    proc = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if proc.returncode:
        raise AssertionError(
            f"真正 Rust --exact 失敗 {proc.returncode}: {' '.join(cmd)}\n"
            + proc.stdout[-1500:] + proc.stderr[-1500:]
        )
    return proc.stdout + proc.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "enum ResolvedUseKind {",
        "fn scan_resolved_term_uses(",
        "fn scan_resolved_block_uses(",
        "fn parameter_is_consumed(",
        "ResolvedUseKind::WholeMove",
        "ResolvedUseKind::All",
        "visible.remove(&declaration.name)",
    ):
        if checker.count(marker) < 1:
            raise AssertionError(f"缺少唯一詞彙 use-def 核心：{marker}")
    if checker.count("fn scan_resolved_block_uses(") != 1:
        raise AssertionError("同一詞彙 use-def 被定義成多個正式可寫來源")
    if "fn block_moves_source(" in checker or "fn term_moves_source(" in checker:
        raise AssertionError("仍保留獨立解析 move 同名的第二份權威")
    listed = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    for suffix in CASES:
        matches = [name for name in names if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in used:
            raise AssertionError(f"非空真 Rust 個別案例不唯一：{suffix}")
        name = matches[0]
        used.add(name)
        output = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"真 Rust --exact 不成功：{name}")
    print(f"G3 共同 use-def 第三批：{len(used)} 筆互異真 Rust --exact 通過")
    print("仍需 G3 其它動靜責任全矩陣才可正式驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as err:
        print(f"G3 共同 use-def 第三批失敗：{err}", file=sys.stderr)
        sys.exit(1)
