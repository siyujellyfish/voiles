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

/// R02.13：兩套正式 Parser 共用的語句分隔規則。
/// 右大括號結束目前區塊但不可在此消耗，換行序列則須全數消耗；
/// 其它 token 一律拒絕並保留原來的錯誤游標。
pub(crate) fn finish_statement(tokens: &[Token], cursor: &mut usize) -> bool {
    if symbol(tokens, *cursor, '}') { return true; }
    if !newline(tokens, *cursor) { return false; }
    skip_newlines(tokens, cursor);
    true
}

/// R02.12：兩套正式 Parser 共用連續換行略過。
/// 只略過實際 Newline token；不吞掉 End、符號及其它詞彙。
pub(crate) fn skip_newlines(tokens: &[Token], cursor: &mut usize) {
    while newline(tokens, *cursor) { *cursor += 1; }
}

/// R02.12：公開運算式與私有結構解析器共用唯一 bool 常值判讀。
/// 不將普通 Name、其它保留字或大小寫變體當成布林常值。
pub(crate) fn boolean_keyword(word: &str) -> Option<bool> {
    match word {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
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

/// R02.11：共用真正左結合算術解析折疊。各 Parser 仍只由自身
/// 的 operand 解析器決定可用後綴、AST 型別及語法診斷；本層
/// 只在 next 成功提供一組已核准運算子／右運算元時建立左結合節點。
/// 任一右運算元解析失敗須立即中止，不可吞掉失敗變成部分 AST。
pub(crate) fn fold_additive<T, E>(
    mut left: T,
    mut next: impl FnMut() -> Result<Option<(char, T)>, E>,
    mut combine: impl FnMut(T, char, T) -> T,
) -> Result<T, E> {
    while let Some((op, right)) = next()? {
        debug_assert!(matches!(op, '+' | '-'), "僅接受核准的加減運算子");
        left = combine(left, op, right);
    }
    Ok(left)
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
    fn 布林常值不誤認其它保留字或不同大小寫名稱() {
        assert_eq!(boolean_keyword("true"), Some(true));
        assert_eq!(boolean_keyword("false"), Some(false));
        for word in ["True", "FALSE", "if", "return", "bool", "true_"] {
            assert_eq!(boolean_keyword(word), None);
        }
    }

    #[test]
    fn 共同語句分隔_右大括號不消耗_換行連續消耗_非法來源保持位置() {
        let tokens=[
            Token {kind:TokenKind::Newline,start:0,end:1},
            Token {kind:TokenKind::Newline,start:1,end:2},
            Token {kind:TokenKind::Symbol('}'),start:2,end:3},
            Token {kind:TokenKind::Keyword("return".into()),start:4,end:10},
            Token {kind:TokenKind::End,start:10,end:10},
        ];
        let mut cursor=0;
        assert!(finish_statement(&tokens,&mut cursor));
        assert_eq!(cursor,2);
        assert!(finish_statement(&tokens,&mut cursor));
        assert_eq!(cursor,2,"右大括號應由所屬 block 自己消耗");
        for pos in [3,4,5] {
            let mut cursor=pos;
            assert!(!finish_statement(&tokens,&mut cursor));
            assert_eq!(cursor,pos,"錯誤分隔不可跨越保留字或 End");
        }
    }

    #[test]
    fn 連續換行只消耗_newline_不跨越原來的其它_token() {
        let tokens = [
            Token {kind:TokenKind::Newline,start:0,end:1},
            Token {kind:TokenKind::Newline,start:1,end:2},
            Token {kind:TokenKind::Keyword("return".into()),start:2,end:8},
            Token {kind:TokenKind::Newline,start:8,end:9},
            Token {kind:TokenKind::End,start:9,end:9},
        ];
        let mut cursor=0;
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,2);
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,2,"不可跨越 return 保留字");
        cursor=3;
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,4);
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,4,"不可消耗 End");
    }

    #[test]
    fn 真正左結合折疊保留求值順序及錯誤立即傳回() {
        let mut operators = [('-', 3_i64), ('+', 2), ('-', 4)].into_iter();
        let value = fold_additive(
            10_i64,
            || Ok::<_, &'static str>(operators.next()),
            |left, op, right| if op == '+' {left + right} else {left - right},
        ).unwrap();
        assert_eq!(value, 5, "10 - 3 + 2 - 4 必須依左結合求值");

        let mut called = 0;
        let result = fold_additive(
            7_i64,
            || {
                called += 1;
                if called == 1 { Ok(Some(('+', 5))) }
                else { Err("右運算元語法錯誤") }
            },
            |left, _, right| left + right,
        );
        assert_eq!(result, Err("右運算元語法錯誤"));
        assert_eq!(called, 2, "右運算元失敗不得被忽略而輸出部分樹");
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
