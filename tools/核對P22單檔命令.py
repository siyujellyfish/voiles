"""透過公開 CLI 與實體檔案核對 P22 單檔檢查相容性。"""
import argparse
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    cli = parser.parse_args().cli.resolve(strict=True)
    example = Path(__file__).resolve().parents[1] / "examples/入門/基本加法.voil"
    with TemporaryDirectory() as directory:
        path = Path(directory) / "含空白 中文.voil"
        # 範例 main 回傳 5；靜態檢查成功的退出碼仍為 0。
        path.write_text(example.read_text(encoding="utf-8"), encoding="utf-8")
        result = subprocess.run([cli, path.name], cwd=directory, capture_output=True)
        if (result.returncode, result.stdout, result.stderr) != (0, b"", b""):
            raise SystemExit(f"入門範例靜態檢查失敗 {result}")
        path.write_text("fn main()->int{\n    return true\n}\n", encoding="utf-8")
        result = subprocess.run([cli, path.name], cwd=directory, capture_output=True,
                                encoding="utf-8")
        prefix = f"{path.name}:2:12: 靜態錯誤:"
        if result.returncode != 1 or result.stdout or not result.stderr.startswith(prefix):
            raise SystemExit(f"路徑、診斷位置或退出狀態不符 {result}")
        def verify(args, code, stdout="", error=None):
            result = subprocess.run([cli, *args], cwd=directory, capture_output=True,
                                    encoding="utf-8")
            if (result.returncode != code or result.stdout != stdout
                    or (error not in result.stderr if error else bool(result.stderr))):
                raise SystemExit(f"命令契約不符 {args} {result}")

        multi = example.parent / "多檔/main.voil"
        for entry, value in [(example, 5), (multi, 42)]:
            verify(["check", entry], 0)
            verify(["run", entry], 0, f"{value}\n")
        verify(["--version"], 0, "voiles P22\n")
        help_result = subprocess.run([cli, "--help"], cwd=directory, capture_output=True,
                                     encoding="utf-8")
        if (help_result.returncode or help_result.stderr or
                any(label not in help_result.stdout for label in ["check", "run", "--version", "--help"])):
            raise SystemExit(f"說明不符 {help_result}")
        for args in [["check"], ["run", path, path], ["--version", "extra"], ["--unknown"]]:
            verify(args, 2, error="用法")
        verify(["run", "bad.txt"], 2, error="副檔名")
        verify(["run", "missing.voil"], 1, error="檔案讀取錯誤")
        verify(["run", path.name], 1, error=path.name)
        path.write_text("fn main()->int{\nreturn 0 - 300\n}\n", encoding="utf-8")
        verify(["run", path.name], 0, "-300\n")
        path.write_text("fn main()->int{\nreturn 9223372036854775807 + 1\n}\n", encoding="utf-8")
        verify(["check", path.name], 0)
        verify(["run", path.name], 1, error="執行期錯誤")
        path.write_text("import missing\nfn main()->int{\nreturn 0\n}\n", encoding="utf-8")
        verify(["check", path.name], 1, error=f"{path.name}:1:1: 匯入失敗")
        path.write_text("fn main()->int{\nreturn 9223372036854775807\n}\n", encoding="utf-8")
        verify(["run", path.name], 0, "9223372036854775807\n")
        dep = Path(directory) / "shared.voil"
        dep.write_text('share message:str = "A"\n', encoding="utf-8")
        path.write_text("import shared\nfn main()->int{\nreturn shared.message[0][0]\n}\n",
                        encoding="utf-8")
        verify(["run", path.name], 0, "65\n")
        path.write_text('import message from shared as msg\nfn main()->int{\nmsg = "B"\nreturn 0\n}\n',
                        encoding="utf-8")
        for mode in ["check", "run"]:
            verify([mode, path.name], 1, error="靜態錯誤")
    print("P22 公開 CLI 單檔、多檔、靜態與動態失敗、退出碼及舊命令回歸通過")


if __name__ == "__main__":
    main()
