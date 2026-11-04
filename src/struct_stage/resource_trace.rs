/// G5/F12：橋接只有在同一真正 Checker 已核准該 return 控制邊
/// 及當時整份靜態義務快照存在時，才准根據受限來源產生真事件。
/// ResourceId 必須另外由 runtime 自己配置、由 closed_bridge_events 核帳。
#[cfg(test)]
fn require_checked_bridge_return(
    edges: &std::collections::BTreeMap<usize, VerifiedControlEdge>,
    snapshots: &std::collections::BTreeMap<usize, Vec<VerifiedExitSnapshot>>,
    statement: &Stmt,
) -> Result<(), Diagnostic> {
    let span = statement.node_span();
    let valid = matches!(statement.kind, StmtKind::Return(_))
        && edges.get(&span.start).is_some_and(|edge|
            edge.span == span && edge.flow == FlowState::Returned)
        && snapshots.get(&span.start).is_some_and(|choices|
            !choices.is_empty() && choices.iter().all(|snapshot|
                snapshot.edge == VerifiedControlEdge {
                    span, flow: FlowState::Returned,
                } && !snapshot.scope_sizes.is_empty()
                    && snapshot.scope_sizes.iter().sum::<usize>() == snapshot.bindings.len()
                    && snapshot.active_borrows.is_empty()
                    && snapshot.cleanup_owners
                        == VerifiedExitSnapshot::cleanup_from_bindings(&snapshot.bindings)));
    if valid { Ok(()) } else {
        Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：資源橋接缺少真 Checker 同位置 return 的已驗證靜態安全責任".into(),
            span: None,
        })
    }
}

/// G5/F12：驗證即將執行的受限資源來源持有者，在同一
/// Checker 已核准 return 的靜態快照裡真由 DeclarationId 認領。
/// ResourceId 仍由原 runtime 分配；若該型別無資源（純 Copyable），
/// 只能核對活著的宣告，禁止虛構應清理資源。
#[cfg(test)]
fn require_checked_bridge_owner(
    snapshots: &std::collections::BTreeMap<usize, Vec<VerifiedExitSnapshot>>,
    statement: &Stmt,
    owner: DeclarationId,
    needs_resource_cleanup: bool,
) -> Result<(), Diagnostic> {
    let valid = snapshots.get(&statement.start).is_some_and(|choices|
        !choices.is_empty() && choices.iter().all(|snapshot| {
            let sources: Vec<_> = snapshot.bindings.iter()
                .filter(|binding| binding.declaration == owner && binding.loan.is_none())
                .collect();
            let matched_binding = sources.len() == 1
                && sources[0].value.state() == ValueState::Available;
            let required = snapshot.cleanup_owners.iter().any(|item|
                item.declaration == owner && matches!(item.need,
                    StaticCleanupNeed::Required | StaticCleanupNeed::Conditional));
            matched_binding && (if needs_resource_cleanup {
                required
            } else {
                !required
            })
        }));
    if valid { Ok(()) } else {
        Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：真資源持有者缺少相同 Checker 宣告身分／退出靜態清理責任".into(),
            span: None,
        })
    }
}

/// F12/B09：所有受限橋接都必須檢查**真 runtime 已產生的**事件，
/// 不由 Checker 計畫偽造 ResourceId，且不替代原路徑專用的逆序清理驗證。
#[cfg(test)]
fn closed_bridge_events(
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
) -> Result<Vec<crate::owned_str_runtime::ResourceEvent>, Diagnostic> {
    if crate::owned_str_runtime::event_trace_balanced(&events) {
        Ok(events)
    } else {
        Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：真實資源事件身分、次序或清理閉合不一致".into(),
            span: None,
        })
    }
}

#[cfg(test)]
fn closed_bridge_result<T>(
    value: T, events: Vec<crate::owned_str_runtime::ResourceEvent>,
) -> Result<(T, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    closed_bridge_events(events).map(|events| (value, events))
}

// 對應 P16.6-R01 無語意變更之實作拆檔。
/// 成功僅代表完整檢查了這個明確受限的 P13 子集，不代表 P13 Phase 已通過。
/// P16 私有測試橋接：先完成真正 .voil 的來源／型別／所有權檢查，
/// 再對可靜態確定的單一結構建立式建立真實 UTF-8 擁有者資源追蹤。
/// 不推定主程式控制流程、一般運算式、轉義解碼或 P17 直譯執行。
/// A2/F02：只在通過正式 Checker 後以同一原 Parser 節點讀取
/// 已確認的具名建構欄位；裸 name() 的語法尚未分類，真正結構
/// 存在性仍必須由每條橋接原有的 structs／宣告證據核對。
fn trace_struct_constructor<'a>(term: &'a Term)
    -> Option<(&'a str, &'a [(String, Term)])> {
    match &term.kind {
        TermKind::Construct(name, args) => Some((name, args.as_slice())),
        TermKind::UnresolvedZeroArgs(name) => Some((name, &[])),
        _ => None,
    }
}

#[cfg(test)]
fn trace_string_struct_source(
    text: &str, variable: &str, fail_after: Option<&str>,
) -> Result<crate::struct_initialization::BuildTrace, Diagnostic> {
    use crate::struct_initialization::{build_string_struct, NamedString, StringField};
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(term.kind, TermKind::Literal("str")) { return None; }
        let raw = text.get(term.start..term.end)?;
        let content = raw.strip_prefix('"')?.strip_suffix('"')?;
        // 此處沒有完整字串轉義求值器：不能把原始碼中的
        // 反斜線轉義當成實際 UTF-8 緩衝區內容。
        if content.contains('\\') || content.contains('\n') || content.contains('\r') {
            return None;
        }
        Some(content)
    }
    let VerifiedPrivateSource { source: _, structs, functions: funcs, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：來源不存在 main"))?;
    // 只接通單一確定執行的建立式，避免把不可達分支或
    // 多次執行的程式偷當成已支援的執行期控制流程。
    let [declaration, trailing_return] = main.body.as_slice() else {
        return Err(unsupported("P16：來源追蹤只接受單次宣告接直接返回"));
    };
    if !matches!(&trailing_return.kind, StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind: TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)), start, end,
    } }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：返回及控制流程尚未接通資源追蹤"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, trailing_return)?;
    let (ty, mutable, args) = match &declaration.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
            Some((owner, args))
                if declaration.name == variable && &declaration.ty == owner =>
                (declaration.ty.as_str(), declaration.mutable, args),
            _ => return Err(unsupported("P16：不存在單次具名字串結構建立式")),
        },
        _ => return Err(unsupported("P16：不存在單次具名字串結構建立式")),
    };
    // 這條最早的 P16 來源橋接也必須消費 Checker 實際完成的
    // 宣告來源身分與已驗證可變權限，而非只憑 Parser 的建構式形狀。
    if !declarations.matches(DeclarationId(declaration.start), ty, mutable) {
        return Err(unsupported("P16：具名結構建立缺少 Checker 已驗證持有者身分及型別"));
    }
    require_checked_bridge_owner(
        &exit_snapshots, trailing_return, DeclarationId(declaration.start), true)?;
    let definition = structs.get(ty)
        .ok_or_else(|| unsupported("P16：結構型別不存在"))?;
    let mut fields = Vec::new();
    for field in &definition.fields {
        if field.ty != "str" {
            return Err(unsupported("P16：資源橋接只證明 str 欄位"));
        }
        let default = match &field.default {
            Some(term) => Some(plain_literal(text, term)
                .ok_or_else(|| unsupported("P16：僅接受未經轉義的字串預設常值"))?),
            None => None,
        };
        fields.push(StringField {
            name: &field.name, default, immutable: field.immutable,
        });
    }
    let mut named = Vec::new();
    for (field, term) in args {
        named.push(NamedString {
            field, value: plain_literal(text, term)
                .ok_or_else(|| unsupported("P16：僅接受未經轉義的具名字串常值"))?,
        });
    }
    // 模擬失敗只能放在此次真正會求值的具名或未覆蓋預設步驟。
    // 沒有預設、也未被具名提供的 var 欄位雖存在於結構宣告中，
    // 卻不會建立資源，不可把「指定失敗卻未發生」算成清理證據。
    if let Some(name) = fail_after {
        let in_named = named.iter().any(|arg| arg.field == name);
        let in_defaults = fields.iter().any(|field|
            field.name == name && field.default.is_some());
        if !in_named && !in_defaults {
            return Err(unsupported(
                "P16：失敗注入欄位不存在，或未被此結構求值計畫執行"));
        }
    }
    let mut result = build_string_struct(&fields, &named, fail_after);
    result.resources = closed_bridge_events(result.resources)?;
    Ok(result)
}

