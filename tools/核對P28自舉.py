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


    # 第一代重新編譯自身的 voiles 編譯器及數字核心，兩代比較完整可執行檔。
    next_stage = build(root / "stage2", stage)
    for filename in ["voiles", "compiler.rs", "digits.rs"]:
        assert (stage.parent / filename).read_bytes() == (next_stage.parent / filename).read_bytes(), filename
    repo = Path(__file__).resolve().parents[1]
    assert (stage.parent / "compiler.rs").read_bytes() == (repo / "bootstrap/compiler_seed.inc").read_bytes()

    def invoke(cli, *args):
        p = subprocess.run([cli, *args], capture_output=True, encoding="utf-8", timeout=30)
        return p.returncode, p.stdout, p.stderr

    # 修改編譯器自身的一個合法發射動作，必須反映在兩代的輸出。
    compiler = repo / "bootstrap/compiler.voil"
    modified = root / "compiler.voil"
    modified.write_text(compiler.read_text(encoding="utf-8").replace('host.emit("fn ")',
                        'host.emit("/*probe*/fn ")'), encoding="utf-8")
    for cli in [seed, stage, next_stage]:
        code, generated, error = invoke(cli, "bootstrap-compiler", modified)
        assert code == 0 and "/*probe*/fn " in generated and not error
    cases = [
        ("fn main()->int{ return true }", "check", 1),
        ("fn main()->int{ return 9223372036854775807 + 1 }", "run", 1),
        ('share s:str = "A"\nfn main()->int{\nconst a:str = s.borrow()\ns = "B"\nreturn a[0][0]\n}', "check", 1),
        ("import host\nfn main()->int{ return host.unknown() }", "bootstrap-compiler", 1),
        ("import host\nfn main()->int{ return 3 + 4 }", "bootstrap-compiler", 0),
    ]
    for raw, mode, expected in cases:
        source.write_text(raw, encoding="utf-8")
        actual = [invoke(cli, mode, source) for cli in [seed, stage, next_stage]]
        assert actual[0] == actual[1] == actual[2] and actual[0][0] == expected, actual
    multi = repo / "examples/入門/多檔/main.voil"
    for idx, cli in enumerate([seed, stage, next_stage]):
        assert invoke(cli, "run", multi) == (0, "42\n", "")
        for action in ["build", "pack"]:
            exe = root / f"{action}-{idx}"
            assert invoke(cli, action, multi, "-o", exe)[0] == 0
            assert invoke(exe) == (42, "", "")

    # 改變 voiles 掃描器的合法分類，真實的自舉解析結果必須跟著改變。
    scanner_dir = repo / "bootstrap/lexer"
    scanner = (scanner_dir / "scanner.voil").read_text(encoding="utf-8")
    original = "if(keyword(key)){ kind = 1 } else { kind = 2 }"
    assert scanner.count(original) == 1
    (root / "scanner.voil").write_text(scanner.replace(original, "kind = 1"), encoding="utf-8")
    entry = root / "lexer.voil"
    entry.write_text((scanner_dir / "main.voil").read_text(encoding="utf-8"), encoding="utf-8")
    for cli in (stage, next_stage):
        tokens = root / "compiler.tokens"
        for lexer, success in ((scanner_dir / "main.voil", True), (entry, False)):
            code, rows, error = invoke(cli, "run", lexer, "--", compiler)
            assert code == 0 and not error and rows.endswith("\n0\n"), (code, error)
            tokens.write_text(rows[:-2], encoding="utf-8")
            code, generated, error = invoke(cli, "bootstrap-compiler", compiler, "--tokens", tokens)
            if success:
                assert code == 0 and not error and generated == (repo / "bootstrap/compiler_seed.inc").read_text(encoding="utf-8")
            else:
                assert code == 1 and not generated and "預期函式名稱" in error, (code, error)
