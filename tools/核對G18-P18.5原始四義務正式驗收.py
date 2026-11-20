"""P18.5 四原義務與完整依賴回歸。"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / "AI-build/G18-P18-V0-原六項義務清冊-20261108.json"
GATES = (
    ("T1", "tools/核對G18-P18.5-T1六類來源四欄.py"),
    ("T2", "tools/核對G18-P18.5-T2六組跨功能正反.py"),
    ("T3", "tools/核對G18-P18.5-T3同源防偽與原子拒絕.py"),
    ("T4", "tools/核對G18-P18.5-T4同版本完整回歸.py"),
)

def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--test-bin", type=Path, required=True)
    args.add_argument("--check", required=True, action="store_true")
    opts = args.parse_args()
    binary = str(opts.test_bin.resolve(strict=True))
    registry = json.loads(REGISTER.read_text(encoding="utf-8"))
    rows = [item for item in registry["obligations"] if item["phase"]=="P18.5"]
    if (registry["current_status"]["P18.5"] != "Passed" or len(rows) != len(GATES)
            or any(item["status"] != "Closed" or item["id"] != name
                   or item["closure_proof"]["gate"] != path for item,(name,path) in zip(rows,GATES))):
        raise SystemExit("P18.5 原始義務或獨立 Gate 不符")
    for name, path in GATES:
        subprocess.run([sys.executable, "-X", "utf8", str(ROOT/path), "--test-bin", binary], check=True)
        print(f"{name} 通過", flush=True)
    print("P18.5 四原義務驗收成功")

if __name__=="__main__":
    main()