/// P16：真正 .voil 來源經完整靜態檢查後，才交給
/// 持續存活的 UTF-8 結構執行期模型。僅驗證「單次建構、
/// 整筆 .move()、直接 return 0」可證明的受限直線來源。
#[cfg(test)]
fn trace_persistent_struct_move_source(
    text: &str, fail_after: Option<&str>,
) -> Result<(Option<String>, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{
        BuildStatus, NamedString, OwnedStructRuntime, StringField, StructRuntimeError,
    };
    use crate::verified_resource_ir::WholeStructResourcePlan;
    let reject = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(source: &'a str, value: &Term) -> Option<&'a str> {
        if !matches!(value.kind, TermKind::Literal("str")) { return None; }
        let value=source.get(value.start..value.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!value.contains('\\') && !value.contains('\n')
            && !value.contains('\r')).then_some(value)
    }
    let VerifiedPrivateSource { source: _, structs, functions, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main=functions.iter().find(|function|function.name=="main")
        .ok_or_else(||reject("P16：字串結構來源須有合法 main"))?;
    let [declare, transfer, last]=main.body.as_slice() else {
        return Err(reject("P16：整筆結構來源只接受建構、移動、返回三語句"));
    };
    if !matches!(&last.kind, StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind: TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)), start, end,
    } }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：非直接 return 0 的控制流程尚無資源執行證明"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, last)?;
    let (owner, struct_name, named_args)=match &declare.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
            Some((actual, args))
                if &declaration.ty==actual =>
                (declaration.name.as_str(),declaration.ty.as_str(),args),
            _=>return Err(reject("P16：第一語句須建立具名 str 欄位結構")),
        },
        _=>return Err(reject("P16：第一語句須建立具名 str 欄位結構")),
    };
    let target=match &transfer.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref() {
            Some(Term {kind:TermKind::Move(source),..})
                if declaration.ty==struct_name && source==owner =>
                declaration.name.as_str(),
            _=>return Err(reject("P16：第二語句須將剛建立的整個結構 .move() 至新持有者")),
        },
        _=>return Err(reject("P16：第二語句須將剛建立的整個結構 .move() 至新持有者")),
    };
    // 先消費 Checker 真正通過所有權、借用及完整初始化證明後交付的
    // 直線整筆移交事實，不能只憑 AST 形狀自行造出資源移動許可。
    let proof = declarations.verified_direct_move(
        DeclarationId(declare.start), DeclarationId(transfer.start), struct_name)
        .ok_or_else(||reject("P16：缺少 Checker 已驗證整筆結構移交來源證據"))?;
    require_checked_bridge_owner(&exit_snapshots, last,
        DeclarationId(transfer.start), true)?;
    let plan = WholeStructResourcePlan::new(proof.source, proof.target)
        .map_err(|_|reject("P16：整筆結構來源與新持有者不可共用宣告身分"))?;
    let definition=structs.get(struct_name)
        .ok_or_else(||reject("P16：結構宣告型別不存在"))?;
    let fields=definition.fields.iter().map(|field| {
        if field.ty!="str" {
            return Err(reject("P16：此資源橋接只支援 str 欄位"));
        }
        let default=field.default.as_ref().map(|value|
            plain_literal(text,value).ok_or_else(||
                reject("P16：此資源橋接不支援帶轉義的字串預設值")))
            .transpose()?;
        Ok(StringField {
            name:field.name.as_str(),default,immutable:field.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let named=named_args.iter().map(|(name,value)|
        plain_literal(text,value).map(|literal|NamedString {
            field:name.as_str(),value:literal,
        }).ok_or_else(||reject("P16：此資源橋接只支援無轉義具名字串常值"))
    ).collect::<Result<Vec<_>,Diagnostic>>()?;
    let mut runtime=OwnedStructRuntime::new();
    let status=runtime.construct(owner,&fields,&named,fail_after);
    if matches!(&status, Err(StructRuntimeError::EvaluationFailed(_))) {
        // 真實失敗注入下不得建立第二持有者，已建立的部分
        // 資源必須在回滾時依序釋放並完成事件帳簿平衡。
        let events=runtime.finish()
            .map_err(|_|reject("P16：失敗回滾後資源身分未完整清理"))?;
        plan.verify_failed_build_events(&events).map_err(|_|
            reject("P16：多欄位建構失敗的來源身分與逆序清理責任不一致"))?;
        return closed_bridge_result(None, events);
    }
    match status {
        Ok(BuildStatus::Complete)=>{},
        Ok(BuildStatus::PartiallyInitialized(_)) =>
            return Err(reject("P16：尚未全欄位初始化的結構不能整筆移動")),
        Ok(_) | Err(_) =>
            return Err(reject("P16：整體資源建立失敗或注入切點不在求值計畫中")),
    }
    runtime.move_whole(owner,target)
        .map_err(|_|reject("P16：整個結構未能安全轉交單一持有者"))?;
    let observed=fields.last().map(|field|field.name).ok_or_else(||
        reject("P16：此來源橋接須有可觀察的 str 欄位"))?;
    let retained=runtime.read_field(target,observed)
        .map_err(|_|reject("P16：移動後新持有者欄位不再有效"))?.to_owned();
    let events=runtime.finish()
        .map_err(|_|reject("P16：整筆移動後未完整清理全部資源身分"))?;
    plan.verify_moved_events(&events).map_err(|_|
        reject("P16：多欄位移交及逆序清理與共同資源責任 IR 不一致"))?;
    closed_bridge_result(Some(retained), events)
}

/// P16 私有字串函式返回證據：只模擬已通過來源檢查且
/// 呼叫端／被呼叫端都屬唯一可證明直線路徑的資源交接。
/// 這不是一般函式直譯器，不能拿此入口運行任意控制流程。
#[cfg(test)]
fn trace_string_return_source(
    text: &str, provider: &str, receiver: &str,
) -> Result<(String, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::owned_str_runtime::OwnedStrRuntime;
    use crate::verified_resource_ir::NamedResourceReturn;
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(term.kind, TermKind::Literal("str")) { return None; }
        let bytes = text.get(term.start..term.end)?;
        let raw = bytes.strip_prefix('"')?.strip_suffix('"')?;
        if raw.contains('\\') || raw.contains('\n') || raw.contains('\r') {
            return None;
        }
        Some(raw)
    }
    let VerifiedPrivateSource { source: _, structs: _, functions: funcs, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：缺少 main"))?;
    let called = funcs.iter().find(|fun| fun.name == provider
        && fun.params.is_empty() && fun.result_ty == "str")
        .ok_or_else(|| unsupported("P16：須為無參數且回傳 str 的函式"))?;
    let [declaration, final_return] = main.body.as_slice() else {
        return Err(unsupported("P16：呼叫端僅接受單次資源接收及直接返回"));
    };
    let valid_receiver = matches!(&declaration.kind, StmtKind::Declare(declaration)
        if declaration.name == receiver && declaration.ty == "str" && declaration.mutable
            && matches!(declaration.initializer.as_ref(),
                Some(Term {kind:TermKind::UnresolvedZeroArgs(name_called),..})
                    if name_called == provider));
    if !valid_receiver {
        return Err(unsupported("P16：呼叫端須以 var str 接收單次函式返回"));
    }
    if !matches!(&final_return.kind, StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind: TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)), start, end,
    } }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：呼叫端其他返回路徑尚未接通執行期"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, final_return)?;
    if let Some(provider_return) = called.body.last() {
        require_checked_bridge_return(&control_edges, &exit_snapshots, provider_return)?;
    }
    require_checked_bridge_owner(&exit_snapshots, final_return,
        DeclarationId(declaration.start), true)?;
    // 已驗證具名來源以真正宣告起點識別返回前後的唯一持有者。
    // 直接 return "字串" 沒有來源宣告，維持原 P16 事件橋接，不偽造 DeclarationId。
    let (content, named_return) = match called.body.as_slice() {
        [Stmt {kind: StmtKind::Return(term), ..}] =>
            (plain_literal(text, term), None),
        [local @ Stmt {kind: StmtKind::Declare(local_declaration), ..},
         Stmt {kind: StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
            kind: TermKind::Move(returned_name), start: return_start, ..
        } }), ..}] if local_declaration.ty == "str"
            && local_declaration.name.as_str() == returned_name.as_str()
            && local_declaration.initializer.is_some() => {
            let value = local_declaration.initializer.as_ref()
                .expect("已預檢具名返回有初始值");
            let from = DeclarationId(local.start);
            let to = DeclarationId(declaration.start);
            let verified = declarations.verified_named_return(from, *return_start, "str")
                .ok_or_else(|| unsupported("P16：缺少 Checker 已驗證具名 str 返回來源證據"))?;
            if !declarations.matches(to, "str", true) {
                return Err(unsupported("P16：呼叫端接收者不是 Checker 已驗證的 str 來源"));
            }
            let proof = NamedResourceReturn::new(verified.source, to)
                .map_err(|_| unsupported("P16：具名返回來源及接收者不可有相同宣告身分"))?;
            (plain_literal(text, value), Some(proof))
        }
        _ => (None, None),
    };
    let content = content.ok_or_else(|| unsupported(
        "P16：被呼叫端僅接受獨立 str 常值或具名 str 的整筆移動返回"))?;
    // 以 Rust 真正 String 的單一持有者驗證作用域交接。
    // 呼叫端根作用域尚無同名來源，接收必須原子成功。
    let mut runtime = OwnedStrRuntime::new();
    runtime.enter_scope();
    runtime.declare("__p16_result", true, content)
        .expect("合法來源應可建立唯一 str 緩衝區");
    let returned = runtime.return_named("__p16_result")
        .expect("剛建立且仍有效的字串可返回");
    runtime.leave_scope()
        .expect("被呼叫端作用域必須能安全退出");
    runtime.accept_return(receiver, returned)
        .expect("呼叫端作用域預先為空且未耗盡資源身分");
    let retained = runtime.read(receiver)
        .expect("返回資源必須仍由呼叫端唯一持有").to_owned();
    let events = runtime.finish()
        .expect("呼叫端退出後每個資源必須恰好釋放一次");
    if let Some(proof) = named_return {
        proof.verify_actual_events(&events)
            .map_err(|_| unsupported("P16：具名返回資源事件與宣告身分及清理責任不一致"))?;
    }
    closed_bridge_result(retained, events)
}

