//! voiles 單檔靜態檢查與模組程式命令。

use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;
use voiles_core::{check_file, CheckStatus};

mod native_build;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    const HELP: &str = "用法：voiles <檔案.voil>（單檔靜態檢查）
      voiles check <入口.voil>（模組靜態檢查）
      voiles run <入口.voil>（執行並顯示 main 回傳值）
      voiles build <入口.voil> -o <執行檔>
      voiles pack <入口.voil> -o <獨立執行檔>
      voiles --help | --version";
    match args.first().and_then(|arg| arg.to_str()) {
        Some("bootstrap-digits") if args.len() == 2 => {
            match voiles_core::EntryProgram::prepare(Path::new(&args[1])).and_then(|p| p.bootstrap_digits()) {
                Ok(source) => print!("{source}"),
                Err(error) => { eprintln!("{error}"); return ExitCode::FAILURE; }
            }
        }
        Some("--help") if args.len() == 1 => println!("{HELP}"),
        Some("--version") if args.len() == 1 => println!("voiles P22"),
        Some(command @ ("check" | "run")) if args.len() == 2 => {
            let path = Path::new(&args[1]);
            if path.extension() != Some(OsStr::new("voil")) {
                eprintln!("用法錯誤：入口必須使用 .voil 副檔名");
                return ExitCode::from(2);
            }
            let result = voiles_core::EntryProgram::prepare(path).and_then(|program| {
                if command == "run" {
                    println!("{}", program.execute()?);
                }
                Ok(())
            });
            if let Err(error) = result {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
        Some(command @ ("build" | "pack")) if args.len() == 4 && args[2] == "-o" => {
            if let Err(error) = native_build::build(Path::new(&args[1]), Path::new(&args[3]), command == "pack") {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
        Some("check" | "run" | "build" | "pack") => {
            eprintln!("{HELP}");
            return ExitCode::from(2);
        }
        Some(option) if option.starts_with("--") && !option.ends_with(".voil") => {
            eprintln!("{HELP}");
            return ExitCode::from(2);
        }
        _ if args.len() == 1 => return check_single(Path::new(&args[0])),
        _ => {
            eprintln!("用法錯誤：請提供唯一的 .voil 檔案路徑");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn check_single(path: &Path) -> ExitCode {
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
