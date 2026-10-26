//! P10：已核准 MVP-A 型別檢查，僅 int64、bool 與零參數函式結果。
//! 不推定 P11 確定初始化、變更權限，也不納入 Optional／struct／Copyable。

use std::collections::HashMap;

use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind, Type};
use crate::source::Source;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TypedValue {
    ty: Type,
    /// 只有可由目前式子字面形式直接確知時才記錄，不讀取／執行函式。
    integer: Option<i64>,
}

impl TypedValue {
    fn new(ty: Type) -> Self { Self { ty, integer: None } }
}

struct TypeChecker<'a> {
    source: &'a Source,
    functions: HashMap<String, Type>,
    globals: HashMap<String, (usize, Type)>,
    probe_move: bool,
}

impl<'a> TypeChecker<'a> {
    fn error(&self, text: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: text.into(),
            span: Some(self.source.span(start, end)),
        }
    }

    fn require(&self, actual: Type, expected: Type, expr: &Expr, text: &str) -> Result<(), Diagnostic> {
        if actual == expected { Ok(()) }
        else { Err(self.error(text, expr.start, expr.end)) }
    }

    /// 型別環境只記錄已在目前詞彙位置合法可見的名稱；
    /// 可用性與名稱重複已由 P09 優先驗證。
    fn variable_type(&self, name: &str, pos: usize,
        scopes: &[HashMap<String, Type>]) -> Option<Type> {
        for scope in scopes.iter().rev() {
            if let Some(ty) = scope.get(name) { return Some(*ty); }
        }
        self.globals.get(name)
            .and_then(|(declaration, ty)| (*declaration < pos).then_some(*ty))
    }

    fn integer(&self, literal: &str, expr: &Expr) -> Result<i64, Diagnostic> {
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
            return Err(self.error("整數字面值格式不合法", expr.start, expr.end));
        }
        let number = u128::from_str_radix(&normalized, radix)
            .map_err(|_| self.error("整數字面值超出有號 64 位元範圍", expr.start, expr.end))?;
        if number > i64::MAX as u128 {
            return Err(self.error("整數字面值超出有號 64 位元範圍", expr.start, expr.end));
        }
        Ok(number as i64)
    }

    fn expression(&self, expr: &Expr,
        scopes: &[HashMap<String, Type>]) -> Result<TypedValue, Diagnostic> {
        match &expr.kind {
            ExprKind::IntLiteral(text) => Ok(TypedValue {
                ty: Type::Int,
                integer: Some(self.integer(text, expr)?),
            }),
            ExprKind::BoolLiteral(_) => Ok(TypedValue::new(Type::Bool)),
            ExprKind::Name(name) => {
                let ty = self.variable_type(name, expr.start, scopes)
                    .ok_or_else(|| self.error("無法解析具名來源型別", expr.start, expr.end))?;
                Ok(TypedValue::new(ty))
            }
            ExprKind::ZeroArgCall(name) => {
                let ty = *self.functions.get(name)
                    .ok_or_else(|| self.error("無法解析無參數函式結果型別", expr.start, expr.end))?;
                // 不試算遞迴／函式回傳的靜態值。
                Ok(TypedValue::new(ty))
            }
            ExprKind::Group(inner) => self.expression(inner, scopes),
            ExprKind::ZeroArgMethod(inner, method) if self.probe_move
                && method == "move" && matches!(inner.kind, ExprKind::Name(_)) =>
                self.expression(inner, scopes),
            ExprKind::Add(left, right) | ExprKind::Subtract(left, right) => {
                let lhs = self.expression(left, scopes)?;
                let rhs = self.expression(right, scopes)?;
                self.require(lhs.ty, Type::Int, left, "整數加減左運算元必須為 int")?;
                self.require(rhs.ty, Type::Int, right, "整數加減右運算元必須為 int")?;
                let known = match (lhs.integer, rhs.integer) {
                    (Some(a), Some(b)) => {
                        let sum = if matches!(&expr.kind, ExprKind::Add(_, _)) {
                            a.checked_add(b)
                        } else {
                            a.checked_sub(b)
                        };
                        Some(sum.ok_or_else(|| {
                            self.error("已確定的整數運算超出有號 64 位元範圍", expr.start, expr.end)
                        })?)
                    }
                    _ => None,
                };
                Ok(TypedValue { ty: Type::Int, integer: known })
            }
            // P06 曾為完整語言的後續功能辨識形狀；
            // 此處絕不以形狀存在就把它們納入 P01 MVP-A 型別集合。
            ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) |
            ExprKind::IndexAccess(_, _) | ExprKind::FirstIndexWrite(_, _) |
            ExprKind::ZeroArgMethod(_, _) =>
                Err(self.error("首版不支援此運算式型別或操作", expr.start, expr.end)),
        }
    }

    fn initializer(&self, binding: &Binding, scopes: &[HashMap<String, Type>])
        -> Result<(), Diagnostic> {
        if let Some(ref expr) = binding.initializer {
            let found = self.expression(expr, scopes)?;
            self.require(found.ty, binding.ty, expr, "宣告初始化型別與明確型別不符")?;
        }
        Ok(())
    }

    fn block(&self, block: &Block, scopes: &mut Vec<HashMap<String, Type>>, result: Type)
        -> Result<(), Diagnostic> {
        scopes.push(HashMap::new());
        for stmt in &block.statements {
            match &stmt.kind {
                StmtKind::Binding(b) => {
                    self.initializer(b, scopes)?;
                    scopes.last_mut().unwrap().insert(b.name.clone(), b.ty);
                }
                StmtKind::Assignment { name, value } => {
                    let declared = self.variable_type(name, stmt.start, scopes)
                        .ok_or_else(|| self.error("賦值目標型別不可解析", stmt.start, stmt.end))?;
                    let actual = self.expression(value, scopes)?;
                    self.require(actual.ty, declared, value, "賦值右值必須與宣告型別一致")?;
                    // const 再賦值與 var 初始化路徑由 P11 驗證。
                }
                StmtKind::Return(expr) => {
                    let actual = self.expression(expr, scopes)?;
                    self.require(actual.ty, result, expr, "回傳值型別與函式標註不一致")?;
                }
                StmtKind::IfChain { branches, otherwise } => {
                    for (condition, body) in branches {
                        let cond = self.expression(condition, scopes)?;
                        self.require(cond.ty, Type::Bool, condition, "if 條件必須具有 bool 型別")?;
                        self.block(body, scopes, result)?;
                    }
                    if let Some(body) = otherwise {
                        self.block(body, scopes, result)?;
                    }
                }
            }
        }
        scopes.pop();
        Ok(())
    }
}

