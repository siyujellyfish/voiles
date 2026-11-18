// G18／P18.4-B4／P18.6-M3：僅接通方法直接實參中的
// 有限層原 Checker 純 int helper；不重解析、不開放
// helper 本體遞迴、str 資源或跨 self 借用副作用。

/// B4／M2：原 Checker ScalarHIR 已核准的 main 持有者 int 欄位，
/// 在**真正到達的語句**位置由 P16 MixedStructRuntime 讀出值，
/// 再只對同一原已驗證的運算式工作圖進行值槽投影。
/// 這不是第二份 Lexer/Parser/Checker，也不給任意結構或欄位授權；
/// 不容許 bool／str／Move／Borrow 或未經核准的 async 函式混入。
fn private_self_bind_owner_int_read(
    expr: &PrivateScalarReturnExpr,
    owner_id: DeclarationId,
    owner: &str,
    field: &str,
    observed: i64,
) -> Option<PrivateScalarReturnExpr> {
    match expr {
        PrivateScalarReturnExpr::CopyStructIntField {
            declaration, owner: read_owner, field: read_field,
        } if *declaration == owner_id
            && read_owner == owner && read_field == field =>
            Some(PrivateScalarReturnExpr::Constant(observed)),
        PrivateScalarReturnExpr::Constant(_)
        | PrivateScalarReturnExpr::Parameter(_) => Some(expr.clone()),
        PrivateScalarReturnExpr::Addition(left, right) =>
            Some(PrivateScalarReturnExpr::Addition(
                Box::new(private_self_bind_owner_int_read(
                    left, owner_id, owner, field, observed,
                )?),
                Box::new(private_self_bind_owner_int_read(
                    right, owner_id, owner, field, observed,
                )?),
            )),
        PrivateScalarReturnExpr::Subtraction(left, right) =>
            Some(PrivateScalarReturnExpr::Subtraction(
                Box::new(private_self_bind_owner_int_read(
                    left, owner_id, owner, field, observed,
                )?),
                Box::new(private_self_bind_owner_int_read(
                    right, owner_id, owner, field, observed,
                )?),
            )),
        PrivateScalarReturnExpr::Call {
            name, arguments, awaited: false,
        } => {
            let bound = arguments.iter().map(|argument| {
                let PrivateScalarCallArgument::Int(value) = argument else {
                    return None;
                };
                Some(PrivateScalarCallArgument::Int(
                    private_self_bind_owner_int_read(
                        value, owner_id, owner, field, observed,
                    )?,
                ))
            }).collect::<Option<Vec<_>>>()?;
            Some(PrivateScalarReturnExpr::Call {
                name: name.clone(), arguments: bound, awaited: false,
            })
        }
        _ => None,
    }
}

/// 與原 PrivateSelfApprovedHelper 身分同一來源；僅允許
/// 任意有限 AST 層數的純 int 直接函式實參。
fn private_self_resolve_nested_argument_call<'a>(
    plan: &PrivateScalarReturnExpr,
    helpers: &'a BTreeMap<String, PrivateSelfApprovedHelper>,
) -> Option<&'a PrivateSelfApprovedHelper> {
    let PrivateScalarReturnExpr::Call {
        name, arguments, awaited: false,
    } = plan else { return None; };
    let helper = helpers.get(name)?;
    if helper.name != *name
        || arguments.len() != helper.parameters.len()
        || !arguments.iter().all(|arg| matches!(
            arg,
            PrivateScalarCallArgument::Int(expr) if
                private_self_approved_int_call_arithmetic(expr, helpers)
        )) {
        return None;
    }
    Some(helper)
}

/// prepare 只能計算一致性快照，不能在此產生執行期框架事件。
fn private_self_nested_argument_value(
    expr: &PrivateScalarReturnExpr,
    caller: &BTreeMap<DeclarationId, i64>,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
) -> Result<i64, PrivateScalarCallRuntimeError> {
    // B4／M3：Checker 原先即可能核准「子 helper + int」
    // 作上一層 Call 參數。Snapshot 僅計算原 ScalarHIR 真值，
    // 此時不可假造 Entered、Returned 或 self BorrowTicket。
    if !private_self_approved_int_call_arithmetic(expr, helpers) {
        return Err(PrivateScalarCallRuntimeError::StateMismatch);
    }
    private_self_arithmetic_call_snapshot(expr, caller, helpers)
}

