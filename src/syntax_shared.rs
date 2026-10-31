//! P16.6-R02：公開 MVP-A 與私有 P13～P16 共用的詞彙判斷和 int64 字面值規則。
//! 只統一兩者均已核准的基礎契約；語法擴充、資源語意和入口權限不由此模組決定。

use crate::lexer::{Token, TokenKind};

pub(crate) fn symbol(tokens: &[Token], cursor: usize, ch: char) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Symbol(c)) if *c == ch)
}

pub(crate) fn keyword(tokens: &[Token], cursor: usize, word: &str) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Keyword(s)) if s == word)
}

pub(crate) fn newline(tokens: &[Token], cursor: usize) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Newline))
}

pub(crate) fn end(tokens: &[Token], cursor: usize) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::End) | None)
}

/// R02.7：真正供公開 P06 與私有 P13 運算式解析器共用的
/// 整數加減運算子判讀，僅允許兩者均已核准的 +／-。
/// 左結合及來源跨度由呼叫者既有遞迴解析程序維持。
pub(crate) fn additive_operator(tokens: &[Token], cursor: usize) -> Option<char> {
    match tokens.get(cursor).map(|token| &token.kind) {
        Some(TokenKind::Symbol(ch)) if *ch == '+' || *ch == '-' => Some(*ch),
        _ => None,
    }
}

/// R02.10：公開檔案 Parser、公開算式 Parser、私有 P13 Parser
/// 真正共用的符號消耗原語。僅精確匹配指定 Symbol 時推進一次，
/// 並返回來源原有 token 終點；失敗不消耗任何符號或改寫錯誤位置。
pub(crate) fn take_symbol(
    tokens: &[Token], cursor: &mut usize, expected: char,
) -> Option<usize> {
    let token = tokens.get(*cursor)?;
    if !matches!(&token.kind, TokenKind::Symbol(ch) if *ch == expected) {
        return None;
    }
    *cursor += 1;
    Some(token.end)
}

/// R02.8：公開與私有 Parser 實際共用的名稱 token 消耗原語。
/// 僅在 Name 時推進游標，失敗不能消耗任何 token；
/// 診斷文字／位置由呼叫端保留原先所屬的解析模式。
pub(crate) fn take_name(tokens: &[Token], cursor: &mut usize) -> Option<String> {
    let TokenKind::Name(value) = &tokens.get(*cursor)?.kind else { return None };
    *cursor += 1;
    Some(value.clone())
}

