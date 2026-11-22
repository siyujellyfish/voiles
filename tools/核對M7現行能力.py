"""以現行 CLI 重跑 P31 原始案例，並獨立核對凍結的舊單檔入口。"""
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

from m7_public_cases import verify


cli = Path(sys.argv[1]).resolve(strict=True)
matrix = Path(__file__).resolve().parents[1] / "tests" / "m7" / "preview1.json"
cases = json.loads(matrix.read_text(encoding="utf-8"))
updated = {
    "借用衝突": "reject",
    "回傳型別錯誤": "reject",
    "Optional元素": 2,
    "字串插值": 0,
    "例外": "reject",  # 原始案例缺少具名 except，合法形式另有四入口測試。
    "命令列與檔案": 0,
}
with TemporaryDirectory() as directory:
    path = Path(directory) / "main.voil"
    for case in cases:
        path.write_text(case["source"], encoding="utf-8")
        result = subprocess.run([cli, path], encoding="utf-8", capture_output=True, timeout=30)
        expected = case["legacy"]
        assert result.returncode == (0 if expected else 1), (case["id"],result)
        assert not result.stdout and bool(result.stderr) != expected, (case["id"],result)
        value = updated.get(case["id"], case["value"])
        assert value is not None, case["id"]
        verify(cli, [(case["source"],value)])
        print(case["id"], "通過")
