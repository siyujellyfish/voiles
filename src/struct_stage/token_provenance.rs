// A1／F01：正式私有 Parser 已建構完 AST，保守核對它與
// 同一份真 Lexer tokens 的函式標頭及全部區塊宣告的來源。
// 這不建立第二棵 AST，不簽發借用、移動或 ResourceId 權限。
fn verified_private_source_tokens(
    text: &str, tokens: &[Token],
    structs: &HashMap<String, StructDecl>, functions: &[Function],
) -> bool {
    use crate::common_ir::{FrontendMode, NodeRole, NodeSpan, SourceIdentityAudit};

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
                ),
            StmtKind::If { branch, no } => {
                block(&branch.body, tokens, audit)
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
            StmtKind::ForRange { body, .. } | StmtKind::While { body, .. } =>
                block(body, tokens, audit),
            StmtKind::Assign(_) | StmtKind::FieldAssign { .. }
            | StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue => true,
        }
    }
    fn function(
        fun: &Function, tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        crate::syntax_shared::checked_function_header_tokens(
            fun.start, fun.end, fun.body.start,
            &fun.name, &fun.result_ty, tokens, audit,
        ) && block(&fun.body, tokens, audit)
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
            && definition.methods.iter().all(|fun| function(fun, tokens, &mut audit))
    });
    structs_ok && functions.iter().all(|fun| function(fun, tokens, &mut audit))
}
