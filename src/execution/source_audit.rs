//! P17-A：從同一份正式 Parser AST 唯讀核對真正的來源詞彙值。
//! P10 HIR 證明型別與跨度，但並不允許相同跨度內偷換常值、名稱或呼叫目標。
//! 這是執行授權的防偽閘門，不重建 Parser、不加入新語法或資源事件。

use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, StmtKind};
use crate::syntax_shared::ScalarLiteral;

fn expression_matches(expr: &Expr, source: &str) -> bool {
    let span = expr.node_span();
    let Some(fragment) = source.get(span.start..span.end) else { return false };
    if fragment.is_empty() || !source.is_char_boundary(span.start)
        || !source.is_char_boundary(span.end) {
        return false;
    }
    match &expr.kind {
        ExprKind::Scalar(ScalarLiteral::Integer(value)) =>
            fragment == value,
        ExprKind::Scalar(ScalarLiteral::Boolean(value)) =>
            fragment == if *value { "true" } else { "false" },
        ExprKind::Name(name) => fragment == name,
        ExprKind::Call(call) => {
            // P01 已核准無參數呼叫。函式目標名稱來自原 Lexer 的名稱，
            // 不能替換成另一個同型別函式然後沿用原 P10 HIR。
            let Some(suffix) = fragment.strip_prefix(call.name.as_str()) else {
                return false;
            };
            let rest = suffix.trim();
            rest.starts_with('(') && rest.ends_with(')')
        }
        ExprKind::Group(inner) => {
            fragment.trim_start().starts_with('(')
                && fragment.trim_end().ends_with(')')
                && inner.start >= expr.start && inner.end <= expr.end
                && expression_matches(inner, source)
        }
        ExprKind::BinaryInt(binary) =>
            binary.left.start >= expr.start && binary.right.end <= expr.end
                && binary.left.end <= binary.right.start
                && expression_matches(&binary.left, source)
                && expression_matches(&binary.right, source),
        // 完整語言預備節點仍非 P01 公開可執行範圍。
        ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) |
        ExprKind::IndexAccess(_, _) | ExprKind::FirstIndexWrite(_, _) |
        ExprKind::ZeroArgMethod(_, _) => false,
    }
}

fn declaration_matches(binding: &Binding, source: &str) -> bool {
    let Some(fragment) = source.get(binding.start..binding.end) else { return false };
    let prefix = if binding.mutable { "var" } else { "const" };
    let Some(remaining) = fragment.strip_prefix(prefix) else { return false };
    let Some(remaining) = remaining.trim_start().strip_prefix(binding.name.as_str())
        else { return false };
    remaining.trim_start().starts_with(':')
        && binding.initializer.as_ref().is_none_or(|expr| expression_matches(expr, source))
}

fn block_matches(block: &Block, source: &str) -> bool {
    let Some(fragment) = source.get(block.start..block.end) else { return false };
    if !fragment.trim_start().starts_with('{')
        || !fragment.trim_end().ends_with('}') {
        return false;
    }
    for statement in &block.statements {
        let Some(text) = source.get(statement.start..statement.end) else { return false };
        if text.is_empty() || statement.start < block.start || statement.end > block.end {
            return false;
        }
        let matches = match &statement.kind {
            StmtKind::Binding(binding) => declaration_matches(binding, source),
            StmtKind::Assignment(assignment) => {
                let Some(rest) = text.strip_prefix(assignment.name.as_str()) else {
                    return false;
                };
                rest.trim_start().starts_with('=')
                    && expression_matches(&assignment.expression, source)
            }
            StmtKind::Return(value) =>
                text.starts_with("return") && expression_matches(&value.expression, source),
            StmtKind::IfChain(chain) => {
                text.starts_with("if")
                    && chain.arms.iter().all(|arm|
                        expression_matches(&arm.branch.condition, source)
                        && block_matches(&arm.branch.body, source)
                    )
                    && chain.otherwise.as_ref().is_none_or(|body| block_matches(body, source))
            }
        };
        if !matches { return false; }
    }
    true
}

