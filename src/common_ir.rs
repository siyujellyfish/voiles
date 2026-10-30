//! P16.6-R02.2：公開 MVP-A 與私有安全前端共用的「已核准交集」語意 AST。
//! 兩套舊解析器目前仍存在，且各自的完整語法、診斷和入口權限不變。
//! 只有可以保留原宣告位置、順序與語意的結點才可以投影到此結構。
// R02.2 此刻只由私有／公開差異回歸消耗；正式 Checker 遷移前，避免非測試編譯死碼警告。
#![allow(dead_code)]

use crate::expr::{Expr as PublicExpr, ExprKind as PublicExprKind};
use crate::parser::{self, Item, StmtKind as PublicStmtKind};
use crate::ownership_ledger::DeclarationId;

/// R02.5：只允許公開 MVP-A 與已核准 P13 交集的具體純值型別。
/// 其他型別（含非 Copyable str、struct、容器）不可用任意字串冒充。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType { Int, Bool }

impl ScalarType {
    pub(crate) fn from_name(ty: &str) -> Option<Self> {
        match ty { "int" => Some(Self::Int), "bool" => Some(Self::Bool), _ => None }
    }
}

// parser::Type 現為此型別的同一個名稱，不再需要轉換兩套等價型別。


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
        declaration: DeclarationId,
        name: String,
        ty: ScalarType,
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
    pub(crate) result_ty: ScalarType,
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
                declaration: DeclarationId(binding.start),
                name: binding.name.clone(),
                ty: binding.ty,
                mutable: binding.mutable,
                init: map_optional(binding.initializer.as_ref(), public_expression)?,
            },
            PublicStmtKind::Assignment { name, value } => ScalarStmt::Assign {
                name: name.clone(), value: public_expression(value)?,
            },
            PublicStmtKind::IfChain { branches, otherwise } => {
                // P01 的平面 else-if 與私有 P13 的巢狀 If(no=[If])
                // 必須產生同一棵保留原分支順序的共同 AST。
                if branches.is_empty() { return None; }
                let mut tail = map_optional(otherwise.as_ref(),
                    |block| public_statements(&block.statements))?;
                for (condition, yes) in branches.iter().rev() {
                    tail = Some(vec![ScalarStmt::If {
                        condition: public_expression(condition)?,
                        yes: public_statements(&yes.statements)?,
                        no: tail,
                    }]);
                }
                tail?.into_iter().next()?
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
            result_ty: fun.returns,
            body: public_statements(&fun.body.statements)?,
        })
    }).collect::<Option<Vec<_>>>()?;
    Some(SharedAst { functions })
}

// 外層 Option 表示投影成功／失敗；內層 Option 才表示語法原本不存在。
// 不可把缺少 initializer 或缺少 else 誤當成「無法投影」。
fn map_optional<T, U>(
    source: Option<&T>, project: impl FnOnce(&T) -> Option<U>,
) -> Option<Option<U>> {
    match source {
        Some(value) => Some(Some(project(value)?)),
        None => Some(None),
    }
}