/// VA16-03-A：真正 .voil 受限「建構→var str 欄位賦值」
/// 由同一靜態檢查過的 AST 交由 UTF-8 持續資源模型執行。
/// 其他操作及未核准右值不能被資源橋接假裝成已執行。
#[cfg(test)]
fn trace_persistent_struct_field_write_source(
    text: &str, fail_after_rhs: bool,
) -> Result<(String, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{BuildStatus, NamedString,
        OwnedStructRuntime, StringField, StructRuntimeError};
    use crate::struct_eval_plan::{self, FieldInfo};
    use crate::verified_resource_ir::FieldOverwritePlan;
    let reject=|message:&str| Diagnostic {
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn unescaped_literal<'a>(source:&'a str, term:&Term)->Option<&'a str> {
        if !matches!(&term.kind,TermKind::Literal("str")) { return None; }
        let raw=source.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\')&&!raw.contains('\n')
            &&!raw.contains('\r')).then_some(raw)
    }
    let VerifiedPrivateSource { source: _, structs, functions, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main=functions.iter().find(|function|function.name=="main")
        .ok_or_else(||reject("P16：結構欄位來源缺少 main"))?;
    let [declaration,assignment,last]=main.body.as_slice() else {
        return Err(reject("P16：僅支援一次結構建構、欄位賦值與直接返回"));
    };
    if !matches!(&last.kind,StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind:TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)),start,end,
    } }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：欄位資源清理僅證明直接 return 0"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, last)?;
    let (owner,ty,mutable,args)=match &declaration.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
            Some((ctor, args))
                if &declaration.ty==ctor =>
                (declaration.name.as_str(),declaration.ty.as_str(),declaration.mutable,args),
            _=>return Err(reject("P16：來源須先宣告具名字串結構")),
        },
        _=>return Err(reject("P16：來源須先宣告具名字串結構")),
    };
    let (field,new_value)=match &assignment.kind {
        StmtKind::FieldAssign {receiver,field,value} if receiver==owner=>
            (field.as_str(),unescaped_literal(text,value)
                .ok_or_else(||reject("P16：欄位覆寫只支援未轉義的獨立 str 常值"))?),
        _=>return Err(reject("P16：來源須有針對同一結構的欄位賦值")),
    };
    // 只以 Checker 在真正完成欄位 RHS 型別、借用及可變權限驗證後
    // 提交的來源／寫入位置／欄位證據授權此一資源橋接。
    declarations.verified_field_write(
        DeclarationId(declaration.start), assignment.start, ty, field,
    ).ok_or_else(||reject("P16：缺少 Checker 已驗證欄位寫入身分與位置證據"))?;
    require_checked_bridge_owner(&exit_snapshots, last,
        DeclarationId(declaration.start), true)?;
    let definition=structs.get(ty)
        .ok_or_else(||reject("P16：結構型別不存在"))?;
    let fields=definition.fields.iter().map(|member| {
        if member.ty!="str" {
            return Err(reject("P16：此私有執行期僅接通 str 欄位"));
        }
        let default=member.default.as_ref().map(|term|
            unescaped_literal(text,term).ok_or_else(||
                reject("P16：尚無被核准的字串轉義求值器")))
            .transpose()?;
        Ok(StringField {
            name:member.name.as_str(),default,immutable:member.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let named=args.iter().map(|(field,value)|
        unescaped_literal(text,value).map(|text_value|NamedString {
            field:field.as_str(),value:text_value,
        }).ok_or_else(||reject("P16：具名引數只支援未轉義 str 常值"))
    ).collect::<Result<Vec<_>,Diagnostic>>()?;
    // VB16 正式結構求值計畫是唯一的欄位實際完成順序證據。
    // 未初始化 var 欄位不能假造舊資源，與源程式中欄位宣告順序亦不可混淆。
    let schema: Vec<_>=fields.iter().map(|f|FieldInfo {
        name:f.name,has_default:f.default.is_some(),immutable:f.immutable,
    }).collect();
    let provided: Vec<_>=named.iter().map(|n|n.field).collect();
    let initialized=struct_eval_plan::plan(&schema,&provided)
        .map_err(|_|reject("P16：欄位覆寫不能建立一致的具名求值順序"))?;
    let old_index=initialized.steps.iter().position(|step|step.field==field);
    let proof=FieldOverwritePlan::new(DeclarationId(declaration.start),
        initialized.steps.len(),old_index)
        .map_err(|_|reject("P16：欄位覆寫原值來源及資源身分不完整"))?;
    let mut rt=OwnedStructRuntime::new();
    let ready=rt.construct_with_access(owner,mutable,&fields,&named,None)
        .map_err(|_|reject("P16：真實來源結構無法安全完成建立"))?;
    if !matches!(ready,BuildStatus::Complete|BuildStatus::PartiallyInitialized(_)) {
        return Err(reject("P16：非可執行的結構狀態"));
    }
    let actual=rt.assign_field_checked(owner,field,new_value,fail_after_rhs);
    if fail_after_rhs {
        if !matches!(actual,Err(StructRuntimeError::EvaluationFailed(_))) {
            return Err(reject("P16：右值失敗注入未由資源模型確實發生"));
        }
    } else {
        actual.map_err(|_|reject("P16：合法右值未能安全原子提交"))?;
    }
    let observed=rt.read_field(owner,field)
        .map_err(|_|reject("P16：原持有者欄位沒有可讀資源"))?
        .to_owned();
    let resources=rt.finish()
        .map_err(|_|reject("P16：結構欄位覆寫未完整平衡資源身分"))?;
    proof.verify_actual_events(&resources,fail_after_rhs)
        .map_err(|_|reject("P16：欄位覆寫與已驗證來源的所有權／資源清理順序不一致"))?;
    closed_bridge_result(observed, resources)
}

/// P16.6 混合 int／bool／str 受限來源橋接。
/// 先完整通過現行靜態 Checker，才執行真實純值複製或
/// 含非 Copyable 欄位的整筆資源移動／失敗回滾。
#[cfg(test)]
fn trace_mixed_struct_source(text:&str,fail_after:Option<&str>)
    -> Result<(Option<i64>,Vec<crate::owned_str_runtime::ResourceEvent>),Diagnostic> {
    use crate::mixed_struct_runtime::{
        MixedStructRuntime,MixedFieldSpec,NamedMixed,SeedValue,ScalarKind,Observed,MixedError,
    };
    use crate::struct_eval_plan::{self,FieldInfo};
    use crate::verified_resource_ir::MixedResourcePlan;
    let reject=|message:&str| Diagnostic {
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn literal<'a>(text:&'a str,term:&Term,kind:ScalarKind)->Option<SeedValue<'a>> {
        let raw=text.get(term.start..term.end)?;
        match (kind,&term.kind) {
            (ScalarKind::Str,TermKind::Literal("str"))=>{
                let text=raw.strip_prefix('"')?.strip_suffix('"')?;
                (!text.contains('\\')&&!text.contains('\n')
                    &&!text.contains('\r')).then_some(SeedValue::Str(text))
            },
            (ScalarKind::Bool,TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(_)))=>match raw {
                "true"=>Some(SeedValue::Bool(true)),
                "false"=>Some(SeedValue::Bool(false)),_=>None
            },
            (ScalarKind::Int,TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)))=>{
                if !raw.chars().all(|ch|ch.is_ascii_digit()||ch=='_') {
                    return None;
                }
                raw.replace('_',"").parse().ok().map(SeedValue::Int)
            },
            _=>None,
        }
    }
    let VerifiedPrivateSource { source: _, structs, functions, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main=functions.iter().find(|fun|fun.name=="main")
        .ok_or_else(||reject("P16：缺少 main"))?;
    let [declare,transfer,end]=main.body.as_slice() else {
        return Err(reject("P16：混合結構執行路徑僅支援建構、整筆複製／移動、返回"));
    };
    if !matches!(&end.kind,StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind:TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)),start,end,
    } }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：尚未證明非直接返回的清理順序"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, end)?;
    let (owner,ty,args)=match &declare.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
            Some((ctor, args))
                if &declaration.ty==ctor =>
                (declaration.name.as_str(),declaration.ty.as_str(),args),
            _=>return Err(reject("P16：必須先建構混合結構")),
        },
        _=>return Err(reject("P16：必須先建構混合結構")),
    };
    let (target,moved)=match &transfer.kind {
        StmtKind::Declare(declaration) if declaration.ty==ty =>
            match declaration.initializer.as_ref().map(|term| &term.kind) {
                Some(TermKind::Move(source)) if source==owner =>
                    (declaration.name.as_str(),true),
                Some(TermKind::Name(source)) if source==owner =>
                    (declaration.name.as_str(),false),
                _=>return Err(reject("P16：只支援整體結構來源的已核准複製或 .move()")),
            },
        _=>return Err(reject("P16：第二語句必須建立同型別結構持有者")),
    };
    let source_id = DeclarationId(declare.start);
    let target_id = DeclarationId(transfer.start);
    // 多個純值欄位的複製仍由真正 Checker 的 Copyable 與型別
    // 安全結果決定，不能假造一筆非 Copyable 資源移交事件。
    let source_mutability = match &declare.kind {
        StmtKind::Declare(declaration) => declaration.mutable,
        _ => unreachable!("已驗證具名建構"),
    };
    let target_mutability = match &transfer.kind {
        StmtKind::Declare(declaration) => declaration.mutable,
        _ => unreachable!("已驗證具名接收"),
    };
    if !declarations.matches(source_id, ty, source_mutability)
        || !declarations.matches(target_id, ty, target_mutability) {
        return Err(reject("P16：混合結構來源與目標不是 Checker 已驗證型別及宣告身分"));
    }
    if moved && declarations.verified_direct_move(source_id, target_id, ty).is_none() {
        return Err(reject("P16：混合結構整筆移交缺少 Checker 已驗證來源證據"));
    }
    let definition=structs.get(ty)
        .ok_or_else(||reject("P16：混合型別結構不存在"))?;
    require_checked_bridge_owner(&exit_snapshots, end, target_id,
        definition.fields.iter().any(|field| field.ty == "str"))?;
    let mut fields=Vec::new();
    for field in &definition.fields {
        let kind=match field.ty.as_str() {
            "int"=>ScalarKind::Int,"bool"=>ScalarKind::Bool,"str"=>ScalarKind::Str,
            _=>return Err(reject("P16：本次混合執行期尚未支援其他欄位型別")),
        };
        let default=field.default.as_ref().map(|term|
            literal(text,term,kind).ok_or_else(||
                reject("P16：尚未證明此混合欄位預設運算式可直接執行")))
            .transpose()?;
        fields.push(MixedFieldSpec {
            name:field.name.as_str(),kind,default,immutable:field.immutable,
        });
    }
    let mut named=Vec::new();
    for (name,term) in args {
        let field=fields.iter().find(|field|field.name==name)
            .ok_or_else(||reject("P16：混合欄位名稱未核准"))?;
        let value=literal(text,term,field.kind)
            .ok_or_else(||reject("P16：只接受可完整解碼的混合欄位獨立常值"))?;
        named.push(NamedMixed {field:name.as_str(),value});
    }
    // 只採已通過靜態來源驗證的原始具名／預設求值次序。
    // int／bool 是純值：不得把其求值步驟記成虛構 UTF-8 資源。
    let schema:Vec<_>=fields.iter().map(|field|FieldInfo {
        name:field.name,has_default:field.default.is_some(),
        immutable:field.immutable,
    }).collect();
    let provided:Vec<_>=named.iter().map(|value|value.field).collect();
    let eval_plan=struct_eval_plan::plan(&schema,&provided)
        .map_err(|_|reject("P16：混合結構求值計畫不能建立安全資源次序"))?;
    let step_str=|name:&str|fields.iter().find(|f|f.name==name)
        .is_some_and(|f|f.kind==ScalarKind::Str);
    let total_str=eval_plan.steps.iter()
        .filter(|step|step_str(&step.field)).count();
    let mut before_failure=0usize;
    if let Some(cut)=fail_after {
        for step in &eval_plan.steps {
            if step_str(&step.field) { before_failure+=1; }
            if step.field==cut { break; }
        }
    }
    let proof=MixedResourcePlan::new(
        DeclarationId(declare.start),DeclarationId(transfer.start),total_str)
        .map_err(|_|reject("P16：混合結構宣告來源不可重疊"))?;
    let mut rt=MixedStructRuntime::new();
    match rt.construct(owner,&fields,&named,fail_after) {
        Ok(())=>{},
        Err(MixedError::EvaluationFailed(_))=>{
            let events=rt.finish().map_err(|_|
                reject("P16：混合欄位失敗未能逆序清理資源"))?;
            proof.verify_failed_build(&events,before_failure)
                .map_err(|_|reject("P16：混合失敗點已建立的 str 清理責任與資源 IR 不一致"))?;
            return closed_bridge_result(None, events);
        },
        Err(_)=>return Err(reject("P16：混合結構求值計畫無法安全完成")),
    }
    if moved {
        rt.move_whole(owner,target).map_err(|_|
            reject("P16：混合結構整筆移動未完成"))?;
    } else {
        rt.copy_whole(owner,target).map_err(|_|
            reject("P16：不可從非 Copyable 混合結構隱含複製"))?;
    }
    let number=match rt.field(target,"num") {
        Ok(Observed::Int(value))=>value,
        _=>return Err(reject("P16：本次混合來源須有可觀察的 int num 欄位")),
    };
    let events=rt.finish().map_err(|_|
        reject("P16：混合結構退出後資源身分未平衡"))?;
    proof.verify_completed(&events,moved)
        .map_err(|_|reject("P16：混合純值複製／唯一 str 移交與兩張安全責任表不一致"))?;
    closed_bridge_result(Some(number), events)
}

