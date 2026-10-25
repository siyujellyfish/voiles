//! P02 Rust 工程架構冒煙測試，不判斷 voiles 原始程式的合法性。

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use voiles_core::{
    check_file, CheckStatus, Diagnostic, DiagnosticKind, SourcePosition, SourceSpan,
};

#[test]
fn 中文診斷分類與半開位置資料可由獨立測試存取() {
    assert_eq!(DiagnosticKind::Lexical.to_string(), "詞彙錯誤");
    assert_eq!(DiagnosticKind::Syntax.to_string(), "語法錯誤");
    assert_eq!(DiagnosticKind::Static.to_string(), "靜態錯誤");
    let span = SourceSpan {
        start: SourcePosition { line: 1, column: 2 },
        end: SourcePosition { line: 1, column: 5 },
    };
    let diagnostic = Diagnostic {
        kind: DiagnosticKind::Syntax,
        message: Box::<str>::from("測試文字"),
        span: Some(span),
    };
    assert_eq!(diagnostic.span, Some(span));
    assert_eq!(&*diagnostic.message, "測試文字");
}

#[test]
fn 可讀來源不會被未完成前端虛報為通過() {
    let result = check_file(Path::new("tests/fixtures/voiles/P01-正01.voil"));
    assert_eq!(result.status, CheckStatus::Failed);
    assert!(result.io_error.is_none());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn 無法讀取檔案保留真實_io_錯誤種類() {
    let result = check_file(Path::new("tests/fixtures/voiles/__p02_missing_file__.voil"));
    assert_eq!(result.status, CheckStatus::Failed);
    assert_eq!(result.io_error, Some(ErrorKind::NotFound));
    assert!(result.diagnostics.is_empty());
}

fn 執行命令(args: &[&str]) -> std::process::Output {
    let path = std::env::var_os("VOILES_CLI")
        .expect("P02 測試須指定 VOILES_CLI");
    Command::new(path)
        .args(args)
        .output()
        .expect("voiles 命令須可執行")
}

#[test]
fn 命令缺少多餘引數及非_voil_副檔名皆失敗() {
    for args in [
        Vec::<&str>::new(),
        vec!["a.voil", "b.voil"],
        vec!["not_voiles.txt"],
    ] {
        let output = 執行命令(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn 命令辨識_io_錯誤並說明前端仍未實作() {
    let missing = 執行命令(&["tests/fixtures/voiles/__p02_missing_file__.voil"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8(missing.stderr).unwrap().contains("檔案讀取錯誤"));

    let readable = 執行命令(&["tests/fixtures/voiles/P01-正01.voil"]);
    assert_eq!(readable.status.code(), Some(1));
    assert!(readable.stdout.is_empty());
    let stderr = String::from_utf8(readable.stderr).unwrap();
    assert!(stderr.contains("前端尚未實作"));
    assert!(!stderr.contains("語法錯誤"));
}
