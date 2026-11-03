//! P10：已核准 MVP-A 型別檢查，僅 int64、bool 與零參數函式結果。
//! 不推定 P11 確定初始化、變更權限，也不納入 Optional／struct／Copyable。

use std::collections::HashMap;
use std::cell::RefCell;

use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind, Type};
use crate::source::Source;
use crate::ownership_ledger::DeclarationId;
use crate::common_ir::{ResolvedNames, TypedDeclaration, VerifiedScalarHir};

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
    globals: HashMap<String, TypedDeclaration<Type>>,
    probe_move: bool,
    resolved: Option<&'a ResolvedNames>,
    verified_hir: RefCell<VerifiedScalarHir>,
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
    fn visible_declaration(&self, name: &str, pos: usize,
        scopes: &[HashMap<String, TypedDeclaration<Type>>])
        -> Option<TypedDeclaration<Type>> {
        crate::common_ir::lookup_lexical(scopes.iter(), name).copied()
            .or_else(|| self.globals.get(name).copied()
                .filter(|binding| binding.declaration.0 < pos))
    }

    fn resolved_type(&self, span: crate::common_ir::NodeSpan, name: &str,
        scopes: &[HashMap<String, TypedDeclaration<Type>>]) -> Option<Type> {
        let lexical = self.visible_declaration(name, span.start, scopes)?;
        match self.resolved {
            // 只有 P09 來源身分與實際可見的詞彙綁定型別完全相符才能授權。
            Some(facts) => facts.verified_reference(span, lexical)
                .map(|use_| {
                    self.verified_hir.borrow_mut().record_reference(use_);
                    use_.resolved.ty
                }),
            None => Some(lexical.ty),
        }
    }

    fn integer(&self, literal: &str, expr: &Expr) -> Result<i64, Diagnostic> {
        use crate::syntax_shared::{integer_i64, IntegerError};
        integer_i64(literal).map_err(|error| {
            let message = match error {
                IntegerError::InvalidDigits => "整數字面值格式不合法",
                IntegerError::OutOfRange => "整數字面值超出有號 64 位元範圍",
            };
            self.error(message, expr.start, expr.end)
        })
    }

    fn expression(&self, expr: &Expr,
        scopes: &[HashMap<String, TypedDeclaration<Type>>]) -> Result<TypedValue, Diagnostic> {
        let value = match &expr.kind {
            ExprKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(text)) =>
                Ok(TypedValue {
                    ty: Type::Int,
                    integer: Some(self.integer(text, expr)?),
                }),
            ExprKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(_)) =>
                Ok(TypedValue::new(Type::Bool)),
            ExprKind::Name(name) => {
                let ty = self.resolved_type(expr.node_span(), name, scopes)
                    .ok_or_else(|| self.error(
                    "無法解析具名來源型別", expr.start, expr.end))?;
                Ok(TypedValue::new(ty))
            }
            ExprKind::ZeroArgCall(call) => {
                let ty = *self.functions.get(&call.name)
                    .ok_or_else(|| self.error("無法解析無參數函式結果型別", expr.start, expr.end))?;
                // 不試算遞迴／函式回傳的靜態值。
                Ok(TypedValue::new(ty))
            }
            ExprKind::Group(inner) => self.expression(inner, scopes),
            ExprKind::ZeroArgMethod(inner, method) if self.probe_move
                && method == "move" && matches!(inner.kind, ExprKind::Name(_)) =>
                self.expression(inner, scopes),
            ExprKind::Add(binary) | ExprKind::Subtract(binary) => {
                let lhs = self.expression(&binary.left, scopes)?;
                let rhs = self.expression(&binary.right, scopes)?;
                if !crate::common_ir::arithmetic_operand_is_ready(Some(lhs.ty), true) {
                    return Err(self.error("整數加減左運算元必須為 int",
                        binary.left.start, binary.left.end));
                }
                if !crate::common_ir::arithmetic_operand_is_ready(Some(rhs.ty), true) {
                    return Err(self.error("整數加減右運算元必須為 int",
                        binary.right.start, binary.right.end));
                }
                let known = match (lhs.integer, rhs.integer) {
                    (Some(a), Some(b)) => {
                        let sum = if binary.operator == '+' {
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
                if self.resolved.is_some() {
                    self.verified_hir.borrow_mut().record_arithmetic(
                        crate::common_ir::VerifiedScalarArithmetic {
                            span: expr.node_span(),
                            left: binary.left.node_span(),
                            right: binary.right.node_span(),
                            operator: binary.operator,
                        });
                }
                Ok(TypedValue { ty: Type::Int, integer: known })
            }
            // P06 曾為完整語言的後續功能辨識形狀；
            // 此處絕不以形狀存在就把它們納入 P01 MVP-A 型別集合。
            ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) |
            ExprKind::IndexAccess(_, _) | ExprKind::FirstIndexWrite(_, _) |
            ExprKind::ZeroArgMethod(_, _) =>
                Err(self.error("首版不支援此運算式型別或操作", expr.start, expr.end)),
        }?;
        if self.resolved.is_some() {
            self.verified_hir.borrow_mut()
                .record_expression(expr.node_span(), value.ty);
        }
        Ok(value)
    }

    fn initializer(&self, binding: &Binding,
        scopes: &[HashMap<String, TypedDeclaration<Type>>])
        -> Result<(), Diagnostic> {
        if let Some(ref expr) = binding.initializer {
            let found = self.expression(expr, scopes)?;
            self.require(found.ty, binding.ty, expr, "宣告初始化型別與明確型別不符")?;
        }
        Ok(())
    }

    fn block(&self, block: &Block,
        scopes: &mut Vec<HashMap<String, TypedDeclaration<Type>>>, result: Type)
        -> Result<(), Diagnostic> {
        scopes.push(HashMap::new());
        for stmt in &block.statements {
            match &stmt.kind {
                StmtKind::Binding(b) => {
                    self.initializer(b, scopes)?;
                    scopes.last_mut().unwrap().insert(b.name.clone(),
                        TypedDeclaration::new(DeclarationId(b.start), b.ty, b.mutable));
                }
                StmtKind::Assignment(assignment) => {
                    let name = &assignment.name;
                    let value = &assignment.expression;
                    // 正式 P09 已綁定賦值目標的宣告身分；不得在 P10 按同名
                    // 字串自行挑選另一個遮蔽宣告。
                    let declared = self.visible_declaration(name, stmt.start, scopes)
                        .and_then(|binding| match self.resolved {
                            Some(names) => names.verified_assignment(stmt.node_span(), binding)
                                .map(|use_| {
                                    self.verified_hir.borrow_mut().record_assignment(use_);
                                    use_.resolved.ty
                                }),
                            None => Some(binding.ty),
                        }).ok_or_else(|| self.error(
                        "賦值目標型別不可解析", stmt.start, stmt.end))?;
                    let actual = self.expression(value, scopes)?;
                    self.require(actual.ty, declared, value, "賦值右值必須與宣告型別一致")?;
                    // const 再賦值與 var 初始化路徑由 P11 驗證。
                }
                StmtKind::Return(expr) => {
                    let actual = self.expression(expr, scopes)?;
                    // P10 已核准 int／bool 型別；P11 仍獨立證明每條路徑
                    // 的讀前初始化，不在這裡製造未經檢查的資源權限。
                    if !crate::common_ir::scalar_return_is_ready(
                        Some(actual.ty), result, true,
                    ) {
                        return Err(self.error(
                            "回傳值型別與函式標註不一致", expr.start, expr.end,
                        ));
                    }
                }
                StmtKind::IfChain(chain) => {
                    for arm in &chain.arms {
                        let cond = self.expression(&arm.branch.condition, scopes)?;
                        if !crate::common_ir::boolean_condition_is_ready(Some(cond.ty),true) {
                            return Err(self.error("if 條件必須具有 bool 型別",
                                arm.branch.condition.start,arm.branch.condition.end));
                        }
                        self.block(&arm.branch.body, scopes, result)?;
                    }
                    if let Some(body) = &chain.otherwise {
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
    check_with_move_probe(file, source, false, None)
}

/// 供正式 P09→P10 資料流使用的已解析引用／宣告型別證據入口。
pub(crate) fn check_with_names(
    file: &File, source: &Source, names: &crate::common_ir::ResolvedNames,
) -> Result<(), Diagnostic> {
    check_with_move_probe(file, source, false, Some(names))
}

/// P13 測試模式不擴大 P01 公開介面。
pub(crate) fn check_move_probe(file: &File, source: &Source) -> Result<(), Diagnostic> {
    check_with_move_probe(file, source, true, None)
}

/// 正式 P10 通過後才交付 P11 的具型別來源證據。
pub(crate) fn check_with_names_and_hir(
    file: &File, source: &Source, names: &ResolvedNames,
) -> Result<VerifiedScalarHir, Diagnostic> {
    check_with_hir_internal(file, source, false, Some(names))
}

fn check_with_move_probe<'a>(
    file: &File, source: &'a Source, probe_move: bool,
    resolved: Option<&'a ResolvedNames>,
) -> Result<(), Diagnostic> {
    check_with_hir_internal(file, source, probe_move, resolved).map(|_| ())
}

fn check_with_hir_internal<'a>(
    file: &File, source: &'a Source, probe_move: bool,
    resolved: Option<&'a ResolvedNames>,
) -> Result<VerifiedScalarHir, Diagnostic> {
    let mut functions = HashMap::new();
    let mut globals = HashMap::new();
    for item in &file.items {
        match item {
            Item::Function(f) => { functions.insert(f.name.clone(), f.result_ty); }
            Item::Binding(b) => { globals.insert(b.name.clone(),
                TypedDeclaration::new(DeclarationId(b.start), b.ty, b.mutable)); }
        }
    }
    let checker = TypeChecker {
        source, functions, globals, probe_move, resolved,
        verified_hir: RefCell::new(VerifiedScalarHir::default()),
    };
    for item in &file.items {
        match item {
            Item::Binding(b) => checker.initializer(b, &[])?,
            Item::Function(f) => checker.block(&f.body, &mut Vec::new(), f.result_ty)?,
        }
    }
    let verified_hir = checker.verified_hir.into_inner();
    // 正式 P10→P11 不可僅靠 debug_assert 判定相同 AST 節點是否
    // 被不同控制路徑簽成不一致的型別、宣告來源或 var 目標。
    // P10 公開與私有 P13 檢查器現在共用完整 UTF-8 來源跨度、
    // 已驗證具名引用及對應運算式型別一致性的同一 HIR 閘門。
    let source_text = std::str::from_utf8(&source.bytes).ok();
    if !source_text.is_some_and(|text| verified_hir.source_spans_are_valid(text)) {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P10：相同來源位置的已驗證型別或宣告身分證據衝突".into(),
            span: None,
        });
    }
    Ok(verified_hir)
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
    fn 公開正式回傳使用共同_int_bool_安全裁決且診斷不退步() {
        for code in [
            "fn helper()->int{\nreturn 1\n}\nfn main()->int{\nreturn helper()\n}\n",
            "fn flag()->bool{\nreturn true\n}\nfn main()->int{\nreturn 0\n}\n",
        ] {
            assert!(inspect(code).is_ok(), "合法函式返回不得退步：{code}");
        }
        for (code, message) in [
            ("fn main()->int{\nreturn true\n}\n", "回傳值型別與函式標註不一致"),
            ("fn flag()->bool{\nreturn 1\n}\nfn main()->int{\nreturn 0\n}\n",
             "回傳值型別與函式標註不一致"),
        ] {
            let error = inspect(code).expect_err("純值結果不符必須拒絕");
            assert_eq!(error.kind, DiagnosticKind::Static);
            assert_eq!(error.message.as_ref(), message);
            assert!(error.span.is_some(), "診斷應保留真正來源位置");
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
    fn f04_篡改相同型別身分或宣告型別皆不得繞過真正詞彙綁定() {
        let code = "fn main()->int{\nvar n:int = 1\nif(true){\nvar n:int = 2\nn = 3\nreturn n\n}\nreturn n\n}\n";
        let source = source::Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let ast = parser::parse(&source, &tokens.tokens).unwrap();
        let original = names::resolve_with_facts(&ast, &source).unwrap();
        let outer = DeclarationId(code.find("var n:int = 1").unwrap());
        let inner = DeclarationId(code.find("var n:int = 2").unwrap());
        let write = code.find("n = 3").unwrap();
        let read = code.find("return n").unwrap() + "return ".len();
        assert_eq!(original.references.get(&read), Some(&inner));
        assert_eq!(original.assignments.get(&write), Some(&inner));
        assert_eq!(check_with_names(&ast, &source, &original), Ok(()));

        let mut changed = names::resolve_with_facts(&ast, &source).unwrap();
        changed.references.insert(read, outer);
        assert_eq!(&*check_with_names(&ast, &source, &changed).unwrap_err().message,
            "無法解析具名來源型別",
            "型別相同但宣告位置不同，不可在 P10 偷渡非可見持有者");
        changed.references.insert(read, inner);
        changed.assignments.insert(write, outer);
        assert_eq!(&*check_with_names(&ast, &source, &changed).unwrap_err().message,
            "賦值目標型別不可解析");

        changed.assignments.insert(write, inner);
        changed.declarations.insert(inner,
            TypedDeclaration::new(inner, Type::Bool, true));
        assert_eq!(&*check_with_names(&ast, &source, &changed).unwrap_err().message,
            "賦值目標型別不可解析",
            "篡改 P09 型別事實不能改變正式 Parser 宣告的 int 身分");
        changed.declarations.insert(inner,
            TypedDeclaration::new(inner, Type::Int, false));
        assert_eq!(&*check_with_names(&ast, &source, &changed).unwrap_err().message,
            "賦值目標型別不可解析",
            "P09 var/const 權限被偽造時，P10 不得視為同一個 TypedDeclaration");
        changed.declarations.insert(inner,
            TypedDeclaration::new(outer, Type::Int, true));
        assert_eq!(&*check_with_names(&ast, &source, &changed).unwrap_err().message,
            "賦值目標型別不可解析",
            "P09 宣告表鍵與宣告值的 DeclarationId 不能不一致");
    }

    #[test]
    fn 正式_p10_賦值目標型別直接由_p09_已解析身分提供() {
        let code = "fn main()->int{\nvar item:int = 1\nif(true){\nvar item:bool = false\nitem = true\n}\nitem = 3\nreturn item\n}\n";
        let source = source::Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let ast = parser::parse(&source, &lexed.tokens).unwrap();
        let mut facts = names::resolve_with_facts(&ast, &source).unwrap();
        let inner = code.find("item = true").unwrap();
        let outer = code.find("item = 3").unwrap();
        assert_eq!(facts.assignment_type(inner), Some(Type::Bool));
        assert_eq!(facts.assignment_type(outer), Some(Type::Int));
        assert!(check_with_names(&ast, &source, &facts).is_ok());
        // 篡改後不退回依名稱猜測；對已驗證來源事實採缺失即拒絕。
        facts.assignments.remove(&inner);
        let err = check_with_names(&ast, &source, &facts).unwrap_err();
        assert_eq!(err.message.as_ref(), "賦值目標型別不可解析");
    }

    #[test]
    fn 真正名稱階段的宣告身分供正式型別階段辨識遮蔽() {
        let code = "fn main()->int{\nvar item:int = 2\nif(true){\nvar item:bool = true\nif(item){\nreturn 1\n}\n}\nreturn item\n}\n";
        let source = source::Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let ast = parser::parse(&source, &tokens.tokens).unwrap();
        let names = names::resolve_with_facts(&ast, &source).unwrap();
        assert_eq!(check_with_names(&ast, &source, &names), Ok(()));
        let inner_name = code.find("if(item)").unwrap() + 3;
        assert_eq!(names.reference_type(inner_name), Some(Type::Bool));
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