/// P16.3：真實 .voil 的已知 bool 分支與內外詞彙作用域。
/// 只追蹤完整靜態核准的單層「外層結構；if 的內層結構；
/// 直接或提前 return 0」。失敗點只會出現在實際走到的分支。
#[cfg(test)]
fn trace_scope_struct_source(
    text:&str,fail_inner:Option<&str>,
)->Result<Vec<crate::owned_str_runtime::ResourceEvent>,Diagnostic> {
    use crate::struct_initialization::{BuildStatus,NamedString,
        OwnedStructRuntime,StringField,StructRuntimeError};
    use crate::struct_eval_plan::{self,FieldInfo};
    use crate::verified_resource_ir::ScopedResourcePlan;
    let reject=|message:&str|Diagnostic{
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn plain<'a>(source:&'a str,term:&Term)->Option<&'a str> {
        if !matches!(&term.kind,TermKind::Literal("str")) {return None;}
        let raw=source.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\')&&!raw.contains('\n')
            &&!raw.contains('\r')).then_some(raw)
    }
    let VerifiedPrivateSource { source: _, structs, functions, plan: VerifiedSafetyPlan { declarations, scalar_hir, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main=functions.iter().find(|fun|fun.name=="main")
        .ok_or_else(||reject("P16：區塊清理來源缺少 main"))?;
    let [outside,branch,final_return]=main.body.as_slice() else {
        return Err(reject("P16：區塊橋接僅接受外層宣告、if、返回"));
    };
    let returns_zero=|term:&Term|matches!(&term.kind,
        TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)) if text.get(term.start..term.end)==Some("0"));
    if !matches!(&final_return.kind,StmtKind::Return(term) if returns_zero(term)) {
        return Err(reject("P16：外層必須以直接 return 0 結束"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, final_return)?;
    require_checked_bridge_owner(
        &exit_snapshots, final_return, DeclarationId(outside.start), true)?;
    let (cond,yes,no)=match &branch.kind {
        StmtKind::If{branch,no}=>(&branch.condition,&branch.body,no),
        _=>return Err(reject("P16：須有一個可靜態確定的 if 分支")),
    };
    // 真實分支只允許消費整份 Checker 通過後交付的型別 HIR。
    // 單靠原始 if(true)/if(false) 字串並不足以授權 P16 動態事件。
    if scalar_hir.expression(cond.node_span())
        .is_none_or(|typed| typed.ty != crate::common_ir::ScalarType::Bool) {
        return Err(reject("P16：作用域分支缺少正式 Checker 已驗證 bool HIR"));
    }
    let condition=if matches!(&cond.kind,TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(_))) {
        match text.get(cond.start..cond.end) {
            Some("true")=>true,Some("false")=>false,
            _=>return Err(reject("P16：if bool 來源常值不明")),
        }
    } else {
        return Err(reject("P16：未知條件不可以模擬單一資源路徑"));
    };
    let executed=if condition {Some(yes.as_slice())}
        else {no.as_ref().map(|otherwise| otherwise.as_slice())};
    // 以同一真正已驗證 AST、具名／預設求值計畫計算每個
    // *確實執行* 的作用域所取得 str 數量；未走到的 else 不得建帳。
    let count_planned=|stmt:&Stmt,stop:Option<&str>|->Result<usize,Diagnostic> {
        let (ty,args)=match &stmt.kind {
            StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
                Some((ctor, args))
                    if &declaration.ty==ctor => (declaration.ty.as_str(),args),
                _=>return Err(reject("P16：單層分支僅證明已核准結構建立")),
            },
            _=>return Err(reject("P16：單層分支僅證明已核准結構建立")),
        };
        let def=structs.get(ty)
            .ok_or_else(||reject("P16：作用域建構類別不存在"))?;
        let fields:Vec<_>=def.fields.iter().map(|member|FieldInfo {
            name:member.name.as_str(),has_default:member.default.is_some(),
            immutable:member.immutable,
        }).collect();
        let named:Vec<_>=args.iter().map(|(name,_)|name.as_str()).collect();
        let order=struct_eval_plan::plan(&fields,&named)
            .map_err(|_|reject("P16：作用域資源順序缺乏安全求值計畫"))?;
        let mut count=0;
        for step in &order.steps {
            let field=def.fields.iter().find(|f|f.name==step.field)
                .ok_or_else(||reject("P16：資源步驟缺少對應欄位"))?;
            if field.ty!="str" {
                return Err(reject("P16：作用域橋接限 str 資源欄位"));
            }
            count+=1;
            if stop==Some(step.field.as_str()) { break; }
        }
        Ok(count)
    };
    let outer_count=count_planned(outside,None)?;
    let scoped=executed.and_then(|body|body.first())
        .map(|inner|count_planned(inner,fail_inner)
            .map(|count|(DeclarationId(inner.start),count)))
        .transpose()?;
    let proof=ScopedResourcePlan::new(DeclarationId(outside.start),outer_count,scoped)
        .map_err(|_|reject("P16：分支內外兩個宣告不可以共享資源來源身分"))?;
    let construct=|runtime:&mut OwnedStructRuntime,
        stmt:&Stmt,fail_after:Option<&str>|
        ->Result<Result<(),StructRuntimeError>,Diagnostic> {
        let (name,ty,mutable,args)=match &stmt.kind {
            StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
                Some((ctor, args))
                    if &declaration.ty==ctor =>
                    (declaration.name.as_str(),declaration.ty.as_str(),declaration.mutable,args),
                _=>return Err(reject("P16：區塊橋接僅執行明確結構建立")),
            },
            _=>return Err(reject("P16：區塊橋接僅執行明確結構建立")),
        };
        if !declarations.matches(DeclarationId(stmt.start), ty, mutable) {
            return Err(reject("P16：實際執行分支的結構持有者缺少 Checker 已驗證來源身分或寫入權限"));
        }
        let def=structs.get(ty)
            .ok_or_else(||reject("P16：區塊結構型別未宣告"))?;
        let mut fields=Vec::new();
        for member in &def.fields {
            if member.ty!="str" {
                return Err(reject("P16：此路徑僅支援真實 str 資源欄位"));
            }
            let default=member.default.as_ref().map(|term|
                plain(text,term).ok_or_else(||
                    reject("P16：區塊橋接沒有字串轉義求值器")))
                .transpose()?;
            fields.push(StringField{
                name:member.name.as_str(),default,immutable:member.immutable,
            });
        }
        let named=args.iter().map(|(field,term)|
            plain(text,term).map(|value|NamedString{
                field:field.as_str(),value,
            }).ok_or_else(||reject("P16：區塊具名來源限未轉義 str 常值"))
        ).collect::<Result<Vec<_>,Diagnostic>>()?;
        Ok(runtime.construct_with_access(name,mutable,&fields,&named,fail_after)
            .and_then(|status|match status {
                BuildStatus::Complete=>Ok(()),
                _=>Err(StructRuntimeError::IncompleteStruct),
            }))
    };
    let mut rt=OwnedStructRuntime::new();
    construct(&mut rt,outside,None)?
        .map_err(|_|reject("P16：外層結構初始化失敗"))?;
    if let Some(body)=executed {
        let [inner]=body else {
            // 可選末尾提前 return 0；此情況在分支清理後
            // 須直接退出 main，而不得再次執行已不可達語句。
            if let [inner,Stmt{kind:StmtKind::Return(term),..}]=body {
                if !returns_zero(term) {
                    return Err(reject("P16：不支援其他分支返回運算式"));
                }
                rt.enter_scope();
                match construct(&mut rt,inner,fail_inner)? {
                    Ok(())=>rt.leave_scope().map_err(|_|
                        reject("P16：提前 return 前區塊清理失敗"))?,
                    Err(StructRuntimeError::EvaluationFailed(_))=>
                        rt.leave_scope().map_err(|_|
                            reject("P16：分支失敗回滾區塊清理失敗"))?,
                    Err(_)=>return Err(reject("P16：區塊建構失敗")),
                }
                let events=rt.finish().map_err(|_|
                    reject("P16：提前 return 外層資源未反序清理"))?;
                proof.verify_actual_events(&events).map_err(|_|
                    reject("P16：提前返回分支的內外層資源責任次序不一致"))?;
                return closed_bridge_events(events);
            }
            return Err(reject("P16：分支不能省略未驗證的控制流程"));
        };
        rt.enter_scope();
        match construct(&mut rt,inner,fail_inner)? {
            Ok(())=>{},
            Err(StructRuntimeError::EvaluationFailed(_))=>{},
            Err(_)=>return Err(reject("P16：分支建立錯誤")),
        }
        rt.leave_scope().map_err(|_|
            reject("P16：區塊作用域離開清理失敗"))?;
    } else if fail_inner.is_some() {
        return Err(reject("P16：未執行的 else 分支不能被當成失敗注入"));
    }
    let events=rt.finish().map_err(|_|
        reject("P16：外層退出後資源帳簿未平衡"))?;
    proof.verify_actual_events(&events).map_err(|_|
        reject("P16：可達分支與未執行分支的清理責任不一致"))?;
    closed_bridge_events(events)
}

