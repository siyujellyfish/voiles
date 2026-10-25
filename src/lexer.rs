//! P05 內部詞彙器：保留來源跨度，不定義未核准語言語意。

use crate::{Diagnostic, DiagnosticKind};
use crate::source::{Source, Unit, UnitKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Keyword(String),
    Name(String),
    Integer(String),
    Decimal(String),
    StringLiteral(String),
    Symbol(char),
    Arrow,
    Range,
    Newline,
    Invalid,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug)]
pub(crate) struct LexResult {
    pub(crate) tokens: Vec<Token>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

struct Lexer<'a> {
    source: &'a Source,
    cursor: usize,
    parentheses: usize,
    braces: Vec<usize>,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn peek(&self, offset: usize) -> Option<char> {
        match self.source.units.get(self.cursor + offset).map(|u| u.kind) {
            Some(UnitKind::Valid(c)) => Some(c),
            _ => None,
        }
    }

    fn unit(&self) -> Option<Unit> {
        self.source.units.get(self.cursor).copied()
    }

    fn emit(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token { kind, start, end });
    }

    fn report(&mut self, kind: DiagnosticKind, text: &str, start: usize, end: usize) {
        self.diagnostics.push(Diagnostic {
            kind,
            message: Box::from(text),
            span: Some(self.source.span(start, end)),
        });
    }

    fn new(source: &'a Source) -> Self {
        Self {
            source,
            cursor: 0,
            parentheses: 0,
            braces: Vec::new(),
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn go(mut self) -> LexResult {
        while let Some(unit) = self.unit() {
            let start = unit.start;
            let ch = match unit.kind {
                UnitKind::Valid(ch) => ch,
                UnitKind::Invalid(_) => {
                    // 來源層 P04 已對每個原始非法位元組生成一次詞彙錯誤。
                    // 專用無效標記保留至語法層，不讓前後詞彙被合併。
                    self.emit(TokenKind::Invalid, start, unit.end);
                    self.cursor += 1;
                    continue;
                }
            };
            match ch {
                ' ' | '\t' => { self.cursor += 1; }
                '\r' | '\n' => self.newline(start, ch),
                '#' => self.line_comment(),
                '"' if self.peek(1) == Some('"') && self.peek(2) == Some('"') => {
                    self.multiline_comment(start);
                }
                '"' => self.string(start),
                'a'..='z' | 'A'..='Z' | '_' => self.name(start),
                '0'..='9' => self.number(start),
                '-' if self.peek(1) == Some('>') => {
                    self.cursor += 2;
                    self.emit(TokenKind::Arrow, start, self.previous_end());
                }
                '.' if self.peek(1) == Some('.') => {
                    self.cursor += 2;
                    self.emit(TokenKind::Range, start, self.previous_end());
                }
                '(' => {
                    self.parentheses += 1;
                    self.cursor += 1;
                    self.emit(TokenKind::Symbol('('), start, unit.end);
                }
                ')' => {
                    self.parentheses = self.parentheses.saturating_sub(1);
                    self.cursor += 1;
                    self.emit(TokenKind::Symbol(')'), start, unit.end);
                }
                '{' => {
                    self.braces.push(start);
                    self.cursor += 1;
                    self.emit(TokenKind::Symbol('{'), start, unit.end);
                }
                '}' => {
                    if self.braces.pop().is_none() {
                        self.report(DiagnosticKind::Syntax, "不成對的區塊右大括號", start, unit.end);
                    }
                    self.cursor += 1;
                    self.emit(TokenKind::Symbol('}'), start, unit.end);
                }
                '\u{FFFD}' => {
                    // P04 對合法編碼的原始 U+FFFD 已有詞彙診斷。
                    self.cursor += 1;
                    self.emit(TokenKind::Invalid, start, unit.end);
                }
                other => {
                    self.cursor += 1;
                    self.emit(TokenKind::Symbol(other), start, unit.end);
                }
            }
        }
        for start in self.braces.drain(..).collect::<Vec<_>>() {
            self.report(DiagnosticKind::Syntax, "區塊左大括號缺少右大括號", start, start + 1);
        }
        let end = self.source.bytes.len();
        self.emit(TokenKind::End, end, end);
        LexResult { tokens: self.tokens, diagnostics: self.diagnostics }
    }

    fn previous_end(&self) -> usize {
        self.source.units[self.cursor - 1].end
    }

    fn newline(&mut self, start: usize, ch: char) {
        if ch == '\r' && self.peek(1) == Some('\n') {
            self.cursor += 2;
        } else {
            self.cursor += 1;
        }
        // 圓括號內的換行不是語句分隔。
        if self.parentheses == 0 {
            self.emit(TokenKind::Newline, start, self.previous_end());
        }
    }

    fn line_comment(&mut self) {
        while let Some(ch) = self.peek(0) {
            if ch == '\r' || ch == '\n' { break; }
            self.cursor += 1;
        }
    }

    fn multiline_comment(&mut self, start: usize) {
        self.cursor += 3;
        while self.cursor < self.source.units.len() {
            if self.peek(0) == Some('"') &&
                self.peek(1) == Some('"') && self.peek(2) == Some('"') {
                self.cursor += 3;
                return;
            }
            self.cursor += 1;
        }
        self.report(DiagnosticKind::Syntax, "多行註解缺少結束標記", start, self.source.bytes.len());
    }

    fn name(&mut self, start: usize) {
        let mut value = String::new();
        while let Some(ch) = self.peek(0) {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                value.push(ch);
                self.cursor += 1;
            } else { break; }
        }
        const WORDS: &[&str] = &[
            "fn", "const", "var", "share", "if", "else", "return",
            "true", "false", "struct", "async", "while", "for", "import",
            "and", "or", "not", "none", "break", "continue", "try", "except",
        ];
        let kind = if WORDS.contains(&value.as_str()) {
            TokenKind::Keyword(value)
        } else {
            TokenKind::Name(value)
        };
        self.emit(kind, start, self.previous_end());
    }

    fn number(&mut self, start: usize) {
        let mut digits = String::new();
        let radix = if self.peek(0) == Some('0') {
            match self.peek(1) {
                Some('x') => Some('x'),
                Some('b') => Some('b'),
                Some('o') => Some('o'),
                _ => None,
            }
        } else { None };
        if let Some(prefix) = radix {
            digits.push('0');
            digits.push(prefix);
            self.cursor += 2;
            while let Some(ch) = self.peek(0) {
                let valid = match prefix {
                    'x' => ch.is_ascii_hexdigit(),
                    'b' => matches!(ch, '0' | '1'),
                    'o' => matches!(ch, '0'..='7'),
                    _ => false,
                };
                if ch == '_' || valid {
                    digits.push(ch);
                    self.cursor += 1;
                } else { break; }
            }
            self.emit(TokenKind::Integer(digits), start, self.previous_end());
            return;
        }
        while let Some(ch) = self.peek(0) {
            if ch.is_ascii_digit() || ch == '_' {
                digits.push(ch);
                self.cursor += 1;
            } else { break; }
        }
        if self.peek(0) == Some('.') && self.peek(1) != Some('.') {
            digits.push('.');
            self.cursor += 1;
            while let Some(ch) = self.peek(0) {
                if ch.is_ascii_digit() || ch == '_' {
                    digits.push(ch);
                    self.cursor += 1;
                } else { break; }
            }
            self.emit(TokenKind::Decimal(digits), start, self.previous_end());
        } else {
            self.emit(TokenKind::Integer(digits), start, self.previous_end());
        }
    }

    fn string(&mut self, start: usize) {
        self.cursor += 1; // 起始雙引號
        let mut result = String::new();
        while let Some(ch) = self.peek(0) {
            if ch == '"' {
                self.cursor += 1;
                self.emit(TokenKind::StringLiteral(result), start, self.previous_end());
                return;
            }
            if ch == '\r' || ch == '\n' {
                // 未批准字串換行語法，保守不吞掉後續整段程式。
                self.report(DiagnosticKind::Syntax, "字串缺少結束雙引號", start, self.previous_end());
                self.emit(TokenKind::Invalid, start, self.previous_end());
                return;
            }
            if ch == '\\' {
                let escape_start = self.unit().unwrap().start;
                self.cursor += 1;
                match self.peek(0) {
                    Some('u') => {
                        self.cursor += 1;
                        result.push(self.unicode_escape(escape_start));
                    }
                    Some('"') => { result.push('"'); self.cursor += 1; }
                    Some('\\') => { result.push('\\'); self.cursor += 1; }
                    Some('n') => { result.push('\n'); self.cursor += 1; }
                    Some('r') => { result.push('\r'); self.cursor += 1; }
                    Some('t') => { result.push('\t'); self.cursor += 1; }
                    Some(other) => {
                        self.cursor += 1;
                        self.report(DiagnosticKind::Lexical, "字串含未知跳脫序列", escape_start, self.previous_end());
                        result.push(other);
                    }
                    None => {
                        self.report(DiagnosticKind::Syntax, "字串跳脫不完整", escape_start, self.source.bytes.len());
                        break;
                    }
                }
            } else {
                result.push(ch);
                self.cursor += 1;
            }
        }
        self.report(DiagnosticKind::Syntax, "字串缺少結束雙引號", start, self.source.bytes.len());
        self.emit(TokenKind::Invalid, start, self.source.bytes.len());
    }

    /// 已核准的 Unicode 轉義：底線視為替代值，空白與過長是語法錯誤。
    fn unicode_escape(&mut self, start: usize) -> char {
        if self.peek(0) != Some('{') {
            self.report(DiagnosticKind::Syntax, "Unicode 跳脫缺少左大括號", start, self.previous_end());
            return '\u{FFFD}';
        }
        self.cursor += 1;
        let mut value = String::new();
        while let Some(ch) = self.peek(0) {
            if ch == '}' {
                self.cursor += 1;
                let end = self.previous_end();
                if value.chars().any(char::is_whitespace) {
                    self.report(DiagnosticKind::Syntax, "Unicode 跳脫內不允許空白", start, end);
                    return '\u{FFFD}';
                }
                if value.chars().filter(|c| c.is_ascii_hexdigit()).count() > 6 {
                    self.report(DiagnosticKind::Syntax, "Unicode 跳脫最多六位十六進位數字", start, end);
                    return '\u{FFFD}';
                }
                if value.contains('_') { return '\u{FFFD}'; }
                let decoded = u32::from_str_radix(&value, 16).ok().and_then(char::from_u32);
                return decoded.unwrap_or('\u{FFFD}');
            }
            if ch == '\r' || ch == '\n' {
                self.report(DiagnosticKind::Syntax, "Unicode 跳脫缺少右大括號", start, self.previous_end());
                return '\u{FFFD}';
            }
            value.push(ch);
            self.cursor += 1;
        }
        self.report(DiagnosticKind::Syntax, "Unicode 跳脫缺少右大括號", start, self.source.bytes.len());
        '\u{FFFD}'
    }
}

