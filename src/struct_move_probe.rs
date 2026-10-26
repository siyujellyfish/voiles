//! P13 S15／S21：針對真實 .voil 詞彙的明確移動「禁止形狀」檢查。
//! 只做必要拒絕，不宣稱對有效程式、結構型別或方法借用提供完整靜態證明。
//! 完整結構 AST、名稱、型別、借用與 P13 正式入口仍待整合。
#![allow(dead_code)]

use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::source::Source;

/// 這只是「禁止形狀」檢查；成功表示未發現本試驗辨識的違例，
/// 不表示整份程式文法、所有權或型別合法。
pub(crate) fn reject_forbidden_move_shapes(text: &str) -> Result<(), Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let lexed = lexer::scan(&source);
    if let Some(error) = lexed.diagnostics.into_iter().next() {
        return Err(error);
    }
    let tokens = &lexed.tokens;
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(&token.kind, TokenKind::Name(method) if method == "move") {
            continue;
        }
        if !symbol(tokens.get(index.wrapping_sub(1)), '.') {
            continue;
        }
        if !symbol(tokens.get(index + 1), '(') || !symbol(tokens.get(index + 2), ')') {
            return Err(diagnostic(&source, token,
                "P13 試驗只辨識零參數的明確 .move()"));
        }

        let receiver = tokens.get(index.wrapping_sub(2));
        if symbol(tokens.get(index.wrapping_sub(3)), '.') {
            // receiver = instance.field，S21 禁止移動部分欄位。
            return Err(diagnostic(&source, token,
                "S21：不可只移動結構實例的單一欄位"));
        }
        match receiver.map(|t| &t.kind) {
            Some(TokenKind::Name(name)) if name == "self" => {
                return Err(diagnostic(&source, token,
                    "S15：結構方法不可移動或消耗整個 self"));
            }
            Some(TokenKind::Name(_)) => {
                // 只代表形狀未觸犯 S15／S21，不證明該名稱有合法型別與所有權。
            }
            _ => {
                return Err(diagnostic(&source, token,
                    "P13 試驗不接受非具名來源的整筆移動"));
            }
        }
    }
    Ok(())
}

fn symbol(token: Option<&Token>, expected: char) -> bool {
    matches!(token.map(|value| &value.kind),
        Some(TokenKind::Symbol(actual)) if *actual == expected)
}

fn diagnostic(source: &Source, token: &Token, message: &str) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::Static,
        message: message.into(),
        span: Some(source.span(token.start, token.end)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 真實_voil_禁止方法內整筆消耗_self() {
        let source = include_str!("../tests/fixtures/p13_struct_probe/方法禁止消耗_self.voil");
        let err = reject_forbidden_move_shapes(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("S15"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 真實_voil_禁止移動單一結構欄位() {
        let source = include_str!("../tests/fixtures/p13_struct_probe/禁止單欄位移動.voil");
        let err = reject_forbidden_move_shapes(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("S21"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 未偵測到禁用形狀不代表整體語意已獲批准() {
        let source = include_str!("../tests/fixtures/p13_struct_probe/整筆具名移動形狀.voil");
        assert!(reject_forbidden_move_shapes(source).is_ok());
    }

    #[test]
    fn 字串及註解內文字不可誤判成實際移動() {
        let source = "fn main()->int{\n# self.move() 僅為註解\nvar n:int = 1\nreturn n\n}\n";
        assert!(reject_forbidden_move_shapes(source).is_ok());
        let string = "struct S{\nconst note:str = \"self.move()\"\n}\n";
        assert!(reject_forbidden_move_shapes(string).is_ok());
    }

    #[test]
    fn 非具名及含參數的移動形式不能偷渡為通過() {
        for source in [
            "fn main()->int{\nreturn helper().move()\n}\n",
            "fn main()->int{\nvar a:int = 1\nreturn (a).move()\n}\n",
            "fn main()->int{\nvar a:int = 1\nreturn a.move(1)\n}\n",
        ] {
            assert!(reject_forbidden_move_shapes(source).is_err());
        }
    }
}