/// P16：真實 .voil 結構函式返回的私有所有權證據。
/// 限無參數 provider 單次建構與具名整筆 .move() 返回，
/// main 以唯一全新結構綁定接收，避免虛構任意函式執行期。
#[cfg(test)]
fn trace_persistent_struct_return_source(
    text: &str, provider: &str, receiver: &str, fail_after: Option<&str>,
) -> Result<(Option<String>, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{
        BuildStatus, NamedString, OwnedStructRuntime, StringField, StructRuntimeError,
    };
    use crate::verified_resource_ir::WholeStructResourcePlan;
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(&term.kind, TermKind::Literal("str")) { return None; }
        let raw=text.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\') && !raw.contains('\n')
            && !raw.contains('\r')).then_some(raw)
    }
    let VerifiedPrivateSource { source: _, structs, functions, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main=functions.iter().find(|function| function.name=="main")
        .ok_or_else(||unsupported("P16：缺少有效 main"))?;
    let callee=functions.iter().find(|function| function.name==provider
        && function.params.is_empty())
        .ok_or_else(||unsupported("P16：須有無參數資源建構函式"))?;
    let [declaration,return_final]=callee.body.as_slice() else {
        return Err(unsupported("P16：被呼叫函式僅接受一次建立與一次整筆返回"));
    };
    let (local,ty,named_args)=match &declaration.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref().and_then(trace_struct_constructor) {
            Some((actual, args))
                if &declaration.ty==actual && declaration.ty==callee.result_ty =>
                (declaration.name.as_str(),declaration.ty.as_str(),args),
            _=>return Err(unsupported("P16：被呼叫端須先建立具名結構持有者")),
        },
        _=>return Err(unsupported("P16：被呼叫端須先建立具名結構持有者")),
    };
    if !matches!(&return_final.kind,StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind:TermKind::Move(name),..
    } }) if name==local) {
        return Err(unsupported("P16：被呼叫端必須以具名整筆 .move() 返回結構"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, return_final)?;
    let [receive,main_return]=main.body.as_slice() else {
        return Err(unsupported("P16：main 須單次接收結構後直接返回"));
    };
    if !matches!(&receive.kind,StmtKind::Declare(declaration)
        if declaration.name==receiver && declaration.ty==ty
            && matches!(declaration.initializer.as_ref(),
                Some(Term{kind:TermKind::UnresolvedZeroArgs(called),..})
                    if called==provider)) {
        return Err(unsupported("P16：main 須以新結構宣告接收無參數函式結果"));
    }
    if !matches!(&main_return.kind,StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind:TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)),start,end,
    } }) if text.get(*start..*end)==Some("0")) {
        return Err(unsupported("P16：main 的其他返回路徑尚未接通"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, main_return)?;
    let moved = match &return_final.kind {
        StmtKind::Return(term) => term,
        _ => unreachable!("上方已驗證 return .move() 形狀"),
    };
    let target_mutable = match &receive.kind {
        StmtKind::Declare(declaration) => declaration.mutable,
        _ => unreachable!("上方已驗證接收宣告"),
    };
    let from = DeclarationId(declaration.start);
    let to = DeclarationId(receive.start);
    require_checked_bridge_owner(&exit_snapshots, main_return, to, true)?;
    let checked_return = declarations.verified_named_return(from, moved.start, ty)
        .ok_or_else(||unsupported("P16：缺少 Checker 已驗證整筆結構返回證據"))?;
    if !declarations.matches(to, ty, target_mutable) {
        return Err(unsupported("P16：整筆結構接收者不是 Checker 已驗證的來源"));
    }
    let plan=WholeStructResourcePlan::new(checked_return.source, to)
        .map_err(|_|unsupported("P16：跨函式整筆結構返回的來源及目的宣告身分不可重疊"))?;
    let definition=structs.get(ty)
        .ok_or_else(||unsupported("P16：結構類別不存在"))?;
    let fields=definition.fields.iter().map(|field| {
        if field.ty!="str" {
            return Err(unsupported("P16：此返回資源橋接只支援 str 欄位"));
        }
        let default=field.default.as_ref().map(|term|
            plain_literal(text,term).ok_or_else(||unsupported(
                "P16：此返回資源橋接尚未實作轉義字串預設求值")))
            .transpose()?;
        Ok(StringField {
            name:field.name.as_str(),default,immutable:field.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let args=named_args.iter().map(|(name,term)| {
        let value=plain_literal(text,term)
            .ok_or_else(||unsupported("P16：具名引數限未經轉義 str 常值"))?;
        Ok(NamedString{field:name.as_str(),value})
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let observe=fields.last().map(|f|f.name)
        .ok_or_else(||unsupported("P16：來源須至少有一個可觀察的 str 欄位"))?;
    let mut runtime=OwnedStructRuntime::new();
    runtime.enter_scope();
    let status=runtime.construct(local,&fields,&args,fail_after);
    if matches!(&status,Err(StructRuntimeError::EvaluationFailed(_))) {
        runtime.leave_scope().map_err(|_|
            unsupported("P16：被呼叫端失敗清理作用域未關閉"))?;
        let events=runtime.finish().map_err(|_|
            unsupported("P16：失敗構造後所有資源未完整釋放"))?;
        plan.verify_failed_build_events(&events).map_err(|_|
            unsupported("P16：結構返回失敗時已建立欄位未完整逆序回滾"))?;
        return closed_bridge_result(None, events);
    }
    if !matches!(&status,Ok(BuildStatus::Complete)) {
        return Err(unsupported("P16：返回結構未完成初始化或失敗注入切點不合法"));
    }
    let transferred=runtime.return_whole(local)
        .map_err(|_|unsupported("P16：被呼叫端返回不能轉交整筆結構所有權"))?;
    runtime.leave_scope()
        .map_err(|_|unsupported("P16：被呼叫端作用域清理失敗"))?;
    runtime.accept_return(receiver,transferred)
        .map_err(|_|unsupported("P16：呼叫端無法取得返回結構的唯一所有權"))?;
    let value=runtime.read_field(receiver,observe)
        .map_err(|_|unsupported("P16：函式返回後原資源無效"))?.to_owned();
    let events=runtime.finish()
        .map_err(|_|unsupported("P16：呼叫端退出後資源清理身分不完整"))?;
    plan.verify_returned_events(&events).map_err(|_|
        unsupported("P16：跨函式整筆結構返回與資源責任帳簿不一致"))?;
    closed_bridge_result(Some(value), events)
}

/// P16.2 內部來源／資源橋接：只模擬明確的「建立單一 str、
/// 隨即整筆 .move() 至新 var、函式正常返回」這個可證安全路徑。
/// 非一般執行器；先由真實 Lexer／Parser／Checker 保證合法性。
#[cfg(test)]
fn trace_named_string_move_source(
    text: &str,
) -> Result<Vec<crate::owned_str_runtime::ResourceEvent>, Diagnostic> {
    use crate::owned_str_runtime::OwnedStrRuntime;
    use crate::verified_resource_ir::NamedResourceTransfer;
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    let VerifiedPrivateSource { source: _, structs: _, functions: funcs, plan: VerifiedSafetyPlan { declarations, scalar_hir: _, control_edges, exit_snapshots, .. } } =
        verified_private_source(text)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：具名字串移動來源缺少 main"))?;
    let [first, second, trailing_return] = main.body.as_slice() else {
        return Err(unsupported("P16：具名字串移動只接受連續建立、移動、返回"));
    };
    if !matches!(&trailing_return.kind, StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term {
        kind: TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_)), start, end,
    } }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：尚未證明此返回或其他控制流程的資源事件"));
    }
    require_checked_bridge_return(&control_edges, &exit_snapshots, trailing_return)?;
    let (owner, mutable, literal) = match &first.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref() {
            Some(literal) if declaration.ty == "str"
                && matches!(&literal.kind, TermKind::Literal("str")) =>
                (declaration.name.as_str(),declaration.mutable,literal),
            _=>return Err(unsupported("P16：第一個來源必須是獨立 str 常值持有者")),
        },
        _ => return Err(unsupported("P16：第一個來源必須是獨立 str 常值持有者")),
    };
    let (target, from) = match &second.kind {
        StmtKind::Declare(declaration) => match declaration.initializer.as_ref() {
            Some(Term {kind:TermKind::Move(from),..})
                if declaration.ty=="str" && declaration.mutable =>
                (declaration.name.as_str(),from.as_str()),
            _=>return Err(unsupported("P16：第二個來源僅接受 var str 整筆移動")),
        },
        _ => return Err(unsupported("P16：第二個來源僅接受 var str 整筆移動")),
    };
    if from != owner {
        return Err(unsupported("P16：資源轉移來源必須是剛建立的具名持有者"));
    }
    // 只在真正來源 Lexer／Parser／Checker 完整驗證之後建立持有者證據，
    // 以宣告起點而非可遮蔽的字串名稱標定唯一所有者。
    if !declarations.matches(DeclarationId(first.start), "str", mutable)
        || !declarations.matches(DeclarationId(second.start), "str", true) {
        return Err(unsupported("P16：資源持有者未出現在已驗證宣告身分與型別表"));
    }
    let evidence = declarations.verified_direct_move(
        DeclarationId(first.start), DeclarationId(second.start), "str",
    ).ok_or_else(|| unsupported("P16：不存在已驗證來源的直接具名整筆轉移證據"))?;
    require_checked_bridge_owner(
        &exit_snapshots, trailing_return, DeclarationId(second.start), true)?;
    let transfer = NamedResourceTransfer::new(evidence.source, evidence.target)
        .map_err(|_| unsupported("P16：具名移動的來源宣告身分不唯一"))?;
    let raw = text.get(literal.start..literal.end)
        .ok_or_else(|| unsupported("P16：字串常值位置不是合法 UTF-8 邊界"))?;
    let value = raw.strip_prefix('"').and_then(|text| text.strip_suffix('"'))
        .filter(|text| !text.contains('\\') && !text.contains('\n')
            && !text.contains('\r'))
        .ok_or_else(|| unsupported("P16：未實作字串轉義求值，僅可追蹤直接常值"))?;
    let mut runtime = OwnedStrRuntime::new();
    runtime.declare(owner, mutable, value)
        .map_err(|_| unsupported("P16：獨立 UTF-8 資源建立失敗"))?;
    runtime.move_named(owner, target)
        .map_err(|_| unsupported("P16：具名來源未能安全轉交資源"))?;
    let events = runtime.finish()
        .map_err(|_| unsupported("P16：具名移動後資源最終清理責任不完整"))?;
    // P16.6-R03.2：實際建立／移交／逆序清理事件與借用、資源帳簿
    // 都要由同一已驗證來源的 DeclarationId 審核成功後才能回傳。
    transfer.verify_actual_events(&events)
        .map_err(|_| unsupported("P16：字串事件與靜態所有權／資源責任無法一致證明"))?;
    // F12：不能僅在測試外圍額外模擬抽象帳簿。真正跑完 UTF-8
    // OwnedStrRuntime 的已核准直線橋接，還必須以相同 Checker 已驗證
    // DeclarationId 及真實資源事件核對兩張所有權／責任帳簿。
    use crate::owned_str_runtime::ResourceEvent;
    use crate::resource_borrow_pipeline::SafetyPipeline;
    let [ResourceEvent::Created(created), ResourceEvent::Moved(moved),
         ResourceEvent::Released(released)] = events.as_slice() else {
        return Err(unsupported("P16：實際 UTF-8 資源事件序列不完整"));
    };
    if created != moved || created != released {
        return Err(unsupported("P16：資源建立、移交和釋放不是同一實例"));
    }
    let mut pipeline = SafetyPipeline::new();
    pipeline.declare(evidence.source, CopyPermission::Forbidden)
        .map_err(|_| unsupported("P16：來源擁有權責任建立失敗"))?;
    pipeline.register_resource(evidence.source, *created)
        .map_err(|_| unsupported("P16：資源建立責任未能登記"))?;
    declarations.apply_verified_direct_move(
        &mut pipeline, evidence.source, evidence.target, "str")
        .map_err(|_| unsupported("P16：Checker 已驗證移交與資源安全交易不一致"))?;
    if pipeline.record_cleanup(evidence.target)
        .map_err(|_| unsupported("P16：最終資源責任不可安全清理"))? != 1 {
        return Err(unsupported("P16：已移交持有者須恰好清理一個真實字串資源"));
    }
    pipeline.audit_finished()
        .map_err(|_| unsupported("P16：真實 UTF-8 資源清理後仍有未結責任"))?;
    closed_bridge_events(events)
}

