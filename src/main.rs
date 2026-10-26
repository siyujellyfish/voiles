//! voiles MVP-A 單檔靜態檢查命令，不執行 voiles 或輸出執行檔。

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
        // 保護非預期的無診斷失敗，不將其偽裝成任何語法診斷。
        if result.diagnostics.is_empty() {
            eprintln!("檢查失敗：缺少可顯示的詳細診斷");
        }
        ExitCode::from(1)
    }
}
