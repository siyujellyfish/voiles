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
    let mut machine = Machine {
        tokens: scanned.tokens.into_iter().filter(|t| !matches!(t.kind, TokenKind::Newline)).collect(),
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
