// F01：正式公開 Parser 產物的完整節點身分盤點。
// 僅由原解析器建構的 AST 正向讀取；不作第二次 Token 解析、
// 不改變公開 MVP-A 的核准語法或診斷來源。
pub(crate) fn verified_public_syntax_tree(file: &File, source: &str) -> bool {
    use crate::common_ir::{FrontendMode, NodeRole, NodeSpan, SourceIdentityAudit};

    fn expression(
        value: &Expr, parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = value.node_span();
        if !audit.record(NodeRole::Expression, span, Some(parent)) {
            return false;
        }
        match &value.kind {
            ExprKind::Add(binary) | ExprKind::Subtract(binary) =>
                expression(&binary.left, span, audit)
                    && expression(&binary.right, span, audit),
            ExprKind::Group(inner) | ExprKind::ZeroArgMethod(inner, _) =>
                expression(inner, span, audit),
            ExprKind::IndexAccess(owner, index)
            | ExprKind::FirstIndexWrite(owner, index) =>
                expression(owner, span, audit) && expression(index, span, audit),
            ExprKind::Scalar(_) | ExprKind::Name(_) | ExprKind::ZeroArgCall(_)
            | ExprKind::DecimalLiteral(_) | ExprKind::StringLiteral(_) => true,
        }
    }

    fn block(
        value: &Block, parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = NodeSpan::new(value.start, value.end);
        audit.record(NodeRole::Block, span, Some(parent))
            && value.checked_direct_statements(FrontendMode::PublicMvpA, audit.source)
            && value.statements.iter().all(|stmt| statement(stmt, span, audit))
    }

    fn statement(
        value: &Stmt, parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = value.node_span();
        if !audit.record(NodeRole::Statement, span, Some(parent)) {
            return false;
        }
        match &value.kind {
            StmtKind::Binding(decl) => {
                let declared = NodeSpan::new(decl.start, decl.end);
                audit.record(NodeRole::Declaration, declared, Some(span))
                    && decl.initializer.as_ref().is_none_or(
                        |expr| expression(expr, declared, audit)
                    )
            }
            StmtKind::Assignment(value) => expression(&value.expression, span, audit),
            StmtKind::Return(value) => expression(&value.expression, span, audit),
            StmtKind::IfChain(chain) => {
                chain.end <= span.end
                    && !chain.arms.is_empty()
                    && chain.arms.iter().all(|arm|
                        arm.start >= span.start
                            && expression(&arm.branch.condition, span, audit)
                            && block(&arm.branch.body, span, audit)
                    )
                    && chain.otherwise.as_ref().is_none_or(
                        |otherwise| block(otherwise, span, audit)
                    )
            }
        }
    }

    let mut audit = SourceIdentityAudit::new(source, FrontendMode::PublicMvpA);
    let root = NodeSpan::new(file.start, file.end);
    if audit.source.len() != file.end || file.start != 0 {
        return false;
    }
    let valid = file.items.iter().all(|item| match item {
        Item::Function(function) => {
            let span = NodeSpan::new(function.start, function.end);
            audit.record(NodeRole::Function, span, Some(root))
                && block(&function.body, span, &mut audit)
        }
        Item::Binding(decl) => {
            let span = NodeSpan::new(decl.start, decl.end);
            audit.record(NodeRole::Declaration, span, Some(root))
                && decl.initializer.as_ref().is_none_or(
                    |expr| expression(expr, span, &mut audit)
                )
        }
    });
    valid && audit.count() > 0
}
