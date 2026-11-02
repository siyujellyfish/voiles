//! P09 首版名稱解析：函式前向／遞迴、變數詞彙作用域及文字先宣告。
//! Optional 縮限、完整模組可見性及所有權規則不在 MVP-A 中推定。

use std::collections::HashMap;
use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind, Type};
use crate::source::Source;
use crate::ownership_ledger::DeclarationId;
use crate::common_ir::TypedDeclaration;

// 與 P13 安全檢查、共用 AST 及 P16 資源責任表採同一宣告身分型別。
// 只改資料表示，不擴張 P09 的名稱可見性／前向參照規則。

struct Resolver<'a> {
    source: &'a Source,
    functions: HashMap<String, Type>,
    globals: HashMap<String, TypedDeclaration<Type>>,
    facts: crate::common_ir::ResolvedNames,
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
        scopes: &[HashMap<String, TypedDeclaration<Type>>])
        -> Option<TypedDeclaration<Type>> {
        if let Some(binding) = crate::common_ir::lookup_lexical(scopes.iter(), name) {
            return Some(*binding);
        }
        self.globals.get(name).copied()
            .filter(|binding| binding.declaration.0 < pos)
    }

    fn check_expr(&mut self, e: &Expr, scopes: &[HashMap<String, TypedDeclaration<Type>>]) -> Result<(), Diagnostic> {
        match &e.kind {
            ExprKind::Name(name) => {
                let declaration = self.binding_for_reference(name, e.start, scopes)
                    .ok_or_else(|| self.error(
                        "找不到已宣告且可讀取的具名變數", e.start, e.end))?;
                self.facts.references.insert(e.start, declaration.declaration);
                Ok(())
            }
            ExprKind::ZeroArgCall(call) => {
                if self.functions.contains_key(&call.name) {
                    Ok(())
                } else {
                    Err(self.error("找不到合法的無參數函式宣告", e.start, e.end))
                }
            }
            ExprKind::Add(binary) | ExprKind::Subtract(binary) => {
                self.check_expr(&binary.left, scopes)?;
                self.check_expr(&binary.right, scopes)
            }
            ExprKind::IndexAccess(a,b) | ExprKind::FirstIndexWrite(a,b) => {
                self.check_expr(a,scopes)?;
                self.check_expr(b,scopes)
            }
            ExprKind::Group(a) | ExprKind::ZeroArgMethod(a,_) =>
                self.check_expr(a,scopes),
            ExprKind::Scalar(_) |
            ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) => Ok(()),
        }
    }

    fn initializer(&mut self, b: &Binding,
        scopes: &[HashMap<String, TypedDeclaration<Type>>]) -> Result<(), Diagnostic> {
        if let Some(value) = &b.initializer {
            self.check_expr(value, scopes)?;
        }
        Ok(())
    }

    fn block(&mut self, block: &Block,
        scopes: &mut Vec<HashMap<String, TypedDeclaration<Type>>>) -> Result<(), Diagnostic> {
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
                    let id = DeclarationId(b.start);
                    let declaration = TypedDeclaration::new(id, b.ty, b.mutable);
                    self.facts.declarations.insert(id, declaration);
                    scopes.last_mut().unwrap().insert(b.name.clone(), declaration);
                }
                StmtKind::Assignment(assignment) => {
                    let name = &assignment.name;
                    let value = &assignment.expression;
                    let declaration = self.binding_for_reference(name, statement.start, scopes)
                        .ok_or_else(|| self.error(
                            "賦值目標尚未宣告", statement.start, statement.end))?;
                    self.facts.assignments.insert(statement.start, declaration.declaration);
                    self.check_expr(value, scopes)?;
                }
                StmtKind::Return(expr) => self.check_expr(expr, scopes)?,
                StmtKind::IfChain(chain) => {
                    for arm in &chain.arms {
                        self.check_expr(&arm.branch.condition, scopes)?;
                        self.block(&arm.branch.body, scopes)?;
                    }
                    if let Some(branch) = &chain.otherwise {
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
    resolve_with_facts(file, source).map(|_| ())
}

/// 與原 P09 完全相同的名稱檢查，同時回傳已驗證之具體來源身分。
pub(crate) fn resolve_with_facts(
    file: &File, source: &Source,
) -> Result<crate::common_ir::ResolvedNames, Diagnostic> {
    let mut functions = HashMap::new();
    let mut globals = HashMap::new();
    let mut facts = crate::common_ir::ResolvedNames::default();
    for item in &file.items {
        match item {
            Item::Function(f) => {
                if functions.insert(f.name.clone(), f.result_ty).is_some() {
                    // 首版只有零引數、單一簽章；不推導完整語言的多載政策。
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "首版函式名稱不能有歧義或重複無參數簽章".into(),
                        span: Some(source.span(f.start,f.end)),
                    });
                }
            }
            Item::Binding(b) => {
                let declaration = TypedDeclaration::new(
                    DeclarationId(b.start), b.ty, b.mutable);
                if globals.insert(b.name.clone(), declaration).is_some() {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "頂層作用域重複宣告變數".into(),
                        span: Some(source.span(b.start,b.end)),
                    });
                }
                facts.declarations.insert(declaration.declaration, declaration);
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
    let mut resolver=Resolver {source,functions,globals,facts};
    for item in &file.items {
        match item {
            Item::Binding(b) => resolver.initializer(b,&[])?,
            Item::Function(f) => resolver.block(&f.body,&mut Vec::new())?,
        }
    }
    Ok(resolver.facts)
}

