//! P13：以真正 .voil 詞彙解析結構欄位，建立 S19 複製能力的私有試驗。
//! 這不是完整 struct 文法、公開 API 或 P13 Phase 驗收。
//! 欄位型別的複製資格必須由呼叫端提供，不得在這裡猜測。
#![allow(dead_code)]

use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::CopyPermission;
use crate::source::Source;
use crate::struct_ownership::{self, StructOwnershipError};

struct StructParser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    cursor: usize,
}

impl StructParser<'_> {
    fn token(&self) -> &Token {
        // 詞彙器一定會產生 EOF。
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }

    fn error(&self, message: &str) -> Diagnostic {
        let token = self.token();
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: message.into(),
            span: Some(self.source.span(token.start, token.end)),
        }
    }

    fn is_symbol(&self, expected: char) -> bool {
        matches!(self.token().kind, TokenKind::Symbol(value) if value == expected)
    }

    fn is_keyword(&self, expected: &str) -> bool {
        matches!(&self.token().kind, TokenKind::Keyword(value) if value == expected)
    }

    fn consume_symbol(&mut self, expected: char, message: &str) -> Result<(), Diagnostic> {
        if !self.is_symbol(expected) { return Err(self.error(message)); }
        self.cursor += 1;
        Ok(())
    }

    fn name(&mut self) -> Result<String, Diagnostic> {
        match &self.token().kind {
            TokenKind::Name(name) => {
                let value = name.clone();
                self.cursor += 1;
                Ok(value)
            }
            _ => Err(self.error("結構宣告需要合法名稱")),
        }
    }

    fn newlines(&mut self) {
        while matches!(self.token().kind, TokenKind::Newline) { self.cursor += 1; }
    }

    fn initializer(&mut self, ty: &str) -> Result<(), Diagnostic> {
        // 只接受可靜態確認型別相同的單一字面值；不暗中放行無效初值。
        let valid = match &self.token().kind {
            TokenKind::Integer(_) => ty == "int",
            TokenKind::Decimal(_) => ty == "float",
            TokenKind::StringLiteral(_) => ty == "str",
            TokenKind::Keyword(value) if value == "true" || value == "false" => ty == "bool",
            _ => false,
        };
        if !valid {
            return Err(self.error("結構初值試驗僅接受與標註型別相同的單一字面值"));
        }
        self.cursor += 1;
        Ok(())
    }

    fn parse(mut self) -> Result<Vec<String>, Diagnostic> {
        use std::collections::HashSet;

        self.newlines();
        if !self.is_keyword("struct") {
            return Err(self.error("P13 結構試驗需要 struct 宣告"));
        }
        self.cursor += 1;
        let _struct_name = self.name()?;
        self.consume_symbol('{', "結構名稱後需要左大括號；泛型結構未開放")?;
        self.newlines();

        let mut seen = HashSet::new();
        let mut types = Vec::new();

        while !self.is_symbol('}') {
            if matches!(self.token().kind, TokenKind::End) {
                return Err(self.error("結構宣告缺少右大括號"));
            }
            if self.is_keyword("struct") {
                return Err(self.error("結構內不得宣告巢狀 struct"));
            }
            if self.is_keyword("share") {
                return Err(self.error("結構欄位只接受 var 或 const，不接受 share"));
            }
            if self.is_keyword("fn") || self.is_keyword("async") {
                return Err(self.error("P13 結構欄位試驗尚未涵蓋方法，不能宣稱已驗收 self"));
            }
            let immutable = self.is_keyword("const");
            if !immutable && !self.is_keyword("var") {
                return Err(self.error("結構欄位必須以 var 或 const 宣告"));
            }
            self.cursor += 1;
            let name = self.name()?;
            if !seen.insert(name) {
                return Err(self.error("同一個結構不得重複欄位名稱"));
            }
            self.consume_symbol(':', "欄位宣告需要型別標註")?;
            let ty = self.name()?;
            if self.is_symbol('=') {
                self.cursor += 1;
                self.initializer(&ty)?;
            } else if immutable {
                return Err(self.error("const 結構欄位必須在宣告時提供預設值"));
            }
            types.push(ty);

            if self.is_symbol('}') { break; }
            if !matches!(self.token().kind, TokenKind::Newline) {
                return Err(self.error("結構欄位之間必須換行"));
            }
            self.newlines();
        }
        self.consume_symbol('}', "結構宣告缺少右大括號")?;
        self.newlines();
        if !matches!(self.token().kind, TokenKind::End) {
            return Err(self.error("此私有試驗僅接受單一頂層 struct，不解析完整程式"));
        }
        Ok(types)
    }
}

/// 此處傳入的 abilities 僅能是「外部已確認」的型別能力；
/// 實驗呼叫者可提供測試映射，但不能據此核准完整語言的 Copyable 清單。
pub(crate) fn inspect_struct_copy(
    text: &str,
    abilities: impl Fn(&str) -> CopyPermission,
) -> Result<(), Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let scanned = lexer::scan(&source);
    if let Some(error) = scanned.diagnostics.into_iter().next() {
        return Err(error);
    }
    let types = StructParser {
        source: &source,
        tokens: &scanned.tokens,
        cursor: 0,
    }.parse()?;
    let permissions: Vec<_> = types.iter().map(|ty| abilities(ty)).collect();
    struct_ownership::whole_struct_copy(&permissions).map_err(|error| {
        let message = match error {
            StructOwnershipError::FieldCopyForbidden =>
                "至少一個欄位型別已確認不可複製，結構整體不可預設複製",
            StructOwnershipError::FieldCopyUndecided =>
                "仍有欄位型別的 Copyable 資格未核准，不可允許結構整體複製",
            _ => "結構複製能力不符合已核准規則",
        };
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: message.into(),
            span: Some(source.span(0, source.bytes.len())),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::CopyPermission::{Allowed, Forbidden, Undecided};

    const SIMPLE: &str =
        "struct Sample{\nvar number:int\nconst flag:bool = true\n}\n";

    #[test]
    fn 真實_voil_結構欄位需全部有外部授權才可整體複製() {
        assert!(inspect_struct_copy(SIMPLE, |name| match name {
            "int" | "bool" => Allowed, _ => Undecided,
        }).is_ok());
    }

    #[test]
    fn 真實_voil_未知及禁止複製的欄位均應阻止整體複製() {
        for expected in [Undecided, Forbidden] {
            let error = inspect_struct_copy(SIMPLE, |name| {
                if name == "int" { Allowed } else { expected }
            }).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn 真實_voil_結構欄位遵守_s01_s02_s03_及_const_初值() {
        for invalid in [
            "struct S{\nshare x:int\n}\n",
            "struct S{\nvar x:int\nvar x:int\n}\n",
            "struct S{\nstruct Inner{\nvar x:int\n}\n}\n",
            "struct S{\nconst x:int\n}\n",
            "struct S<T>{\nvar x:int\n}\n",
            "struct S{\nfn get()->int{\nreturn 1\n}\n}\n",
        ] {
            assert!(inspect_struct_copy(invalid, |_| Allowed).is_err(),
                "私有試驗不得誤收超出能力的結構來源：{invalid}");
        }
    }

    #[test]
    fn 真實_voil_詞彙錯誤與語法錯誤不得被當成複製能力通過() {
        for invalid in ["struct S{\nvar x:int", "struct S{\nvar x:int = \n}\n"] {
            assert!(inspect_struct_copy(invalid, |_| Allowed).is_err());
        }
    }
}
