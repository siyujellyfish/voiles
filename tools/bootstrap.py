"""以 Rust 種子或上一代 voiles 編出下一代工具鏈。每代使用獨立的新目錄。"""
import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def build(output, previous=None):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    flags = ["rustc", "--edition=2021"]
    env = os.environ.copy()
    kernel = []
    if previous:
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
    return cli


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--previous", type=Path)
    args = parser.parse_args()
    build(args.output, args.previous.resolve(strict=True) if args.previous else None)
