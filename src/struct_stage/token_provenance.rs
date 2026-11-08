// A1／F01：正式私有 Parser 已建構完 AST，保守核對它與
// 同一份真 Lexer tokens 的函式標頭及全部區塊宣告的來源。
// 這不建立第二棵 AST，不簽發借用、移動或 ResourceId 權限。
fn verified_private_source_tokens(
    text: &str, tokens: &[Token],
    structs: &HashMap<String, StructDecl>, functions: &[Function],
) -> bool {
    verified_private_token_roles(text, tokens, structs, functions).is_some()
}
/// 真私有 Parser／Checker 入口的所有標頭、成員及深層 name/type token。
fn verified_private_token_roles(
    text: &str, tokens: &[Token],
    structs: &HashMap<String, StructDecl>, functions: &[Function],
) -> Option<std::collections::BTreeSet<crate::common_ir::ModeNodeIdentity>> {
    use crate::common_ir::{FrontendMode, NodeRole, NodeSpan, SourceIdentityAudit};

    /// V7：原 Lexer 的 await 前綴必須與真正 Call AST 明確綁定。
    /// 只稽核 Parser 已建立的 expression，不掃描文字重建第二份 AST；
    /// 既有非 await 子表達式仍由同一遞迴走訪，以防來源巢狀偽裝。
    fn term(
        value: &Term, tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        match &value.kind {
            TermKind::Call(call) => {
                if call.awaited {
                    let Some(index) = tokens.binary_search_by_key(
                        &value.start, |token| token.start,
                    ).ok() else { return false; };
                    let Some(prefix) = tokens.get(index) else { return false; };
                    let Some(name) = tokens.get(index + 1) else { return false; };
                    let Some(left) = tokens.get(index + 2) else { return false; };
                    let close_valid = tokens.iter().any(|token|
                        token.end == value.end
                            && matches!(token.kind, TokenKind::Symbol(')')));
                    if !matches!(&prefix.kind, TokenKind::Name(text) if text == "await")
                        || !matches!(&name.kind, TokenKind::Name(text)
                            if text == &call.name)
                        || !matches!(left.kind, TokenKind::Symbol('('))
                        || prefix.start != value.start || prefix.end > name.start
                        || name.end > left.start || !close_valid
                        || audit.source.get(prefix.start..prefix.end) != Some("await")
                        || audit.source.get(name.start..name.end) != Some(call.name.as_str())
                    {
                        return false;
                    }
                    if !audit.record(NodeRole::Name,
                        NodeSpan::new(prefix.start, prefix.end),
                        Some(value.node_span())) {
                        return false;
                    }
                } else if tokens.binary_search_by_key(
                    &value.start, |token| token.start,
                ).ok().and_then(|index| tokens.get(index).zip(tokens.get(index + 1)))
                    .is_some_and(|(prefix, next)|
                        matches!(&prefix.kind, TokenKind::Name(word) if word == "await")
                            && matches!(next.kind, TokenKind::Name(_))) {
                    // 即使有人將已解析 await Call 的 awaited 身分篡改為 false，
                    // 原相鄰 token 仍足以證明前綴不得冒充同步呼叫。
                    return false;
                }
                call.arguments.iter().all(|arg| term(arg, tokens, audit))
            }
            TermKind::Group(inner) => term(inner, tokens, audit),
            TermKind::BinaryInt(binary) =>
                term(&binary.left, tokens, audit)
                    && term(&binary.right, tokens, audit),
            TermKind::Construct(_, fields) => fields.iter().all(|(_, arg)|
                term(arg, tokens, audit)),
            TermKind::MethodCall(_, _, args) => args.iter().all(|arg|
                term(arg, tokens, audit)),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::Name(_)
            | TermKind::UnresolvedZeroArgs(_) | TermKind::Move(_)
            | TermKind::Borrow(_) | TermKind::FieldMove(_, _)
            | TermKind::FieldRead(_, _) => true,
        }
    }

    fn block(
        body: &crate::syntax_shared::ParsedBlock<Stmt>,
        tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        body.statements.iter().all(|stmt| statement(stmt, tokens, audit))
    }
    fn statement(
        stmt: &Stmt, tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        match &stmt.kind {
            StmtKind::Declare(decl) =>
                crate::syntax_shared::checked_declaration_tokens(
                    decl, &decl.ty, tokens, audit,
                ) && decl.initializer.as_ref().is_none_or(|value|
                    term(value, tokens, audit)),
            StmtKind::If { branch, no } => {
                term(&branch.condition, tokens, audit)
                    && block(&branch.body, tokens, audit)
                    && no.as_ref().is_none_or(|otherwise| match otherwise {
                        crate::syntax_shared::ParsedElse::Block(body) =>
                            block(body, tokens, audit),
                        crate::syntax_shared::ParsedElse::ElseIf(stmt) =>
                            statement(stmt, tokens, audit),
                        #[cfg(test)]
                        crate::syntax_shared::ParsedElse::SyntheticTest(stmts) =>
                            stmts.iter().all(|stmt| statement(stmt, tokens, audit)),
                    })
            }
            StmtKind::While { condition, body } =>
                term(condition, tokens, audit) && block(body, tokens, audit),
            StmtKind::ForRange { body, .. } => block(body, tokens, audit),
            StmtKind::Assign(assign) => term(&assign.expression, tokens, audit),
            StmtKind::FieldAssign { value, .. } => term(value, tokens, audit),
            StmtKind::Return(value) => term(&value.expression, tokens, audit),
            StmtKind::Break | StmtKind::Continue => true,
        }
    }
    fn function(
        fun: &Function, tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        crate::syntax_shared::checked_function_header_tokens(
            fun.start, fun.end, fun.body.start,
            &fun.name, &fun.result_ty, fun.is_async, tokens, audit,
        ) && fun.params.iter().all(|parameter| {
            audit.record(NodeRole::Parameter, parameter.declaration_span,
                Some(NodeSpan::new(fun.start, fun.end)))
                && crate::syntax_shared::checked_typed_member_tokens(
                    &parameter.name, parameter.name_span,
                    &parameter.ty, parameter.type_span,
                    parameter.declaration_span, tokens, audit,
                )
        }) && block(&fun.body, tokens, audit)
    }

    let mut audit = SourceIdentityAudit::new(text, FrontendMode::PrivateVerified);
    let source_span = NodeSpan::new(0, text.len());
    let structs_ok = structs.iter().all(|(name, definition)| {
        let Some(index) = tokens.binary_search_by_key(
            &definition.source_span.start, |token| token.start,
        ).ok() else { return false; };
        let Some(keyword) = tokens.get(index) else { return false; };
        let Some(named) = tokens.get(index + 1) else { return false; };
        matches!(&keyword.kind, TokenKind::Keyword(word) if word == "struct")
            && matches!(&named.kind, TokenKind::Name(word) if word == name)
            && audit.record(NodeRole::Struct, definition.source_span, Some(source_span))
            && audit.record(NodeRole::Name,
                NodeSpan::new(named.start, named.end), Some(definition.source_span))
            && text.get(keyword.start..keyword.end) == Some("struct")
            && text.get(named.start..named.end) == Some(name.as_str())
            && definition.fields.iter().all(|field| {
                let Some(index) = tokens.binary_search_by_key(
                    &field.declaration_span.start, |token| token.start,
                ).ok() else { return false; };
                let Some(keyword) = tokens.get(index) else { return false; };
                let expected = if field.immutable { "const" } else { "var" };
                matches!(&keyword.kind, TokenKind::Keyword(word) if word == expected)
                    && text.get(keyword.start..keyword.end) == Some(expected)
                    && audit.record(NodeRole::Field, field.declaration_span,
                        Some(definition.source_span))
                    && crate::syntax_shared::checked_typed_member_tokens(
                        &field.name, field.name_span,
                        &field.ty, field.type_span,
                        field.declaration_span, tokens, &mut audit,
                    )
            })
            && definition.methods.iter().all(|fun| function(fun, tokens, &mut audit))
    });
    let valid = structs_ok && functions.iter().all(|fun|
        function(fun, tokens, &mut audit));
    valid.then(|| audit.verified_roles().collect())
}
