//! P02 靜態檢查命令骨架，不執行 voiles 程式。

use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;
use voiles_core::{check_file, CheckStatus};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), None) = (args.next(), args.next()) else {
        eprintln!("用法錯誤：請提供唯一的 .voil 檔案路徑");
        return ExitCode::from(1);
    };
    let path = Path::new(&input);
    if path.extension() != Some(OsStr::new("voil")) {
        eprintln!("用法錯誤：來源檔案必須使用 .voil 副檔名");
        return ExitCode::from(1);
    }

    let result = check_file(path);
    if let Some(error) = result.io_error {
        eprintln!("檔案讀取錯誤（{}）：{error:?}", path.display());
        return ExitCode::from(1);
    }
    for diagnostic in &result.diagnostics {
        if let Some(span) = diagnostic.span {
            eprintln!(
                "{}:{}:{}: {}: {}",
                path.display(), span.start.line, span.start.column,
                diagnostic.kind, diagnostic.message
            );
        } else {
            eprintln!("{}: {}: {}", path.display(), diagnostic.kind, diagnostic.message);
        }
    }
    if result.status == CheckStatus::Passed {
        ExitCode::SUCCESS
    } else {
        // 尚無 P04～P12 分析器，錯誤不可謊稱由語法判定。
        if result.diagnostics.is_empty() {
            eprintln!("前端尚未實作：已讀取來源，但目前無法驗證 voiles 語法與靜態語意");
        }
        ExitCode::from(1)
    }
}
