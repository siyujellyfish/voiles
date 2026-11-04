//! P17-C：真正私有來源與 Checker 權限接入既有 UTF-8 資源 Runtime。
//! 限制為「var str 常值→另一 var 的 .move()→return int 常值」單一路徑。
//! 其它語法一律在第一次配置資源之前保守阻擋；公開 CLI 不變。

#[derive(Debug)]
pub(crate) enum PrivateStrExecutionError {
    Static(Diagnostic),
    Unsupported(&'static str),
    Runtime(crate::owned_str_runtime::StrRuntimeError),
    Safety(crate::verified_resource_ir::ResourcePlanError),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PrivateStrExecution {
    pub(crate) result: i64,
    pub(crate) actual_events: Vec<crate::owned_str_runtime::ResourceEvent>,
}

struct CheckedSingleMove {
    from: String,
    to: String,
    decoded_utf8: String,
    source: DeclarationId,
    target: DeclarationId,
    return_value: i64,
}

// 此受限模型的「完整語句預檢」先於任何執行副作用；不得靠 Rust
// runtime 的成功結果倒過來授權原始碼的移動與借用權限。
include!("private_resource_execution_preflight.rs");

/// 唯一私有執行入口：真 P13～P16 Checker 先行，再用真實 Runtime
/// 產生 Created／Moved／Released，並由原安全轉移規則逐 ResourceId 核對。
pub(crate) fn execute_checked_single_str_move(text: &str)
    -> Result<PrivateStrExecution, PrivateStrExecutionError> {
    let verified = verify_source_with_ability(
        text, crate::ownership_policy::approved_leaf_copy_permission,
    ).map_err(PrivateStrExecutionError::Static)?;
    let plan = checked_single_move(&verified)?;
    let mut runtime = crate::owned_str_runtime::OwnedStrRuntime::new();
    runtime.declare(&plan.from, true, &plan.decoded_utf8)
        .map_err(PrivateStrExecutionError::Runtime)?;
    runtime.move_named(&plan.from, &plan.to)
        .map_err(PrivateStrExecutionError::Runtime)?;
    if runtime.read(&plan.from)
        != Err(crate::owned_str_runtime::StrRuntimeError::MovedSource)
        || runtime.read(&plan.to).ok() != Some(plan.decoded_utf8.as_str()) {
        return Err(PrivateStrExecutionError::Unsupported("真資源持有者不符"));
    }
    let actual_events = runtime.finish().map_err(PrivateStrExecutionError::Runtime)?;
    let proof = crate::verified_resource_ir::NamedResourceTransfer::new(
        plan.source, plan.target,
    ).map_err(PrivateStrExecutionError::Safety)?;
    proof.verify_actual_events(&actual_events).map_err(PrivateStrExecutionError::Safety)?;
    if !crate::owned_str_runtime::event_trace_balanced(&actual_events) {
        return Err(PrivateStrExecutionError::Unsupported("真資源事件不平衡"));
    }
    Ok(PrivateStrExecution { result: plan.return_value, actual_events })
}

#[cfg(test)]
include!("private_resource_execution_tests.rs");
