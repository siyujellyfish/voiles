//! P16.6-R02.2：公開 MVP-A 與私有安全前端共用的「已核准交集」語意 AST。
//! 兩套舊解析器目前仍存在，且各自的完整語法、診斷和入口權限不變。
//! 只有可以保留原宣告位置、順序與語意的結點才可以投影到此結構。

use crate::expr::{Expr as PublicExpr, ExprKind as PublicExprKind};
use crate::parser::{self, Item, StmtKind as PublicStmtKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarExpr {
    Int(i64),
    Bool(bool),
    Name(String),
    Binary { left: Box<ScalarExpr>, op: char, right: Box<ScalarExpr> },
    CallZero(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarStmt {
    Declare {
        declaration_start: usize,
        name: String,
        ty: String,
        mutable: bool,
        init: Option<ScalarExpr>,
    },
    Assign { name: String, value: ScalarExpr },
    If { condition: ScalarExpr, yes: Vec<ScalarStmt>, no: Option<Vec<ScalarStmt>> },
    Return(ScalarExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScalarFunction {
    pub(crate) name: String,
    pub(crate) result_ty: String,
    pub(crate) body: Vec<ScalarStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedAst {
    pub(crate) functions: Vec<ScalarFunction>,
}

fn public_expression(expr: &PublicExpr) -> Option<ScalarExpr> {
    let result = match &expr.kind {
        PublicExprKind::IntLiteral(raw) =>
            ScalarExpr::Int(crate::syntax_shared::integer_i64(raw).ok()?),
        PublicExprKind::BoolLiteral(value) => ScalarExpr::Bool(*value),
        PublicExprKind::Name(value) => ScalarExpr::Name(value.clone()),
        PublicExprKind::Add(left, right) => ScalarExpr::Binary {
            left: Box::new(public_expression(left)?),
            op: '+',
            right: Box::new(public_expression(right)?),
        },
        PublicExprKind::Subtract(left, right) => ScalarExpr::Binary {
            left: Box::new(public_expression(left)?),
            op: '-',
            right: Box::new(public_expression(right)?),
        },
        PublicExprKind::Group(inner) => return public_expression(inner),
        PublicExprKind::ZeroArgCall(name) => ScalarExpr::CallZero(name.clone()),
        _ => return None,
    };
    Some(result)
}

fn public_statements(stmts: &[parser::Stmt]) -> Option<Vec<ScalarStmt>> {
    stmts.iter().map(|statement| {
        let value = match &statement.kind {
            PublicStmtKind::Binding(binding) => ScalarStmt::Declare {
                declaration_start: binding.start,
                name: binding.name.clone(),
                ty: match binding.ty {
                    parser::Type::Int => "int",
                    parser::Type::Bool => "bool",
                }.into(),
                mutable: binding.mutable,
                init: binding.initializer.as_ref().map(public_expression).transpose_option()?,
            },
            PublicStmtKind::Assignment { name, value } => ScalarStmt::Assign {
                name: name.clone(), value: public_expression(value)?,
            },
            PublicStmtKind::IfChain { branches, otherwise } => {
                // 未對齊 else-if 的 AST 嵌套前，一律不產生冒充已對齊的結果。
                let [(condition, yes)] = branches.as_slice() else { return None };
                ScalarStmt::If {
                    condition: public_expression(condition)?,
                    yes: public_statements(&yes.statements)?,
                    no: otherwise.as_ref()
                        .map(|block| public_statements(&block.statements))
                        .transpose_option()?,
                }
            }
            PublicStmtKind::Return(value) => ScalarStmt::Return(public_expression(value)?),
        };
        Some(value)
    }).collect()
}

/// 僅供已驗證 P01 MVP-A 的 AST 做無副作用投影；未支援者回傳 None。
pub(crate) fn from_public(file: &parser::File) -> Option<SharedAst> {
    let functions = file.items.iter().map(|item| {
        let Item::Function(fun) = item else { return None };
        Some(ScalarFunction {
            name: fun.name.clone(),
            result_ty: match fun.returns {
                parser::Type::Int => "int", parser::Type::Bool => "bool"
            }.into(),
            body: public_statements(&fun.body.statements)?,
        })
    }).collect::<Option<Vec<_>>>()?;
    Some(SharedAst { functions })
}

trait OptionTranspose<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}
impl<T> OptionTranspose<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> { self }
}
