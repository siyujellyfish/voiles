//! 公開直譯與獨立執行檔共用的 UTF-8 與作業系統接口。
use std::io::Write;

pub(super) enum Value { Int(i64), Text(String), Bytes(Vec<u8>) }

pub(super) fn signature(name:&str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match name {
        "argc" => (&[], "int"),
        "arg" => (&["int"], "str"),
        "read_bytes" => (&["str"], "list[int]"),
        "read_utf8" => (&["str"], "str"),
        "write_utf8" => (&["str","str"], "int"),
        "write_bytes" => (&["str"], "int"),
        "file_len" => (&["str"], "int"),
        "file_byte" => (&["str","int"], "int"),
        "path_join" => (&["str","str"], "str"),
        "stdout" | "stderr" => (&["str"], "int"),
        "process_run" => (&["str"], "int"),
        _ => return None,
    })
}

pub(super) fn variadic_parameter(name: &str) -> Option<&'static str> {
    match name {
        "process_run" => Some("str"),
        "write_bytes" => Some("int"),
        _ => None,
    }
}

pub(super) fn invoke(name:&str, values:&[Value], args:&[std::ffi::OsString]) -> Option<Value> {
    use Value::{Int,Text};
    Some(match (name,values) {
        ("argc",[]) => Int(args.len().try_into().ok()?),
        ("arg",[Int(i)]) => Text(args.get(usize::try_from(*i).ok()?)?.to_str()?.into()),
        ("read_bytes",[Text(path)]) => Value::Bytes(std::fs::read(path).ok()?),
        ("read_utf8",[Text(path)]) => Text(std::fs::read_to_string(path).ok()?),
        ("write_utf8",[Text(path),Text(s)]) => { std::fs::write(path,s.as_bytes()).ok()?; Int(0) }
        ("write_bytes",[Text(path),values @ ..]) => {
            let bytes: Option<Vec<u8>> = values.iter().map(|v| match v {
                Int(n) => u8::try_from(*n).ok(),
                _ => None,
            }).collect();
            std::fs::write(path,bytes?).ok()?;
            Int(0)
        }
        ("file_len",[Text(path)]) => Int(std::fs::metadata(path).ok()?.len().try_into().ok()?),
        ("file_byte",[Text(path),Int(i)]) => {
            use std::io::{Read,Seek,SeekFrom};
            let mut f=std::fs::File::open(path).ok()?;
            f.seek(SeekFrom::Start(u64::try_from(*i).ok()?)).ok()?;
            let mut byte=[0]; f.read_exact(&mut byte).ok()?; Int(i64::from(byte[0]))
        }
        ("path_join",[Text(a),Text(b)]) => Text(std::path::Path::new(a).join(b).to_str()?.into()),
        ("stdout",[Text(s)]) => { let mut out=std::io::stdout().lock(); out.write_all(s.as_bytes()).ok()?; out.flush().ok()?; Int(0) }
        ("stderr",[Text(s)]) => { let mut out=std::io::stderr().lock(); out.write_all(s.as_bytes()).ok()?; out.flush().ok()?; Int(0) }
        ("process_run",[Text(program),rest @ ..]) => {
            let mut command=std::process::Command::new(program);
            for arg in rest { let Text(s)=arg else {return None}; command.arg(s); }
            Int(i64::from(command.status().ok()?.code()?))
        }
        _=>return None,
    })
}
