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

    # 自舉解析器真正載入相依模組、解析限定呼叫並產生可連結的模組身分名稱。
    rust_source = root / "expression.rs"
    native = root / "expression"
    project = root / "project"
    project.mkdir()
    module = project / "math.voil"
    entry_module = project / "main.voil"
    module.write_text("fn combine(a:int,b:int)->int{ return a*10+b }\n", encoding="utf-8")
    entry_module.write_text("import math\nfn main()->int{ return math.combine(4,2) }\n", encoding="utf-8")
    generated_projects = [invoke(cli, "bootstrap-project", entry_module) for cli in (seed, stage, next_stage)]
    assert generated_projects[0] == generated_projects[1] == generated_projects[2]
    code, generated, error = generated_projects[2]
    assert code == 0 and not error and "v_math_combine" in generated and "v_main_main" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Limit} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    assert invoke(next_stage, "check", entry_module) == (0, "", "")
    assert invoke(next_stage, "run", entry_module) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"module-{mode}"
        assert invoke(next_stage, mode, entry_module, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_projects = (
        ("import missing\nfn main()->int{ return 0 }", None),
        ("import math\nfn main()->int{ return math.unknown() }", None),
        ("import math\nfn main()->int{ return math.combine(1) }", None),
        ("import math\nfn main()->int{ return math._hidden() }", "fn _hidden()->int{ return 1 }\n"),
    )
    for entry_text, module_text in invalid_projects:
        entry_module.write_text(entry_text, encoding="utf-8")
        if module_text is not None:
            module.write_text(module_text, encoding="utf-8")
        elif "missing" not in entry_text:
            module.write_text("fn combine(a:int,b:int)->int{ return a*10+b }\n", encoding="utf-8")
        results = [invoke(cli, "bootstrap-project", entry_module) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        assert invoke(next_stage, "check", entry_module)[0] == 1
    entry_module.write_text("import math\nfn main()->int{ return 0 }", encoding="utf-8")
    module.write_text("import main\nfn value()->int{ return 1 }", encoding="utf-8")
    results = [invoke(cli, "bootstrap-project", entry_module) for cli in (seed, stage, next_stage)]
    assert all(code == 1 and "模組循環" in diagnostic for code, output, diagnostic in results), results

    # 改變 voiles 掃描器的合法分類，真實的自舉解析結果必須跟著改變。
    scanner_dir = repo / "bootstrap/lexer"
    scanner = (scanner_dir / "scanner.voil").read_text(encoding="utf-8")
    original = "if(key > 0 and keyword(key)){ kind = 1 } else { kind = 2 }"
    assert scanner.count(original) == 1
    (root / "scanner.voil").write_text(scanner.replace(original, "kind = 1"), encoding="utf-8")
    entry = root / "lexer.voil"
    entry.write_text((scanner_dir / "main.voil").read_text(encoding="utf-8"), encoding="utf-8")
    for cli in (stage, next_stage):
        tokens = root / "compiler.tokens"
        for lexer, success in ((scanner_dir / "main.voil", True), (entry, False)):
            code, rows, error = invoke(cli, "bootstrap-run", lexer, "--", compiler, "--compact")
            assert code == 0 and not error and rows.endswith("\n0\n"), (code, error)
            tokens.write_text(rows[:-2], encoding="utf-8")
            code, generated, error = invoke(cli, "bootstrap-compiler", compiler, "--tokens", tokens)
            if success:
                assert code == 0 and not error and generated == (repo / "bootstrap/compiler_seed.inc").read_text(encoding="utf-8")
            else:
                assert code == 1 and not generated and "預期函式名稱" in error, (code, error)

    # 編譯並執行 voiles 產生的 Rust，與公開四入口交叉核對。
    public = root / "public.voil"
    rust_source = root / "expression.rs"
    native = root / "expression"
    for expr, expected in [("2 + 5 * 8", 42), ("(2 + 5) * 6", 42),
                           ("95 - 8 - 45", 42), ("252 / 2 / 3", 42),
                           ("(0 - 7) / 3 + 44", 42), ("1 / 0", None),
                           ("9223372036854775807 * 2", None)]:
        body = f"fn main()->int{{ return {expr} }}"
        public.write_text(body, encoding="utf-8")
        source.write_text("import host\n" + body, encoding="utf-8")
        result = invoke(next_stage, "bootstrap-compiler", source)
        assert result[0] == 0 and not result[2], result
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + result[1] + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        status = 1 if expected is None else expected
        assert invoke(native) == (status, "", "")
        assert invoke(next_stage, "check", public) == (0, "", "")
        interpreted = invoke(next_stage, "run", public)
        if expected is None:
            assert interpreted[0] == 1 and "運算失敗" in interpreted[2], interpreted
        else:
            assert interpreted == (0, f"{expected}\n", ""), interpreted
        for mode in ("build", "pack"):
            exe = root / f"public-{mode}"
            assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
            execution = invoke(exe)
            assert execution == (status, "", interpreted[2]), execution
            exe.unlink()
    for expr in ("1 + )", "(2 + 3", "4 * / 2"):
        source.write_text("import host\nfn main()->int{ return " + expr + " }", encoding="utf-8")
        for cli in (seed, stage, next_stage):
            code, generated, diagnostic = invoke(cli, "bootstrap-compiler", source)
            assert code == 1 and not generated and diagnostic, (expr, code, diagnostic)

    # 用可觀察的游標前進，核對迴圈真實次數及每次回邊的預算檢查。
    loop = ("import host\nfn main()->int{ while(host.at(\"x\") == 1){ host.expect(\"x\") } "
            "if(host.at_end() >= 1){ return 42 } else { return 0 } }")
    source.write_text(loop, encoding="utf-8")
    code, generated, error = invoke(next_stage, "bootstrap-compiler", source)
    assert (code, error) == (0, ""), error
    harness = """
type R=Result<i64,Fault>; enum Fault{Overflow}
struct Machine{items:i64,steps:i64}
impl Machine{
    fn enter(&mut self,_:usize)->Result<(),Fault>{self.steps+=1; Ok(())}
    fn at(&self,_:&str)->R{Ok(i64::from(self.items>0))}
    fn expect(&mut self,_:&str)->R{self.items-=1;Ok(0)}
    fn at_end(&self)->R{Ok(i64::from(self.items==0))}
}
fn main(){
    for n in [0,3]{
        let mut s=Machine{items:n,steps:0};
        assert_eq!(v_main(&mut s,0).ok(),Some(42));
        assert_eq!((s.items,s.steps),(0,n+1));
    }
}
"""
    rust_source.write_text(harness + generated, encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (0, "", "")

    for op, ok in [("!=",True),("==",True),("<",True),(">",True),
                   ("<=",True),(">=",True),("! =",False),("< =",False)]:
        body = f"fn main()->int{{ if(1 {op} 2){{ return 42 }} else {{ return 0 }} }}"
        public.write_text(body, encoding="utf-8")
        source.write_text("import host\n" + body, encoding="utf-8")
        generated = invoke(next_stage, "bootstrap-compiler", source)
        assert (generated[0] == 0) == ok, (op, generated)
        assert (invoke(next_stage, "check", public)[0] == 0) == ok, op

    # int 參數、參數值及多實參呼叫由自舉編譯器產生真 Rust，再與公開四入口比較。
    # main 刻意先於 combine，驗證名稱階段完成整份宣告後才核對前向呼叫。
    functions = "fn main()->int{ return combine(4,2) }\nfn combine(a:int,b:int)->int{ return a*10+b }"
    source.write_text("import host\n" + functions, encoding="utf-8")
    code, generated, error = invoke(next_stage, "bootstrap-compiler", source)
    assert (code, error) == (0, ""), error
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(functions, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"function-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    # 具 int 預設欄位的結構由自舉來源宣告、空建立並讀取，生成真 Rust 結構。
    structures = ("struct Box{\nvar value:int = 40\nvar extra:int = 2\n}\n"
                  "fn main()->int{\nvar b:Box = Box(extra=1,value=39,)\n"
                  "b.value = b.value+1\nb.extra = b.extra+1\nreturn b.value+b.extra\n}")
    source.write_text("import host\n" + structures, encoding="utf-8")
    generated_structs = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_structs[0] == generated_structs[1] == generated_structs[2]
    code, generated, error = generated_structs[2]
    assert code == 0 and not error and "struct S_Box" in generated and "v_b.f_value = " in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(structures, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"struct-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_structures = (
        "struct Box{ var value:int } fn main()->int{ return 0 }",
        "struct Box{ var value:int = 1 var value:int = 2 } fn main()->int{ return 0 }",
        "struct Box{ var value:bool = 1 } fn main()->int{ return 0 }",
        "fn main()->int{ var b:Missing = Missing() return 0 }",
        "struct Box{ var value:int = 1 } struct Other{ var value:int = 2 } fn main()->int{ var b:Box = Other() return 0 }",
        "struct Box{ var value:int = 1 } fn main()->int{ var b:Box = Box() return b.missing }",
        "struct Box{ var value:int = 1 } fn main()->int{ var b:Box = Box(value=1,value=2) return 0 }",
        "struct Box{ var value:int = 1 } fn main()->int{ var b:Box = Box(missing=1) return 0 }",
    )
    for invalid in invalid_structures:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    for invalid in (
        "fn bad(a:bool)->int{ return 0 }",
        "fn bad(a:int,)->int{ return a }",
        "fn bad(a:int)->int{ return a }\nfn main()->int{ return bad(1,) }",
    ):
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results

    # 名稱階段在整份宣告完成後核對，允許前向及遞迴呼叫，同時拒絕名稱與簽章錯誤。
    for invalid in (
        "fn bad(a:int)->int{ return a }\nfn main()->int{ return bad() }",
        "fn bad(a:int)->int{ return a }\nfn main()->int{ return bad(1,2) }",
        "fn main()->int{ return missing(1) }",
        "fn main(a:int,a:int)->int{ return a }",
        "fn main()->int{ return missing }",
        "fn same()->int{ return 1 }\nfn same()->int{ return 2 }",
    ):
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1
