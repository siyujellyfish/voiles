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
mod ownership_model;
mod ownership_policy;
mod container_copy_policy;
mod borrow_safety;
mod borrow_ownership_gate;
mod borrow_lifetime_probe;
mod resource_obligation_audit;
mod resource_borrow_pipeline;
mod owned_str_runtime;
mod ownership_ledger;
mod ownership_probe;
mod struct_ownership;
mod struct_source_probe;
mod struct_move_probe;
mod struct_stage;
mod struct_eval_plan;
mod struct_initialization;

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

#[cfg(test)]
mod p13_private_boundary_tests {
    use super::*;

    /// 以真正的檔案路徑驗證對外公開 API，而非僅測試 parser。
    fn check_public_source(source_text: &str, suffix: &str) -> CheckResult {
        let file = std::env::temp_dir().join(format!(
            "voiles_p13_private_{}_{}.voil", std::process::id(), suffix
        ));
        std::fs::write(&file, source_text).expect("應可建立單次驗證的暫存來源");
        let result = check_file(&file);
        std::fs::remove_file(&file).expect("應移除本次建立的暫存來源");
        result
    }

    #[test]
    fn 方案_b_公開_check_file_仍拒絕真實私有_struct_正例() {
        let src =
            include_str!("../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        assert!(struct_stage::inspect(src).is_ok());
        let result = check_public_source(src, "struct");
        assert_eq!(result.status, CheckStatus::Failed);
        assert!(result.io_error.is_none());
        assert!(result.diagnostics.iter().any(|diag| diag.kind == DiagnosticKind::Syntax));
    }

    #[test]
    fn 方案_b_公開_check_file_不接受內部_move_正例_但保留_mvp_a() {
        let move_src = "fn main()->int{\nvar x:int = 1\nreturn x.move()\n}\n";
        assert!(struct_stage::inspect(move_src).is_ok());
        let denied = check_public_source(move_src, "move");
        assert_eq!(denied.status, CheckStatus::Failed);
        assert!(denied.diagnostics.iter().any(|diag| diag.kind == DiagnosticKind::Syntax));
        let passed = check_public_source("fn main()->int{\nreturn 0\n}\n", "mvp");
        assert_eq!(passed.status, CheckStatus::Passed);
        assert!(passed.diagnostics.is_empty());
    }

    #[test]
    fn d01_b_容器類別能力並不開放其語法() {
        for (ty, suffix) in [("list[int]", "d01list"), ("dict{str,int}", "d01dict")] {
            assert_eq!(ownership_policy::approved_leaf_copy_permission(ty),
                ownership_model::CopyPermission::Undecided);
            let program = format!("fn main()->int{{\nvar item:{ty}\nreturn 0\n}}\n");
            assert!(struct_stage::inspect(&program).is_err());
            let output = check_public_source(&program, suffix);
            assert_eq!(output.status, CheckStatus::Failed);
            assert!(output.diagnostics.iter().any(|d| d.kind == DiagnosticKind::Syntax));
        }
    }

    #[test]
    fn 方案_b_真實結構安全子集仍只能在私有檢查器通過() {
        let source_text =
            include_str!("../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        assert!(struct_stage::inspect(source_text).is_ok());
        let source = source::Source::new(source_text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let error = parser::parse(&source, &tokens.tokens).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax);
    }

    #[test]
    fn 方案_b_明確整筆移動只通過私有檢查_不偷渡到公開_mvp_a() {
        let source_text = "fn main()->int{\nvar n:int = 1\nvar m:int = n.move()\nreturn m\n}\n";
        assert!(struct_stage::inspect(source_text).is_ok());
        let source = source::Source::new(source_text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let error = parser::parse(&source, &tokens.tokens).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax);
    }
}
