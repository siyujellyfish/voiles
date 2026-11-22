"""比對程式引數、原始位元組、UTF-8、路徑、程序退出與輸出。"""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from m7_public_cases import verify

cli = Path(sys.argv[1]).resolve(strict=True)
def run(*args, cwd):
    return subprocess.run(args, cwd=cwd, capture_output=True, encoding="utf-8", timeout=60)

with TemporaryDirectory() as temp:
    root = Path(temp)
    src = root / "main.voil"
    src.write_text("""fn main()->int{
const root:str = arg(0)
const path:str = path_join(root,"甲.txt")
write_utf8(path,"甲")
const text:str = read_utf8(path)
stdout(text)
stderr("乙")
const status:int = process_run("/bin/sh","-c","exit 38")
return status + argc() + file_len(path)
}""", encoding="utf-8")
    data = root / "甲.txt"
    for mode in ("check", "run", "build", "pack"):
        exe = root / mode
        args = [cli, mode, src]
        if mode in ("build", "pack"):
            args += ["-o", exe]
        if mode == "run":
            args += ["--", root]
        result = run(*args, cwd=root)
        if mode != "run":
            assert (result.returncode,result.stdout,result.stderr)==(0,"",""), result
            assert not data.exists()
        if mode == "check":
            continue
        if mode != "run":
            result = run(exe,root,cwd=root)
        assert (result.returncode,result.stdout,result.stderr)==((0,"甲42\n","乙") if mode=="run" else (42,"甲","乙")), result
        assert data.read_text(encoding="utf-8")=="甲"
        data.unlink()
    raw = root / "raw.bin"
    raw.write_bytes(bytes([255, 42]))
    verify(cli, [
        (f'fn main()->int{{ return file_byte("{raw}",1) }}',42),
        (f'fn main()->int{{ const text:str = read_utf8("{raw}")\nreturn 0 }}',"runtime"),
        (f'fn main()->int{{ return file_byte("{raw}",9) }}',"runtime"),
        ('fn main()->int{ const s:str = arg(0)\nreturn 0 }',"runtime"),
        ('fn main()->int{ return process_run("/no/such/voiles-command") }',"runtime"),
        ('fn main()->int{ return file_len(4) }',"reject"),
        ('fn main()->int{ return read_utf8("x") }',"reject"),
        ('fn argc()->int{ return 42 }\nfn main()->int{ return argc() }',42),
    ])

    binary = root / "輸出.bin"
    verify(cli, [
        (f'fn main()->int{{ write_bytes("{binary}",0,255,42)\nreturn file_byte("{binary}",2) }}',42),
        (f'fn main()->int{{ return write_bytes("{binary}",55,256) }}',"runtime"),
        (f'fn main()->int{{ return write_bytes("{binary}","x") }}',"reject"),
    ])
    assert binary.read_bytes() == bytes([0,255,42])
    verify(cli, [(f'fn main()->int{{ return write_bytes("{binary}") }}',0)])
    assert binary.read_bytes() == b""