/// 真 Checker 每個不同函式 DeclarationId 須以實參順序
/// 一一綁定，不得將 int 值相等誤判為參數身份相同。
fn private_self_nested_argument_frame(
    helper: &PrivateSelfApprovedHelper,
    arguments: &[PrivateScalarCallArgument],
    caller: &BTreeMap<DeclarationId, i64>,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
) -> Result<BTreeMap<DeclarationId, i64>, PrivateScalarCallRuntimeError> {
    if arguments.len() != helper.parameters.len() {
        return Err(PrivateScalarCallRuntimeError::StateMismatch);
    }
    let mut frame = BTreeMap::new();
    for (id, argument) in helper.parameters.iter().zip(arguments) {
        let PrivateScalarCallArgument::Int(expr) = argument else {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        };
        let value = private_self_nested_argument_value(expr, caller, helpers)?;
        if frame.insert(*id, value).is_some() {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
    }
    Ok(frame)
}

/// Runtime 在原 main frame 順序真正求每個參數，僅當全部
/// 子實參成功後才 Enter 父 helper；每次進退使用同一
/// SharedCallExit 事件帳及原方法 BorrowTicket 取得前的順序。
fn private_self_execute_nested_argument_call(
    plan: &PrivateScalarReturnExpr,
    caller: &BTreeMap<DeclarationId, i64>,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
    events: &mut Vec<PrivateSharedCallExitEvent>,
    entered: &mut usize,
    completed: &mut usize,
) -> Result<i64, PrivateScalarCallRuntimeError> {
    let helper = private_self_resolve_nested_argument_call(plan, helpers)
        .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
    let PrivateScalarReturnExpr::Call { arguments, .. } = plan else {
        return Err(PrivateScalarCallRuntimeError::StateMismatch);
    };
    let mut frame = BTreeMap::new();
    for (id, argument) in helper.parameters.iter().zip(arguments) {
        let PrivateScalarCallArgument::Int(expr) = argument else {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        };
        // 左至右逐一由同一原 ScalarHIR 執行 Call／算術；
        // 子呼叫成功後才計算上層運算，整份參數成功後才進
        // 父 helper 框架；溢位不得偽造父 FunctionEntered。
        let value = private_self_execute_arithmetic_call(
            expr, caller, helpers, events, entered, completed,
        )?;
        if frame.insert(*id, value).is_some() {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
    }
    events.push(PrivateSharedCallExitEvent::FunctionEntered);
    *entered += 1;
    let result = helper.returned.evaluate(&frame);
    if result.is_ok() {
        events.push(PrivateSharedCallExitEvent::Returned);
        events.push(PrivateSharedCallExitEvent::FunctionExited {
            returned: true,
        });
        *completed += 1;
    } else {
        events.push(PrivateSharedCallExitEvent::UnwindStarted);
        events.push(PrivateSharedCallExitEvent::FunctionExited {
            returned: false,
        });
    }
    result
}

/// B4／M3：原 Checker 已允許 BinaryInt 左右各為已核准的
/// Copyable int 普通函式值；不應因來源頂端不是 Call 就
/// 推斷不可執行。只讀原 ScalarHIR 樹與已封存 helper 計畫。
fn private_self_approved_int_call_arithmetic(
    expr: &PrivateScalarReturnExpr,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
) -> bool {
    match expr {
        PrivateScalarReturnExpr::Constant(_)
        | PrivateScalarReturnExpr::Parameter(_) => true,
        PrivateScalarReturnExpr::Addition(left, right)
        | PrivateScalarReturnExpr::Subtraction(left, right) =>
            private_self_approved_int_call_arithmetic(left, helpers)
                && private_self_approved_int_call_arithmetic(right, helpers),
        PrivateScalarReturnExpr::Call { .. } =>
            private_self_resolve_nested_argument_call(expr, helpers).is_some(),
        _ => false,
    }
}

/// 只在 prepare 計算必須與真執行相符的 int 快照，
/// 不產生任何動態 FunctionEntered 或 BorrowTicket。
fn private_self_arithmetic_call_snapshot(
    expr: &PrivateScalarReturnExpr,
    caller: &BTreeMap<DeclarationId, i64>,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
) -> Result<i64, PrivateScalarCallRuntimeError> {
    match expr {
        PrivateScalarReturnExpr::Constant(_)
        | PrivateScalarReturnExpr::Parameter(_) => expr.evaluate(caller),
        PrivateScalarReturnExpr::Addition(left, right) => {
            let lhs = private_self_arithmetic_call_snapshot(left, caller, helpers)?;
            let rhs = private_self_arithmetic_call_snapshot(right, caller, helpers)?;
            lhs.checked_add(rhs).ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)
        }
        PrivateScalarReturnExpr::Subtraction(left, right) => {
            let lhs = private_self_arithmetic_call_snapshot(left, caller, helpers)?;
            let rhs = private_self_arithmetic_call_snapshot(right, caller, helpers)?;
            lhs.checked_sub(rhs).ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)
        }
        PrivateScalarReturnExpr::Call { arguments, .. } => {
            let helper = private_self_resolve_nested_argument_call(expr, helpers)
                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
            let frame = private_self_nested_argument_frame(
                helper, arguments, caller, helpers,
            )?;
            helper.returned.evaluate(&frame)
        }
        _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
    }
}

/// 真動態 expression evaluator 依原 Parser 左到右順序逐個
/// 求值，僅在原 Checker 完整核准的 Call 抵達時使用已
/// 驗證的普通函式工作框架。外層純值算術溢位時絕不
/// 補造不存在的父函式進退或借用票據。
fn private_self_execute_arithmetic_call(
    expr: &PrivateScalarReturnExpr,
    caller: &BTreeMap<DeclarationId, i64>,
    helpers: &BTreeMap<String, PrivateSelfApprovedHelper>,
    events: &mut Vec<PrivateSharedCallExitEvent>,
    entered: &mut usize,
    completed: &mut usize,
) -> Result<i64, PrivateScalarCallRuntimeError> {
    match expr {
        PrivateScalarReturnExpr::Constant(_)
        | PrivateScalarReturnExpr::Parameter(_) => expr.evaluate(caller),
        PrivateScalarReturnExpr::Addition(left, right) => {
            let lhs = private_self_execute_arithmetic_call(
                left, caller, helpers, events, entered, completed,
            )?;
            let rhs = private_self_execute_arithmetic_call(
                right, caller, helpers, events, entered, completed,
            )?;
            lhs.checked_add(rhs).ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)
        }
        PrivateScalarReturnExpr::Subtraction(left, right) => {
            let lhs = private_self_execute_arithmetic_call(
                left, caller, helpers, events, entered, completed,
            )?;
            let rhs = private_self_execute_arithmetic_call(
                right, caller, helpers, events, entered, completed,
            )?;
            lhs.checked_sub(rhs).ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)
        }
        PrivateScalarReturnExpr::Call { .. } =>
            private_self_execute_nested_argument_call(
                expr, caller, helpers, events, entered, completed,
            ),
        _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
    }
}
