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

    # if／else 是互斥詞彙環境，兩臂各自可有同名唯讀 str；巢狀分支也必須還原。
    scoped_if = ('fn main()->int{\nif(1==1){\nconst text:str = "42"\n'
                 'return text.to_int()\n}else{\nconst text:str = "0"\n'
                 'return text.to_int()\n}\n}')
    nested_if = ('fn main()->int{\nif(1==1){\nif(0==1){\n'
                 'const text:str = "0"\nreturn text.to_int()\n}else{\n'
                 'const text:str = "42"\nreturn text.to_int()\n}\n'
                 '}else{\nreturn 0\n}\n}')
    for case_number, program in enumerate((scoped_if, nested_if)):
        source.write_text("import host\n" + program, encoding="utf-8")
        generated_branches = [invoke(cli, "bootstrap-compiler", source)
                              for cli in (seed, stage, next_stage)]
        assert generated_branches[0] == generated_branches[1] == generated_branches[2]
        code, generated, error = generated_branches[2]
        assert code == 0 and not error and generated.count("let v_text: String = String::from(") == 2
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", "")
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public) == (0, "", "")
        assert invoke(next_stage, "run", public) == (0, "42\n", "")
        for mode in ("build", "pack"):
            exe = root / f"scoped-if-{case_number}-{mode}"
            built = invoke(next_stage, mode, public, "-o", exe)
            assert built == (0, "", ""), (mode, program, built)
            assert invoke(exe) == (42, "", "")

    invalid_if_scopes = (
        'fn main()->int{ if(1==1){const hidden:str = "42" return hidden.to_int()} '
        'else{return hidden.to_int()} }',
        'fn main()->int{ if(1==1){return hidden.to_int()} '
        'else{const hidden:str = "42" return hidden.to_int()} }',
        'fn main()->int{ if(1==1){const text:str = "42" const text:str = "7" '
        'return text.to_int()} else{return 0} }',
    )
    for invalid in invalid_if_scopes:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    # 自舉 var str 建立真正擁有的 Rust String，依分支／例外詞彙作用域清理。
    owned_string_programs = (
        'fn main()->int{\nvar text:str = "42"\nreturn text.to_int()\n}',
        'fn main()->int{\nvar text:str = "abcd"\nreturn len(text)+38\n}',
        ('fn main()->int{\nvar text:str = "非數字"\ntry{\n'
         'return text.to_int()\n}\nexcept TypeConversationError{\nreturn 42\n}\n}'),
        ('fn main()->int{\nif(1==1){\nvar text:str = "42"\nreturn text.to_int()\n'
         '}else{\nvar text:str = "0"\nreturn text.to_int()\n}\n}'),
    )
    for case_number, program in enumerate(owned_string_programs):
        source.write_text("import host\n" + program, encoding="utf-8")
        three_generations = [invoke(cli, "bootstrap-compiler", source)
                             for cli in (seed, stage, next_stage)]
        assert three_generations[0] == three_generations[1] == three_generations[2]
        code, generated, error = three_generations[2]
        assert code == 0 and not error and 'let mut v_text: String = String::from(' in generated, (case_number, code, generated, error)
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", "")
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public) == (0, "", "")
        assert invoke(next_stage, "run", public) == (0, "42\n", "")
        for mode in ("build", "pack"):
            exe = root / f"owned-string-{case_number}-{mode}"
            built = invoke(next_stage, mode, public, "-o", exe)
            assert built == (0, "", ""), (case_number, mode, built)
            assert invoke(exe) == (42, "", "")

    invalid_owned_string = (
        'fn main()->int{var text:str = "42" var text:str = "0" return text.to_int()}',
        'fn main()->int{var text:str = 42 return 0}',
    )
    for invalid in invalid_owned_string:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        failed = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in failed), failed
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    # 受限 var str 重新賦值先建立新擁有字串，原有 String 由 Rust RAII 清理。
    # 覆蓋、分支、例外與 Unicode 長度均以三代自舉生成的真原生程式比對公開四入口。
    string_reassignment_cases = (
        ('fn main()->int{\nvar text:str = "0"\ntext = "18"\n'
         'text = "42"\nreturn text.to_int()\n}'),
        ('fn main()->int{\nvar text:str = "0"\ntext = "海龜"\n'
         'return len(text)+40\n}'),
        ('fn main()->int{\nvar text:str = "0"\nif(1==1){\n'
         'text = "42"\nreturn text.to_int()\n}else{\n'
         'text = "1"\nreturn text.to_int()\n}\n}'),
        ('fn main()->int{\nvar text:str = "0"\ntry{\n'
         'text = "非數字"\nreturn text.to_int()\n}\n'
         'except TypeConversationError{\ntext = "42"\n'
         'return text.to_int()\n}\n}'),
        ('fn main()->int{\nvar text:str = "0"\n'
         'match(1){case 1{\ntext = "42"\nreturn text.to_int()\n}'
         'case _{\nreturn 0\n}}\n}'),
    )
    for case_number, program in enumerate(string_reassignment_cases):
        source.write_text("import host\n" + program, encoding="utf-8")
        generations = [invoke(cli, "bootstrap-compiler", source)
                       for cli in (seed, stage, next_stage)]
        assert generations[0] == generations[1] == generations[2], (case_number, generations)
        code, generated, error = generations[2]
        assert code == 0 and not error, (case_number, code, error)
        assert 'v_text = String::from(' in generated, (case_number, generated)
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", ""), (case_number, invoke(native))
        public.write_text(program, encoding="utf-8")
        checked = invoke(next_stage, "check", public)
        assert checked == (0, "", ""), (case_number, program, checked)
        assert invoke(next_stage, "run", public) == (0, "42\n", ""), case_number
        for mode in ("build", "pack"):
            exe = root / f"string-reassignment-{case_number}-{mode}"
            built = invoke(next_stage, mode, public, "-o", exe)
            assert built == (0, "", ""), (case_number, mode, built)
            assert invoke(exe) == (42, "", ""), (case_number, mode)

    # 未捕捉的轉換錯誤也不得保留被覆寫的舊擁有者。
    unhandled_reassignment = ('fn main()->int{\nvar text:str = "42"\n'
                              'text = "非數字"\nreturn text.to_int()\n}')
    source.write_text("import host\n" + unhandled_reassignment, encoding="utf-8")
    generated_failures = [invoke(cli, "bootstrap-compiler", source)
                          for cli in (seed, stage, next_stage)]
    assert generated_failures[0] == generated_failures[1] == generated_failures[2]
    code, generated, error = generated_failures[2]
    assert code == 0 and not error and "String::from" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(unhandled_reassignment, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"unhandled-string-reassignment-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    invalid_string_reassignment = (
        'fn main()->int{const text:str = "42" text = "1" return text.to_int()}',
        'fn main()->int{var text:str = "42" text = 1 return text.to_int()}',
        'fn main()->int{var text:str = "42" text = true return text.to_int()}',
        'fn main()->int{text = "42" return 0}',
        ('fn main()->int{const text:str = "a"\ntry{\nreturn text.to_int()\n}'
         'except TypeConversationError as msg{\nmsg = "42"\nreturn 0\n}}'),
    )
    for invalid in invalid_string_reassignment:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source)
                   for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic
                   for code, output, diagnostic in results), (invalid, results)
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1, invalid

    # 直接 move 是真正 String 所有權移交；來源此後不可再使用。
    owned_move_cases = (
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nreturn second.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "海龜"\n'
         'var second:str = first.move()\nreturn len(second)+40\n}'),
        ('fn main()->int{\nvar first:str = "4"\n'
         'var middle:str = first.move()\n'
         'var last:str = middle.move()\nreturn last.to_int()+38\n}'),
        ('fn main()->int{\nvar first:str = "0"\nfirst = "42"\n'
         'var second:str = first.move()\nreturn second.to_int()\n}'),
    )
    for case_number, program in enumerate(owned_move_cases):
        source.write_text("import host\n" + program, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source)
                   for cli in (seed, stage, next_stage)]
        assert results[0] == results[1] == results[2], (case_number, results)
        code, generated, error = results[2]
        expected_move = ("let mut v_middle: String = v_first;" if case_number == 2
                         else "let mut v_second: String = v_first;")
        assert code == 0 and not error and expected_move in generated, (
            case_number, code, error, generated)
        assert ".clone()" not in generated, (case_number, generated)
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", ""), case_number
        public.write_text(program, encoding="utf-8")
        checked = invoke(next_stage, "check", public)
        assert checked == (0, "", ""), (case_number, checked)
        assert invoke(next_stage, "run", public) == (0, "42\n", ""), case_number
        for mode in ("build", "pack"):
            exe = root / f"owned-move-{case_number}-{mode}"
            built = invoke(next_stage, mode, public, "-o", exe)
            assert built == (0, "", ""), (case_number, mode, built)
            assert invoke(exe) == (42, "", ""), (case_number, mode)

    invalid_owned_moves = (
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nreturn first.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nreturn len(first)\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\n'
         'var third:str = first.move()\nreturn 0\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first\nreturn len(second)\n}'),
    )
    for program in invalid_owned_moves:
        source.write_text("import host\n" + program, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source)
                   for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not generated and error
                   for code, generated, error in results), (program, results)
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1, program

    # move 之後可以重新初始化原持有者；賦值也可用明確 move 取代既有字串。
    # 比對三代自舉真正的 Rust String 移交、公開完整四入口與原生產物。
    str_reinitialization_cases = (
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nfirst = "42"\nreturn first.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nfirst = "0"\n'
         'first = second.move()\nreturn first.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\nvar second:str = "0"\n'
         'second = first.move()\nreturn second.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\n'
         'var third:str = "0"\nthird = second.move()\nreturn third.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "海龜"\nvar second:str = "0"\n'
         'second = first.move()\nreturn len(second)+40\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nfirst = "0"\n'
         'first = "42"\nreturn first.to_int()\n}'),
    )
    for case_number, program in enumerate(str_reinitialization_cases):
        source.write_text("import host\n" + program, encoding="utf-8")
        generated_by = [invoke(cli, "bootstrap-compiler", source)
                        for cli in (seed, stage, next_stage)]
        assert generated_by[0] == generated_by[1] == generated_by[2], (case_number, generated_by)
        code, generated, error = generated_by[2]
        assert code == 0 and not error, (case_number, code, error)
        assert "let mut v_first: String = String::from(" in generated
        expected_updates = (
            ("v_first = String::from(",),
            ("v_first = v_second;",),
            ("v_second = v_first;",),
            ("v_third = v_second;",),
            ("v_second = v_first;",),
            ("v_first = String::from(",),
        )
        assert all(fragment in generated for fragment in expected_updates[case_number]), (
            case_number, generated)
        assert ".clone()" not in generated
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", ""), (case_number, invoke(native))
        public.write_text(program, encoding="utf-8")
        checked = invoke(next_stage, "check", public)
        assert checked == (0, "", ""), (case_number, checked, program)
        assert invoke(next_stage, "run", public) == (0, "42\n", ""), case_number
        for mode in ("build", "pack"):
            exe = root / f"str-reinitialize-{case_number}-{mode}"
            built = invoke(next_stage, mode, public, "-o", exe)
            assert built == (0, "", ""), (case_number, mode, built)
            assert invoke(exe) == (42, "", ""), (case_number, mode)

    invalid_str_reinitialization = (
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nreturn first.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\n'
         'var second:str = first.move()\nfirst = 42\nreturn 0\n}'),
        ('fn main()->int{\nvar first:str = "42"\nvar second:str = "0"\n'
         'second = first\nreturn second.to_int()\n}'),
        ('fn main()->int{\nvar first:str = "42"\nvar second:str = "0"\n'
         'second = first.move()\nsecond = first.move()\nreturn 0\n}'),
    )
    for program in invalid_str_reinitialization:
        source.write_text("import host\n" + program, encoding="utf-8")
        rejected = [invoke(cli, "bootstrap-compiler", source)
                    for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not generated and diagnostic
                   for code, generated, diagnostic in rejected), (program, rejected)
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1, program

    # const str 現由不可變的 Rust String 持有：可明確移交，但不可變更綁定。
    const_owner_move_cases = (
        ('fn main()->int{\nconst first:str = "42"\n'
         'var second:str = "0"\nsecond = first.move()\n'
         'return second.to_int()\n}'),
        ('fn main()->int{\nconst first:str = "42"\n'
         'var second:str = first.move()\nreturn second.to_int()\n}'),
        ('fn main()->int{\nconst first:str = "海龜"\n'
         'var second:str = first.move()\nreturn len(second)+40\n}'),
    )
    for case_number, program in enumerate(const_owner_move_cases):
        source.write_text("import host\n" + program, encoding="utf-8")
        generations = [invoke(cli, "bootstrap-compiler", source)
                       for cli in (seed, stage, next_stage)]
        assert generations[0] == generations[1] == generations[2], (case_number, generations)
        code, generated, diagnostic = generations[2]
        assert code == 0 and not diagnostic and (
            'let v_first: String = String::from(' in generated), (case_number, generated, diagnostic)
        assert (("v_second = v_first;" in generated)
                if case_number == 0 else ("let mut v_second: String = v_first;" in generated))
        assert ".clone()" not in generated
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", ""), case_number
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public) == (0, "", ""), case_number
        assert invoke(next_stage, "run", public) == (0, "42\n", ""), case_number
        for mode in ("build", "pack"):
            exe = root / f"const-owner-move-{case_number}-{mode}"
            assert invoke(next_stage, mode, public, "-o", exe) == (0, "", ""), case_number
            assert invoke(exe) == (42, "", ""), case_number

    invalid_const_owner_move = (
        ('fn main()->int{\nconst first:str = "42"\n'
         'var second:str = first.move()\nreturn first.to_int()\n}'),
        ('fn main()->int{\nconst first:str = "42"\n'
         'var second:str = first.move()\nvar third:str = first.move()\nreturn 0\n}'),
        ('fn main()->int{\nconst first:str = "42"\n'
         'first = "7"\nreturn first.to_int()\n}'),
        ('fn main()->int{\nconst first:str = "42"\n'
         'var second:str = first.move()\nfirst = "42"\nreturn 0\n}'),
    )
    for program in invalid_const_owner_move:
        source.write_text("import host\n" + program, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source)
                   for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not generated and diagnostic
                   for code, generated, diagnostic in results), (program, results)
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1, program

    # 自身移交拒絕：只能在初始化及轉移完整的前置值之間交換所有權。
    self_move = 'fn main()->int{\nvar text:str = "42"\ntext = text.move()\nreturn 0\n}'
    source.write_text("import host\n" + self_move, encoding="utf-8")
    assert all(invoke(cli, "bootstrap-compiler", source)[0] == 1
               for cli in (seed, stage, next_stage))

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

    # list[int] 常值與公開 with 唯讀索引共用已解析的區域容器身分。
    lists = ("fn main()->int{\nvar xs:list[int] = [3,42]\n"
             "with xs[1] as item {\nreturn item\n}\n}")
    source.write_text("import host\n" + lists, encoding="utf-8")
    generated_lists = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_lists[0] == generated_lists[1] == generated_lists[2]
    code, generated, error = generated_lists[2]
    assert (code == 0 and not error and "Vec<i64> = vec![" in generated
            and "v_xs.get(i)" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(lists, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"list-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    mutations = "fn main()->int{\nvar xs:list[int] = [39]\nreturn xs.append(40)+xs.pop()\n}"
    source.write_text("import host\n" + mutations, encoding="utf-8")
    generated_mutations = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_mutations[0] == generated_mutations[1] == generated_mutations[2]
    code, generated, error = generated_mutations[2]
    assert code == 0 and not error and ".push(40i64)" in generated and ".pop()" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(mutations, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"list-mutation-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    dictionaries = ('fn main()->int{\nvar empty:dict{str,int} = {}\n'
                    'var entries:dict{str,int} = {"甲":40,"乙":2}\n'
                    'return len(empty)+len(entries)+40\n}')
    source.write_text("import host\n" + dictionaries, encoding="utf-8")
    generated_dicts = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_dicts[0] == generated_dicts[1] == generated_dicts[2]
    code, generated, error = generated_dicts[2]
    assert (code == 0 and not error and "Vec<(&'static str, i64)>" in generated
            and '"甲"' in generated and ".len() as i64" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(dictionaries, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"dict-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    dict_lookup = ('fn main()->int{\nvar entries:dict{str,int} = {"答案":40}\n'
                   'with entries["答案"] as item {\nreturn item+2\n}\n}')
    source.write_text("import host\n" + dict_lookup, encoding="utf-8")
    generated_lookup = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_lookup[0] == generated_lookup[1] == generated_lookup[2]
    code, generated, error = generated_lookup[2]
    assert (code == 0 and not error and '.find(|(k, _)| *k == "答案")' in generated
            and "字典查找缺少鍵" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(dict_lookup, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"dict-lookup-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    dict_set = ('fn main()->int{\nvar entries:dict{str,int} = {"既有":1}\n'
                'return entries.set("既有",9)+entries.set("新增",2)+39\n}')
    source.write_text("import host\n" + dict_set, encoding="utf-8")
    generated_set = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_set[0] == generated_set[1] == generated_set[2]
    code, generated, error = generated_set[2]
    assert (code == 0 and not error and ".iter_mut().find" in generated
            and '.push(("新增", v_dict_value))' in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(dict_set, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"dict-set-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_dicts = (
        'fn main()->int{ var xs:dict{str,int} = {1:2} return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a":true} return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a":1,"a":2} return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a" 1} return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a":1,} return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a":1} with xs[1] as item { return item } return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {"a":1} with xs["a"] as xs { return xs } return 0 }',
        'fn main()->int{ with missing["a"] as item { return item } return 0 }',
        'fn main()->int{ var xs:dict{str,int} = {} return xs.set(1,2) }',
        'fn main()->int{ var xs:dict{str,int} = {} return xs.set("a",true) }',
        'fn main()->int{ var xs:dict{str,int} = {} return xs.set("a") }',
        'fn main()->int{ var xs:dict{str,int} = {} return xs.set("a",1,2) }',
        'fn main()->int{ var xs:dict{str,int} = {"a":1} with xs["a"] as item { return xs.set("b",2) } }',
    )
    for invalid in invalid_dicts:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    missing_key = ('fn main()->int{\nvar entries:dict{str,int} = {"有":1}\n'
                   'with entries["無"] as item {\nreturn item\n}\n}')
    source.write_text("import host\n" + missing_key, encoding="utf-8")
    generated_missing_key = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_missing_key[0] == generated_missing_key[1] == generated_missing_key[2]
    code, generated, error = generated_missing_key[2]
    assert code == 0 and not error and "字典查找缺少鍵" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(missing_key, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"dict-missing-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    nested_lists = ('fn main()->int{\nvar matrix:list[list[int]] = [[1,2],[],[42]]\n'
                    'with matrix[0] as row {\nreturn len(row)+40\n}\n}')
    source.write_text("import host\n" + nested_lists, encoding="utf-8")
    generated_nested = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_nested[0] == generated_nested[1] == generated_nested[2]
    code, generated, error = generated_nested[2]
    assert (code == 0 and not error and "Vec<Vec<i64>>" in generated
            and "vec![vec![1i64,2i64],vec![],vec![42i64]]" in generated
            and "let v_row: &Vec<i64>" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(nested_lists, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"nested-list-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    nested_index = ('fn main()->int{\nvar matrix:list[list[int]] = [[1,2],[42]]\n'
                    'with matrix[0] as row {\nwith row[1] as item {\n'
                    'return item+40\n}\n}\n}')
    source.write_text("import host\n" + nested_index, encoding="utf-8")
    generated_nested_index = [invoke(cli, "bootstrap-compiler", source)
                              for cli in (seed, stage, next_stage)]
    assert generated_nested_index[0] == generated_nested_index[1] == generated_nested_index[2]
    code, generated, error = generated_nested_index[2]
    assert (code == 0 and not error and generated.count(".get(i)") == 2
            and "let v_row: &Vec<i64>" in generated
            and "let v_item: i64" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(nested_index, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"nested-index-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_nested_lists = (
        "fn main()->int{ var xs:list[list[int]] = [[true]] return 0 }",
        "fn main()->int{ var xs:list[list[int]] = [1] return 0 }",
        "fn main()->int{ var xs:list[list[int]] = [[1,]] return 0 }",
        "fn main()->int{ var xs:list[list[int]] = [[1],] return 0 }",
        "fn main()->int{ var xs:list[list[int]] = [[1]] with xs[0] as xs { return len(xs) } return 0 }",
        "fn main()->int{ var xs:list[list[int]] = [[1]] with xs[0] as row { return row.append(2) } }",
        "fn main()->int{ var xs:list[list[int]] = [[1]] with xs[0] as row { with row[0] as row { return row } } }",
        "fn main()->int{ var xs:list[list[int]] = [[1]] with xs[0] as row { with row[0] as item { with item[0] as value { return value } } } }",
    )
    for invalid in invalid_nested_lists:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    missing_row = ('fn main()->int{\nvar matrix:list[list[int]] = [[1]]\n'
                   'with matrix[2] as row {\nreturn len(row)\n}\n}')
    source.write_text("import host\n" + missing_row, encoding="utf-8")
    generated_missing_row = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_missing_row[0] == generated_missing_row[1] == generated_missing_row[2]
    code, generated, error = generated_missing_row[2]
    assert code == 0 and not error and "清單索引越界" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(missing_row, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"nested-list-missing-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    missing_item = ('fn main()->int{\nvar matrix:list[list[int]] = [[1]]\n'
                    'with matrix[0] as row {\nwith row[4] as item {\n'
                    'return item\n}\n}\n}')
    source.write_text("import host\n" + missing_item, encoding="utf-8")
    generated_missing_item = [invoke(cli, "bootstrap-compiler", source)
                              for cli in (seed, stage, next_stage)]
    assert generated_missing_item[0] == generated_missing_item[1] == generated_missing_item[2]
    code, generated, error = generated_missing_item[2]
    assert code == 0 and not error and generated.count("清單索引越界") == 2
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(missing_item, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"nested-item-missing-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    nested_dict = ('fn main()->int{\nvar empty:dict{str,list[int]} = {}\n'
                   'var rows:dict{str,list[int]} = {"主":[1,2],"空":[]}\n'
                   'with rows["主"] as row {\nwith row[1] as item {\n'
                   'return item+len(empty)+40\n}\n}\n}')
    source.write_text("import host\n" + nested_dict, encoding="utf-8")
    generated_nested_dict = [invoke(cli, "bootstrap-compiler", source)
                             for cli in (seed, stage, next_stage)]
    assert generated_nested_dict[0] == generated_nested_dict[1] == generated_nested_dict[2]
    code, generated, error = generated_nested_dict[2]
    assert (code == 0 and not error and "Vec<(&'static str, Vec<i64>)>" in generated
            and "let v_empty: Vec<(&'static str, Vec<i64>)> = vec![]" in generated
            and '("主",vec![1i64,2i64]),("空",vec![])' in generated
            and "let v_row: &Vec<i64>" in generated
            and "let v_item: i64" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(nested_dict, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"nested-dict-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_nested_dicts = (
        'fn main()->int{ var xs:dict{str,list[int]} = {1:[2]} return 0 }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[true]} return 0 }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":1} return 0 }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[1],"a":[]} return 0 }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[1],} return 0 }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[1]} with xs["a"] as row { return row.append(2) } }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[1]} return xs.set("a",2) }',
        'fn main()->int{ var xs:dict{str,list[int]} = {"a":[1]} with xs["a"] as row { with row[0] as row { return row } } }',
    )
    for invalid in invalid_nested_dicts:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    missing_nested_key = ('fn main()->int{\nvar rows:dict{str,list[int]} = {"有":[1]}\n'
                          'with rows["無"] as row {\nreturn len(row)\n}\n}')
    source.write_text("import host\n" + missing_nested_key, encoding="utf-8")
    generated_missing_nested_key = [invoke(cli, "bootstrap-compiler", source)
                                    for cli in (seed, stage, next_stage)]
    assert generated_missing_nested_key[0] == generated_missing_nested_key[1] == generated_missing_nested_key[2]
    code, generated, error = generated_missing_nested_key[2]
    assert code == 0 and not error and "字典查找缺少鍵" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(missing_nested_key, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"nested-dict-missing-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    integer_match = ('fn main()->int{\nmatch(2+3){\ncase 4{ return 1 }\n'
                     'case 5{ return 42 }\ncase _{ return 0 }\n}\n}')
    source.write_text("import host\n" + integer_match, encoding="utf-8")
    generated_matches = [invoke(cli, "bootstrap-compiler", source)
                         for cli in (seed, stage, next_stage)]
    assert generated_matches[0] == generated_matches[1] == generated_matches[2]
    code, generated, error = generated_matches[2]
    assert (code == 0 and not error and "match 2i64.checked_add(3i64)" in generated
            and "4i64 =>" in generated and "5i64 =>" in generated
            and "_ =>" in generated and generated.count("match 2i64") == 1)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(integer_match, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"integer-match-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    negative_match = ('fn main()->int{\nmatch(0-1){\ncase -1{ return 42 }\n}\n'
                      'return 0\n}')
    source.write_text("import host\n" + negative_match, encoding="utf-8")
    generated_negative_matches = [invoke(cli, "bootstrap-compiler", source)
                                  for cli in (seed, stage, next_stage)]
    assert generated_negative_matches[0] == generated_negative_matches[1] == generated_negative_matches[2]
    code, generated, error = generated_negative_matches[2]
    assert code == 0 and not error and "-1i64 =>" in generated and "_ => {}," in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(negative_match, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")

    boolean_match = ('fn main()->int{\nmatch(true){\ncase false{ return 1 }\n'
                     'case true{ return 42 }\n}\n}')
    source.write_text("import host\n" + boolean_match, encoding="utf-8")
    generated_boolean_matches = [invoke(cli, "bootstrap-compiler", source)
                                 for cli in (seed, stage, next_stage)]
    assert generated_boolean_matches[0] == generated_boolean_matches[1] == generated_boolean_matches[2]
    code, generated, error = generated_boolean_matches[2]
    assert (code == 0 and not error and "match true" in generated
            and "false =>" in generated and "true =>" in generated
            and "_ => {}," not in generated and generated.count("match true") == 1)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(boolean_match, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"boolean-match-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    boolean_fallthrough = ('fn main()->int{\nmatch(false){\ncase true{ return 1 }\n}\n'
                           'return 42\n}')
    source.write_text("import host\n" + boolean_fallthrough, encoding="utf-8")
    generated_boolean_fallthrough = [invoke(cli, "bootstrap-compiler", source)
                                     for cli in (seed, stage, next_stage)]
    assert generated_boolean_fallthrough[0] == generated_boolean_fallthrough[1] == generated_boolean_fallthrough[2]
    code, generated, error = generated_boolean_fallthrough[2]
    assert code == 0 and not error and "_ => {}," in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(boolean_fallthrough, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")

    # 互斥 case 各自宣告相同名稱不衝突，名稱也不可從前一臂洩漏至下一臂。
    scoped_match = ('fn main()->int{\nmatch(1){\ncase 0{\n'
                    'const text:str = "0"\nreturn text.to_int()\n}\n'
                    'case 1{\nconst text:str = "42"\nreturn text.to_int()\n}\n}\n'
                    'return 1\n}')
    source.write_text("import host\n" + scoped_match, encoding="utf-8")
    match_scopes = [invoke(cli, "bootstrap-compiler", source)
                    for cli in (seed, stage, next_stage)]
    assert match_scopes[0] == match_scopes[1] == match_scopes[2]
    code, generated, error = match_scopes[2]
    assert code == 0 and not error and generated.count("let v_text: String = String::from(") == 2
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(scoped_match, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"scoped-match-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    evaluation_exception = ('fn main()->int{\ntry{\nreturn 1/0\n}\n'
                            'except EvaluationError{\nreturn 42\n}\n}')
    source.write_text("import host\n" + evaluation_exception, encoding="utf-8")
    generated_exceptions = [invoke(cli, "bootstrap-compiler", source)
                            for cli in (seed, stage, next_stage)]
    assert generated_exceptions[0] == generated_exceptions[1] == generated_exceptions[2]
    code, generated, error = generated_exceptions[2]
    assert (code == 0 and not error and "match (|| -> R" in generated
            and "Err(Fault::Overflow)" in generated
            and "Err(error) => return Err(error)" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(evaluation_exception, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"evaluation-exception-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    exception_message = ('fn main()->int{\ntry{\nreturn 1/0\n}\n'
                         'except EvaluationError as msg{\nreturn len(msg)+38\n}\n}')
    source.write_text("import host\n" + exception_message, encoding="utf-8")
    generated_messages = [invoke(cli, "bootstrap-compiler", source)
                          for cli in (seed, stage, next_stage)]
    assert generated_messages[0] == generated_messages[1] == generated_messages[2]
    code, generated, error = generated_messages[2]
    assert (code == 0 and not error and 'let v_msg: &\'static str = "運算失敗"' in generated
            and "v_msg.chars().count() as i64" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(exception_message, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"exception-message-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    conversion_exception = ('fn main()->int{\nconst text:str = "水母"\n'
                            'try{\nreturn text.to_int()\n}\n'
                            'except TypeConversationError{\nreturn 42\n}\n}')
    source.write_text("import host\n" + conversion_exception, encoding="utf-8")
    generated_conversions = [invoke(cli, "bootstrap-compiler", source)
                             for cli in (seed, stage, next_stage)]
    assert generated_conversions[0] == generated_conversions[1] == generated_conversions[2]
    code, generated, error = generated_conversions[2]
    assert (code == 0 and not error and 'let v_text: String = String::from("水母")' in generated
            and "v_text.parse::<i64>()" in generated
            and "Err(Fault::Conversion)" in generated)
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(conversion_exception, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"conversion-exception-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    conversion_message = ('fn main()->int{\nconst text:str = "x"\n'
                          'try{\nreturn text.to_int()\n}\n'
                          'except TypeConversationError as msg{\nreturn len(msg)+36\n}\n}')
    successful_conversion = ('fn main()->int{\nconst text:str = "42"\n'
                             'try{\nreturn text.to_int()\n}\n'
                             'except TypeConversationError{\nreturn 1\n}\n}')
    for program in (conversion_message, successful_conversion):
        source.write_text("import host\n" + program, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert results[0] == results[1] == results[2]
        code, generated, error = results[2]
        assert code == 0 and not error
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", "")
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public) == (0, "", "")
        assert invoke(next_stage, "run", public) == (0, "42\n", "")

    mismatched_conversion = ('fn main()->int{\nconst text:str = "x"\n'
                             'try{\nreturn text.to_int()\n}\n'
                             'except EvaluationError{\nreturn 42\n}\n}')
    source.write_text("import host\n" + mismatched_conversion, encoding="utf-8")
    mismatch_results = [invoke(cli, "bootstrap-compiler", source)
                        for cli in (seed, stage, next_stage)]
    assert mismatch_results[0] == mismatch_results[1] == mismatch_results[2]
    code, generated, error = mismatch_results[2]
    assert code == 0 and not error and "Err(Fault::Overflow)" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(mismatched_conversion, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1

    successful_try = ('fn main()->int{\ntry{\nreturn 42\n}\n'
                      'except EvaluationError{\nreturn 1\n}\n}')
    nested_exception = ('fn main()->int{\ntry{\ntry{\nreturn 9223372036854775807+1\n}\n'
                        'except EvaluationError{\nreturn 42\n}\n}\n'
                        'except EvaluationError{\nreturn 1\n}\n}')
    for program in (successful_try, nested_exception):
        source.write_text("import host\n" + program, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert results[0] == results[1] == results[2]
        code, generated, error = results[2]
        assert code == 0 and not error
        rust_source.write_text(
            "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
            "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
            + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
            encoding="utf-8")
        subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
        assert invoke(native) == (42, "", "")
        public.write_text(program, encoding="utf-8")
        assert invoke(next_stage, "check", public) == (0, "", "")
        assert invoke(next_stage, "run", public) == (0, "42\n", "")

    # 自舉產生器的 try 保護臂、處理臂須與公開靜態檢查使用相同詞彙作用域。
    # 兩個獨立臂可以各自宣告同名 str；不能把名稱從一個臂洩漏到另一個臂。
    scoped_strings = ('fn main()->int{\ntry{\nconst text:str = "非數字"\n'
                      'return text.to_int()\n}\nexcept TypeConversationError{\n'
                      'const text:str = "42"\nreturn text.to_int()\n}\n}')
    source.write_text("import host\n" + scoped_strings, encoding="utf-8")
    scoped_generated = [invoke(cli, "bootstrap-compiler", source)
                        for cli in (seed, stage, next_stage)]
    assert scoped_generated[0] == scoped_generated[1] == scoped_generated[2]
    code, generated, error = scoped_generated[2]
    assert code == 0 and not error and generated.count("let v_text: String = String::from(") == 2
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Conversion,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native) == (42, "", "")
    public.write_text(scoped_strings, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public) == (0, "42\n", "")
    for mode in ("build", "pack"):
        exe = root / f"scoped-exception-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe) == (42, "", "")

    invalid_exceptions = (
        "fn main()->int{ try{ return 1 } return 0 }",
        "fn main()->int{ try{ return 1 } except Unknown{ return 2 } }",
        'fn main()->int{ try{ return 1/0 } except EvaluationError as msg{ msg = "x" return 0 } }',
        "fn main()->int{ try{ return 1 } except EvaluationError as msg{ return 0 } return len(msg) }",
        'fn main()->int{ try{ const secret:str = "x" return secret.to_int() } '
        'except TypeConversationError{ return secret.to_int() } }',
        'fn main()->int{ try{ return 1 } except EvaluationError{ '
        'const hidden:str = "42" return hidden.to_int() } return hidden.to_int() }',
        'fn main()->int{ try{ const private:str = "42" return private.to_int() } '
        'except EvaluationError{ return 0 } return private.to_int() }',
    )
    for invalid in invalid_exceptions:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    invalid_matches = (
        'fn main()->int{ match(1){ case 0{ const secret:str = "42" '
        'return secret.to_int() } case 1{ return secret.to_int() } } return 0 }',
        'fn main()->int{ match(1){ case 0{ const local:str = "42" '
        'return local.to_int() } } return local.to_int() }',
        "fn main()->int{ match(1){} return 0 }",
        "fn main()->int{ match(1){ case 1{ return 0 } case 0x1{ return 1 } } return 0 }",
        "fn main()->int{ match(1){ case true{ return 1 } } return 0 }",
        "fn main()->int{ match(true){ case 1{ return 1 } } return 0 }",
        "fn main()->int{ match(true){ case true{ return 1 } case true{ return 0 } } }",
        "fn main()->int{ match(true){ case true{ return 1 } case false{ return 0 } case _{ return 2 } } }",
        "fn main()->int{ match(1){ case 1+0{ return 1 } } return 0 }",
        "fn main()->int{ match(1){ case _{ return 0 } case 1{ return 1 } } }",
    )
    for invalid in invalid_matches:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    invalid_lists = (
        "fn main()->int{ var xs:list[int] = [true] return 0 }",
        "fn main()->int{ var xs:list[int] = [1,] return 0 }",
        "fn main()->int{ return len(xs) }",
        "fn main()->int{ var xs:list[int] = [1,2 return 0 }",
        "fn main()->int{ var xs:list[int] = [] var xs:list[int] = [] return 0 }",
        "fn main()->int{ var xs:list[int] = [] return xs.append(true) }",
        "fn main()->int{ var xs:list[int] = [] return xs.append() }",
        "fn main()->int{ var xs:list[int] = [1] return xs.pop(1) }",
        "fn main()->int{ var xs:list[int] = [1] with missing[0] as item { return item } return 0 }",
        "fn main()->int{ var xs:list[int] = [1] with xs[0] as xs { return xs } return 0 }",
        "fn main()->int{ var xs:list[int] = [1] with xs[0] as item { return xs.append(2) } }",
        "fn main()->int{ var xs:list[int] = [1] with xs[0] as item { return xs.pop() } }",
    )
    for invalid in invalid_lists:
        source.write_text("import host\n" + invalid, encoding="utf-8")
        results = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
        assert all(code == 1 and not output and diagnostic for code, output, diagnostic in results), results
        public.write_text(invalid, encoding="utf-8")
        assert invoke(next_stage, "check", public)[0] == 1

    # 越界是已通過靜態檢查後的執行期錯誤，三代生成物與公開兩種產物都必須失敗。
    missing_item = ("fn main()->int{\nvar xs:list[int] = [1]\n"
                    "with xs[2] as item {\nreturn item\n}\n}")
    source.write_text("import host\n" + missing_item, encoding="utf-8")
    generated_missing = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_missing[0] == generated_missing[1] == generated_missing[2]
    code, generated, error = generated_missing[2]
    assert code == 0 and not error and "清單索引越界" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(missing_item, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"list-missing-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

    empty_pop = "fn main()->int{\nvar xs:list[int] = []\nreturn xs.pop()\n}"
    source.write_text("import host\n" + empty_pop, encoding="utf-8")
    generated_empty = [invoke(cli, "bootstrap-compiler", source) for cli in (seed, stage, next_stage)]
    assert generated_empty[0] == generated_empty[1] == generated_empty[2]
    code, generated, error = generated_empty[2]
    assert code == 0 and not error and "清單為空" in generated
    rust_source.write_text(
        "type R=Result<i64,Fault>; enum Fault{Overflow,Syntax(String)} struct Machine; "
        "impl Machine{fn enter(&mut self,_:usize)->Result<(),Fault>{Ok(())}}\n"
        + generated + "fn main(){std::process::exit(v_main(&mut Machine,0).unwrap_or(1) as i32)}",
        encoding="utf-8")
    subprocess.run(["rustc", "--edition=2021", rust_source, "-o", native], check=True)
    assert invoke(native)[0] == 1
    public.write_text(empty_pop, encoding="utf-8")
    assert invoke(next_stage, "check", public) == (0, "", "")
    assert invoke(next_stage, "run", public)[0] == 1
    for mode in ("build", "pack"):
        exe = root / f"list-empty-{mode}"
        assert invoke(next_stage, mode, public, "-o", exe) == (0, "", "")
        assert invoke(exe)[0] == 1

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
