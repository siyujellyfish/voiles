//! P11 MVP-A 確定初始化與賦值權限。
//! 不替未決的不可達碼一般合法性或完整語言字串／所有權創建語意。

use std::collections::HashMap;
use std::rc::Rc;

use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind};
use crate::source::Source;
use crate::ownership_ledger::DeclarationId;
use crate::common_ir::{TypedDeclaration, ScalarType, VerifiedScalarHir};

#[derive(Debug, Clone, Copy)]
struct Variable {
    declaration: TypedDeclaration<ScalarType>,
    initialized: bool,
}

#[derive(Debug, Clone)]
struct Flow {
    // 分支快照共用唯讀作用域；寫入時才複製實際受影響的那一層。
    scopes: Vec<Rc<HashMap<String, Variable>>>,
    reachable: bool,
}

impl Flow {
    fn lookup(&self, name: &str) -> Option<Variable> {
        crate::common_ir::lookup_lexical(
            self.scopes.iter().map(|scope| scope.as_ref()), name,
        ).copied()
    }

    fn write(&mut self, name: &str) -> Option<&mut Variable> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                return Rc::make_mut(scope).get_mut(name);
            }
        }
        None
    }
}

struct Analyzer<'a> {
    source: &'a Source,
    /// 正式 P09 已完成的來源身分證據；獨立 P11 測試仍可使用原入口。
    resolved: Option<&'a crate::common_ir::ResolvedNames>,
    typed: Option<&'a VerifiedScalarHir>,
}

impl Analyzer<'_> {
    fn err(&self, msg: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: msg.into(),
            span: Some(self.source.span(start, end)),
        }
    }

    fn read(&self, expr: &Expr, state: &Flow) -> Result<(), Diagnostic> {
        match &expr.kind {
            ExprKind::Name(name) => {
                let var = state.lookup(name)
                    .filter(|candidate| self.resolved.is_none_or(|facts| {
                        facts.verified_reference(expr.node_span(),
                            candidate.declaration).is_some_and(|reference| {
                            self.typed.is_none_or(|hir|
                                hir.reference(expr.node_span()) == Some(reference))
                        })
                    }))
                    .ok_or_else(|| self.err(
                        "具名來源不存在可見宣告", expr.start, expr.end
                    ))?;
                if !var.initialized {
                    return Err(self.err("變數在所有可達路徑完成初始化前不可讀取", expr.start, expr.end));
                }
            }
            ExprKind::Group(inner) | ExprKind::ZeroArgMethod(inner, _) =>
                self.read(inner, state)?,
            ExprKind::Add(binary) | ExprKind::Subtract(binary) => {
                self.read(&binary.left, state)?;
                self.read(&binary.right, state)?;
            }
            ExprKind::IndexAccess(a, b) | ExprKind::FirstIndexWrite(a, b) => {
                self.read(a, state)?;
                self.read(b, state)?;
            }
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) |
            ExprKind::ZeroArgCall(_) | ExprKind::DecimalLiteral(_) |
            ExprKind::StringLiteral(_) => {}
        }
        Ok(())
    }

    fn bind(&self, b: &Binding, state: &mut Flow) -> Result<(), Diagnostic> {
        if let Some(ref initializer) = b.initializer {
            self.read(initializer, state)?;
        }
        // 首版局部 var 允許延後初始化，但不能因宣告本身當成已初始化。
        let scope = state.scopes.last_mut().expect("全檔至少有頂層作用域");
        Rc::make_mut(scope).insert(b.name.clone(), Variable {
            declaration: TypedDeclaration::new(DeclarationId(b.start), b.ty, b.mutable),
            initialized: b.initializer.is_some(),
        });
        Ok(())
    }

    fn block(&self, block: &Block, state: &mut Flow) -> Result<(), Diagnostic> {
        state.scopes.push(Rc::new(HashMap::new()));
        for statement in &block.statements {
            if !state.reachable {
                // 不可達語句的全面合法性仍未決，P11 不推定如何處置；
                // 繼續性與可達狀態推導只使用已到達的語句。
                break;
            }
            match &statement.kind {
                StmtKind::Binding(b) => self.bind(b, state)?,
                StmtKind::Assignment { name, value } => {
                    // 先依 AY02 驗證來源讀取，再處理目標初始化狀態。
                    self.read(value, state)?;
                    let target = state.lookup(name)
                        .filter(|candidate| self.resolved.is_none_or(|facts| {
                            facts.verified_assignment(statement.node_span(),
                                candidate.declaration).is_some_and(|assignment| {
                                self.typed.is_none_or(|hir|
                                    hir.assignment(statement.node_span()) == Some(assignment))
                            })
                        }))
                        .ok_or_else(|| self.err(
                            "賦值目標沒有可見宣告", statement.start, statement.end
                        ))?;
                    if !target.declaration.mutable {
                        return Err(self.err("const 宣告不可再次賦值", statement.start, statement.end));
                    }
                    state.write(name).expect("已確認目標有名稱繫結").initialized = true;
                }
                StmtKind::Return(value) => {
                    self.read(value, state)?;
                    state.reachable = false;
                }
                StmtKind::IfChain { branches, otherwise } => {
                    let entry = state.clone();
                    let mut exits = Vec::new();
                    for arm in branches {
                        // 所有一般 bool 條件的具名來源都要先初始化。
                        self.read(&arm.condition, &entry)?;
                        let mut branch = entry.clone();
                        self.block(&arm.body, &mut branch)?;
                        if branch.reachable { exits.push(branch); }
                    }
                    if let Some(body) = otherwise {
                        let mut branch = entry.clone();
                        self.block(body, &mut branch)?;
                        if branch.reachable { exits.push(branch); }
                    } else {
                        // 無 else 就存在條件全部不成立的可能路徑。
                        exits.push(entry);
                    }
                    if exits.is_empty() {
                        state.reachable = false;
                    } else {
                        // 對分支以外原有的每筆宣告，以其來源身分驗證
                        // 所有可達後繼路徑均已初始化；內層新宣告不外洩。
                        for (scope_idx, outer_scope) in state.scopes.iter_mut().enumerate() {
                            for (name, var) in Rc::make_mut(outer_scope).iter_mut() {
                                var.initialized = exits.iter().all(|exit| {
                                    exit.scopes[scope_idx].get(name).is_some_and(|v| {
                                        v.declaration == var.declaration && v.initialized
                                    })
                                });
                            }
                        }
                    }
                }
            }
        }
        state.scopes.pop();
        Ok(())
    }
}

