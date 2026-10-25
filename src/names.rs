//! P09 首版名稱解析：函式前向／遞迴、變數詞彙作用域及文字先宣告。
//! Optional 縮限、完整模組可見性及所有權規則不在 MVP-A 中推定。

use std::collections::HashMap;
use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind, Type};
use crate::source::Source;

struct Resolver<'a> {
    source: &'a Source,
    functions: HashMap<String, Type>,
    globals: HashMap<String, usize>,
}

impl<'a> Resolver<'a> {
    fn error(&self, text: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: Box::from(text),
            span: Some(self.source.span(start, end)),
        }
    }

    fn local_or_global(&self, name: &str, pos: usize,
        scopes: &[HashMap<String, usize>]) -> bool {
        for scope in scopes.iter().rev() {
            if scope.contains_key(name) { return true; }
        }
        self.globals.get(name).is_some_and(|global_start| *global_start < pos)
    }

    fn check_expr(&self, e: &Expr, scopes: &[HashMap<String, usize>]) -> Result<(), Diagnostic> {
        match &e.kind {
            ExprKind::Name(name) => {
                if self.local_or_global(name, e.start, scopes) {
                    Ok(())
                } else {
                    Err(self.error("找不到已宣告且可讀取的具名變數", e.start, e.end))
                }
            }
            ExprKind::ZeroArgCall(name) => {
                if self.functions.contains_key(name) {
                    Ok(())
                } else {
                    Err(self.error("找不到合法的無參數函式宣告", e.start, e.end))
                }
            }
            ExprKind::Add(a,b) | ExprKind::Subtract(a,b) |
            ExprKind::IndexAccess(a,b) | ExprKind::FirstIndexWrite(a,b) => {
                self.check_expr(a,scopes)?;
                self.check_expr(b,scopes)
            }
            ExprKind::Group(a) | ExprKind::ZeroArgMethod(a,_) =>
                self.check_expr(a,scopes),
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) |
            ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) => Ok(()),
        }
    }

    fn initializer(&self, b: &Binding,
        scopes: &[HashMap<String, usize>]) -> Result<(), Diagnostic> {
        if let Some(value) = &b.initializer {
            self.check_expr(value, scopes)?;
        }
        Ok(())
    }

    fn block(&self, block: &Block,
        scopes: &mut Vec<HashMap<String, usize>>) -> Result<(), Diagnostic> {
        scopes.push(HashMap::new());
        for statement in &block.statements {
            match &statement.kind {
                StmtKind::Binding(b) => {
                    if scopes.last().unwrap().contains_key(&b.name) {
                        return Err(self.error("同一作用域重複宣告變數", b.start, b.end));
                    }
                    // 先檢查初始化來源，再加入當前作用域名稱。
                    // 不因同名宣告預設自己能讀取尚未建立的變數。
                    self.initializer(b, scopes)?;
                    scopes.last_mut().unwrap().insert(b.name.clone(), b.start);
                }
                StmtKind::Assignment { name, value } => {
                    if !self.local_or_global(name, statement.start, scopes) {
                        return Err(self.error("賦值目標尚未宣告", statement.start, statement.end));
                    }
                    self.check_expr(value, scopes)?;
                }
                StmtKind::Return(expr) => self.check_expr(expr, scopes)?,
                StmtKind::IfChain { branches, otherwise } => {
                    for (condition, branch) in branches {
                        self.check_expr(condition, scopes)?;
                        self.block(branch, scopes)?;
                    }
                    if let Some(branch) = otherwise {
                        self.block(branch, scopes)?;
                    }
                }
            }
        }
        scopes.pop();
        Ok(())
    }
}

pub(crate) fn resolve(file: &File, source: &Source) -> Result<(), Diagnostic> {
    let mut functions = HashMap::new();
    let mut globals = HashMap::new();
    for item in &file.items {
        match item {
            Item::Function(f) => {
                if functions.insert(f.name.clone(), f.returns).is_some() {
                    // 首版只有零引數、單一簽章；不推導完整語言的多載政策。
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "首版函式名稱不能有歧義或重複無參數簽章".into(),
                        span: Some(source.span(f.start,f.end)),
                    });
                }
            }
            Item::Binding(b) => {
                if globals.insert(b.name.clone(), b.start).is_some() {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "頂層作用域重複宣告變數".into(),
                        span: Some(source.span(b.start,b.end)),
                    });
                }
            }
        }
    }
    if !functions.contains_key("main") {
        // 缺少入口函式是全檔條件，不能虛構出某行特定錯誤來源。
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "首版單檔來源缺少合法的 fn main()".into(),
            span: None,
        });
    }
    let resolver=Resolver {source,functions,globals};
    for item in &file.items {
        match item {
            Item::Binding(b) => resolver.initializer(b,&[])?,
            Item::Function(f) => resolver.block(&f.body,&mut Vec::new())?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::{lexer::scan, parser::parse, source::Source, DiagnosticKind};

    fn resolve_text(text: &str) -> Result<(),crate::Diagnostic> {
        let source=Source::new(text.as_bytes().to_vec());
        let tokens=scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let file=parse(&source,&tokens.tokens).expect("名稱案例須先可解析");
        resolve(&file,&source)
    }

    #[test]
    fn p01_十九個正向來源名稱繫結均合法() {
        for index in 1..=19 {
            let path=format!("tests/fixtures/voiles/P01-正{index:02}.voil");
            let text=std::fs::read_to_string(&path).unwrap();
            assert!(resolve_text(&text).is_ok(), "{path}");
        }
    }

    #[test]
    fn 函式前向與相互遞迴不因宣告順序被誤拒絕() {
        let text="fn first()->int{\nreturn second()\n}\nfn second()->int{\nreturn first()\n}\nfn main()->int{\nreturn first()\n}\n";
        assert!(resolve_text(text).is_ok());
    }

    #[test]
    fn 同層重複變數名稱與尚未宣告名稱會產生靜態錯誤() {
        for n in [12,13,14] {
            let path=format!("tests/fixtures/voiles/P01-反{n:02}.voil");
            let text=std::fs::read_to_string(path).unwrap();
            let diag=resolve_text(&text).unwrap_err();
            assert_eq!(diag.kind,DiagnosticKind::Static);
        }
    }

    #[test]
    fn 缺少主函式屬全檔錯誤且不杜撰位置() {
        let text=std::fs::read_to_string("tests/fixtures/voiles/P01-反01.voil").unwrap();
        let diag=resolve_text(&text).unwrap_err();
        assert_eq!(diag.kind,DiagnosticKind::Static);
        assert!(diag.span.is_none());
    }

    #[test]
    fn 函式可讀之前宣告的頂層變數卻不能前向讀取() {
        assert!(resolve_text(
            "fn main()->int{\nreturn later\n}\nconst later:int = 2\n").is_err());
        assert!(resolve_text(
            "const earlier:int = 2\nfn main()->int{\nreturn earlier\n}\n").is_ok());
    }

    #[test]
    fn 內層同名變數合法遮蔽_離開區塊恢復外層名稱() {
        assert!(resolve_text(
            "fn main()->int{\nvar n:int = 1\nif (true){\nvar n:int = 2\n}\nreturn n\n}\n"
        ).is_ok());
    }
}
