from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from bootstrap import build

seed = Path(sys.argv[1]).resolve(strict=True)
with TemporaryDirectory() as tmp:
    root = Path(tmp)
    stage = build(root / "stage1", seed)
    source = root / "main.voil"
    for literal in ["0", "42", "0b1011", "0o735", "0x7fff_ffff_ffff_ffff",
                    "9223372036854775807", "9223372036854775808", "0b102", "0x"]:
        source.write_text(f"fn main()->int{{ return {literal} }}", encoding="utf-8")
        def result(cli):
            p = subprocess.run([cli, "run", source], capture_output=True, encoding="utf-8", check=False)
            return p.returncode, p.stdout, p.stderr
        reference = result(seed)
        assert result(stage) == reference, (literal, reference, result(stage))
        if literal in ["9223372036854775808", "0b102", "0x"]:
            assert reference[0] == 1 and reference[2], reference
        else:
            assert reference == (0, f"{int(literal, 0)}\n", ""), reference
