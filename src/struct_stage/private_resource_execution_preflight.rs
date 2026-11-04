//! P17-C：同一正式 AST、原始 Lexer token 與 Checker 的安全證據。

fn checked_single_move(checked: &VerifiedPrivateSource)
    -> Result<CheckedSingleMove, PrivateStrExecutionError> {
    use PrivateStrExecutionError::Unsupported as Block;
    if !checked.structs.is_empty() || checked.functions.len() != 1 {
        return Err(Block("本切片僅接受沒有 struct 的單一 main"));
    }
    let main = &checked.functions[0];
    if main.name != "main" || main.result_ty != "int" || !main.params.is_empty() {
        return Err(Block("本切片僅接受無參數的 main()->int"));
    }
    let [first, second, last] = main.body.statements.as_slice() else {
        return Err(Block("本切片只接受兩次資源宣告與尾端 return"));
    };
    let (StmtKind::Declare(owner), StmtKind::Declare(target), StmtKind::Return(ret)) =
        (&first.kind, &second.kind, &last.kind)
    else { return Err(Block("本切片僅接受宣告、宣告、return 的直線路徑")); };
    if owner.ty != "str" || target.ty != "str"
        || !owner.mutable || !target.mutable || owner.name == target.name {
        return Err(Block("此切片要求兩個不同的 var str 宣告"));
    }
    let Some(literal) = owner.initializer.as_ref() else {
        return Err(Block("str 必須以常值立即初始化"));
    };
    if !matches!(literal.kind, TermKind::Literal("str")) {
        return Err(Block("來源只能是獨立字串常值"));
    }
    // 不能直接切掉引號推定跳脫文字。只允許原正式 Lexer 的同跨度
    // StringLiteral 解碼結果，且兩個跨度必須與 Checker 已核准 AST 相同。
    let decoded_utf8 = checked.tokens.iter().find_map(|token| {
        if token.start == literal.start && token.end == literal.end {
            if let TokenKind::StringLiteral(value) = &token.kind {
                return Some(value.clone());
            }
        }
        None
    }).ok_or(Block("字面值缺少同次 Lexer 的已解碼證據"))?;
    let Some(moving) = target.initializer.as_ref() else {
        return Err(Block("目的必須有明確 .move() 右值"));
    };
    if !matches!(&moving.kind, TermKind::Move(name) if name == &owner.name) {
        return Err(Block("目的必須移動此來源持有者"));
    }
    let from_id = DeclarationId(owner.start);
    let to_id = DeclarationId(target.start);
    if !checked.plan.declarations.direct_moves.contains(&(from_id, to_id)) {
        return Err(Block("缺少真 Checker 簽發的具名整筆移動證據"));
    }
    for (id, decl) in [(from_id, owner), (to_id, target)] {
        if !checked.plan.declarations.typed_declaration(id).is_some_and(|fact|
            fact.declaration == id && fact.ty == "str" && fact.mutable == decl.mutable) {
            return Err(Block("宣告身分／型別／權限不符合 Checker"));
        }
    }
    let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
        &ret.expression.kind
    else { return Err(Block("此切片只接入整數常值 return")); };
    if !checked.plan.scalar_hir.expression(ret.expression.node_span())
        .is_some_and(|proof| proof.ty == crate::common_ir::ScalarType::Int) {
        return Err(Block("return 缺少真私有 Checker 的 int 型別 HIR"));
    }
    let return_value = crate::syntax_shared::integer_i64(raw)
        .map_err(|_| Block("return 整數超出既有 int64 範圍"))?;
    Ok(CheckedSingleMove {
        from: owner.name.clone(), to: target.name.clone(),
        decoded_utf8, source: from_id, target: to_id, return_value,
    })
}