/// 同一 UTF-8 來源的真正 AST 詞彙內容，與已核准 P01 形式一致才放行。
/// 只有 `PreparedProgram::prepare` 完成原正式 P09/P10/P11/P12 之後
/// 才能呼叫這項附加防偽檢查；本函式**不是**獨立的語言 Checker。
pub(crate) fn atoms_match_source(file: &File, source: &str) -> bool {
    file.items.iter().all(|item| match item {
        Item::Binding(binding) => declaration_matches(binding, source),
        Item::Function(function) => {
            let Some(header) = source.get(function.start..function.body.start) else {
                return false;
            };
            let Some(rest) = header.strip_prefix("fn") else { return false };
            let Some(rest) = rest.trim_start().strip_prefix(function.name.as_str())
                else { return false };
            rest.trim_start().starts_with('(')
                && block_matches(&function.body, source)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::{ControlEvent, PreparedProgram, Value};

    fn validated(code: &str) -> PreparedProgram {
        PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap()
    }

    #[test]
    fn p17_真實標量來源與巢狀算術_群組_函式正常求值() {
        let code = "fn part()->int{\nreturn 5\n}\nfn main()->int{\nreturn (2+(part()-1))\n}\n";
        let plan = validated(code);
        assert!(atoms_match_source(&plan.ast, code));
        assert_eq!(plan.execute(), Ok(Value::Int(6)));
    }

    #[test]
    fn p17_相同跨度換整數常值不可沿用舊型別_hir_執行() {
        let code = "fn main()->int{\nreturn 2\n}\n";
        let mut plan = validated(code);
        let Item::Function(function) = &mut plan.ast.items[0] else { panic!("真來源須是 fn") };
        let StmtKind::Return(value) = &mut function.body.statements[0].kind
            else { panic!("真來源須是 return") };
        let ExprKind::Scalar(ScalarLiteral::Integer(integer)) = &mut value.expression.kind
            else { panic!("真來源須是整數") };
        *integer = "9".into();
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_布林條件內容改寫不能憑原跨度取得真控制邊() {
        let code = "fn main()->int{\nif (true){\nreturn 2\n}\nreturn 3\n}\n";
        let mut plan = validated(code);
        let Item::Function(function) = &mut plan.ast.items[0] else { panic!() };
        let StmtKind::IfChain(chain) = &mut function.body.statements[0].kind else { panic!() };
        chain.arms[0].branch.condition.kind = ExprKind::Scalar(ScalarLiteral::Boolean(false));
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_具名讀取的節點文字改寫不能偷用原宣告身分() {
        let code = "fn main()->int{\nvar a:int = 2\nvar b:int = 9\nreturn a\n}\n";
        let mut plan = validated(code);
        let Item::Function(function) = &mut plan.ast.items[0] else { panic!() };
        let StmtKind::Return(value) = &mut function.body.statements[2].kind else { panic!() };
        value.expression.kind = ExprKind::Name("b".into());
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_同型別函式目標換名不能借用已驗證呼叫型別() {
        let code = "fn alpha()->int{\nreturn 2\n}\nfn bravo()->int{\nreturn 9\n}\nfn main()->int{\nreturn alpha()\n}\n";
        let mut plan = validated(code);
        let Item::Function(function) = &mut plan.ast.items[2] else { panic!() };
        let StmtKind::Return(value) = &mut function.body.statements[0].kind else { panic!() };
        let ExprKind::Call(call) = &mut value.expression.kind else { panic!() };
        call.name = "bravo".into();
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_賦值目標偷換同型別名稱須在動態效果前拒絕() {
        let code = "fn main()->int{\nvar a:int = 1\nvar b:int = 2\na = 3\nreturn a\n}\n";
        let mut plan = validated(code);
        let Item::Function(function) = &mut plan.ast.items[0] else { panic!() };
        let StmtKind::Assignment(assignment) = &mut function.body.statements[2].kind else { panic!() };
        assignment.name = "b".into();
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_來源原始字串被改寫不會發出任何真執行事件() {
        let code = "fn main()->int{\nreturn 2\n}\n";
        let mut plan = validated(code);
        let index = plan.source.bytes.iter().position(|b| *b == b'2').unwrap();
        plan.source.bytes[index] = b'9';
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(!outcome.events.iter().any(|event| matches!(event, ControlEvent::FunctionEntered { .. })));
    }
}