#[cfg(test)]
mod tests {
    use super::{resolve, DeclarationId, Resolver, TypedDeclaration};
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
    fn 名稱解析將同名遮蔽的不同使用點繫結至不同來源身分() {
        let text = "fn main()->int{\nvar item:int = 1\nif(true){\nvar item:bool = true\nif(item){\nreturn 1\n}\n}\nreturn item\n}\n";
        let source = Source::new(text.as_bytes().to_vec());
        let tokens = scan(&source);
        let file = parse(&source, &tokens.tokens).unwrap();
        let facts = super::resolve_with_facts(&file, &source).unwrap();
        let first = DeclarationId(text.find("var item:int").unwrap());
        let inner = DeclarationId(text.find("var item:bool").unwrap());
        let inner_use = text.find("if(item)").unwrap() + 3;
        let outer_use = text.rfind("return item").unwrap() + 7;
        assert_eq!(facts.references.get(&inner_use), Some(&inner));
        assert_eq!(facts.references.get(&outer_use), Some(&first));
        assert_eq!(facts.reference_type(inner_use), Some(crate::common_ir::ScalarType::Bool));
        assert_eq!(facts.reference_type(outer_use), Some(crate::common_ir::ScalarType::Int));
    }

    #[test]
    fn f04_公開_p09_詞彙查找直接產生完整已驗證具型別來源() {
        let code = "var top:int = 1\nfn main()->int{\nvar item:int = 1\nif(true){\nconst item:bool = false\nif(item){\nreturn top\n}\n}\nreturn item\n}\n";
        let src = Source::new(code.as_bytes().to_vec());
        let tokens = scan(&src);
        let ast = parse(&src, &tokens.tokens).unwrap();
        let facts = super::resolve_with_facts(&ast, &src).unwrap();
        let global = DeclarationId(code.find("var top:int").unwrap());
        let outer = DeclarationId(code.find("var item:int").unwrap());
        let inner = DeclarationId(code.find("const item:bool").unwrap());
        let typed_global = facts.declaration(global).unwrap();
        let typed_outer = facts.declaration(outer).unwrap();
        let typed_inner = facts.declaration(inner).unwrap();
        assert_eq!(typed_global.ty, crate::common_ir::ScalarType::Int);
        assert_eq!(typed_outer.ty, crate::common_ir::ScalarType::Int);
        assert_eq!(typed_inner.ty, crate::common_ir::ScalarType::Bool);
        assert!(typed_global.mutable);
        assert!(typed_outer.mutable);
        assert!(!typed_inner.mutable);
        let nested = code.find("if(item)").unwrap() + "if(".len();
        let outside = code.rfind("return item").unwrap() + "return ".len();
        let global_use = code.find("return top").unwrap() + "return ".len();
        assert_eq!(facts.reference(nested).unwrap().declaration, inner);
        assert_eq!(facts.reference(outside).unwrap().declaration, outer);
        assert_eq!(facts.reference(global_use).unwrap().declaration, global);
        assert!(typed_inner.matches_use(facts.reference(nested).unwrap()));
        assert!(!typed_outer.matches_use(facts.reference(nested).unwrap()));
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
        let StmtKind::IfChain(chain) = &function.body.statements[1].kind
            else { panic!("缺少內層分支"); };
        let StmtKind::Binding(inner) = &chain.arms[0].branch.body.statements[0].kind
            else { panic!("缺少內層遮蔽宣告"); };

        let resolver = Resolver {
            source: &source,
            functions: HashMap::new(),
            globals: HashMap::new(),
            facts: crate::common_ir::ResolvedNames::default(),
        };
        let mut scopes = vec![HashMap::new()];
        scopes[0].insert(outer.name.clone(),
            TypedDeclaration::new(DeclarationId(outer.start), outer.ty, outer.mutable));
        let outer_id = resolver.binding_for_reference("n", function.end, &scopes).unwrap();
        scopes.push(HashMap::new());
        scopes[1].insert(inner.name.clone(),
            TypedDeclaration::new(DeclarationId(inner.start), inner.ty, inner.mutable));
        let inner_id = resolver.binding_for_reference("n", function.end, &scopes).unwrap();
        assert_ne!(outer_id, inner_id, "H06／AH04 遮蔽不能覆蓋來源身分");
        // 與 R02 共通 AST、P13/P16 的安全資源帳簿共用 DeclarationId，
        // 而非只比較 P09 私有位置整數或相同詞彙名稱。
        let shared = crate::common_ir::from_public(&file).unwrap();
        let crate::common_ir::ScalarStmt::Declare { declaration: shared_outer, .. } =
            &shared.functions[0].body[0] else { panic!("共用 AST 缺少外層宣告"); };
        let crate::common_ir::ScalarStmt::If { yes, .. } =
            &shared.functions[0].body[1] else { panic!("共用 AST 缺少 if"); };
        let [crate::common_ir::ScalarStmt::Declare { declaration: shared_inner, .. }] =
            yes.as_slice() else { panic!("共用 AST 缺少內層同名宣告"); };
        assert_eq!(&outer_id.declaration, shared_outer);
        assert_eq!(&inner_id.declaration, shared_inner);

        assert_eq!(inner_id.declaration.0, inner.start);
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
