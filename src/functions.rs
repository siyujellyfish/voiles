//! P12 MVP-A 函式簽章與單檔入口驗收。
//! 不執行或輸出 voiles 執行檔，也不假設遞迴終止／執行期堆疊。

use crate::{Diagnostic, DiagnosticKind};
use crate::parser::{File, Item, StmtKind, Type};
use crate::source::Source;

pub(crate) fn check(file: &File, source: &Source) -> Result<(), Diagnostic> {
    let mut main_valid = false;
    for item in &file.items {
        if let Item::Function(f) = item {
            if f.name == "main" {
                if f.returns != Type::Int {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "首版入口 fn main() 必須標註 -> int".into(),
                        span: Some(source.span(f.start, f.end)),
                    });
                }
                main_valid = true;
            }
            // P07 明確要求最外層的最後一條語句為 return，
            // P10 已逐一驗證所有 return 的來源型別與函式標註相同。
            if !matches!(f.body.statements.last().map(|s| &s.kind), Some(StmtKind::Return(_))) {
                return Err(Diagnostic {
                    kind: DiagnosticKind::Syntax,
                    message: "函式最外層末尾必須有明確 return".into(),
                    span: Some(source.span(f.start, f.end)),
                });
            }
        }
    }
    if !main_valid {
        // 即使 P09 已作相同存在性驗證，仍保留 P12 入口不變量。
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "首版來源必須有 fn main()->int".into(),
            span: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, names, parser, source, types, flow};

    fn check_source(text: &str) -> Result<(), Diagnostic> {
        let source = source::Source::new(text.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        assert!(lexed.diagnostics.is_empty(), "測試案例必須安全完成詞彙分析");
        let ast = parser::parse(&source, &lexed.tokens)?;
        names::resolve(&ast, &source)?;
        types::check(&ast, &source)?;
        flow::check(&ast, &source)?;
        check(&ast, &source)
    }

    #[test]
    fn p01_十九份首版正向案例完成主函式與回傳檢查() {
        for n in 1..=19 {
            let path = format!("tests/fixtures/voiles/P01-正{n:02}.voil");
            let s = std::fs::read_to_string(&path).unwrap();
            assert!(check_source(&s).is_ok(), "函式驗收失敗：{path}");
        }
    }

    #[test]
    fn main_必須回傳_int_不得只是一個_bool_函式() {
        let source = "fn main()->bool{\nreturn true\n}\n";
        let e = check_source(source).unwrap_err();
        assert_eq!(e.kind, DiagnosticKind::Static);
        assert!(e.span.is_some());
        assert!(e.message.contains("main"));
    }

    #[test]
    fn 獨立函式可以回傳_bool_而_main_保持_int() {
        assert!(check_source(
            "fn okay()->bool{\nreturn true\n}\nfn main()->int{\nreturn 0\n}\n"
        ).is_ok());
    }

    #[test]
    fn 遞迴與前向函式宣告在靜態檢查階段允許() {
        assert!(check_source(
            "fn first()->int{\nreturn second()\n}\nfn second()->int{\nreturn first()\n}\nfn main()->int{\nreturn 0\n}\n"
        ).is_ok());
    }

    #[test]
    fn 分支可提前_return_但函式最外層仍須尾端_return() {
        assert!(check_source(
            "fn main()->int{\nif (true){\nreturn 1\n}else{\nreturn 2\n}\nreturn 0\n}\n"
        ).is_ok());
        let source = source::Source::new(b"fn main()->int{\nif (true){\nreturn 1\n}\n}\n".to_vec());
        let lex = lexer::scan(&source);
        let err = parser::parse(&source, &lex.tokens).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Syntax);
    }
}
