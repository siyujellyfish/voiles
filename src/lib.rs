//! voiles 最小 Rust 前端核心介面。
//! P12 已完成 P01 MVP-A 子集靜態檢查，尚無直譯或產生執行檔能力。

mod source;
mod lexer;
mod expr;
mod parser;
mod ast_debug;
mod diagnostics;
mod names;
mod types;
mod flow;
mod functions;

use std::fmt;
use std::io::ErrorKind;
use std::path::Path;

/// 單檔 P01 MVP-A 靜態檢查狀態，Passed 不代表可執行或已驗證所有權。
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

/// 從原始位元組讀取單一來源檔，依序執行 P04～P12 首版靜態檢查。
/// 若全部已核准 MVP-A 規則通過才回報 Passed；不執行 voiles，
/// 不宣稱其他完整語言功能或資源／所有權安全已驗證。
/// IO 失敗時仍回報 Failed、空診斷與真正的 ErrorKind。
pub fn check_file(path: &Path) -> CheckResult {
    match std::fs::read(path) {
        Ok(bytes) => {
            let source = source::Source::new(bytes);
            let mut diagnostics = source.lexical_diagnostics();
            let lexical = lexer::scan(&source);
            diagnostics.extend(lexical.diagnostics);
            // 只有安全完成詞彙後才建立 AST；各階段失敗不得杜撰後續靜態證明。
            if diagnostics.is_empty() {
                match parser::parse(&source, &lexical.tokens) {
                    Ok(ast) => {
                        if let Err(error) = names::resolve(&ast, &source) {
                            diagnostics.push(error);
                        } else if let Err(error) = types::check(&ast, &source) {
                            diagnostics.push(error);
                        } else if let Err(error) = flow::check(&ast, &source) {
                            diagnostics.push(error);
                        } else if let Err(error) = functions::check(&ast, &source) {
                            diagnostics.push(error);
                        }
                    }
                    Err(error) => diagnostics.push(error),
                }
            }
            diagnostics::sort_diagnostics(&mut diagnostics);
            let status = if diagnostics.is_empty() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            };
            CheckResult { status, diagnostics, io_error: None }
        },
        Err(error) => CheckResult {
            status: CheckStatus::Failed,
            diagnostics: Vec::new(),
            io_error: Some(error.kind()),
        },
    }
}
