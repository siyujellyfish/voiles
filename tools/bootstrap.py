"""以 Rust 種子或上一代 voiles 編出下一代工具鏈。每代使用獨立的新目錄。"""
import argparse
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]


def build(output, previous=None, *, standalone=False):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    try:
        with TemporaryDirectory(dir=output) as scratch:
            staging = Path(scratch)
            compile_stage(staging, previous, standalone)
            for name in ["digits.rs", "compiler.rs", "libvoiles_core.rlib", "voiles"]:
                if (staging / name).exists():
                    (staging / name).rename(output / name)
    except BaseException:
        # 目錄由本次操作獨占建立；只清理由本次建置發布的產物。
        for name in ["digits.rs", "compiler.rs", "libvoiles_core.rlib", "voiles"]:
            (output / name).unlink(missing_ok=True)
        output.rmdir()
        raise
    return output / "voiles"


def compile_stage(output, previous, standalone):
    flags = ["rustc", "--edition=2021", "-C", "strip=debuginfo",
             "--remap-path-prefix", f"{ROOT}=/voiles",
             "--remap-path-prefix", f"{output}=/voiles-stage"]
    if standalone:
        flags.extend(["-C", "target-feature=+crt-static", "--target", "x86_64-unknown-linux-gnu"])
    env = os.environ.copy()
    kernel = []
    if previous:
        compiler_source = output / "compiler.rs"
        compiler_input = ROOT / "bootstrap/compiler.voil"
        rows = subprocess.check_output([previous, "run", ROOT / "bootstrap/lexer/main.voil",
                                        "--", compiler_input], cwd=ROOT, encoding="utf-8")
        stream, status = rows.rsplit("\n", 2)[:2]
        if status != "0":
            raise ValueError("自舉掃描器未成功返回")
        token_file = output / "compiler.tokens"
        token_file.write_text(stream + "\n", encoding="utf-8")
        compiled = subprocess.check_output([previous, "bootstrap-compiler", compiler_input,
                                            "--tokens", token_file],
                                            cwd=ROOT, encoding="utf-8")
        compiler_source.write_text(compiled, encoding="utf-8")
        env["VOILES_COMPILER_RS"] = str(compiler_source)
        generated = subprocess.check_output([previous, "bootstrap-digits", ROOT / "bootstrap/digits.voil"],
                                            cwd=ROOT, encoding="utf-8")
        source = output / "digits.rs"
        source.write_text(generated, encoding="utf-8")
        kernel = ["--cfg", "voiles_bootstrap"]
        env["VOILES_DIGITS_RS"] = str(source)
    lib = output / "libvoiles_core.rlib"
    subprocess.run([*flags, "--crate-name", "voiles_core", "--crate-type", "rlib", *kernel,
                    "src/lib.rs", "-o", lib], cwd=ROOT, env=env, check=True)
    cli = output / "voiles"
    subprocess.run([*flags, "--crate-name", "voiles", "--extern", f"voiles_core={lib}",
                    "src/main.rs", "-o", cli], cwd=ROOT, check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--previous", type=Path)
    args = parser.parse_args()
    build(args.output, args.previous.resolve(strict=True) if args.previous else None)
