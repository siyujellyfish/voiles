//! F00：從真正公開 check_file 取得完整診斷、起訖行列與順序。
//! 此探針只讀取使用者指定的現存來源，不更改任何語言語意。

use std::path::Path;
use voiles_core::check_file;

fn 原文十六進位(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let (Some(path), None) = (args.next(), args.next()) else {
        eprintln!("F00 探針需要且只能接受一個 UTF-8 .voil 來源路徑");
        std::process::exit(2);
    };
    let result = check_file(Path::new(&path));
    println!("{:?}\t{:?}\t{}", result.status, result.io_error, result.diagnostics.len());
    for diagnostic in &result.diagnostics {
        let position = |select: fn(voiles_core::SourceSpan) -> voiles_core::SourcePosition,
                        field: fn(voiles_core::SourcePosition) -> usize| {
            diagnostic.span.map(|span| field(select(span)).to_string())
                .unwrap_or_else(|| "-".to_owned())
        };
        println!(
            "{:?}\t{}\t{}\t{}\t{}\t{}",
            diagnostic.kind,
            position(|span| span.start, |point| point.line),
            position(|span| span.start, |point| point.column),
            position(|span| span.end, |point| point.line),
            position(|span| span.end, |point| point.column),
            原文十六進位(diagnostic.message.as_bytes()),
        );
    }
}