/// R02.9：公開 MVP-A 的純值型別消耗原語，唯一合法集合由
/// 共通 ScalarType 定義。私有 P13 的 str／struct 型別不由此函式擴張。
/// 若 token 不屬 int／bool，游標及錯誤位置必須完全不變。
pub(crate) fn take_scalar_type(
    tokens: &[Token], cursor: &mut usize,
) -> Option<crate::common_ir::ScalarType> {
    let TokenKind::Name(name) = &tokens.get(*cursor)?.kind else { return None };
    let ty = crate::common_ir::ScalarType::from_name(name)?;
    *cursor += 1;
    Some(ty)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerError {
    InvalidDigits,
    OutOfRange,
}

/// 不更動十進位、二／八／十六進位、底線分隔與有號 64 位元上限的既有規則。
pub(crate) fn integer_i64(literal: &str) -> Result<i64, IntegerError> {
    let (radix, digits) = if let Some(rest) = literal.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = literal.strip_prefix("0b") {
        (2, rest)
    } else if let Some(rest) = literal.strip_prefix("0o") {
        (8, rest)
    } else {
        (10, literal)
    };
    let normalized: String = digits.chars().filter(|c| *c != '_').collect();
    if normalized.is_empty() || !normalized.chars().all(|c| c.to_digit(radix).is_some()) {
        return Err(IntegerError::InvalidDigits);
    }
    let value = u128::from_str_radix(&normalized, radix)
        .map_err(|_| IntegerError::OutOfRange)?;
    if value > i64::MAX as u128 {
        return Err(IntegerError::OutOfRange);
    }
    Ok(value as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, source::Source};

    #[test]
    fn 指定符號只在完全匹配時移動且回傳原本_utf8_位元組終點() {
        let tokens=[
            Token {kind:TokenKind::Name("字".into()),start:0,end:3},
            Token {kind:TokenKind::Symbol('('),start:3,end:4},
            Token {kind:TokenKind::Symbol(')'),start:4,end:5},
            Token {kind:TokenKind::End,start:5,end:5},
        ];
        let mut cursor=0;
        assert_eq!(take_symbol(&tokens,&mut cursor,'('),None);
        assert_eq!(cursor,0);
        cursor=1;
        assert_eq!(take_symbol(&tokens,&mut cursor,')'),None);
        assert_eq!(cursor,1);
        assert_eq!(take_symbol(&tokens,&mut cursor,'('),Some(4));
        assert_eq!(cursor,2);
        assert_eq!(take_symbol(&tokens,&mut cursor,')'),Some(5));
        assert_eq!(cursor,3);
        for pos in [3,4] {
            let mut at=pos;
            assert_eq!(take_symbol(&tokens,&mut at,'('),None);
            assert_eq!(at,pos,"結束 token／超界不可以推進游標");
        }
    }

    #[test]
    fn 公開純值型別須由共同_scalar_type_決定且拒絕私有型別() {
        let tokens=[
            Token {kind:TokenKind::Name("int".into()),start:0,end:3},
            Token {kind:TokenKind::Name("bool".into()),start:4,end:8},
            Token {kind:TokenKind::Name("str".into()),start:9,end:12},
            Token {kind:TokenKind::Name("Custom".into()),start:13,end:19},
            Token {kind:TokenKind::Keyword("return".into()),start:20,end:26},
            Token {kind:TokenKind::End,start:26,end:26},
        ];
        let mut cursor=0;
        assert_eq!(take_scalar_type(&tokens,&mut cursor),
            Some(crate::common_ir::ScalarType::Int));
        assert_eq!(take_scalar_type(&tokens,&mut cursor),
            Some(crate::common_ir::ScalarType::Bool));
        assert_eq!(cursor,2);
        for _ in 0..3 {
            assert_eq!(take_scalar_type(&tokens,&mut cursor),None);
            assert_eq!(cursor,2,"私有 str 不能成為公開 MVP-A 型別");
        }
        for pos in 3..=tokens.len() {
            let mut at=pos;
            assert_eq!(take_scalar_type(&tokens,&mut at),None);
            assert_eq!(at,pos);
        }
    }

    #[test]
    fn 名稱原語只消耗真正_name_其他_token_及結束位置不變() {
        let tokens = [
            Token {kind:TokenKind::Name("水母_7".into()),start:0,end:8},
            Token {kind:TokenKind::Symbol('('),start:8,end:9},
            Token {kind:TokenKind::Keyword("return".into()),start:9,end:15},
            Token {kind:TokenKind::End,start:15,end:15},
        ];
        let mut cursor=0usize;
        assert_eq!(take_name(&tokens,&mut cursor),Some("水母_7".into()));
        assert_eq!(cursor,1);
        for _ in 0..3 {
            assert_eq!(take_name(&tokens,&mut cursor),None);
            assert_eq!(cursor,1,"不合法名稱絕不能跳過原先錯誤位置");
        }
        for position in [2,3,4] {
            let mut cursor=position;
            assert_eq!(take_name(&tokens,&mut cursor),None);
            assert_eq!(cursor,position);
        }
    }

    #[test]
    fn 兩套真正解析器共用的加減規則只能回傳核准二元運算子() {
        let source = Source::new(b"+ - * /".to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        assert_eq!(additive_operator(&tokens.tokens, 0), Some('+'));
        assert_eq!(additive_operator(&tokens.tokens, 1), Some('-'));
        for cursor in 2..=tokens.tokens.len() {
            assert_eq!(additive_operator(&tokens.tokens, cursor), None);
        }
    }

    #[test]
    fn p16_6_r02_字面值正反界線在共用規則中一致() {
        for (raw, expected) in [
            ("0", Ok(0)), ("001", Ok(1)), ("0x2a", Ok(42)),
            ("0b101010", Ok(42)), ("0o52", Ok(42)),
            ("9_223_372_036_854_775_807", Ok(i64::MAX)),
            ("0x", Err(IntegerError::InvalidDigits)),
            ("0b102", Err(IntegerError::InvalidDigits)),
            ("9223372036854775808", Err(IntegerError::OutOfRange)),
        ] {
            assert_eq!(integer_i64(raw), expected, "整數 {raw}");
        }
    }

    #[test]
    fn p16_6_r02_兩種解析模式使用相同詞彙位置及結束標記() {
        let source = Source::new(b"fn main()->int{\nreturn 0\n}\n".to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        assert!(keyword(&tokens.tokens, 0, "fn"));
        assert!(!symbol(&tokens.tokens, 0, 'f'));
        let end_pos = tokens.tokens.len() - 1;
        assert!(end(&tokens.tokens, end_pos));
        assert!(end(&tokens.tokens, tokens.tokens.len()));
        assert!(!newline(&tokens.tokens, 0));
        assert!(tokens.tokens.iter().enumerate().any(|(i, _)| newline(&tokens.tokens, i)));
    }
}
