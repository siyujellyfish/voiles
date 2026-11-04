// 靜態先於副作用：唯一私有 Checker 已驗證的 AST + 原 Lexer token +
// DeclarationId + TypedHIR → 全部可求值欄位值。未知語意維持拒絕。

fn checked_field_kind(value: &str)
    -> Result<crate::mixed_struct_runtime::ScalarKind, ConstructorExecutionError> {
    use crate::mixed_struct_runtime::ScalarKind;
    match value {
        "int" => Ok(ScalarKind::Int),
        "bool" => Ok(ScalarKind::Bool),
        "str" => Ok(ScalarKind::Str),
        _ => Err(ConstructorExecutionError::Unsupported(
            "P17-F 僅接通 int/bool/str 欄位，巢狀結構／float 不推測求值",
        )),
    }
}

fn checked_scalar(term: &Term, kind: crate::mixed_struct_runtime::ScalarKind,
                  tokens: &[Token]) -> Result<CheckedScalar, ConstructorExecutionError> {
    use crate::mixed_struct_runtime::ScalarKind;
    use ConstructorExecutionError::Unsupported as Block;
    match (kind, &term.kind) {
        (ScalarKind::Int, TermKind::Scalar(
            crate::syntax_shared::ScalarLiteral::Integer(text),
        )) => {
            let value = crate::syntax_shared::integer_i64(text)
                .map_err(|_| Block("整數欄位未通過既定 i64 規則"))?;
            Ok(CheckedScalar::Int(value))
        }
        (ScalarKind::Bool, TermKind::Scalar(
            crate::syntax_shared::ScalarLiteral::Boolean(value),
        )) => Ok(CheckedScalar::Bool(*value)),
        (ScalarKind::Str, TermKind::Literal("str")) => {
            // 不能在 AST 上另外切片解碼原始文字；用同一份正式 Lexer token。
            let value = tokens.iter().find_map(|token| {
                if token.start == term.start && token.end == term.end {
                    if let TokenKind::StringLiteral(value) = &token.kind {
                        return Some(value.clone());
                    }
                }
                None
            }).ok_or(Block("字串欄位缺少同一 Lexer 的已解碼來源"))?;
            Ok(CheckedScalar::Str(value))
        }
        _ => Err(Block("欄位運算式不是此最小切片可獨立證明的純常值")),
    }
}

impl PreparedPrivateConstructor {
    fn prepare(text: &str) -> Result<Self, ConstructorExecutionError> {
        use ConstructorExecutionError::Unsupported as Block;
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(ConstructorExecutionError::Static)?;
        if verified.structs.len() != 1 || verified.functions.len() != 1 {
            return Err(Block("P17-F 僅接通單一 struct 與 main 的來源"));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int" || !main.params.is_empty() {
            return Err(Block("P17-F 僅接通無參數 main()->int"));
        }
        let [one, two] = main.body.statements.as_slice() else {
            return Err(Block("P17-F 僅接通單筆建立式與 int return"));
        };
        let (StmtKind::Declare(binding), StmtKind::Return(ret)) =
            (&one.kind, &two.kind)
        else { return Err(Block("P17-F 不接通其它控制流程、移動與借用語句")); };
        let definition = verified.structs.get(&binding.ty)
            .ok_or(Block("建立式宣告必須是唯一已驗證結構型別"))?;
        if !definition.methods.is_empty() || definition.fields.is_empty() {
            return Err(Block("目前不接通結構方法或無欄位結構的執行"));
        }
        let declaration = DeclarationId(binding.start);
        if !verified.plan.declarations.typed_declaration(declaration).is_some_and(
            |proof| proof.declaration == declaration
                && proof.ty == binding.ty.as_str()
                && proof.mutable == binding.mutable
        ) {
            return Err(Block("結構持有者缺少原 Checker 宣告、型別、可寫性證據"));
        }
        let initializer = binding.initializer.as_ref()
            .ok_or(Block("結構必須明確建立並初始化"))?;
        let supplied: Vec<(String, Term)> = match &initializer.kind {
            TermKind::Construct(name, args) if name == &binding.ty => args.clone(),
            TermKind::UnresolvedZeroArgs(name) if name == &binding.ty => Vec::new(),
            _ => return Err(Block("此切片只支援同型別具名建立式")),
        };
        // 按真正 Parser 的具名引數次序，不使用 HashMap 的遍歷順序。
        let mut seen = std::collections::HashSet::new();
        let mut named = Vec::new();
        for (name, value) in &supplied {
            if !seen.insert(name.as_str()) {
                return Err(Block("重複具名欄位不得進入求值"));
            }
            let field = definition.field(name)
                .ok_or(Block("未知欄位不得進入求值"))?;
            named.push(CheckedNamedSeed {
                field: name.clone(),
                value: checked_scalar(value, checked_field_kind(&field.ty)?, &verified.tokens)?,
            });
        }
        // 被具名覆蓋的預設**不可**求值，其餘按原 struct 欄位宣告順序。
        let mut fields = Vec::new();
        for field in &definition.fields {
            let kind = checked_field_kind(&field.ty)?;
            let covered = seen.contains(field.name.as_str());
            let default = if covered {
                None
            } else {
                let source = field.default.as_ref()
                    .ok_or(Block("此切片不接通尚未初始化的結構欄位"))?;
                Some(checked_scalar(source, kind, &verified.tokens)?)
            };
            fields.push(CheckedFieldSeed {
                name: field.name.clone(), kind, default, immutable: field.immutable,
            });
        }
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
            &ret.expression.kind
        else { return Err(Block("目前只支援整數常值返回")); };
        if !verified.plan.scalar_hir.expression(ret.expression.node_span())
            .is_some_and(|proof| proof.ty == crate::common_ir::ScalarType::Int) {
            return Err(Block("return 缺少真正私有 Checker 的 int HIR"));
        }
        let return_value = crate::syntax_shared::integer_i64(raw)
            .map_err(|_| Block("return 不符合既有 int64 範圍"))?;
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, instance: binding.name.clone(),
            fields, named, return_value,
        })
    }
}
