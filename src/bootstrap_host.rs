//! 私有自舉編譯器的宿主 ABI。沿用正式 lexer，宿主提供有界的記號與輸出緩衝區。
use crate::lexer::{self, Token, TokenKind};
use crate::source::Source;
use std::path::Path;

#[derive(Debug)]
enum Fault { Syntax(String), Overflow, Limit }
type R = Result<i64, Fault>;

struct Machine {
    tokens: Vec<Token>,
    cursor: usize,
    output: String,
    steps: usize,
}

impl Machine {
    fn enter(&mut self, depth: usize) -> Result<(), Fault> {
        let budget = crate::execution::ExecutionBudget::default();
        self.steps += 1;
        if depth >= budget.max_call_frames || self.steps > budget.max_steps {
            return Err(Fault::Limit);
        }
        Ok(())
    }
    fn token(&self) -> &TokenKind { &self.tokens[self.cursor].kind }
    fn at(&self, text: &str) -> R {
        let matched = match self.token() {
            TokenKind::Keyword(n) | TokenKind::Name(n) => n == text,
            TokenKind::Symbol(c) => text.len() == c.len_utf8() && text.starts_with(*c),
            TokenKind::Arrow => text == "->",
            _ => false,
        };
        Ok(i64::from(matched))
    }
    fn at_end(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::End))) }
    fn at_integer(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::Integer(_)))) }
    fn expect(&mut self, text: &str) -> R {
        if self.at(text)? == 0 { return Err(Fault::Syntax(format!("預期 {text}"))); }
        self.cursor += 1;
        Ok(0)
    }
    fn emit(&mut self, text: &str) -> R {
        self.output.push_str(text);
        Ok(0)
    }
    fn name(&mut self) -> R {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期函式名稱".into()));
        };
        let ident = format!("v_{name}");
        self.cursor += 1;
        self.emit(&ident)
    }
    fn method(&mut self) -> R {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期自舉宿主函式".into()));
        };
        if !matches!(name.as_str(), "at" | "at_end" | "at_integer" | "expect" |
                    "emit" | "name" | "method" | "literal") {
            return Err(Fault::Syntax("未知的自舉宿主函式".into()));
        }
        let ident = format!("s.{name}");
        self.cursor += 1;
        self.emit(&ident)
    }
    fn literal(&mut self) -> R {
        let value = match self.token() {
            TokenKind::Integer(n) => {
                let v = crate::syntax_shared::integer_i64(n)
                    .map_err(|_| Fault::Syntax("整數常值無效".into()))?;
                format!("{v}i64")
            }
            TokenKind::StringLiteral(s) => format!("{s:?}"),
            _ => return Err(Fault::Syntax("預期整數或字串常值".into())),
        };
        self.cursor += 1;
        self.emit(&value)
    }
}

#[cfg(not(voiles_bootstrap))]
include!("../bootstrap/compiler_seed.inc");
#[cfg(voiles_bootstrap)]
include!(env!("VOILES_COMPILER_RS"));

pub fn compile_bootstrap_source(path: &Path) -> Result<String, String> {
    let source = Source::new(std::fs::read(path).map_err(|e| format!("{}：{e}", path.display()))?);
    if let Some(d) = source.lexical_diagnostics().first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    let scanned = lexer::scan(&source);
    if let Some(d) = scanned.diagnostics.first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    compile_tokens(path, &source, scanned.tokens)
}

/// 自舉時由上一代的公開 run 入口執行 scanner.voil。Rust 只從已定位的
/// 原始切片取得詞值，並拒絕順序、字元邊界或種類不相容的記號資料。
pub fn compile_bootstrap_tokens(path: &Path, rows: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}：{e}", path.display()))?;
    let source = Source::new(bytes);
    if let Some(d) = source.lexical_diagnostics().first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    let input = std::str::from_utf8(&source.bytes).map_err(|e| e.to_string())?;
    let mut tokens = Vec::new();
    let mut previous = 0;
    let mut ended = false;
    for row in rows.lines() {
        let invalid = || format!("{}：自舉記號資料無效", path.display());
        let mut fields = row.split_ascii_whitespace();
        let mut number = || fields.next().and_then(|s| s.parse::<usize>().ok()).ok_or_else(invalid);
        let (code, start, end) = (number()?, number()?, number()?);
        if fields.next().is_some() || ended || start < previous { return Err(invalid()); }
        let gap = input.get(previous..start).ok_or_else(invalid)?;
        if !gap.bytes().all(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r')) { return Err(invalid()); }
        let raw = input.get(start..end).ok_or_else(invalid)?;
        let kind = match code {
            1 | 2 if raw.starts_with(|c:char| c.is_ascii_alphabetic() || c == '_')
                && raw.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {
                if code == 1 { TokenKind::Keyword(raw.into()) } else { TokenKind::Name(raw.into()) }
            }
            3 if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) => TokenKind::Integer(raw.into()),
            4 if raw.starts_with('"') => {
                // 字串跳脫仍由正式字串解碼器處理，只讀取此一記號的切片。
                let literal = lexer::scan(&Source::new(raw.as_bytes().to_vec()));
                if let Some(d) = literal.diagnostics.first() {return Err(format!("{}：{}",path.display(),d.message));}
                match literal.tokens.as_slice() {
                    [Token {kind: TokenKind::StringLiteral(value),end:len,..}, Token {kind:TokenKind::End,..}] if *len == raw.len() =>
                        TokenKind::StringLiteral(value.clone()),
                    _ => return Err(invalid()),
                }
            }
            5 if raw.chars().count() == 1 => TokenKind::Symbol(raw.chars().next().unwrap()),
            6 if raw == "->" => TokenKind::Arrow,
            7 if matches!(raw, "\r" | "\n" | "\r\n") => TokenKind::Newline,
            9 if start == input.len() && raw.is_empty() => { ended = true; TokenKind::End }
            _ => return Err(invalid()),
        };
        previous = end;
        tokens.push(Token { kind, start, end });
    }
    if !ended { return Err(format!("{}：自舉記號缺少檔尾", path.display())); }
    compile_tokens(path, &source, tokens)
}

fn compile_tokens(path: &Path, source: &Source, tokens: Vec<Token>) -> Result<String,String> {
    let mut machine = Machine {
        tokens: tokens.into_iter().filter(|t| !matches!(t.kind, TokenKind::Newline)).collect(),
        cursor: 0, steps: 0, output: String::new(),
    };
    v_main(&mut machine, 0).map_err(|e| {
        let pos = source.position(machine.tokens[machine.cursor].start);
        let message = match e { Fault::Syntax(s) => s, Fault::Limit => "自舉編譯預算耗盡".into(),
            Fault::Overflow => "自舉編譯整數溢位".into() };
        format!("{}:{}:{}: {message}", path.display(), pos.line, pos.column)
    })?;
    Ok(machine.output)
}