/// 只在 P09 名稱解析及 P07 安全解析成功時呼叫；
/// 不意味 P11 definite initialization／可變權限／P12 main 等驗收完成。
pub(crate) fn check(file: &File, source: &Source) -> Result<(), Diagnostic> {
    check_with_move_probe(file, source, false)
}

/// P13 測試模式不擴大 P01 公開介面。
pub(crate) fn check_move_probe(file: &File, source: &Source) -> Result<(), Diagnostic> {
    check_with_move_probe(file, source, true)
}

fn check_with_move_probe(file: &File, source: &Source, probe_move: bool) -> Result<(), Diagnostic> {
    let mut functions = HashMap::new();
    let mut globals = HashMap::new();
    for item in &file.items {
        match item {
            Item::Function(f) => { functions.insert(f.name.clone(), f.returns); }
            Item::Binding(b) => { globals.insert(b.name.clone(), (b.start, b.ty)); }
        }
    }
    let checker = TypeChecker { source, functions, globals, probe_move };
    for item in &file.items {
        match item {
            Item::Binding(b) => checker.initializer(b, &[])?,
            Item::Function(f) => checker.block(&f.body, &mut Vec::new(), f.returns)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, names, parser, source};

    fn inspect(code: &str) -> Result<(), Diagnostic> {
        let source = source::Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty(), "測試語句不得包含詞彙錯誤");
        let file = parser::parse(&source, &tokens.tokens).expect("測試應能建立安全的 AST");
        names::resolve(&file, &source)?;
        check(&file, &source)
    }

    #[test]
    fn 十九個首版正向來源具有可確認的基本型別() {
        for index in 1..=19 {
            let path = format!("tests/fixtures/voiles/P01-正{index:02}.voil");
            let code = std::fs::read_to_string(&path).unwrap();
            assert!(inspect(&code).is_ok(), "應為合法型別：{path}");
        }
    }

    #[test]
    fn 已核准的型別錯誤與可靜態知的溢位真實拒絕() {
        for number in [8, 20, 29, 30] {
            let path = format!("tests/fixtures/voiles/P01-反{number:02}.voil");
            let source = std::fs::read_to_string(&path).unwrap();
            let result = inspect(&source).expect_err(&format!("{path} 必須拒絕"));
            assert_eq!(result.kind, DiagnosticKind::Static);
            assert!(result.span.is_some());
        }
    }

    #[test]
    fn int_不會偷偷轉成_bool_或_bool_轉成_int() {
        for code in [
            "fn main()->int{\nvar flag:bool = 1\nreturn 0\n}\n",
            "fn main()->int{\nconst n:int = true\nreturn 0\n}\n",
            "fn main()->int{\nreturn false\n}\n",
            "fn main()->int{\nreturn 1 + true\n}\n",
            "fn main()->int{\nvar n:int = 1\nif (n){\nreturn 0\n}\nreturn 0\n}\n",
        ] {
            let error = inspect(code).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn 零參數函式結果與回傳型別須吻合() {
        assert!(inspect(
            "fn helper()->int{\nreturn 3\n}\nfn main()->int{\nreturn helper() + 1\n}\n"
        ).is_ok());
        for code in [
            "fn helper()->bool{\nreturn true\n}\nfn main()->int{\nreturn helper()\n}\n",
            "fn helper()->bool{\nreturn true\n}\nfn main()->int{\nreturn helper() + 1\n}\n",
        ] {
            assert_eq!(inspect(code).unwrap_err().kind, DiagnosticKind::Static);
        }
    }

    #[test]
    fn 進位底線與前導零僅在_i64_界內可用() {
        for code in [
            "fn main()->int{\nreturn 0x_FF\n}\n",
            "fn main()->int{\nreturn 0b1010\n}\n",
            "fn main()->int{\nreturn 0o77\n}\n",
            "fn main()->int{\nreturn 007\n}\n",
            "fn main()->int{\nreturn 9223372036854775807\n}\n",
        ] {
            assert!(inspect(code).is_ok(), "應合法：{code}");
        }
        for code in [
            "fn main()->int{\nreturn 0x_8000000000000000\n}\n",
            "fn main()->int{\nreturn 9223372036854775808\n}\n",
            "fn main()->int{\nreturn 9223372036854775807 + 1\n}\n",
        ] {
            assert_eq!(inspect(code).unwrap_err().kind, DiagnosticKind::Static);
        }
    }

    #[test]
    fn 同名遮蔽後取用局部型別_不會讀到外層型別() {
        assert!(inspect(
            "fn main()->int{\nvar item:int = 2\nif (true){\nvar item:bool = true\n}\nreturn item\n}\n"
        ).is_ok());
        assert_eq!(inspect(
            "fn main()->int{\nvar item:int = 2\nif (true){\nvar item:bool = true\nreturn item\n}\nreturn 0\n}\n"
        ).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 具有任意執行期值的加減不能誤報確定溢位() {
        assert!(inspect(
            "fn helper()->int{\nreturn 9223372036854775807\n}\nfn main()->int{\nreturn helper() + 1\n}\n"
        ).is_ok(), "P10 不試算函式或假設其不變與無副作用");
    }
}
