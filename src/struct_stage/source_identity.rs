// F01：由現行私有 Parser 唯一建立的 struct／欄位／參數／
// 運算式／語句與巢狀區塊，採用和公開 Parser 完全相同的
// UTF-8 來源、模式、角色與父節點包含關係契約。
// 不解析第二份 AST，不核准任何額外所有權或控制流程語意。
fn verified_private_syntax_tree(
    text: &str, structs: &HashMap<String, StructDecl>, functions: &[Function],
) -> bool {
    verified_private_syntax_roles(text, structs, functions).is_some()
}
/// A1：直接從唯一已驗證私有 AST 走訪得到帶模式的全角色來源清冊。
fn verified_private_syntax_roles(
    text: &str, structs: &HashMap<String, StructDecl>, functions: &[Function],
) -> Option<std::collections::BTreeSet<crate::common_ir::ModeNodeIdentity>> {
    use crate::common_ir::{FrontendMode, NodeRole, NodeSpan, SourceIdentityAudit};

    fn term(
        value: &Term, parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = value.node_span();
        if !audit.record(NodeRole::Expression, span, Some(parent)) {
            return false;
        }
        match &value.kind {
            TermKind::Group(inner) => term(inner, span, audit),
            TermKind::BinaryInt(binary) =>
                term(&binary.left, span, audit)
                    && term(&binary.right, span, audit),
            TermKind::Construct(_, arguments) =>
                arguments.iter().all(|(_, expression)| term(expression, span, audit)),
            TermKind::Call(call) =>
                call.arguments.iter().all(|expression| term(expression, span, audit)),
            TermKind::MethodCall(_, _, arguments) =>
                arguments.iter().all(|expression| term(expression, span, audit)),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::Name(_)
            | TermKind::Move(_) | TermKind::Borrow(_) | TermKind::FieldMove(_, _)
            | TermKind::FieldRead(_, _) => true,
        }
    }

    fn block(
        value: &crate::syntax_shared::ParsedBlock<Stmt>,
        parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = NodeSpan::new(value.start, value.end);
        audit.record(NodeRole::Block, span, Some(parent))
            && value.checked_direct_statements(FrontendMode::PrivateVerified, audit.source)
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
            StmtKind::Declare(decl) => {
                let dspan = NodeSpan::new(decl.start, decl.end);
                audit.record(NodeRole::Declaration, dspan, Some(span))
                    && decl.initializer.as_ref().is_none_or(|x| term(x, dspan, audit))
            }
            StmtKind::Assign(value) => term(&value.expression, span, audit),
            StmtKind::FieldAssign { value, .. } => term(value, span, audit),
            StmtKind::Return(value) => term(&value.expression, span, audit),
            StmtKind::If { branch, no } =>
                branch.start >= span.start
                    && term(&branch.condition, span, audit)
                    && block(&branch.body, span, audit)
                    && no.as_ref().is_none_or(|otherwise| match otherwise {
                        crate::syntax_shared::ParsedElse::Block(block_value) =>
                            block(block_value, span, audit),
                        crate::syntax_shared::ParsedElse::ElseIf(stmt) =>
                            statement(stmt, span, audit),
                        #[cfg(test)]
                        crate::syntax_shared::ParsedElse::SyntheticTest(stmts) =>
                            stmts.iter().all(|stmt| statement(stmt, span, audit)),
                    }),
            StmtKind::While { condition, body } =>
                term(condition, span, audit)
                    && block(body, span, audit),
            StmtKind::ForRange { iterator, iterator_start, body, .. } => {
                let end = match iterator_start.checked_add(iterator.len()) {
                    Some(end) => end, None => return false,
                };
                audit.record(NodeRole::Parameter,
                    NodeSpan::new(*iterator_start, end), Some(span))
                    && audit.source.get(*iterator_start..end) == Some(iterator.as_str())
                    && block(body, span, audit)
            }
            StmtKind::Break | StmtKind::Continue => true,
        }
    }

    /// F01：名稱與型別的真正來源 token 必須精確包含原字串，
    /// 而非在方法或欄位文字內用 find(name) 猜一個表面合法的位置。
    fn typed_member_tokens(
        name: &str, name_span: NodeSpan,
        ty: &str, type_span: NodeSpan,
        parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        name_span.end <= type_span.start
            && audit.record(NodeRole::Name, name_span, Some(parent))
            && audit.record(NodeRole::Type, type_span, Some(parent))
            && audit.source.get(name_span.start..name_span.end) == Some(name)
            && audit.source.get(type_span.start..type_span.end) == Some(ty)
    }

    fn function(
        value: &Function, parent: NodeSpan, audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        let span = NodeSpan::new(value.start, value.end);
        if !audit.record(NodeRole::Function, span, Some(parent)) {
            return false;
        }
        value.params.iter().all(|parameter| {
            let pspan = parameter.declaration_span;
            parameter.declaration_start == pspan.start
                && parameter.name_span.start == pspan.start
                && parameter.type_span.end == pspan.end
                && audit.record(NodeRole::Parameter, pspan, Some(span))
                && typed_member_tokens(
                    &parameter.name, parameter.name_span,
                    &parameter.ty, parameter.type_span,
                    pspan, audit,
                )
        }) && block(&value.body, span, audit)
    }

    let mut audit = SourceIdentityAudit::new(text, FrontendMode::PrivateVerified);
    let full = NodeSpan::new(0, text.len());
    let checked = structs.values().all(|definition| {
        let span = definition.source_span;
        audit.record(NodeRole::Struct, span, Some(full))
            && definition.fields.iter().all(|field| {
                audit.record(NodeRole::Field, field.declaration_span, Some(span))
                    && field.declaration_span.start < field.name_span.start
                    && field.type_span.end <= field.declaration_span.end
                    && audit.source.get(field.declaration_span.start..field.declaration_span.end)
                        .is_some_and(|raw| raw.starts_with(
                            if field.immutable { "const" } else { "var" }
                        ))
                    && typed_member_tokens(
                        &field.name, field.name_span,
                        &field.ty, field.type_span,
                        field.declaration_span, &mut audit,
                    )
                    && field.default.as_ref().is_none_or(|expression|
                        term(expression, field.declaration_span, &mut audit)
                    )
            })
            && definition.methods.iter().all(|method| function(method, span, &mut audit))
    }) && functions.iter().all(|fun| function(fun, full, &mut audit));
    (checked && audit.count() > 0).then(|| audit.verified_roles().collect())
}
