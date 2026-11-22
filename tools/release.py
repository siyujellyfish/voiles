"""從種子連建兩代，重製及驗證 Linux x86-64 預覽發行包。"""
import argparse
from hashlib import sha256
from io import BytesIO
import os
from pathlib import Path
import platform
import subprocess
import tarfile
from tempfile import TemporaryDirectory
from bootstrap import ROOT, build

VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
NAME = f"voiles-{VERSION}-linux-x86_64"


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, encoding="utf-8").strip()


def package(binary, source, toolchain, dest):
    members = {"voiles": binary.read_bytes(), "SOURCE": f"{source}\n".encode("utf-8"),
               "TOOLCHAIN": f"{toolchain}\n".encode("utf-8")}
    for path in ["release/使用說明.md", "release/變更紀錄.md", "src/Unicode-License.txt"]:
        members[Path(path).name] = (ROOT / path).read_bytes()
    for path in sorted((ROOT / "examples/入門").rglob("*.voil")):
        members[str(path.relative_to(ROOT))] = path.read_bytes()
    with tarfile.open(dest, "w:xz", format=tarfile.GNU_FORMAT) as archive:
        for name, data in sorted(members.items()):
            info = tarfile.TarInfo(f"{NAME}/{name}")
            info.mode = 0o755 if name == "voiles" else 0o644
            info.size = len(data)
            archive.addfile(info, BytesIO(data))


def validate(archive, work):
    with tarfile.open(archive, "r:xz") as packed:
        packed.extractall(work, filter="data")
    release = work / NAME
    binary = release / "voiles"
    elf = output("readelf", "-lWd", binary)
    if "INTERP" in elf or "(NEEDED)" in elf:
        raise RuntimeError("發行 CLI 含動態相依")
    if os.environ.get("GITHUB_ACTIONS"):
        isolate = ["sudo", "-n", "chroot", release]
    else:
        isolate = ["bwrap", "--ro-bind", release, "/", "--chdir", "/", "--"]
    def verify(args, code, stdout):
        proc = subprocess.run(args, cwd=release, capture_output=True, encoding="utf-8", timeout=60)
        if (proc.returncode, proc.stdout, proc.stderr) != (code, stdout, ""):
            raise RuntimeError(str(proc))
    verify([*isolate, "/voiles", "--version"], 0, f"voiles {VERSION}\n")
    verify([*isolate, "/voiles", "check", "examples/入門/基本加法.voil"], 0, "")
    verify([*isolate, "/voiles", "run", "examples/入門/多檔/main.voil"], 0, "42\n")
    for action in ["build", "pack"]:
        exe = work / action
        verify([binary, action, "examples/入門/基本加法.voil", "-o", exe], 0, "")
        verify([exe], 5, "")


def release(seed, directory):
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise RuntimeError("此發行目標需要 Linux x86-64")
    if output("git", "status", "--porcelain", "--untracked-files=normal"):
        raise RuntimeError("發行包須對應可追溯的來源提交，請使用乾淨 checkout")
    source = output("git", "rev-parse", "HEAD")
    toolchain = output("rustc", "-vV") + "\n" + output("cc", "--version").splitlines()[0] + "\n" + output("ld", "--version").splitlines()[0]
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        seed = seed or build(root / "seed")
        first = build(root / "stage1", seed, standalone=True)
        second = build(root / "stage2", first, standalone=True)
        a, b = root / "a.tar.xz", root / "b.tar.xz"
        package(first, source, toolchain, a)
        package(second, source, toolchain, b)
        digest = sha256(b.read_bytes()).hexdigest()
        if sha256(a.read_bytes()).hexdigest() != digest:
            raise RuntimeError("兩代發行包 SHA-256 不同")
        validate(b, root / "extracted")
        directory.mkdir(parents=True, exist_ok=False)
        name = NAME + ".tar.xz"
        (directory / name).write_bytes(b.read_bytes())
        (directory / "SHA256SUMS").write_text(f"{digest}  {name}\n", encoding="utf-8")
        print(f"{digest}  {name}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--seed", type=Path)
    args = parser.parse_args()
    release(args.seed.resolve(strict=True) if args.seed else None, args.directory)
