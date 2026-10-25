//! voiles 最小 Rust 前端核心介面。
//! P02 僅處理架構和讀檔；詞彙、語法與靜態檢查交由 P04～P12。

use std::fmt;
use std::io::ErrorKind;
use std::path::Path;

/// 檢查狀態；尚無真正分析器時不准回報 Passed。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Failed,
}

/// 一基行號與一基原始 UTF-8 位元組欄位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// 起點包含、終點不包含的來源位置範圍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

/// 診斷的三種階段類別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Lexical,
    Syntax,
    Static,
}

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Lexical => "詞彙錯誤",
            Self::Syntax => "語法錯誤",
            Self::Static => "靜態錯誤",
        };
        f.write_str(label)
    }
}

/// 單筆診斷的訊息和可缺失原始來源位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: Box<str>,
    pub span: Option<SourceSpan>,
}

/// 單檔檢查結果；IO 失敗時不得產生假造的語言診斷。
#[derive(Debug)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub io_error: Option<ErrorKind>,
}

/// 以原始位元組讀取單一來源檔案。
///
/// P02 暫態：對可讀檔案也回報 Failed、空診斷、無 IO 錯誤，
/// 表示目前還沒有語言分析器，並非證明其來源語法非法。
/// 讀檔失敗時回報 Failed、空診斷和真正的 IO 錯誤種類。
pub fn check_file(path: &Path) -> CheckResult {
    match std::fs::read(path) {
        Ok(_source) => CheckResult {
            status: CheckStatus::Failed,
            diagnostics: Vec::new(),
            io_error: None,
        },
        Err(error) => CheckResult {
            status: CheckStatus::Failed,
            diagnostics: Vec::new(),
            io_error: Some(error.kind()),
        },
    }
}
