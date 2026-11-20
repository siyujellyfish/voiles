"""逐項執行 P18 測試矩陣，防止名稱缺漏、重複與空群。"""
import argparse
from pathlib import Path
import re
import subprocess


def verify(rows, prefix=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("--test-bin", type=Path, required=True)
    binary = str(parser.parse_args().test_bin.resolve(strict=True))
    listing = subprocess.check_output([binary, "--list"], encoding="utf-8")
    available = {line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")}
    names = [name for _, cases in rows for name in cases]
    if not names or any(not cases for _, cases in rows) or len(set(names)) != len(names):
        raise SystemExit("矩陣有重複或空白來源")
    if not set(names) <= available or (prefix and set(names) != {n for n in available if n.startswith(prefix)}):
        raise SystemExit("Rust 名稱清單與矩陣不符")
    for label, cases in rows:
        for case in cases:
            output = subprocess.check_output([binary, "--exact", case, "--test-threads=1"], encoding="utf-8")
            if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", output):
                raise SystemExit(f"{case} 缺少單一真實通過結果")
        print(f"{label} 通過 {len(cases)} 項")
