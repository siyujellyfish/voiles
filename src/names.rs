//! P09 首版名稱解析：函式前向／遞迴、變數詞彙作用域及文字先宣告。
//! Optional 縮限、完整模組可見性及所有權規則不在 MVP-A 中推定。

use std::collections::HashMap;
use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind, Type};
use crate::source::Source;

/// 每筆變數宣告具有獨立的原始來源身分；名稱只是查找鍵。
/// 不把同名遮蔽的內層變數誤當外層來源，亦不代表 Optional 旗標已實作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BindingId {
    declaration_start: usize,
}

struct Resolver<'a> {
    source: &'a Source,
    functions: HashMap<String, Type>,
    globals: HashMap<String, BindingId>,
}

impl<'a> Resolver<'a> {
    fn error(&self, text: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: Box::from(text),
            span: Some(self.source.span(start, end)),
        }
    }

    /// 查找實際宣告身分，而非只回傳同名字串是否存在。
    /// 函式前向參照由獨立函式表處理；一般頂層值仍遵守 AW04。
    fn binding_for_reference(&self, name: &str, pos: usize,
        scopes: &[HashMap<String, BindingId>]) -> Option<BindingId> {
        for scope in scopes.iter().rev() {
            if let Some(id) = scope.get(name) { return Some(*id); }
        }
        self.globals.get(name).copied().filter(|id| id.declaration_start < pos)
    }

    fn check_expr(&self, e: &Expr, scopes: &[HashMap<String, BindingId>]) -> Result<(), Diagnostic> {
        match &e.kind {
            ExprKind::Name(name) => {
                if self.binding_for_reference(name, e.start, scopes).is_some() {
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
        scopes: &[HashMap<String, BindingId>]) -> Result<(), Diagnostic> {
        if let Some(value) = &b.initializer {
            self.check_expr(value, scopes)?;
        }
        Ok(())
    }

    fn block(&self, block: &Block,
        scopes: &mut Vec<HashMap<String, BindingId>>) -> Result<(), Diagnostic> {
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
                    scopes.last_mut().unwrap().insert(b.name.clone(), BindingId { declaration_start: b.start });
                }
                StmtKind::Assignment { name, value } => {
                    if self.binding_for_reference(name, statement.start, scopes).is_none() {
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
                if globals.insert(b.name.clone(), BindingId { declaration_start: b.start }).is_some() {
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
    use super::{resolve, BindingId, Resolver};
    use std::collections::HashMap;
    use crate::{lexer::scan, parser::{parse, Item, StmtKind}, source::Source, DiagnosticKind};

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

    #[test]
    fn 同名變數維持各自宣告來源身分且內層退出恢復外層() {
        let text = "fn main()->int{\nvar n:int = 1\nif (true){\nvar n:int = 2\n}\nreturn n\n}\n";
        let source = Source::new(text.as_bytes().to_vec());
        let tokens = scan(&source).tokens;
        let file = parse(&source, &tokens).unwrap();
        let Item::Function(function) = &file.items[0] else { panic!("缺少 main"); };
        let StmtKind::Binding(outer) = &function.body.statements[0].kind
            else { panic!("缺少外層名稱"); };
        let StmtKind::IfChain { branches, .. } = &function.body.statements[1].kind
            else { panic!("缺少內層分支"); };
        let StmtKind::Binding(inner) = &branches[0].1.statements[0].kind
            else { panic!("缺少內層遮蔽宣告"); };

        let resolver = Resolver {
            source: &source,
            functions: HashMap::new(),
            globals: HashMap::new(),
        };
        let mut scopes = vec![HashMap::new()];
        scopes[0].insert(outer.name.clone(), BindingId { declaration_start: outer.start });
        let outer_id = resolver.binding_for_reference("n", function.end, &scopes).unwrap();
        scopes.push(HashMap::new());
        scopes[1].insert(inner.name.clone(), BindingId { declaration_start: inner.start });
        let inner_id = resolver.binding_for_reference("n", function.end, &scopes).unwrap();
        assert_ne!(outer_id, inner_id, "H06／AH04 遮蔽不能覆蓋來源身分");
        assert_eq!(inner_id.declaration_start, inner.start);
        scopes.pop();
        assert_eq!(
            resolver.binding_for_reference("n", function.end, &scopes),
            Some(outer_id),
            "離開內層詞彙作用域應回復原本外層宣告身分"
        );
        // Optional 旗標 AJ04／AI06 不屬本 MVP-A 名稱解析器，未據此實作。
    }

    #[test]
    fn 名稱解析不會把頂層變數的前向讀取誤當函式前向() {
        let invalid = "fn main()->int{\nreturn later\n}\nvar later:int = 3\n";
        let failure = resolve_text(invalid).unwrap_err();
        assert_eq!(failure.kind, DiagnosticKind::Static);
        assert!(failure.span.is_some());
        let valid = "fn later()->int{\nreturn 3\n}\nfn main()->int{\nreturn later()\n}\n";
        assert!(resolve_text(valid).is_ok());
    }

    #[test]
    fn 首版不把_struct_及模組語法當成已實作_下劃線名稱也不推斷可見性() {
        let unsupported = "struct Sample{\nvar value:int = 1\n}\nfn main()->int{\nreturn 0\n}\n";
        let source = Source::new(unsupported.as_bytes().to_vec());
        let tokens = scan(&source).tokens;
        assert!(parse(&source, &tokens).is_err(), "首版 AS05 必須拒絕 struct 語法");
        assert!(resolve_text(
            "fn _helper()->int{\nreturn 1\n}\nfn main()->int{\nreturn _helper()\n}\n"
        ).is_ok(), "同檔名稱可被查找，不能由此聲稱跨模組可見性已實作");
    }

}
