//! 私有自舉編譯器的宿主 ABI。沿用正式 lexer，宿主提供有界的記號與輸出緩衝區。
use crate::lexer::{self, Token, TokenKind};
use crate::source::Source;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug)]
enum Fault { Syntax(String), Overflow, Limit }
type R = Result<i64, Fault>;

struct Machine {
    tokens: Vec<Token>,
    cursor: usize,
    output: String,
    steps: usize,
    functions: BTreeMap<String, Vec<String>>,
    current_function: Option<String>,
    calls: Vec<(String, usize)>,
    call_stack: Vec<usize>,
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
    fn matched_width(&self, text: &str) -> usize {
        if matches!(text, "!=" | "==" | "<=" | ">=") {
            let first = &self.tokens[self.cursor];
            let Some(second) = self.tokens.get(self.cursor + 1) else {return 0};
            return if first.end == second.start
                && first.kind == TokenKind::Symbol(char::from(text.as_bytes()[0]))
                && second.kind == TokenKind::Symbol('=') {2} else {0};
        }
        usize::from(match self.token() {
            TokenKind::Keyword(n) | TokenKind::Name(n) => n == text,
            TokenKind::Symbol(c) => text.len() == c.len_utf8() && text.starts_with(*c),
            TokenKind::Arrow => text == "->",
            _ => false,
        })
    }
    fn at(&self, text: &str) -> R { Ok(i64::from(self.matched_width(text) != 0)) }
    fn at_end(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::End))) }
    fn at_integer(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::Integer(_)))) }
    fn at_call(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|token|
                token.kind == TokenKind::Symbol('('))))
    }
    fn expect(&mut self, text: &str) -> R {
        let width = self.matched_width(text);
        if width == 0 { return Err(Fault::Syntax(format!("預期 {text}"))); }
        self.cursor += width;
        Ok(0)
    }
    fn emit(&mut self, text: &str) -> R {
        self.output.push_str(text);
        Ok(0)
    }
    fn name(&mut self) -> R {
        let name = self.take_name()?;
        self.emit_name(&name)
    }
    fn take_name(&mut self) -> Result<String, Fault> {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期函式名稱".into()));
        };
        let name = name.clone();
        self.cursor += 1;
        Ok(name)
    }
    fn emit_name(&mut self, name: &str) -> R { self.emit(&format!("v_{name}")) }
    fn function(&mut self) -> R {
        let name = self.take_name()?;
        if self.functions.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複函式 {name}")));
        }
        self.functions.insert(name.clone(), Vec::new());
        self.current_function = Some(name.clone());
        self.emit_name(&name)
    }
    fn parameter(&mut self) -> R {
        let name = self.take_name()?;
        let Some(function) = self.current_function.as_ref() else {
            return Err(Fault::Syntax("參數不在函式中".into()));
        };
        let parameters = self.functions.get_mut(function).expect("目前函式已登錄");
        if parameters.contains(&name) {
            return Err(Fault::Syntax(format!("重複參數 {name}")));
        }
        parameters.push(name.clone());
        self.emit_name(&name)
    }
    fn value(&mut self) -> R {
        let name = self.take_name()?;
        let known = self.current_function.as_ref()
            .and_then(|function| self.functions.get(function))
            .is_some_and(|parameters| parameters.contains(&name));
        if !known { return Err(Fault::Syntax(format!("未知名稱 {name}"))); }
        self.emit_name(&name)
    }
    fn finish_function(&mut self) -> R {
        self.current_function.take().ok_or_else(|| Fault::Syntax("函式邊界無效".into()))?;
        Ok(0)
    }
    fn call(&mut self) -> R {
        let name = self.take_name()?;
        let index = self.calls.len();
        self.calls.push((name.clone(), 0));
        self.call_stack.push(index);
        self.emit_name(&name)
    }
    fn argument(&mut self) -> R {
        let Some(index) = self.call_stack.last().copied() else {
            return Err(Fault::Syntax("實參不在呼叫中".into()));
        };
        self.calls[index].1 += 1;
        Ok(0)
    }
    fn finish_call(&mut self) -> R {
        self.call_stack.pop().ok_or_else(|| Fault::Syntax("呼叫邊界無效".into()))?;
        Ok(0)
    }
    fn finish_names(&mut self) -> R {
        if self.current_function.is_some() || !self.call_stack.is_empty() {
            return Err(Fault::Syntax("名稱解析邊界無效".into()));
        }
        for (name, actual) in &self.calls {
            let Some(parameters) = self.functions.get(name) else {
                return Err(Fault::Syntax(format!("未知函式 {name}")));
            };
            if *actual != parameters.len() {
                return Err(Fault::Syntax(format!("函式 {name} 預期 {} 個實參，收到 {actual} 個", parameters.len())));
            }
        }
        Ok(0)
    }
    fn method(&mut self) -> R {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期自舉宿主函式".into()));
        };
        if !matches!(name.as_str(), "at" | "at_end" | "at_integer" | "at_call" | "expect" |
                    "emit" | "name" | "function" | "parameter" | "value" |
                    "finish_function" | "call" | "argument" | "finish_call" |
                    "finish_names" | "method" | "literal") {
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
        cursor: 0, steps: 0, output: String::new(), functions: BTreeMap::new(),
        current_function: None, calls: Vec::new(), call_stack: Vec::new(),
    };
    v_main(&mut machine, 0).map_err(|e| {
        let pos = source.position(machine.tokens[machine.cursor].start);
        let message = match e { Fault::Syntax(s) => s, Fault::Limit => "自舉編譯預算耗盡".into(),
            Fault::Overflow => "自舉編譯整數溢位".into() };
        format!("{}:{}:{}: {message}", path.display(), pos.line, pos.column)
    })?;
    Ok(machine.output)
}