/// 須在 P07 解析、P09 名稱及 P10 基本型別成功後才呼叫。
/// 不具備 P12 的完整函式／控制流程驗收。
pub(crate) fn check(file: &File, source: &Source) -> Result<(), Diagnostic> {
    check_with_names_internal(file, source, None, None)
}

/// 正式 P11 靜態流程檢查消耗 P09 的引用及賦值目標來源身分證據。
pub(crate) fn check_with_names(
    file: &File, source: &Source, names: &crate::common_ir::ResolvedNames,
) -> Result<(), Diagnostic> {
    check_with_names_internal(file, source, Some(names), None)
}

/// 正式 P11 消費 P10 成功型別檢查後產生的 HIR；
/// 不允許依 P09 原始表重新推算缺失的 P10 來源證據。
pub(crate) fn check_with_typed_hir(
    file: &File, source: &Source,
    names: &crate::common_ir::ResolvedNames, hir: &VerifiedScalarHir,
) -> Result<(), Diagnostic> {
    check_with_names_internal(file, source, Some(names), Some(hir))
}

fn check_with_names_internal<'a>(
    file: &File, source: &'a Source,
    resolved: Option<&'a crate::common_ir::ResolvedNames>,
    typed: Option<&'a VerifiedScalarHir>,
) -> Result<(), Diagnostic> {
    let analyzer = Analyzer { source, resolved, typed };
    let mut globals = Flow {
        scopes: vec![Rc::new(HashMap::new())],
        reachable: true,
    };
    // 頂層依文字次序檢查每個初始化來源；
    // P09 已經驗證所有名稱的合法宣告及 AW04。
    for item in &file.items {
        if let Item::Binding(b) = item {
            analyzer.bind(b, &mut globals)?;
        }
    }
    // 函式彼此獨立分析，不推測執行期呼叫或遞迴順序。
    for item in &file.items {
        if let Item::Function(f) = item {
            let mut state = globals.clone();
            analyzer.block(&f.body, &mut state)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, names, parser, types, source};

    fn inspect(code: &str) -> Result<(), Diagnostic> {
        let source = source::Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let ast = parser::parse(&source, &tokens.tokens).expect("P11 測試來源應合法解析");
        names::resolve(&ast, &source)?;
        types::check(&ast, &source)?;
        check(&ast, &source)
    }

    #[test]
    fn 正式_p11_依_p09_已解析身分辨識內外同名賦值與讀取() {
        let code = "fn main()->int{\nvar n:int\nif(true){\nvar n:int = 1\nn = 2\n}\nn = 3\nreturn n\n}\n";
        let source = source::Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let ast = parser::parse(&source, &lexed.tokens).unwrap();
        let facts = names::resolve_with_facts(&ast, &source).unwrap();
        let inner = DeclarationId(code.find("var n:int = 1").unwrap());
        let outer = DeclarationId(code.find("var n:int\n").unwrap());
        let write_inner = code.find("n = 2").unwrap();
        let write_outer = code.find("n = 3").unwrap();
        assert_eq!(facts.assignments.get(&write_inner), Some(&inner));
        assert_eq!(facts.assignments.get(&write_outer), Some(&outer));
        assert_eq!(check_with_names(&ast, &source, &facts), Ok(()));
    }

    #[test]
    fn 正式_p11_偽造相同型別但不同宣告身分不能跨作用域繞過檢查() {
        let code = "fn main()->int{\nvar n:int = 1\nif(true){\nvar n:int = 2\n}\nreturn n\n}\n";
        let src = source::Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&src);
        let ast = parser::parse(&src, &lexed.tokens).unwrap();
        let mut facts = names::resolve_with_facts(&ast, &src).unwrap();
        let outer = DeclarationId(code.find("var n:int = 1").unwrap());
        let inner = DeclarationId(code.find("var n:int = 2").unwrap());
        let read_at = code.find("return n").unwrap() + "return ".len();
        assert_eq!(facts.references.get(&read_at), Some(&outer));
        assert_eq!(check_with_names(&ast, &src, &facts), Ok(()));
        // 篡改後即使型別相同，也不能以同名推測重新授權外層引用。
        facts.references.insert(read_at, inner);
        let error = check_with_names(&ast, &src, &facts).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert_eq!(&*error.message, "具名來源不存在可見宣告");
        assert!(error.span.is_some());
        facts.references.remove(&read_at);
        assert!(check_with_names(&ast, &src, &facts).is_err());
    }

    #[test]
    fn f04_正式_p11_必須以_型別加宣告身分_驗證_read_與_assign() {
        let code = "fn main()->int{\nvar value:int = 1\nif(true){\nvar value:int = 2\nvalue = 3\nreturn value\n}\nreturn value\n}\n";
        let source = source::Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let ast = parser::parse(&source, &tokens.tokens).unwrap();
        let mut facts = names::resolve_with_facts(&ast, &source).unwrap();
        let outer = DeclarationId(code.find("var value:int = 1").unwrap());
        let inner = DeclarationId(code.find("var value:int = 2").unwrap());
        let assignment = code.find("value = 3").unwrap();
        let read = code.find("return value").unwrap() + "return ".len();
        assert_eq!(facts.assignments.get(&assignment), Some(&inner));
        assert_eq!(facts.references.get(&read), Some(&inner));
        assert_eq!(check_with_names(&ast, &source, &facts), Ok(()));

        facts.declarations.insert(inner,
            TypedDeclaration::new(inner, ScalarType::Bool, true));
        let err = check_with_names(&ast, &source, &facts).unwrap_err();
        assert_eq!(&*err.message, "賦值目標沒有可見宣告",
            "P11 不能在身份正確時忽視 P09 型別事實遭篡改");
        facts.declarations.insert(inner,
            TypedDeclaration::new(inner, ScalarType::Int, true));
        facts.assignments.insert(assignment, outer);
        let err = check_with_names(&ast, &source, &facts).unwrap_err();
        assert_eq!(&*err.message, "賦值目標沒有可見宣告");
        facts.assignments.insert(assignment, inner);
        facts.declarations.insert(inner,
            TypedDeclaration::new(inner, ScalarType::Bool, true));
        facts.assignments.remove(&assignment);
        let err = check_with_names(&ast, &source, &facts).unwrap_err();
        assert_eq!(&*err.message, "賦值目標沒有可見宣告");
        facts.assignments.insert(assignment, inner);
        facts.declarations.insert(inner,
            TypedDeclaration::new(inner, ScalarType::Int, true));
        facts.references.insert(read, outer);
        let err = check_with_names(&ast, &source, &facts).unwrap_err();
        assert_eq!(&*err.message, "具名來源不存在可見宣告");
        facts.references.insert(read, inner);
        assert_eq!(check_with_names(&ast, &source, &facts), Ok(()));
        facts.declarations.insert(inner,
            TypedDeclaration::new(inner, ScalarType::Int, false));
        assert_eq!(&*check_with_names(&ast, &source, &facts).unwrap_err().message,
            "賦值目標沒有可見宣告",
            "不能以相同 P09 來源及型別但被竄改的 var/const 權限通過 P11");
        facts.declarations.insert(inner,
            TypedDeclaration::new(outer, ScalarType::Int, true));
        assert_eq!(&*check_with_names(&ast, &source, &facts).unwrap_err().message,
            "賦值目標沒有可見宣告",
            "表鍵與 TypedDeclaration 所記載的真實來源身分不一致必須拒絕");
    }

    #[test]
    fn f04_流程合流不得繼承同名同來源但不同型別的偽造狀態() {
        let a = TypedDeclaration::new(DeclarationId(100), ScalarType::Int, true);
        let b = TypedDeclaration::new(DeclarationId(100), ScalarType::Bool, true);
        let mut outer = HashMap::new();
        outer.insert("item".to_owned(), Variable {
            declaration: a, initialized: false,
        });
        let mut exit = HashMap::new();
        exit.insert("item".to_owned(), Variable {
            declaration: b, initialized: true,
        });
        assert_ne!(outer["item"].declaration, exit["item"].declaration);
        assert!(!exit["item"].declaration.matches_use(a.use_at(
            crate::common_ir::NodeSpan::new(200, 204)).resolved));
    }

    #[test]
    fn 首版十九個正例均滿足變數讀前初始化與_const_不再寫入() {
        for i in 1..=19 {
            let path = format!("tests/fixtures/voiles/P01-正{i:02}.voil");
            let code = std::fs::read_to_string(&path).unwrap();
            assert!(inspect(&code).is_ok(), "P11 首版正向失敗：{path}");
        }
    }

    #[test]
    fn 未初始化與_const_再賦值皆是有位置靜態錯誤() {
        for n in [7, 15, 24] {
            let path = format!("tests/fixtures/voiles/P01-反{n:02}.voil");
            let source = std::fs::read_to_string(&path).unwrap();
            let e = inspect(&source).unwrap_err();
            assert_eq!(e.kind, DiagnosticKind::Static);
            assert!(e.span.is_some());
        }
    }

    #[test]
    fn 分支兩側都初始化才可在分支後讀取() {
        let valid = "fn main()->int{\nvar n:int\nvar gate:bool = true\nif (gate){\nn = 1\n}else{\nn = 2\n}\nreturn n\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid = "fn main()->int{\nvar n:int\nif (true){\nn = 1\n}\nreturn n\n}\n";
        assert_eq!(inspect(invalid).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 提早_return_的不可到達後繼路徑不參加初始化合流() {
        let valid = "fn main()->int{\nvar n:int\nif (true){\nreturn 2\n}else{\nn = 3\n}\nreturn n\n}\n";
        assert!(inspect(valid).is_ok());
        // 不以此選定一般不可達碼合法性，也不承諾可執行後端語意。
    }

    #[test]
    fn 賦值來源必須已經初始化且內層遮蔽不能繼承外層狀態() {
        for invalid in [
            "fn main()->int{\nvar a:int\nvar b:int = a\nreturn 0\n}\n",
            "fn main()->int{\nvar a:int\nvar b:int\nb = a\nreturn 0\n}\n",
            "fn main()->int{\nvar a:int = 1\nif (true){\nvar a:int\nreturn a\n}\nreturn 0\n}\n",
        ] {
            assert_eq!(inspect(invalid).unwrap_err().kind, DiagnosticKind::Static);
        }
    }

    #[test]
    fn const_與_var_賦值權限不同且雙分支的初始化後保留狀態() {
        assert_eq!(inspect(
            "fn main()->int{\nconst n:int = 2\nn = 3\nreturn n\n}\n"
        ).unwrap_err().kind, DiagnosticKind::Static);
        assert!(inspect(
            "fn main()->int{\nvar n:int\nn = 1\nn = 2\nreturn n\n}\n"
        ).is_ok());
    }

    #[test]
    fn p16_6_r04_分支快照共用作用域並在寫入時隔離() {
        let mut scope = HashMap::new();
        scope.insert("value".to_owned(), Variable {
            declaration: TypedDeclaration::new(DeclarationId(10), ScalarType::Int, true),
            initialized: false,
        });
        let original = Flow { scopes: vec![Rc::new(scope)], reachable: true };
        let mut changed = original.clone();
        assert!(Rc::ptr_eq(&original.scopes[0], &changed.scopes[0]));
        changed.write("value").expect("原宣告必須存在").initialized = true;
        assert!(!Rc::ptr_eq(&original.scopes[0], &changed.scopes[0]));
        assert!(!original.lookup("value").unwrap().initialized);
        assert!(changed.lookup("value").unwrap().initialized);
        assert_eq!(changed.lookup("value").unwrap().declaration.declaration, DeclarationId(10));
    }

    #[test]
    fn p16_6_r04_多分支與遮蔽正反語意不因快照共享而改變() {
        let mut code = String::from("fn main()->int{\nvar n:int\n");
        for i in 0..24 {
            code.push_str(&format!(
                "if (true){{\nvar shadow_{i}:int = 1\nn = 1\n}}else{{\nn = 2\n}}\n"
            ));
        }
        code.push_str("return n\n}\n");
        assert!(inspect(&code).is_ok());
        assert_eq!(inspect(
            "fn main()->int{\nvar n:int\nif (true){\nn = 1\n}else{\nvar n:int = 2\n}\nreturn n\n}\n"
        ).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 頂層讀取未初始化變數必須被拒絕() {
        let bad = "var a:int\nvar b:int = a\nfn main()->int{\nreturn 0\n}\n";
        assert_eq!(inspect(bad).unwrap_err().kind, DiagnosticKind::Static);
    }
}
