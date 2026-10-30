//! P11 MVP-A 確定初始化與賦值權限。
//! 不替未決的不可達碼一般合法性或完整語言字串／所有權創建語意。

use std::collections::HashMap;
use std::rc::Rc;

use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind};
use crate::source::Source;
use crate::ownership_ledger::DeclarationId;

#[derive(Debug, Clone, Copy)]
struct Variable {
    declaration_start: DeclarationId,
    initialized: bool,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct Flow {
    // 分支快照共用唯讀作用域；寫入時才複製實際受影響的那一層。
    scopes: Vec<Rc<HashMap<String, Variable>>>,
    reachable: bool,
}

impl Flow {
    fn lookup(&self, name: &str) -> Option<Variable> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name).copied())
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
                let var = state.lookup(name).ok_or_else(|| self.err(
                    "具名來源不存在可見宣告", expr.start, expr.end
                ))?;
                if !var.initialized {
                    return Err(self.err("變數在所有可達路徑完成初始化前不可讀取", expr.start, expr.end));
                }
            }
            ExprKind::Group(inner) | ExprKind::ZeroArgMethod(inner, _) =>
                self.read(inner, state)?,
            ExprKind::Add(a, b) | ExprKind::Subtract(a, b) |
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
            declaration_start: DeclarationId(b.start),
            initialized: b.initializer.is_some(),
            mutable: b.mutable,
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
                    let target = state.lookup(name).ok_or_else(|| self.err(
                        "賦值目標沒有可見宣告", statement.start, statement.end
                    ))?;
                    if !target.mutable {
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
                    for (condition, body) in branches {
                        // 所有一般 bool 條件的具名來源都要先初始化。
                        self.read(condition, &entry)?;
                        let mut branch = entry.clone();
                        self.block(body, &mut branch)?;
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
                                        v.declaration_start == var.declaration_start && v.initialized
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
    let analyzer = Analyzer { source };
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
            declaration_start: DeclarationId(10), initialized: false, mutable: true,
        });
        let original = Flow { scopes: vec![Rc::new(scope)], reachable: true };
        let mut changed = original.clone();
        assert!(Rc::ptr_eq(&original.scopes[0], &changed.scopes[0]));
        changed.write("value").expect("原宣告必須存在").initialized = true;
        assert!(!Rc::ptr_eq(&original.scopes[0], &changed.scopes[0]));
        assert!(!original.lookup("value").unwrap().initialized);
        assert!(changed.lookup("value").unwrap().initialized);
        assert_eq!(changed.lookup("value").unwrap().declaration_start, DeclarationId(10));
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