pub(crate) fn scan(source: &Source) -> LexResult {
    Lexer::new(source).go()
}

#[cfg(test)]
mod tests {
    use super::{scan, TokenKind};
    use crate::{source::Source, DiagnosticKind};

    fn kinds(input: &str) -> Vec<TokenKind> {
        scan(&Source::new(input.as_bytes().to_vec())).tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn 首版關鍵字_識別字_數值_大括號和換行分離() {
        let tokens = kinds("fn main()->int{\n  return 0\n}\n");
        assert!(matches!(&tokens[0], TokenKind::Keyword(s) if s=="fn"));
        assert!(matches!(&tokens[1], TokenKind::Name(s) if s=="main"));
        assert!(matches!(tokens[2], TokenKind::Symbol('(')));
        assert!(matches!(tokens[3], TokenKind::Symbol(')')));
        assert!(matches!(tokens[4], TokenKind::Arrow));
        assert!(matches!(tokens[6], TokenKind::Symbol('{')));
        assert!(tokens.iter().any(|t| matches!(t,TokenKind::Newline)));
        assert!(matches!(tokens.last().unwrap(), TokenKind::End));
    }

    #[test]
    fn 可分辨連續底線與不同進位前綴() {
        let ts = kinds("0x_FF 0b1010 0o77 1__000 007\n");
        let vals: Vec<String> = ts.into_iter().filter_map(|k| if let TokenKind::Integer(v)=k {Some(v)} else {None}).collect();
        assert_eq!(vals, ["0x_FF","0b1010","0o77","1__000","007"]);
    }

    #[test]
    fn 浮點詞彙與區間優先辨識不可混淆() {
        let ts = kinds("1..4 1. 00.5\n");
        assert_eq!(ts[0], TokenKind::Integer("1".into()));
        assert_eq!(ts[1], TokenKind::Range);
        assert_eq!(ts[2], TokenKind::Integer("4".into()));
        assert_eq!(ts[3], TokenKind::Decimal("1.".into()));
        assert_eq!(ts[4], TokenKind::Decimal("00.5".into()));
    }

    #[test]
    fn 圓括號內換行是合法續行且註解不產生語句() {
        let ts = kinds("fn main(\n){ # test\n\"\"\"multi\nline\"\"\"\n return 0\n}\n");
        assert!(ts.iter().filter(|t| matches!(t,TokenKind::Newline)).count() >= 3);
        assert!(!ts.iter().any(|t| matches!(t,TokenKind::StringLiteral(_))));
    }

    #[test]
    fn 已核准的_unicode_轉義替代與合法值() {
        let ts = kinds("\"\\u{41}\" \"\\u{1F_600}\" \"\\u{GG}\" \"\\u{}\"");
        assert_eq!(ts[0], TokenKind::StringLiteral("A".into()));
        assert_eq!(ts[1], TokenKind::StringLiteral("\u{FFFD}".into()));
        assert_eq!(ts[2], TokenKind::StringLiteral("\u{FFFD}".into()));
        assert_eq!(ts[3], TokenKind::StringLiteral("\u{FFFD}".into()));
    }

    #[test]
    fn unicode_含空白及超過六位為語法診斷() {
        let s = Source::new("\"\\u{ 41 }\" \"\\u{0000041}\"".as_bytes().to_vec());
        let out = scan(&s);
        assert_eq!(out.diagnostics.len(), 2);
        assert!(out.diagnostics.iter().all(|d|d.kind == DiagnosticKind::Syntax));
    }

    #[test]
    fn unicode_缺少右大括號及未知跳脫各依既有分類() {
        let s = Source::new("\"\\u{41\n\"\\q\"".as_bytes().to_vec());
        let out = scan(&s);
        assert!(out.diagnostics.iter().any(|d|d.kind==DiagnosticKind::Syntax));
        assert!(out.diagnostics.iter().any(|d|d.kind==DiagnosticKind::Lexical));
    }

    #[test]
    fn 不成對大括號有來源位置且不假造語言_ast() {
        let s = Source::new(b"fn main()->int{\n".to_vec());
        let out = scan(&s);
        assert!(out.diagnostics.iter().any(|d|d.message.contains("區塊左大括號")));
        assert!(!out.tokens.is_empty());
    }

    #[test]
    fn 每個非法來源位元組保留非法記號不與鄰字合併() {
        let s = Source::new(vec![b'a',0xFF,b'b']);
        let out = scan(&s);
        assert_eq!(out.tokens[0].kind, TokenKind::Name("a".into()));
        assert_eq!(out.tokens[1].kind, TokenKind::Invalid);
        assert_eq!(out.tokens[2].kind, TokenKind::Name("b".into()));
        assert!(out.diagnostics.is_empty()); // P04 對每位元組另外報詞彙錯誤。
    }
}
