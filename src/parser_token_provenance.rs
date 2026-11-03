// A1／F01：只走訪真正公開 Parser 的唯一 AST，依原 Lexer token
// 補完函式、宣告名稱及型別原始身分。不得重新解析或更改原診斷。
pub(crate) fn verified_public_source_tokens(
    file: &File, source: &str, tokens: &[Token],
) -> bool {
    verified_public_token_roles(file, source, tokens).is_some()
}
/// 真 Parser 宣告標頭及全部巢狀 name/type token 的唯讀身分輸出。
pub(crate) fn verified_public_token_roles(
    file: &File, source: &str, tokens: &[Token],
) -> Option<std::collections::BTreeSet<crate::common_ir::ModeNodeIdentity>> {
    use crate::common_ir::{FrontendMode, NodeRole, NodeSpan, SourceIdentityAudit};

    fn decl(
        value: &Binding, audit: &mut SourceIdentityAudit<'_>, tokens: &[Token],
    ) -> bool {
        let name = match value.ty { Type::Int => "int", Type::Bool => "bool" };
        crate::syntax_shared::checked_declaration_tokens(value, name, tokens, audit)
    }
    fn block(
        value: &Block, tokens: &[Token], audit: &mut SourceIdentityAudit<'_>,
    ) -> bool {
        value.statements.iter().all(|stmt| {
            match &stmt.kind {
                StmtKind::Binding(value) => decl(value, audit, tokens),
                StmtKind::IfChain(chain) =>
                    chain.arms.iter().all(|arm| block(&arm.branch.body, tokens, audit))
                    && chain.otherwise.as_ref().is_none_or(|otherwise|
                        block(otherwise, tokens, audit)),
                StmtKind::Assignment(_) | StmtKind::Return(_) => true,
            }
        })
    }
    let mut audit = SourceIdentityAudit::new(source, FrontendMode::PublicMvpA);
    if file.start != 0 || file.end != source.len() { return None; }
    let valid = file.items.iter().all(|item| match item {
        Item::Function(fun) => {
            let result_type = match fun.result_ty {
                Type::Int => "int", Type::Bool => "bool",
            };
            crate::syntax_shared::checked_function_header_tokens(
                fun.start, fun.end, fun.body.start,
                &fun.name, result_type, tokens, &mut audit,
            ) && block(&fun.body, tokens, &mut audit)
        }
        Item::Binding(value) => {
            // 公開頂層沒有函式包覆，仍核對原始 declaration token。
            if !audit.record(NodeRole::Declaration,
                NodeSpan::new(value.start, value.end),
                Some(NodeSpan::new(file.start, file.end))) {
                return false;
            }
            decl(value, &mut audit, tokens)
        }
    });
    valid.then(|| audit.verified_roles().collect())
}
