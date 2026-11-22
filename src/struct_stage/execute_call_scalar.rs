// G18／P18.1、P18.4：已核准 Copyable int 真普通函式參數子集。
// 原私有 Lexer／Parser／Checker／TypedHIR 是唯一來源權威；
// 本 evaluator 執行原 Checker 的 Copyable int 參數及已驗證巢狀普通函式返回圖，
// 不把靜態 Checker 等同已執行、亦不擴充公開 P01 CLI。
#[derive(Debug)]
enum PrivateScalarCallPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    /// 真執行事件與動態框架責任不一致，保守拒絕對外暴露不可信結果。
    RuntimeTraceMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateScalarCallRuntimeError {
    StateMismatch,
    InjectedFailure,
    IntegerOverflow,
    /// D18-01：是本次呼叫環境預算耗盡，非語言固定遞迴深度。
    CallBudget,
    StepBudget,
    Resource(crate::owned_str_runtime::StrRuntimeError),
    Borrow(crate::borrow_safety::BorrowSafetyError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarCallEvent {
    ArgumentEvaluated(i64),
    /// bool 實參獨立標記，不把 Copyable bool 當成 int 隱含轉型。
    BoolArgumentEvaluated(bool),
    /// V7 E5：直接 await 真到達可啟動安全點後，才由同一
    /// Checker Call 工作圖簽發。下筆必須進入同名 async 函式框架。
    AwaitSafePoint(String),
    MethodAwaitSafePoint(crate::common_ir::NodeSpan),
    FunctionEntered(String),
    ParameterBound(DeclarationId, i64),
    BoolParameterBound(DeclarationId, bool),
    /// 每筆經 Checker 核准的純值區域初始化只在真正寫入框架後記錄。
    LocalInitialized(DeclarationId, i64),
    /// 宣告已達但暫無值槽；不可誤寫成 LocalInitialized。
    LocalDeclaredEmpty(DeclarationId),
    LocalScopeEndedEmpty(DeclarationId),
    BoolDeclaredEmpty(DeclarationId),
    BoolScopeEndedEmpty(DeclarationId),
    /// 只有原 Checker 確認為 var int 的真賦值才留下寫入事件。
    LocalAssigned(DeclarationId, i64),
    /// 原 Checker 已核准的 bool Copyable 專用局部事件（非資源事件）。
    BoolInitialized(DeclarationId, bool),
    BoolAssigned(DeclarationId, bool),
    BoolScopeEnded(DeclarationId),
    /// V1／F4：原 Checker 合法、P16 MixedStructRuntime 真建構
    /// 的純 Copyable int／bool 結構局部；不偽造 UTF-8 ResourceId。
    CopyStructConstructed(DeclarationId),
    /// P16 真 move_whole，原值只轉交新持有者，不複製也不建立資源。
    CopyStructMovedBound { source: DeclarationId, target: DeclarationId },
    CopyStructScopeEnded(DeclarationId),
    /// 原 P16 真 OwnedUtf8 欄位的來源宣告、欄位與 ResourceId
    /// 三重身分；只有實際完成建構後才能發出，不新造資源事件。
    CopyStructResourceOwnerBound { declaration: DeclarationId, field: String, resource_id: u64 },
    /// 原 Checker 已核准的可寫結構 int 欄位，真 P16 持有者更新後才記錄。
    CopyStructIntFieldAssigned(DeclarationId, String, i64),
    /// 唯有 P16 真 bool 欄位更新後才發出的原 Checker 同源事件。
    CopyStructBoolFieldAssigned(DeclarationId, String, bool),
    /// 原 Checker 已核准 var str 真值替換後才可發出的資源身分事件。
    CopyStructStrFieldAssigned(DeclarationId, String, u64),
    /// 只有執行時真正到達的 if 條件才可記錄求值結果。
    ConditionalChecked(bool),
    /// 非資源釋放：只表示 int 區域宣告已離開真動態作用域。
    LocalScopeEnded(DeclarationId),
    /// R05：由真正 OwnedStrRuntime 當下產生的 ResourceId 事件，
    /// 在同一個函式進退序列中保留相對順序，不虛構新資源。
    OwnedResource(crate::owned_str_runtime::ResourceEvent),
    /// 每筆真 str 宣告／移動的目前 DeclarationId 與唯一 ResourceId，
    /// 讓借用票據不能錯綁至另一個仍存活的字串資源。
    ResourceOwnerBound(DeclarationId, u64),
    /// R05：父函式唯一所有權整筆交接給子函式指定 str 參數。
    ResourceTransferred { source: DeclarationId, target: DeclarationId, resource_id: u64 },
    /// Returned 字串直接作為下一呼叫 str 實參：不冒充具名 DeclarationId。
    StrTemporaryTransferred { target: DeclarationId, resource_id: u64 },
    /// 本詞彙框架真配置的字串常值，尚未傳入子函式且無具名持有者。
    StrLiteralTemporaryCreated(u64),
    /// 由真 BorrowSafety 追蹤器簽發與歸還的權限票據。
    SelfRead {
        helpers: Vec<PrivateMethodHelperExit>,
        branches: Vec<bool>,
        owned_arguments: Vec<(Option<DeclarationId>, DeclarationId, u64)>,
        ownership_events: Vec<crate::owned_str_runtime::ResourceEvent>,
        argument_tickets: Vec<crate::borrow_safety::BorrowTicket>,
        site: PrivateScalarSelfRead,
        ticket: crate::borrow_safety::BorrowTicket,
        result: Result<PrivateMethodValue<u64>, PrivateScalarCallRuntimeError>,
        trace: Vec<PrivateSharedCallExitEvent>,
    },
    LoanAcquired(crate::borrow_safety::BorrowTicket),
    /// R05：真取得票據時同步記錄其 UTF-8 資源身分，防止先釋放後歸還。
    LoanResourceLinked(crate::borrow_safety::BorrowTicket, u64),
    LoanReturned(crate::borrow_safety::BorrowTicket),
    /// R05：失敗時正式進入逐框架回滾，不能與一般成功呼叫混淆。
    UnwindStarted,
    BorrowParameterBound(DeclarationId, BorrowFact),
    Returned(i64),
    /// F2：已由原 Checker 確認 bool 的真正返回，不把 0/1 當成 int 返回。
    BoolReturned(bool),
    /// F2：原 Checker str 所有權返回，使用真 P16 ResourceId，並非 i64。
    StrReturned(u64),
    /// 呼叫者經原 DeclarationId 接管同一個 Detached OwnedUtf8。
    StrReturnAccepted(DeclarationId, u64),
    FunctionExited { name: String, returned: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarCallOutcome {
    result: Result<i64, PrivateScalarCallRuntimeError>,
    events: Vec<PrivateScalarCallEvent>,
    /// 真 OwnedStrRuntime 的唯一 ResourceId 事件，獨立於 Copyable 離域。
    resource_events: Vec<crate::owned_str_runtime::ResourceEvent>,
    /// 任何成功、失敗及重複執行都不得保留被呼叫者參數。
    parameter_slots_after_exit: usize,
}

impl PrivateScalarCallOutcome {
    /// R05：完全由已真正發出的事件驗證每個動態框架的進／退、
    /// 已完成 Copyable 局部宣告之逆序離域及單次返回。
    /// 這是 int／bool 純值 trace，**不冒充 OwnedStr ResourceEvent**。
    fn has_balanced_frame_trace(&self) -> bool {
        // 零呼叫框架／零步數預算在任何執行開始前就拒絕：
        // 既有驗收要求完全沒有動態事件，不可虛構 UnwindStarted。
        // 僅在無資源、無參數、零事件且為預算錯誤時承認此預檢。
        if self.events.is_empty() {
            return self.resource_events.is_empty()
                && self.parameter_slots_after_exit == 0
                && matches!(self.result, Err(
                    PrivateScalarCallRuntimeError::CallBudget
                    | PrivateScalarCallRuntimeError::StepBudget
                ));
        }
        struct TraceFrame {
            name: Option<String>,
            /// 每次真正進入函式時，原 Checker 的參數宣告身分只能綁定一次；
            /// int／bool／str 借用及 str Move 共用此欄，遞迴各幀獨立。
            bound_parameters: std::collections::BTreeSet<DeclarationId>,
            locals: Vec<PrivateScalarLocalBinding>,
            moved_copy_struct_sources: std::collections::BTreeSet<DeclarationId>,
            resource_ids: std::collections::HashSet<u64>,
            owner_resources: BTreeMap<DeclarationId, u64>,
            /// 結構的各個原 Checker str 欄位各持有唯一真資源，
            /// 不把同一結構的多欄位塞進單純 DeclarationId 帳。
            struct_owner_resources: BTreeMap<(DeclarationId, String), u64>,
            /// 曾完成建構的欄位身分，即使舊 ResourceId 已釋放，
            /// 重新綁定仍須對應下一筆唯一真替換確認。
            observed_struct_fields: std::collections::BTreeSet<(DeclarationId, String)>,
            /// 依 P16 來源欄位實際建構順序記錄，不以欄位名稱字典序排序。
            struct_field_order: Vec<(DeclarationId, String)>,
            pending_str_field_write: Option<(DeclarationId, String, u64)>,
            /// 共帳整筆結構移動：依 P16 欄位定義次序逐一對帳，
            /// 不接受兩筆都有效但顛倒先後的 Moved(ResourceId)。
            pending_struct_moves: Vec<u64>,
            /// 先完成跨框架持有者交接，下一個真 Moved 事件才算提交。
            pending_transfers: BTreeMap<u64, (DeclarationId, bool)>,
            /// 真 OwnedUtf8 已離開來源所有者，但尚未正式交付呼叫者。
            str_returning: Option<u64>,
            pending_str_results: Vec<u64>,
            loans: Vec<crate::borrow_safety::BorrowTicket>,
            /// 每張活躍票據與真 ResourceId 一一對應，不以名稱推算持有權。
            loan_resources: Vec<(crate::borrow_safety::BorrowTicket, u64)>,
            /// 借用參數須在本次函式退出前從直接呼叫者歸還。
            imported_loans: Vec<crate::borrow_safety::BorrowTicket>,
            returned: bool,
        }
        let mut frames = vec![TraceFrame {
            name: None, bound_parameters: std::collections::BTreeSet::new(),
            locals: Vec::new(),
            moved_copy_struct_sources: std::collections::BTreeSet::new(),
            resource_ids: std::collections::HashSet::new(),
            owner_resources: BTreeMap::new(),
            struct_owner_resources: BTreeMap::new(),
            observed_struct_fields: std::collections::BTreeSet::new(),
            struct_field_order: Vec::new(),
            pending_str_field_write: None,
            pending_struct_moves: Vec::new(),
            pending_transfers: BTreeMap::new(),
            str_returning: None, pending_str_results: Vec::new(),
            loans: Vec::new(), loan_resources: Vec::new(),
            imported_loans: Vec::new(), returned: false,
        }];
        let mut recorded = Vec::new();
        let mut unwinding = false;
        // 一筆已證直接等待安全點須立刻接入其目標函式，
        // 不能重播安全點、插入其它事件或改綁另一個框架。
        let mut awaited_entry: Option<&str> = None;
        let mut awaited_method = None;
        let mut created_ids = std::collections::HashSet::<u64>::new();
        // 僅實際成功 Returned 的子框架，才可以在正式回滾之前
        // 歸還其直接呼叫者的票據；尚未入框架的失敗實參須先回滾。
        let mut returned_imported_loans =
            Vec::<crate::borrow_safety::BorrowTicket>::new();
        for (index, event) in self.events.iter().enumerate() {
            if let Some(span) = awaited_method.take() {
                if !matches!(event, PrivateScalarCallEvent::SelfRead { site, .. } if site.awaited && site.call_span == span) { return false; }
            } else if matches!(event, PrivateScalarCallEvent::SelfRead { site, .. } if site.awaited) { return false; }
            if let Some(target) = awaited_entry {
                if !matches!(event,
                    PrivateScalarCallEvent::FunctionEntered(name)
                        if name.as_str() == target) {
                    return false;
                }
            }
            // P16 整筆結構已真正交付目標後，必須立即收到真共帳
            // 對每一個 str 欄位發出的 Moved(id)，不得穿插其他事件。
            if frames.last().is_some_and(|frame|
                !frame.pending_struct_moves.is_empty()
                    && !matches!(event,
                        PrivateScalarCallEvent::OwnedResource(
                            crate::owned_str_runtime::ResourceEvent::Moved(id)
                        ) if frame.pending_struct_moves.first() == Some(id))
            ) { return false; }
            // 一旦真執行期失敗並開始回滾，後續只能歸還借用、
            // 撤除純值局部、釋放 OwnedStr 與失敗函式離域；
            // 新的求值／綁定／配置／轉移均不應在失敗後復活。
            if unwinding && !matches!(event,
                PrivateScalarCallEvent::LoanReturned(_)
                    | PrivateScalarCallEvent::LocalScopeEnded(_)
                    | PrivateScalarCallEvent::BoolScopeEnded(_)
                    | PrivateScalarCallEvent::CopyStructScopeEnded(_)
                    | PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
                    | PrivateScalarCallEvent::BoolScopeEndedEmpty(_)
                    | PrivateScalarCallEvent::OwnedResource(
                        crate::owned_str_runtime::ResourceEvent::Released(_)
                    )
                    | PrivateScalarCallEvent::FunctionExited {
                        returned: false, ..
                    }
            ) {
                return false;
            }
            if frames.last().is_some_and(|frame|
                frame.pending_str_field_write.is_some())
                && !matches!(event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)) {
                // 新資源身分剛完成覆寫綁定時，下一個事件必須是
                // 同一欄位、同一 ResourceId 的唯一寫入確認。
                return false;
            }
            match event {
                PrivateScalarCallEvent::MethodAwaitSafePoint(span) => {
                    if unwinding || frames.last().is_none_or(|f| f.returned)
                        || frames.iter().any(|f| !f.loans.is_empty() || !f.imported_loans.is_empty()) { return false; }
                    awaited_method = Some(*span);
                }
                PrivateScalarCallEvent::AwaitSafePoint(name) => {
                    let Some(frame) = frames.last() else { return false; };
                    if unwinding || frame.returned
                        || !frame.loans.is_empty() || !frame.imported_loans.is_empty()
                        || frame.pending_str_field_write.is_some()
                        || !frame.pending_struct_moves.is_empty() {
                        return false;
                    }
                    awaited_entry = Some(name);
                }
                PrivateScalarCallEvent::FunctionEntered(name) => {
                    awaited_entry = None;
                    if unwinding || frames.last().is_none_or(|f| f.returned) { return false; }
                    frames.push(TraceFrame {
                        name: Some(name.clone()),
                        bound_parameters: std::collections::BTreeSet::new(),
                        locals: Vec::new(),
                        moved_copy_struct_sources: std::collections::BTreeSet::new(),
                        resource_ids: std::collections::HashSet::new(),
            owner_resources: BTreeMap::new(),
            struct_owner_resources: BTreeMap::new(),
            observed_struct_fields: std::collections::BTreeSet::new(),
            struct_field_order: Vec::new(),
            pending_str_field_write: None,
            pending_struct_moves: Vec::new(),
            pending_transfers: BTreeMap::new(),
            str_returning: None, pending_str_results: Vec::new(),
            loans: Vec::new(), loan_resources: Vec::new(),
            imported_loans: Vec::new(), returned: false,
                    });
                }
                PrivateScalarCallEvent::ParameterBound(id, _)
                    | PrivateScalarCallEvent::BoolParameterBound(id, _) => {
                    // 兩種 Copyable 參數共用同一原 Checker DeclarationId 空間。
                    // 重播相同參數、冒充另一型別，即使框架數與資源帳平衡也拒絕。
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.name.is_none() || frame.returned
                        || !frame.bound_parameters.insert(*id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::ResourceTransferred {
                    source, target, resource_id,
                } => {
                    if frames.len() <= 1 || frames.iter().any(|frame|
                        frame.loan_resources.iter().any(|(_, id)| id == resource_id)) {
                        return false;
                    }
                    let last = frames.len() - 1;
                    let (parents, child) = frames.split_at_mut(last);
                    let parent = &mut parents[last - 1];
                    let child = &mut child[0];
                    if parent.returned || child.returned
                        || parent.owner_resources.get(source) != Some(resource_id)
                        || child.resource_ids.contains(resource_id)
                        || child.pending_transfers.contains_key(resource_id)
                        || child.owner_resources.contains_key(target) {
                        return false;
                    }
                    parent.owner_resources.remove(source);
                    if !parent.resource_ids.remove(resource_id)
                        || !child.resource_ids.insert(*resource_id)
                        || child.pending_transfers.insert(
                            *resource_id, (*target, false),
                        ).is_some() { return false; }
                }
                PrivateScalarCallEvent::StrLiteralTemporaryCreated(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned
                        || !matches!(self.events.get(index.checked_sub(1).unwrap_or(usize::MAX)),
                            Some(PrivateScalarCallEvent::OwnedResource(
                                crate::owned_str_runtime::ResourceEvent::Created(created)
                            )) if created == id)
                        || !frame.resource_ids.contains(id)
                        || frame.pending_str_results.contains(id)
                        || frame.str_returning == Some(*id)
                        || frame.owner_resources.values().any(|owned| owned == id)
                        || frame.struct_owner_resources.values().any(|owned| owned == id) {
                        return false;
                    }
                    frame.pending_str_results.push(*id);
                }
                PrivateScalarCallEvent::StrTemporaryTransferred {
                    target, resource_id,
                } => {
                    if frames.len() <= 1 || frames.iter().any(|frame|
                        frame.loan_resources.iter().any(|(_, id)| id == resource_id)) {
                        return false;
                    }
                    let last = frames.len() - 1;
                    let (parents, child) = frames.split_at_mut(last);
                    let parent = &mut parents[last - 1];
                    let child = &mut child[0];
                    if parent.returned || child.returned
                        || !parent.pending_str_results.contains(resource_id)
                        || !parent.resource_ids.contains(resource_id)
                        || parent.owner_resources.values().any(|id| id == resource_id)
                        || child.resource_ids.contains(resource_id)
                        || child.pending_transfers.contains_key(resource_id)
                        || child.owner_resources.contains_key(target) {
                        return false;
                    }
                    let Some(index) = parent.pending_str_results.iter()
                        .position(|id| id == resource_id) else { return false; };
                    parent.pending_str_results.remove(index);
                    if !parent.resource_ids.remove(resource_id)
                        || !child.resource_ids.insert(*resource_id)
                        || child.pending_transfers.insert(
                            *resource_id, (*target, false),
                        ).is_some() { return false; }
                }
                PrivateScalarCallEvent::BorrowParameterBound(id, fact) => {
                    if frames.len() <= 1 { return false; }
                    let last = frames.len() - 1;
                    let (older, current) = frames.split_at_mut(last);
                    let parent = &older[last - 1];
                    let child = &mut current[0];
                    if child.name.is_none() || child.returned { return false; }
                    let Some(ticket) = parent.loans.iter().copied()
                        .find(|loan| loan.fact() == *fact
                            && !child.imported_loans.contains(loan)) else {
                        return false;
                    };
                    if !child.bound_parameters.insert(*id) { return false; }
                    child.imported_loans.push(ticket);
                }
                PrivateScalarCallEvent::LocalDeclaredEmpty(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry| matches!(entry,
                        PrivateScalarLocalBinding::Int(old)
                            | PrivateScalarLocalBinding::UninitializedInt(old)
                            if old == id
                    )) { return false; }
                    frame.locals.push(PrivateScalarLocalBinding::UninitializedInt(*id));
                }
                PrivateScalarCallEvent::BoolDeclaredEmpty(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry| matches!(entry,
                        PrivateScalarLocalBinding::Bool(old)
                            | PrivateScalarLocalBinding::UninitializedBool(old)
                            if old == id
                    )) { return false; }
                    frame.locals.push(PrivateScalarLocalBinding::UninitializedBool(*id));
                }
                PrivateScalarCallEvent::LocalInitialized(id, _) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::Int(old) if old == id)
                    ) { return false; }
                    if let Some(pending) = frame.locals.iter_mut().find(|entry|
                        matches!(entry, PrivateScalarLocalBinding::UninitializedInt(old)
                            if old == id)
                    ) {
                        *pending = PrivateScalarLocalBinding::Int(*id);
                    } else {
                        frame.locals.push(PrivateScalarLocalBinding::Int(*id));
                    }
                }
                PrivateScalarCallEvent::BoolInitialized(id, _) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::Bool(old) if old == id)
                    ) { return false; }
                    if let Some(pending) = frame.locals.iter_mut().find(|entry|
                        matches!(entry, PrivateScalarLocalBinding::UninitializedBool(old)
                            if old == id)
                    ) {
                        *pending = PrivateScalarLocalBinding::Bool(*id);
                    } else {
                        frame.locals.push(PrivateScalarLocalBinding::Bool(*id));
                    }
                }
                PrivateScalarCallEvent::CopyStructConstructed(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::CopyStruct(old) if old == id)
                    ) { return false; }
                    frame.locals.push(PrivateScalarLocalBinding::CopyStruct(*id));
                }
                PrivateScalarCallEvent::CopyStructMovedBound { source, target } => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || source == target
                        || !frame.locals.iter().any(|entry| matches!(
                            entry, PrivateScalarLocalBinding::CopyStruct(id) if id == source
                        ))
                        || frame.locals.iter().any(|entry| matches!(
                            entry, PrivateScalarLocalBinding::CopyStruct(id) if id == target
                        ))
                        || !frame.moved_copy_struct_sources.insert(*source) {
                        return false;
                    }
                    // 不新建資源：原 Checker DeclarationId 的各 str
                    // 欄位身分全部原子轉交給新 struct 綁定，並沿用
                    // #686 的 observed_struct_fields 事件防重播清冊。
                    let transferred = frame.struct_field_order.iter()
                        .filter(|(owner, _)| owner == source)
                        .map(|(_, field)| (field.clone(),
                            frame.struct_owner_resources
                                .get(&(*source, field.clone())).copied()))
                        .collect::<Vec<_>>();
                    if transferred.iter().any(|(field, maybe_id)| {
                        let Some(id) = maybe_id else { return true; };
                        !frame.resource_ids.contains(id)
                            || frame.pending_struct_moves.contains(id)
                            || frame.struct_owner_resources
                                .contains_key(&(*target, field.clone()))
                            || !frame.observed_struct_fields
                                .contains(&(*source, field.clone()))
                            || frame.observed_struct_fields
                                .contains(&(*target, field.clone()))
                    }) {
                        return false;
                    }
                    for (field, maybe_id) in transferred {
                        let Some(id) = maybe_id else { return false; };
                        let from = (*source, field.clone());
                        let to = (*target, field);
                        if frame.struct_owner_resources.remove(&from) != Some(id)
                            || frame.struct_owner_resources.insert(to.clone(), id).is_some()
                            || !frame.observed_struct_fields.remove(&from)
                            || !frame.observed_struct_fields.insert(to) {
                            return false;
                        }
                        frame.pending_struct_moves.push(id);
                    }
                    for (owner, _) in &mut frame.struct_field_order {
                        if *owner == *source { *owner = *target; }
                    }
                    frame.locals.push(PrivateScalarLocalBinding::CopyStruct(*target));
                }
                PrivateScalarCallEvent::CopyStructResourceOwnerBound {
                    declaration, field, resource_id,
                } => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || field.is_empty()
                        || !frame.resource_ids.contains(resource_id)
                        || !frame.locals.iter().any(|entry| matches!(
                            entry, PrivateScalarLocalBinding::CopyStruct(id) if id == declaration
                        ))
                        || frame.moved_copy_struct_sources.contains(declaration)
                        || frame.owner_resources.values().any(|id| id == resource_id)
                        || frame.struct_owner_resources.values().any(|id| id == resource_id)
                        || frame.struct_owner_resources.contains_key(&(*declaration, field.clone())) {
                        return false;
                    }
                    let key = (*declaration, field.clone());
                    if frame.observed_struct_fields.contains(&key) {
                        // 原欄位先前曾持有真 UTF-8：再綁定只能代表
                        // 一次已驗證的原子替換，下一事件須唯一確認。
                        if frame.pending_str_field_write.is_some() { return false; }
                        frame.pending_str_field_write =
                            Some((*declaration, field.clone(), *resource_id));
                    } else {
                        frame.observed_struct_fields.insert(key.clone());
                        frame.struct_field_order.push(key.clone());
                    }
                    frame.struct_owner_resources.insert(key, *resource_id);
                }
                PrivateScalarCallEvent::CopyStructScopeEnded(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::CopyStruct(previous))
                            if previous == *id) { return false; }
                    if frame.pending_str_field_write.is_some() { return false; }
                    frame.moved_copy_struct_sources.remove(id);
                    // 同一 DeclarationId 可在真 for/while 下一輪再建構，
                    // 不可用上輪舊欄位的歷史誤判成一次覆寫。
                    frame.observed_struct_fields.retain(|(owner, _)| owner != id);
                    frame.struct_field_order.retain(|(owner, _)| owner != id);
                }
                PrivateScalarCallEvent::CopyStructIntFieldAssigned(id, field, _)
                    | PrivateScalarCallEvent::CopyStructBoolFieldAssigned(id, field, _) => {
                    let Some(frame) = frames.last() else { return false; };
                    if field.is_empty() || frame.returned
                        || frame.moved_copy_struct_sources.contains(id)
                        || !frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::CopyStruct(old) if old == id)
                    ) { return false; }
                }
                PrivateScalarCallEvent::CopyStructStrFieldAssigned(id, field, resource_id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.moved_copy_struct_sources.contains(id)
                        || frame.struct_owner_resources.get(&(*id, field.clone()))
                            != Some(resource_id)
                        || frame.pending_str_field_write.as_ref()
                            != Some(&(*id, field.clone(), *resource_id)) {
                        return false;
                    }
                    // 已交付唯一真相符的寫入確認；重播同一事件必拒。
                    frame.pending_str_field_write = None;
                }
                PrivateScalarCallEvent::LocalAssigned(id, _) => {
                    if frames.last().is_none_or(|frame| frame.returned
                        || !frame.locals.iter().any(|entry|
                            matches!(entry, PrivateScalarLocalBinding::Int(old) if old == id)
                        )) { return false; }
                }
                PrivateScalarCallEvent::BoolAssigned(id, _) => {
                    if frames.last().is_none_or(|frame| frame.returned
                        || !frame.locals.iter().any(|entry|
                            matches!(entry, PrivateScalarLocalBinding::Bool(old) if old == id)
                        )) { return false; }
                }
                PrivateScalarCallEvent::LocalScopeEnded(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::Int(previous)) if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::BoolScopeEnded(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::Bool(previous)) if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::LocalScopeEndedEmpty(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::UninitializedInt(previous))
                            if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::BoolScopeEndedEmpty(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::UninitializedBool(previous))
                            if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::SelfRead { site, ticket, trace, result, argument_tickets, owned_arguments, ownership_events, helpers, branches } => {
                    if result.is_err() && (result.as_ref().err() != self.result.as_ref().err() || !matches!(self.events.get(index + 1 + argument_tickets.len()),
                        Some(PrivateScalarCallEvent::UnwindStarted))) { return false; }
                    let Some(frame) = frames.last_mut() else { return false; };
                    let moves = site.arguments.iter().filter_map(|(param, arg)| match arg {
                        PrivateScalarCallArgument::MoveStr { source, .. } => Some((Some(*source), *param)),
                        PrivateScalarCallArgument::LiteralStr(_) => Some((None, *param)), _ => None,
                    });
                    if moves.ne(owned_arguments.iter().map(|(s,p,_)| (*s,*p))) { return false; }
                    use crate::owned_str_runtime::ResourceEvent;
                    let mut created = None;
                    let mut helper_return = None;
                    let returned = match result {
                        Ok(PrivateMethodValue::Str(id)) if site.result_type == "str" => {
                            let Some(steps) = site.selected_steps(branches, true) else { return false; };
                            match steps.last() {
                                Some(PrivateScalarMethodStep::ReturnStr(PrivateScalarStrReturn::Move { declaration, .. }))
                                    if owned_arguments.iter().any(|(_, param, value)| param == declaration && value == id) => {}
                                Some(PrivateScalarMethodStep::ReturnStrCall(_))
                                    if helpers.last().is_some_and(|h| h.result == Ok(PrivateMethodValue::Str(*id)))
                                        && created_ids.insert(*id) && frame.resource_ids.insert(*id) => {
                                    created = Some(*id);
                                    helper_return = Some(*id);
                                }
                                Some(PrivateScalarMethodStep::ReturnStr(PrivateScalarStrReturn::Literal(_)))
                                    if created_ids.insert(*id) && frame.resource_ids.insert(*id) => created = Some(*id),
                                _ => return false,
                            }
                            Some(*id)
                        }
                        Ok(PrivateMethodValue::Int(_)) if site.result_type == "int" => None,
                        Ok(PrivateMethodValue::Bool(_)) if site.result_type == "bool" => None,
                        Err(_) => None,
                        _ => return false,
                    };
                    let order = owned_arguments.iter().map(|(_,_,id)| ResourceEvent::Moved(*id))
                        .chain(created.map(ResourceEvent::Created))
                        .chain(helper_return.map(ResourceEvent::Returned))
                        .chain(returned.map(ResourceEvent::Returned))
                        .chain(owned_arguments.iter().rev().filter(|(_,_,id)| Some(*id) != returned)
                            .map(|(_,_,id)| ResourceEvent::Released(*id)));
                    if order.ne(ownership_events.iter().cloned()) { return false; }
                    let temps: Vec<_> = owned_arguments.iter().filter_map(|(s,_,id)| s.is_none().then_some(*id)).collect();
                    if !frame.pending_str_results.ends_with(&temps) { return false; }
                    frame.pending_str_results.truncate(frame.pending_str_results.len() - temps.len());
                    for (source, _, id) in owned_arguments {
                        if source.is_some_and(|s| frame.owner_resources.remove(&s) != Some(*id)) || !frame.resource_ids.contains(id)
                            || (Some(*id) != returned && !frame.resource_ids.remove(id))
                            || frame.loan_resources.iter().any(|(_, linked)| linked == id) { return false; }
                    }
                    if let Some(id) = returned { frame.pending_str_results.push(id); }
                    recorded.extend_from_slice(ownership_events);
                    let expected: Vec<_> = site.arguments.iter().filter_map(|(_, arg)| match arg {
                        PrivateScalarCallArgument::BorrowStr { fact, .. } => Some(*fact), _ => None,
                    }).collect();
                    if !frame.loans.ends_with(argument_tickets)
                        || argument_tickets.iter().map(|ticket| ticket.fact()).ne(expected) { return false; }
                    {
                        for (offset, arg) in argument_tickets.iter().rev().enumerate() {
                            if !matches!(self.events.get(index + 1 + offset),
                                Some(PrivateScalarCallEvent::LoanReturned(actual)) if actual == arg) { return false; }
                        }
                        returned_imported_loans.extend(argument_tickets.iter().copied());
                    }
                    if unwinding || frame.returned || ticket.fact() != site.loan
                        || !frame.locals.iter().any(|slot| matches!(slot,
                            PrivateScalarLocalBinding::CopyStruct(id) if *id == site.loan.source))
                        || helpers.last().is_some_and(|h| h.result.is_err() && h.result.as_ref().err() != result.as_ref().err())
                        || !site.matches_exit_trace(trace, ticket, argument_tickets, helpers, branches, result.is_ok())
                        || trace.last() != Some(&PrivateSharedCallExitEvent::FunctionExited { returned: result.is_ok() }) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::LoanAcquired(ticket) => {
                    let duplicate = frames.iter().any(|frame| frame.loans.contains(ticket));
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || duplicate { return false; }
                    frame.loans.push(*ticket);
                }
                PrivateScalarCallEvent::LoanResourceLinked(ticket, resource_id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.owner_resources.get(&ticket.fact().source) != Some(resource_id)
                        || frame.returned || !frame.loans.contains(ticket)
                        || frame.loan_resources.iter().any(|(loan, _)| loan == ticket) {
                        return false;
                    }
                    frame.loan_resources.push((*ticket, *resource_id));
                }
                PrivateScalarCallEvent::LoanReturned(ticket) => {
                    // 已成功 Returned 的子函式才可正常還票；若實參
                    // 尚未能進入子框架便失敗，必須先 UnwindStarted。
                    // 不能靠當前棧頂沒有 imported_loans 就繞過此限制。
                    if !unwinding && !returned_imported_loans.contains(ticket) {
                        return false;
                    }
                    returned_imported_loans.retain(|loan| loan != ticket);
                    let Some(frame) = frames.iter_mut().rev()
                        .find(|frame| frame.loans.contains(ticket)) else { return false; };
                    // R05：同一呼叫者取得多張真票據時，既有
                    // OwnedStr／BorrowSafety 退出工作堆疊一律逆序歸還。
                    // 先前只要最後數量清零就接受任意還票順序，可能把
                    // 篡改的多參數／失敗回滾軌跡誤判為可信。
                    // 每張票據必須同時是目前 tracker 及資源連結帳
                    // 的最近一張；不更動語言借用權限或真正歸還流程。
                    if frame.loans.last() != Some(ticket)
                        || frame.loan_resources.last()
                            .is_none_or(|(linked, _)| linked != ticket) {
                        return false;
                    }
                    frame.loans.pop();
                    frame.loan_resources.pop();
                }
                PrivateScalarCallEvent::UnwindStarted => {
                    if unwinding || self.result.is_ok() { return false; }
                    unwinding = true;
                }
                PrivateScalarCallEvent::OwnedResource(resource) => {
                    // 票據未歸還前，所指向的實際 UTF-8 資源不得被移動、
                    // 交還或釋放；獨立平衡的資源帳簿不能取代此交叉因果。
                    let conflicts = match resource {
                        crate::owned_str_runtime::ResourceEvent::Moved(id)
                        | crate::owned_str_runtime::ResourceEvent::Returned(id)
                        | crate::owned_str_runtime::ResourceEvent::Released(id) =>
                            frames.iter().any(|frame| frame.loan_resources.iter()
                                .any(|(_, linked)| linked == id)),
                        crate::owned_str_runtime::ResourceEvent::Created(_) => false,
                    };
                    if conflicts { return false; }
                    let Some(frame) = frames.last_mut() else { return false; };
                    let valid = match resource {
                        crate::owned_str_runtime::ResourceEvent::Created(id) =>
                            !frame.returned && created_ids.insert(*id)
                                && frame.resource_ids.insert(*id),
                        crate::owned_str_runtime::ResourceEvent::Moved(id) => {
                            if !frame.returned && frame.resource_ids.contains(id) {
                                if matches!(resource,
                                    crate::owned_str_runtime::ResourceEvent::Moved(_))
                                    && frame.pending_struct_moves.first() == Some(id) {
                                    // 同一結構多欄位 Move 必須遵守 P16 真欄位順序。
                                    frame.pending_struct_moves.remove(0);
                                    true
                                } else if let Some((_, observed)) =
                                    frame.pending_transfers.get_mut(id) {
                                    if *observed { false } else { *observed = true; true }
                                } else {
                                    // 同框架普通字串移動：撤銷原 DeclarationId。
                                    let former = frame.owner_resources.iter().find_map(|(owner, held)|
                                        (held == id).then_some(*owner));
                                    former.is_some_and(|owner|
                                        frame.owner_resources.remove(&owner) == Some(*id))
                                }
                            } else { false }
                        }
                        crate::owned_str_runtime::ResourceEvent::Returned(id) => {
                            if frame.returned || frame.str_returning.is_some()
                                || !frame.resource_ids.contains(id)
                                || frame.struct_owner_resources.values().any(|v| v == id) {
                                false
                            } else {
                                // 第二次 Returned 只能是真子函式 ExitCall
                                // 留在本幀的待接收 ID；絕不能把尚未轉交的
                                // 具名持有者及匿名暫存同時視為該 ID 的持有者。
                                if let Some(index) = frame.pending_str_results.iter()
                                    .position(|value| value == id) {
                                    if frame.owner_resources.values().any(|held| held == id) {
                                        return false;
                                    }
                                    frame.pending_str_results.remove(index);
                                } else {
                                    let owner = frame.owner_resources.iter()
                                        .find_map(|(owner, value)| (value == id).then_some(*owner));
                                    if let Some(owner) = owner {
                                        frame.owner_resources.remove(&owner);
                                    }
                                }
                                frame.str_returning = Some(*id);
                                true
                            }
                        }
                        crate::owned_str_runtime::ResourceEvent::Released(id) => {
                            let detached = if frame.str_returning == Some(*id) {
                                frame.str_returning = None;
                                true
                            } else if let Some(index) = frame.pending_str_results.iter()
                                .position(|v| v == id) {
                                frame.pending_str_results.remove(index);
                                true
                            } else { false };
                            let scalar = frame.owner_resources.iter().find_map(|(owner, held)|
                                (held == id).then_some(*owner));
                            let structure = frame.struct_owner_resources.iter()
                                .find_map(|(owner, held)| (held == id).then_some(owner.clone()));
                            // 結構的 P16 OwnedUtf8 必須與原宣告、真欄位的
                            // ResourceId 一對一；不能由另一個存活持有者替釋放。
                            let bound = if let Some(owner) = scalar {
                                frame.owner_resources.remove(&owner) == Some(*id)
                            } else if let Some(owner) = structure {
                                frame.struct_owner_resources.remove(&owner) == Some(*id)
                            } else {
                                detached
                            };
                            frame.resource_ids.remove(id) && bound
                        }
                    };
                    if !valid { return false; }
                    recorded.push(resource.clone());
                }
                PrivateScalarCallEvent::ResourceOwnerBound(declaration, resource_id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || !frame.resource_ids.contains(resource_id)
                        || frame.pending_str_results.contains(resource_id)
                        || frame.owner_resources.contains_key(declaration)
                        || frame.owner_resources.values().any(|held| held == resource_id)
                        || frame.struct_owner_resources.values().any(|held| held == resource_id) {
                        return false;
                    }
                    if let Some((target, moved)) = frame.pending_transfers.get(resource_id) {
                        // str Move 參數只有原本簽發的目標 ID 能取得資源；
                        // 並且不得與同框架 int／bool／Borrow 參數共用 ID。
                        if frame.name.is_none() || target != declaration || !moved
                            || !frame.bound_parameters.insert(*declaration) {
                            return false;
                        }
                        frame.pending_transfers.remove(resource_id);
                    }
                    frame.owner_resources.insert(*declaration, *resource_id);
                }
                PrivateScalarCallEvent::StrReturned(id) => {
                    if unwinding { return false; }
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.name.is_none() || frame.returned
                        || frame.str_returning != Some(*id)
                        || !frame.resource_ids.contains(id) {
                        return false;
                    }
                    frame.returned = true;
                    returned_imported_loans.extend(frame.imported_loans.iter().copied());
                }
                PrivateScalarCallEvent::StrReturnAccepted(declaration, id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if unwinding || frame.returned
                        || frame.pending_str_results.last() != Some(id)
                        || !frame.resource_ids.contains(id)
                        || frame.owner_resources.contains_key(declaration)
                        || frame.owner_resources.values().any(|x| x == id) {
                        return false;
                    }
                    frame.pending_str_results.pop();
                    frame.owner_resources.insert(*declaration, *id);
                }
                PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::BoolReturned(_) => {
                    if unwinding { return false; }
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.name.is_none() || frame.returned { return false; }
                    frame.returned = true;
                    returned_imported_loans.extend(frame.imported_loans.iter().copied());
                }
                PrivateScalarCallEvent::FunctionExited { name, returned } => {
                    // 任何未返回的動態函式只能由正式失敗回滾退出；
                    // 不能把成功的 Returned 刪掉後偽造正常無返回退出。
                    if frames.len() <= 1 || (!*returned && !unwinding) { return false; }
                    let Some(frame) = frames.pop() else { return false; };
                    if frame.name.as_deref() != Some(name.as_str())
                        || (unwinding && *returned)
                        || frame.returned != *returned || !frame.locals.is_empty()
                        || frame.resource_ids.iter().any(|id| Some(*id) != frame.str_returning)
                        || (!*returned && frame.str_returning.is_some())
                        || !frame.pending_str_results.is_empty() || !frame.loans.is_empty()
                        || !frame.moved_copy_struct_sources.is_empty()
                        || !frame.owner_resources.is_empty()
                        || !frame.struct_owner_resources.is_empty()
                        || !frame.observed_struct_fields.is_empty()
                        || !frame.struct_field_order.is_empty()
                        || frame.pending_str_field_write.is_some()
                        || !frame.pending_struct_moves.is_empty()
                        || !frame.pending_transfers.is_empty()
                        || !frame.loan_resources.is_empty()
                        || frames.last().is_none_or(|parent|
                            frame.imported_loans.iter().any(|ticket|
                                parent.loans.contains(ticket))) {
                        return false;
                    }
                    if let Some(id) = frame.str_returning {
                        let Some(parent) = frames.last_mut() else { return false; };
                        if !parent.resource_ids.insert(id) {
                            return false;
                        }
                        parent.pending_str_results.push(id);
                    }
                }
                PrivateScalarCallEvent::ArgumentEvaluated(_)
                    | PrivateScalarCallEvent::BoolArgumentEvaluated(_)
                    | PrivateScalarCallEvent::ConditionalChecked(_) => {}
            }
        }
        // R05 共用守門：普通函式保留原 ResourceId／局部／所有權
        // 全細帳，再將**同一真事件序列**投影至與同步 self 共享的
        // 進退、唯一回滾及真 BorrowTicket 單次歸還責任表。
        let mut shared_exit_events = Vec::new();
        let mut recorded_until = 0;
        for (index, event) in self.events.iter().enumerate() {
            if index < recorded_until { continue; }
            if let PrivateScalarCallEvent::SelfRead { trace, result, argument_tickets, .. } = event {
                // 外層逐票／回滾明細已於上方對應到方法內同一份事件。
                recorded_until = index + 1 + argument_tickets.len() + usize::from(result.is_err());
                shared_exit_events.extend_from_slice(trace);
                continue;
            }
            shared_exit_events.extend(match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) =>
                Some(PrivateSharedCallExitEvent::LoanAcquired(*ticket)),
            PrivateScalarCallEvent::FunctionEntered(_) =>
                Some(PrivateSharedCallExitEvent::FunctionEntered),
            PrivateScalarCallEvent::LocalDeclaredEmpty(id)
                | PrivateScalarCallEvent::BoolDeclaredEmpty(id) =>
                Some(PrivateSharedCallExitEvent::LocalDeclaredEmpty(*id)),
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::BoolInitialized(id, _)
                | PrivateScalarCallEvent::CopyStructConstructed(id)
                | PrivateScalarCallEvent::CopyStructMovedBound { target: id, .. } =>
                Some(PrivateSharedCallExitEvent::LocalInitialized(*id)),
            PrivateScalarCallEvent::LocalAssigned(id, _)
                | PrivateScalarCallEvent::CopyStructIntFieldAssigned(id, _, _)
                | PrivateScalarCallEvent::CopyStructBoolFieldAssigned(id, _, _)
                | PrivateScalarCallEvent::BoolAssigned(id, _) =>
                Some(PrivateSharedCallExitEvent::LocalAssigned(*id)),
            PrivateScalarCallEvent::LocalScopeEnded(id)
                | PrivateScalarCallEvent::BoolScopeEnded(id)
                | PrivateScalarCallEvent::CopyStructScopeEnded(id) =>
                Some(PrivateSharedCallExitEvent::LocalScopeEnded(*id)),
            PrivateScalarCallEvent::LocalScopeEndedEmpty(id)
                | PrivateScalarCallEvent::BoolScopeEndedEmpty(id) =>
                Some(PrivateSharedCallExitEvent::LocalScopeEndedEmpty(*id)),
            PrivateScalarCallEvent::Returned(_)
            | PrivateScalarCallEvent::BoolReturned(_)
            | PrivateScalarCallEvent::StrReturned(_) =>
                Some(PrivateSharedCallExitEvent::Returned),
            PrivateScalarCallEvent::UnwindStarted =>
                Some(PrivateSharedCallExitEvent::UnwindStarted),
            PrivateScalarCallEvent::LoanReturned(ticket) =>
                Some(PrivateSharedCallExitEvent::LoanReturned(*ticket)),
            PrivateScalarCallEvent::FunctionExited { returned, .. } =>
                Some(PrivateSharedCallExitEvent::FunctionExited {
                    returned: *returned,
                }),
            _ => None,
            });
        }
        frames.len() == 1 && awaited_method.is_none() && awaited_entry.is_none()
            && frames[0].locals.is_empty()
            && frames[0].resource_ids.is_empty()
            && frames[0].owner_resources.is_empty()
            && frames[0].struct_field_order.is_empty()
            && frames[0].pending_struct_moves.is_empty()
            && frames[0].moved_copy_struct_sources.is_empty()
            && frames[0].pending_transfers.is_empty()
            && frames[0].loans.is_empty()
            && frames[0].loan_resources.is_empty()
            && frames[0].str_returning.is_none()
            && frames[0].pending_str_results.is_empty()
            && self.parameter_slots_after_exit == 0
            // 即使沒有借用票據，失敗仍須留下唯一真 UnwindStarted。
            && unwinding == self.result.is_err()
            && private_shared_call_exit_balanced(
                &shared_exit_events, self.result.is_ok(),
            )
            && recorded == self.resource_events
            && crate::owned_str_runtime::event_trace_balanced(&self.resource_events)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarSelfRead {
    result_type: String,
    awaited: bool,
    body: Vec<PrivateScalarMethodStep>,
    arguments: Vec<(DeclarationId, PrivateScalarCallArgument)>,
    name: String,
    owner: String,
    call_span: crate::common_ir::NodeSpan,
    method_start: usize,
    loan: BorrowFact,
}

impl PrivateScalarSelfRead {
    fn checked(
        span: crate::common_ir::NodeSpan, owner: &str, name: &str, id: DeclarationId, verified: &VerifiedPrivateSource,
    ) -> Option<Self> {
        let declaration = verified.plan.declarations.typed_declaration(id)?;
        let definition = verified.structs.get(declaration.ty)?;
        let cursor = verified.tokens.binary_search_by_key(&span.start, |token| token.start).ok()?;
        let mut parser = Parser { source: &verified.source, tokens: &verified.tokens, cursor };
        let call = parser.atom().ok()?;
        if call.node_span() != span { return None; }
        let (receiver, called, terms, awaited) = match call.kind {
            TermKind::MethodCall(r, n, args) => (r, n, args, false),
            TermKind::AwaitMethodCall(r, n, args) => (r, n, args, true),
            _ => return None,
        };
        if receiver != owner || called != name { return None; }
        let (receiver_fact, argument_facts) = verified.plan.declarations.call_borrows_at(span)?.split_last()?;
        let mut facts = argument_facts.iter();
        let mut actuals = Vec::with_capacity(terms.len());
        let checked = verified.plan.declarations.call_borrows.get(&span.start)?;
        let bindings = &checked.method_bindings;
        for term in terms {
            let leaf = private_self_checked_ungrouped_return(verified, &term).ok()?;
            let arg = match verified.plan.scalar_hir.expression(term.node_span()).map(|f| f.ty) {
                Some(crate::common_ir::ScalarType::Int) => PrivateScalarCallArgument::Int(
                    PrivateScalarReturnExpr::from_checked_bindings(&term, bindings, verified).ok()?),
                Some(crate::common_ir::ScalarType::Bool) => PrivateScalarCallArgument::Bool(
                    PrivateScalarBoolExpr::from_checked(&term, bindings, verified).ok()?),
                _ if matches!(&leaf.kind, TermKind::Literal("str")) => PrivateScalarCallArgument::LiteralStr(private_str_literal(leaf, &verified.tokens).ok()?),
                _ if matches!(&leaf.kind, TermKind::Move(_)) => {
                    let owner = PreparedPrivateMoveCall::checked_move_name(leaf, verified).ok()?;
                    let source = *checked.method_moves.get(&term.node_span())?;
                    if verified.plan.declarations.typed_declaration(source)?.ty != "str" { return None; }
                    PrivateScalarCallArgument::MoveStr { owner: owner.to_owned(), source }
                }
                _ => {
                    let TermKind::Name(owner) = &term.kind else { return None; };
                    let fact = *facts.next()?;
                    let ty = verified.plan.declarations.typed_declaration(fact.source)?;
                    // 字串使用原 Checker 依實參次序簽發的借用事實；執行時再對照存活的名稱與身分。
                    let kind = if ty.mutable { BorrowKind::ExclusiveWrite } else { BorrowKind::SharedRead };
                    if ty.ty != "str" || kind != fact.kind || fact.source == id { return None; }
                    PrivateScalarCallArgument::BorrowStr { owner: owner.clone(), fact }
                }
            };
            actuals.push(arg);
        }
        if facts.next().is_some() { return None; }
        let mut candidates = definition.methods_named(name).filter(|method|
            method.params.len() == actuals.len() && method.params.iter().zip(&actuals).all(|(p, arg)|
                p.ty == match arg {
                    PrivateScalarCallArgument::Int(_) => "int",
                    PrivateScalarCallArgument::Bool(_) => "bool",
                    PrivateScalarCallArgument::BorrowStr { .. } | PrivateScalarCallArgument::MoveStr { .. } | PrivateScalarCallArgument::LiteralStr(_) => "str",
                    _ => return false,
                }));
        let method = candidates.next()?;
        if candidates.next().is_some() || method.is_async != awaited || !matches!(method.result_ty.as_str(), "int" | "bool" | "str") { return None; }
        let arguments = method.params.iter().map(|p| DeclarationId(p.declaration_start)).zip(actuals).collect();
        let safety = verified.plan.method_safety.get(&method.start)?;
        let compiler = PrivateScalarMethodCompiler { verified, method, definition, safety };
        let body = compiler.block(&method.body.statements)?;
        let loan = BorrowFact::new(id, safety.self_borrow);
        if (loan.kind == BorrowKind::ExclusiveWrite && !declaration.mutable)
            || *receiver_fact != loan { return None; }
        Some(Self { awaited, result_type: method.result_ty.clone(), name: name.to_owned(), owner: owner.to_owned(), body,
            call_span: span, method_start: method.start, loan, arguments })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarReturnExpr {
    SelfRead(PrivateScalarSelfRead),
    /// 原 Checker 已證明所有前置 if 分支均真正 Return 的後方
    /// 不可達 expression。只有遭執行到才回報狀態錯誤，絕不造假 int。
    Unreachable,
    Parameter(DeclarationId),
    /// 原 Checker 合法 Copyable int 欄位，執行時只能讀真 P16 結構持有者。
    CopyStructIntField { declaration: DeclarationId, owner: String, field: String },
    Constant(i64),
    /// F2：僅由已驗證 bool 函式的直接 return 建立；非 int 算式來源。
    BoolValue(Box<PrivateScalarBoolExpr>),
    /// 真 OwnedUtf8 回傳不佔 Copyable i64 工作棧。
    StrValue(PrivateScalarStrReturn),
    /// 原 Checker 已核准的同步 str 呼叫作本函式直接返回；
    /// 內層 ExitCall 後同一 OwnedUtf8 再從本 frame 返回，絕不複製。
    StrCall { name: String, arguments: Vec<PrivateScalarCallArgument> },
    Addition(Box<Self>, Box<Self>),
    Subtraction(Box<Self>, Box<Self>),
    /// 原 Checker 唯一具名函式與已驗證 int 實參；真正呼叫會進入新框架。
    Call {
        name: String, arguments: Vec<PrivateScalarCallArgument>,
        /// V7：原 Checker 已核准直接 await，不能與同步函式呼叫共用
        /// 遺失來源模式的工作圖。只有實際到達此節點才 EnterCall。
        awaited: bool,
    },
}

impl PrivateScalarReturnExpr {
    /// 僅由真正 Checker 已交付的 int TypedHIR 準備，不在執行期
    /// 重新猜測文法或由來源文字推論算式樹。
    fn from_checked(term: &Term, params: &[Parameter], verified: &VerifiedPrivateSource)
        -> Result<Self, PrivateScalarCallPrepareError> {
        let bindings = params.iter().map(|param|
            (param.name.clone(), DeclarationId(param.declaration_start))
        ).collect::<BTreeMap<_, _>>();
        Self::from_checked_bindings(term, &bindings, verified)
    }

    /// 共用原 Checker 的 DeclarationId 編譯函式參數及 main 既有 int 持有者。
    fn from_checked_bindings(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        use crate::common_ir::ScalarType;
        let span = term.node_span();
        if verified.plan.scalar_hir.expression(span)
            .is_none_or(|fact| fact.ty != ScalarType::Int) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "int 返回運算式缺少原 Checker TypedHIR Expression 型別",
            ));
        }
        match &term.kind {
            TermKind::MethodCall(owner, name, _) | TermKind::AwaitMethodCall(owner, name, _) => {
                bindings.get(owner).and_then(|id|
                    PrivateScalarSelfRead::checked(span, owner, name, *id, verified)
                ).map(Self::SelfRead).ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "同步方法讀取缺少已驗證的接收者、方法或共享借用事實",
                ))
            }
            TermKind::Name(name) => {
                let id = *bindings.get(name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "int 名稱不是本次原 Checker 已核准的 Copyable int 來源",
                    ),
                )?;
                if verified.plan.scalar_hir.reference(span)
                    .is_none_or(|fact|
                        fact.resolved.declaration != id
                        || fact.resolved.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start && token.end == span.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == name)
                    ) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "返回中具名 int 引用缺少原 Checker 參數身分",
                    ));
                }
                Ok(Self::Parameter(id))
            }
            TermKind::FieldRead(owner, field) => {
                let declaration = *bindings.get(owner).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "結構欄位讀取缺少本函式原 Checker DeclarationId",
                    ),
                )?;
                let fact = verified.plan.declarations.typed_declaration(declaration)
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "結構欄位讀取缺少原 Checker 具型別宣告",
                    ))?;
                let definition = verified.structs.get(fact.ty).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "結構欄位讀取持有者非原 Checker 結構型別",
                    ),
                )?;
                if !definition.fields.iter().any(|candidate|
                    candidate.name == *field && candidate.ty == "int")
                    || !definition.fields.iter().all(|candidate|
                        matches!(candidate.ty.as_str(), "int" | "bool" | "str"))
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(&token.kind, TokenKind::Name(found) if found == owner))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == field)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "結構 int 欄位缺少同源 Copyable 型別／Lexer 欄位身分",
                    ));
                }
                Ok(Self::CopyStructIntField {
                    declaration, owner: owner.clone(), field: field.clone(),
                })
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                if !verified.tokens.iter().any(|token|
                    token.start == span.start && token.end == span.end
                        && matches!(&token.kind, TokenKind::Integer(found) if found == raw)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int 常值缺少原 Lexer 來源",
                    ));
                }
                Ok(Self::Constant(crate::syntax_shared::integer_i64(raw)
                    .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                        "返回 int 常值超出 i64 範圍",
                    ))?))
            }
            TermKind::Group(inner) => {
                if !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int Group 缺少原 Lexer 左右括號來源",
                    ));
                }
                Self::from_checked_bindings(inner, bindings, verified)
            },
            TermKind::Call(call) => {
                // 由原 Parser 真函式宣告／Checker HIR 決定簽章。
                // 不允許用字面來源重新推測或合成隱含轉型。
                let target = verified.functions.iter()
                    .find(|fun| fun.name == call.name)
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀呼叫沒有原 Checker 具名宣告",
                    ))?;
                if target.name == "main" || target.result_ty != "int"
                    || target.is_async != call.awaited
                    || target.params.len() != call.arguments.len()
                    || target.params.iter().any(|param|
                        !matches!(param.ty.as_str(), "int" | "bool" | "str")) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀呼叫只接通原 Checker Copyable int／bool 完整簽章",
                    ));
                }
                let mut arguments = Vec::new();
                let mut checked_borrows = Vec::new();
                for (argument, parameter) in call.arguments.iter().zip(&target.params) {
                    arguments.push(if parameter.ty == "str" {
                        if let TermKind::Move(_) = &argument.kind {
                            let owner = PreparedPrivateMoveCall::checked_move_name(
                                argument, verified,
                            ).map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                                "str Move 實參缺少原 Parser／Lexer 具名來源",
                            ))?;
                            let source = *bindings.get(owner).ok_or(
                                PrivateScalarCallPrepareError::Unsupported(
                                    "str Move 實參缺少本詞彙作用域 DeclarationId",
                                ),
                            )?;
                            if !verified.plan.declarations.typed_declaration(source)
                                .is_some_and(|record| record.ty == "str") {
                                return Err(PrivateScalarCallPrepareError::Unsupported(
                                    "str Move 實參缺少原 Checker str 宣告型別",
                                ));
                            }
                            PrivateScalarCallArgument::MoveStr {
                                owner: owner.to_owned(), source,
                            }
                        } else if !matches!(&argument.kind, TermKind::Name(_)) {
                            PreparedPrivateScalarCall::checked_temporary_str_argument(
                                argument, bindings, verified,
                            )?
                        } else {
                        let TermKind::Name(owner) = &argument.kind else {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "str 預設借用實參只能是原 Parser 具名來源",
                            ));
                        };
                        let id = *bindings.get(owner).ok_or(
                            PrivateScalarCallPrepareError::Unsupported(
                                "str 實參缺少當前原 Checker DeclarationId",
                            ),
                        )?;
                        let declaration = verified.plan.declarations.typed_declaration(id)
                            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                                "str 實參缺少原 Checker 宣告事實",
                            ))?;
                        if declaration.ty != "str" || !verified.tokens.iter().any(|token|
                            token.start == argument.start && token.end == argument.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == owner)
                        ) {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "str 實參缺少來源型別或原 Lexer 名稱",
                            ));
                        }
                        let fact = BorrowFact::new(id, if declaration.mutable {
                            BorrowKind::ExclusiveWrite
                        } else { BorrowKind::SharedRead });
                        checked_borrows.push(fact);
                        PrivateScalarCallArgument::BorrowStr {
                            owner: owner.clone(), fact,
                        }
                        }
                    } else if parameter.ty == "bool" {
                        PrivateScalarCallArgument::Bool(
                            PrivateScalarBoolExpr::from_checked_call_argument(
                                argument, bindings, verified)?,
                        )
                    } else {
                        PrivateScalarCallArgument::Int(
                            Self::from_checked_bindings(argument, bindings, verified)?,
                        )
                    });
                }
                if verified.plan.declarations.call_borrows_at(span).unwrap_or(&[])
                    != checked_borrows.as_slice() {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "str 借用 DeclarationId、權限或數量不符 Checker call_borrows",
                    ));
                }
                Ok(Self::Call {
                    name: call.name.clone(), arguments, awaited: call.awaited,
                })
            }
            TermKind::UnresolvedZeroArgs(name) => {
                // A2/F02：裸 name() 在 Parser 無法先判斷是建構式或零參數函式；
                // 只有正式 Checker 分類、完整函式宣告和 int TypedHIR
                // 全部一致時，才可交給原顯式動態框架執行。
                // 此純值切片若存在任一結構已於 prepare 入口拒絕，
                // 不得藉此把 Struct() 擅自改判為普通函式。
                let target = verified.functions.iter()
                    .find(|fun| fun.name == *name && fun.name != "main")
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "零參數來源缺少原 Checker 唯一普通函式宣告",
                    ))?;
                if target.result_ty != "int" || !target.params.is_empty()
                    || verified.plan.declarations.call_borrows_at(span)
                        .is_some_and(|facts| !facts.is_empty())
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(&token.kind, TokenKind::Name(found) if found == name))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "零參數呼叫缺少原 Parser／Checker 完整簽章或詞彙來源",
                    ));
                }
                Ok(Self::Call {
                    name: name.clone(), arguments: Vec::new(), awaited: false,
                })
            }
            TermKind::BinaryInt(binary) => {
                let left = binary.left.node_span();
                let right = binary.right.node_span();
                let Some(fact) = verified.plan.scalar_hir.arithmetic(span) else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式內加減缺少原 Checker 位置／運算子／來源 HIR",
                    ));
                };
                if fact.left != left || fact.right != right
                    || fact.operator != binary.operator
                    || !matches!(fact.operator, '+' | '-') {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式內加減 HIR 端點或運算子遭偷換",
                    ));
                }
                let left = Box::new(Self::from_checked_bindings(
                    &binary.left, bindings, verified,
                )?);
                let right = Box::new(Self::from_checked_bindings(
                    &binary.right, bindings, verified,
                )?);
                Ok(if fact.operator == '+' {
                    Self::Addition(left, right)
                } else {
                    Self::Subtraction(left, right)
                })
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "此私有呼叫只接通原 Checker 已核准的 Copyable int 加減及括號",
            )),
        }
    }

    /// 在已驗證的原 Parser int 表達式中找出實際具名函式來源。
    /// 僅用於選擇本私有 evaluator 的函式資料，不改動 Checker 決議。
    fn first_source_call<'a>(term: &'a Term) -> Option<&'a str> {
        match &term.kind {
            TermKind::Call(call) => Some(call.name.as_str()),
            TermKind::UnresolvedZeroArgs(name) => Some(name.as_str()),
            TermKind::MethodCall(_, _, args) | TermKind::AwaitMethodCall(_, _, args) => args.iter().find_map(Self::first_source_call),
            TermKind::Group(inner) => Self::first_source_call(inner),
            TermKind::BinaryInt(binary) => Self::first_source_call(&binary.left)
                .or_else(|| Self::first_source_call(&binary.right)),
            _ => None,
        }
    }

    /// V1／F4：這兩類控制邊在原 Parser 中已有確定的常值形狀，
    /// 原 Checker 又已完整核准整個來源。只有可直接證明零迭代
    /// 才能略過其本體呼叫；動態條件與未知整數範圍絕不猜測。
    fn definitely_false_condition(term: &Term) -> bool {
        match &term.kind {
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(false)) => true,
            TermKind::Group(inner) => Self::definitely_false_condition(inner),
            _ => false,
        }
    }

    fn definitely_empty_range(lower: &str, upper: &str) -> bool {
        match (
            crate::syntax_shared::integer_i64(lower),
            crate::syntax_shared::integer_i64(upper),
        ) {
            (Ok(start), Ok(end)) => start >= end,
            _ => false,
        }
    }

    /// R01～R03／V1：只遍歷原 Parser／Checker 已核准且可能執行的來源
    /// AST。此處是靜態候選圖，不提前求值或選擇一般 if 分支。
    /// 原始常值已證的零迭代與無條件 break／continue 後綴不屬
    /// 真工作計畫，不能因其中有尚未接通的 callee 錯拒主路徑。
    fn first_source_call_in_statements(statements: &[Stmt]) -> Option<&str> {
        for statement in statements {
            let found = match &statement.kind {
                StmtKind::Declare(owner) => owner.initializer.as_ref()
                    .and_then(Self::first_source_call),
                StmtKind::Assign(assignment) =>
                    Self::first_source_call(&assignment.expression),
                StmtKind::Return(ret) => Self::first_source_call(&ret.expression),
                StmtKind::If { branch, no } =>
                    Self::first_source_call_in_statements(&branch.body)
                        .or_else(|| no.as_ref().and_then(|other|
                            Self::first_source_call_in_statements(other))),
                StmtKind::While { condition, body } if
                    !Self::definitely_false_condition(condition) =>
                    Self::first_source_call_in_statements(&body.statements),
                StmtKind::ForRange { lower, upper, body, .. } if
                    !Self::definitely_empty_range(lower, upper) =>
                    Self::first_source_call_in_statements(&body.statements),
                _ => None,
            };
            if found.is_some() { return found; }
            if matches!(&statement.kind, StmtKind::Return(_)
                | StmtKind::Break | StmtKind::Continue) {
                break;
            }
        }
        None
    }

    /// R01：由原 Parser AST 收集全部可能的具名函式呼叫，包含
    /// 多層實參及巢狀條件分支。此處只建立來源目錄，
    /// 不選擇動態分支、不創建框架、不執行任何表達式。
    fn collect_source_calls_in_term<'a>(term: &'a Term, names: &mut Vec<&'a str>) {
        match &term.kind {
            TermKind::Call(call) => {
                names.push(call.name.as_str());
                for argument in &call.arguments {
                    Self::collect_source_calls_in_term(argument, names);
                }
            }
            TermKind::UnresolvedZeroArgs(name) => names.push(name.as_str()),
            TermKind::MethodCall(_, _, args) | TermKind::AwaitMethodCall(_, _, args) => {
                for arg in args { Self::collect_source_calls_in_term(arg, names); }
            }
            TermKind::Group(inner) => Self::collect_source_calls_in_term(inner, names),
            TermKind::BinaryInt(binary) => {
                Self::collect_source_calls_in_term(&binary.left, names);
                Self::collect_source_calls_in_term(&binary.right, names);
            }
            // 未經本 evaluator 接通的型別／語法仍須於正式編譯時保守拒絕；
            // 不從來源名稱字面自行推出可執行的其他語意。
            _ => {}
        }
    }

    fn collect_source_calls_in_statements<'a>(
        statements: &'a [Stmt], names: &mut Vec<&'a str>,
    ) {
        for statement in statements {
            match &statement.kind {
                StmtKind::Declare(owner) => {
                    if let Some(initializer) = owner.initializer.as_ref() {
                        Self::collect_source_calls_in_term(initializer, names);
                    }
                }
                StmtKind::Assign(assignment) =>
                    Self::collect_source_calls_in_term(&assignment.expression, names),
                StmtKind::Return(ret) =>
                    Self::collect_source_calls_in_term(&ret.expression, names),
                StmtKind::If { branch, no } => {
                    Self::collect_source_calls_in_term(&branch.branch.condition, names);
                    Self::collect_source_calls_in_statements(&branch.body, names);
                    if let Some(other) = no.as_ref() {
                        Self::collect_source_calls_in_statements(other, names);
                    }
                }
                StmtKind::While { condition, body } => {
                    // 原 Checker／Parser 已證靜態 false 時，真 Runtime
                    // 不建立任何迭代框架；不能強迫該本體的函式進圖。
                    Self::collect_source_calls_in_term(condition, names);
                    if !Self::definitely_false_condition(condition) {
                        Self::collect_source_calls_in_statements(&body.statements, names);
                    }
                }
                StmtKind::ForRange { lower, upper, body, .. } => {
                    if !Self::definitely_empty_range(lower, upper) {
                        Self::collect_source_calls_in_statements(&body.statements, names);
                    }
                }
                _ => {}
            }
            // 真來源直接 break／continue／return 後方沒有可執行
            // HIR 與呼叫事件；不擅自推定條件式內的控制轉移。
            if matches!(&statement.kind,
                StmtKind::Break | StmtKind::Continue | StmtKind::Return(_)) {
                break;
            }
        }
    }

    fn needs_call_stack(&self) -> bool {
        match self {
            Self::SelfRead(_) | Self::Call { .. } | Self::StrCall { .. } => true,
            Self::Addition(left, right) | Self::Subtraction(left, right) =>
                left.needs_call_stack() || right.needs_call_stack(),
            _ => false,
        }
    }

    fn evaluate(&self, frame: &BTreeMap<DeclarationId, i64>)
        -> Result<i64, PrivateScalarCallRuntimeError> {
        match self {
            Self::Parameter(identity) =>
                frame.get(identity).copied().ok_or(PrivateScalarCallRuntimeError::StateMismatch),
            Self::Constant(value) => Ok(*value),
            Self::Addition(left, right) => left.evaluate(frame)?
                .checked_add(right.evaluate(frame)?)
                .ok_or(PrivateScalarCallRuntimeError::IntegerOverflow),
            Self::Subtraction(left, right) => left.evaluate(frame)?
                .checked_sub(right.evaluate(frame)?)
                .ok_or(PrivateScalarCallRuntimeError::IntegerOverflow),
            // 不准從只有 int slot 的舊純值捷徑假造結構欄位常數。
            Self::SelfRead(_) | Self::CopyStructIntField { .. } | Self::Call { .. }
            | Self::BoolValue(..) | Self::StrValue(..)
            | Self::StrCall { .. } | Self::Unreachable =>
                Err(PrivateScalarCallRuntimeError::StateMismatch),
        }
    }

    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Call { name, arguments, .. } => {
                calls.push(name);
                for argument in arguments { argument.collect_calls(calls); }
            }
            Self::Addition(left, right) | Self::Subtraction(left, right) => {
                left.collect_calls(calls);
                right.collect_calls(calls);
            }
            Self::BoolValue(expr) => expr.collect_calls(calls),
            Self::StrValue(..) => {},
            Self::StrCall { name, arguments } => {
                calls.push(name);
                for argument in arguments { argument.collect_calls(calls); }
            },
            Self::SelfRead(site) => {
                site.collect_calls(calls);
            }
            Self::Parameter(_) | Self::CopyStructIntField { .. }
                | Self::Constant(_) | Self::Unreachable => {}
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarLocalInit {
    declaration: DeclarationId,
    value: PrivateScalarReturnExpr,
}

/// 每個實參的真型別已由 Checker 決定：bool 不是 int 可隱含轉型。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarCallArgument {
    Int(PrivateScalarReturnExpr),
    Bool(PrivateScalarBoolExpr),
    /// 唯有正式 Checker call_borrows 與原 Lexer 具名 str 才能進入票據流程。
    BorrowStr { owner: String, fact: BorrowFact },
    /// 真 Parser .move() 以原 Checker 已簽發的 str 身分整筆傳參。
    MoveStr { owner: String, source: DeclarationId },
    /// 只接原 Checker 同步 str-return 函式，完整巢狀參數仍由工作棧求值。
    NestedStrCall { name: String, arguments: Vec<PrivateScalarCallArgument> },
    /// 唯一原 Lexer str 常值：直到真正求值才配置 OwnedUtf8。
    LiteralStr(String),
}

impl PrivateScalarCallArgument {
    fn collect_calls<'a>(&'a self, output: &mut Vec<&'a str>) {
        match self {
            Self::Int(expr) => expr.collect_calls(output),
            Self::Bool(expr) => expr.collect_calls(output),
            Self::NestedStrCall { name, arguments } => {
                output.push(name);
                for argument in arguments { argument.collect_calls(output); }
            }
            Self::BorrowStr { .. } | Self::MoveStr { .. }
                | Self::LiteralStr(_) => {}
        }
    }
}

/// R01／R02：原 Parser 順序不可被「先宣告、後賦值」重排。
/// 此執行計畫保留每一筆 Checker 核准的實際宣告與可寫更新。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarBoolExpr {
    Method(Box<PrivateScalarSelfRead>),
    Constant(bool),
    Bound(DeclarationId),
    /// 只有作為已核准 bool 實參才編譯此同步函式結果；
    /// 不讓布林函式的 i64 暫存值流入 int 表達式。
    Call { name: String, arguments: Vec<PrivateScalarCallArgument> },
    /// 原 Checker 唯一 Copyable bool 欄位，從目前 P16 真結構執行期讀值。
    CopyStructBoolField { declaration: DeclarationId, owner: String, field: String },
}

impl PrivateScalarBoolExpr {
    /// 只從原 Checker 的 bool HIR、同一 Lexer/Parser 真來源編譯條件。
    fn from_checked(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        let span = term.node_span();
        if verified.plan.scalar_hir.expression(span)
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Bool) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "bool 條件缺少原 Checker 已核准的 TypedHIR",
            ));
        }
        match &term.kind {
            TermKind::MethodCall(owner, name, _) | TermKind::AwaitMethodCall(owner, name, _) => {
                bindings.get(owner).and_then(|id| PrivateScalarSelfRead::checked(span, owner, name, *id, verified))
                    .filter(|site| site.result_type == "bool").map(|site| Self::Method(Box::new(site)))
                    .ok_or(PrivateScalarCallPrepareError::Unsupported("布林方法缺少原接收者、簽章或借票事實"))
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) => {
                let text = std::str::from_utf8(&verified.source.bytes).map_err(|_|
                    PrivateScalarCallPrepareError::SourceChanged,
                )?;
                let expected = if *value { "true" } else { "false" };
                if text.get(term.start..term.end) != Some(expected)
                    || !verified.tokens.iter().any(|token|
                        token.start == term.start && token.end == term.end
                            && matches!(&token.kind, TokenKind::Keyword(_))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 常值缺少原 Lexer Token 或文字身分",
                    ));
                }
                Ok(Self::Constant(*value))
            }
            TermKind::Name(name) => {
                let id = *bindings.get(name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "bool 名稱缺少先前原 Checker 宣告",
                    ),
                )?;
                if verified.plan.scalar_hir.reference(span)
                    .is_none_or(|fact|
                        fact.resolved.declaration != id
                            || fact.resolved.ty != crate::common_ir::ScalarType::Bool)
                    || !verified.plan.declarations.typed_declaration(id)
                        .is_some_and(|fact| fact.ty == "bool")
                    || !verified.tokens.iter().any(|token|
                        token.start == term.start && token.end == term.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == name)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 引用缺少真 Checker／Lexer DeclarationId",
                    ));
                }
                Ok(Self::Bound(id))
            }
            TermKind::FieldRead(owner, field) => {
                let declaration = *bindings.get(owner).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "bool 欄位讀取缺少本詞彙作用域 Checker 宣告",
                    ),
                )?;
                let typed = verified.plan.declarations.typed_declaration(declaration)
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "bool 欄位讀取缺少 Checker 型別與 DeclarationId",
                    ))?;
                let definition = verified.structs.get(typed.ty).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "bool 欄位來源不是原 Checker 結構宣告",
                    ),
                )?;
                if !definition.fields.iter().any(|candidate|
                        candidate.name == *field && candidate.ty == "bool")
                    || !definition.fields.iter().all(|candidate|
                        matches!(candidate.ty.as_str(), "int" | "bool" | "str"))
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(&token.kind, TokenKind::Name(found) if found == owner))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == field)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 欄位缺少真 Lexer／Checker 純 Copyable 身分",
                    ));
                }
                Ok(Self::CopyStructBoolField {
                    declaration, owner: owner.clone(), field: field.clone(),
                })
            }
            TermKind::Group(inner) => {
                if !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool Group 缺少原 Lexer 左右括號",
                    ));
                }
                Self::from_checked(inner, bindings, verified)
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "此子集只執行原 Checker 核准 bool 常值／具名宣告／純結構欄位",
            )),
        }
    }

    /// F2：原 Parser/Checker 的 bool 函式可作 bool 形式參數值。
    /// 本子批只允許同步、每項 bool 型別的引數；其它合法函式形狀
    /// 仍保守 Unsupported，不繞過原 Checker，也不猜測整數轉 bool。
    // 混合 int/bool 的 bool-return 呼叫必須收到目前作用域完整的
    // DeclarationId 名稱表；bool-only 投影只可供純 bool 名稱解析。
    // 所有 int/bool 實參仍由原 Checker TypedHIR 逐項確認型別，
    // 不因給予完整名稱表而新增隱含 int/bool 轉型或借用權限。
    fn from_checked_call_argument(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        let span = term.node_span();
        if verified.plan.scalar_hir.expression(span)
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Bool) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "bool 實參沒有原 Checker TypedHIR",
            ));
        }
        match &term.kind {
            TermKind::Group(inner) => {
                if !verified.tokens.iter().any(|t| t.start == span.start
                    && matches!(t.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|t| t.end == span.end
                        && matches!(t.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 函式實參括號缺少原 Lexer 來源",
                    ));
                }
                Self::from_checked_call_argument(inner, bindings, verified)
            }
            TermKind::Call(call) => {
                if call.awaited { return Err(PrivateScalarCallPrepareError::Unsupported(
                    "bool async 返回尚需等待型別與安全點驗收",
                )); }
                Self::checked_sync_call(&call.name, &call.arguments, span,
                    bindings, verified)
            }
            TermKind::UnresolvedZeroArgs(name) => {
                if verified.structs.contains_key(name) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "零參數結構與 bool 函式呼叫不可混同",
                    ));
                }
                Self::checked_sync_call(name, &[], span, bindings, verified)
            }
            _ => Self::from_checked(term, bindings, verified),
        }
    }

    fn checked_sync_call(
        name: &str, actual: &[Term], span: crate::common_ir::NodeSpan,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        let target = verified.functions.iter().find(|fun| fun.name == name)
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "bool 呼叫缺少原 Checker 函式宣告",
            ))?;
        if name == "main" || target.result_ty != "bool" || target.is_async
            || target.params.len() != actual.len()
            || target.params.iter().any(|param|
                !matches!(param.ty.as_str(), "int" | "bool" | "str")) {
            // P15.6：借用不在此預先一律拒絕；以下逐形參生成真
            // BorrowFact，再對原 Checker call_borrows_at(span) 全量比對。
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "bool 呼叫須為原 Checker 同步 int/bool/str 混合形參及 bool 結果",
            ));
        }
        // F2：與 int-return 真工作圖一致，非 Copyable 的 str 實參
        // 必須出自原 Checker 真來源身分，並精確核對 call_borrows。
        // 任何來源型別或權限不符時，尚未動態 EnterCall／借出／移交。
        let mut arguments = Vec::with_capacity(actual.len());
        let mut observed_borrows = Vec::new();
        for (argument, param) in actual.iter().zip(&target.params) {
            arguments.push(match param.ty.as_str() {
                "bool" => PrivateScalarCallArgument::Bool(
                    Self::from_checked_call_argument(argument, bindings, verified)?,
                ),
                "int" => PrivateScalarCallArgument::Int(
                    PrivateScalarReturnExpr::from_checked_bindings(
                        argument, bindings, verified,
                    )?,
                ),
                "str" => {
                    if matches!(&argument.kind, TermKind::Move(_)) {
                        let owner = PreparedPrivateMoveCall::checked_move_name(
                            argument, verified,
                        ).map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                            "bool 返回 str Move 實參缺少原 Lexer/Parser 身分",
                        ))?;
                        let source = *bindings.get(owner).ok_or(
                            PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str Move 實參缺少目前作用域 DeclarationId",
                            ),
                        )?;
                        if !verified.plan.declarations.typed_declaration(source)
                            .is_some_and(|fact| fact.ty == "str") {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str Move 缺少原 Checker 擁有型別",
                            ));
                        }
                        PrivateScalarCallArgument::MoveStr {
                            owner: owner.to_owned(), source,
                        }
                    } else if !matches!(&argument.kind, TermKind::Name(_)) {
                        PreparedPrivateScalarCall::checked_temporary_str_argument(
                            argument, bindings, verified,
                        )?
                    } else {
                        let TermKind::Name(owner) = &argument.kind else {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str 預設借用只支援原 Parser 具名來源",
                            ));
                        };
                        let source = *bindings.get(owner).ok_or(
                            PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str 借用缺少目前作用域 DeclarationId",
                            ),
                        )?;
                        let decl = verified.plan.declarations.typed_declaration(source)
                            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str 借用缺少原 Checker 型別",
                            ))?;
                        if decl.ty != "str"
                            || !verified.tokens.iter().any(|token|
                                token.start == argument.start && token.end == argument.end
                                    && matches!(&token.kind,
                                        TokenKind::Name(found) if found == owner)) {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "bool 返回 str 借用缺少原 Lexer/Checker 來源",
                            ));
                        }
                        let fact = BorrowFact::new(source, if decl.mutable {
                            BorrowKind::ExclusiveWrite
                        } else {
                            BorrowKind::SharedRead
                        });
                        observed_borrows.push(fact);
                        PrivateScalarCallArgument::BorrowStr {
                            owner: owner.clone(), fact,
                        }
                    }
                }
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "bool 返回函式含未核准的形式參數型別",
                )),
            });
        }
        if verified.plan.declarations.call_borrows_at(span).unwrap_or(&[])
            != observed_borrows.as_slice() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "bool 返回 str 實參原 Checker BorrowFact 身分或權限不相符",
            ));
        }
        Ok(Self::Call { name: name.to_owned(), arguments })
    }

    fn collect_calls<'a>(&'a self, output: &mut Vec<&'a str>) {
        if let Self::Method(site) = self {
            for (_, arg) in &site.arguments { arg.collect_calls(output); }
        }
        if let Self::Call { name, arguments } = self {
            output.push(name);
            for argument in arguments { argument.collect_calls(output); }
        }
    }

    /// 所有 bool 取值共用同一種真執行權威，不能在條件與實參各自
    /// 推定欄位初始常值；唯一 P16 持有者與本框架 DeclarationId
    /// 必須同時存在，未到達宣告則零讀取／零資源事件。
    fn evaluate_runtime(
        &self,
        frame: &PrivateScalarActiveFrame,
        structs: &crate::mixed_struct_runtime::MixedStructRuntime,
    ) -> Result<bool, PrivateScalarCallRuntimeError> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Method(_) | Self::Call { .. } => Err(PrivateScalarCallRuntimeError::StateMismatch),
            Self::Bound(id) => frame.bool_values.get(id).copied()
                .ok_or(PrivateScalarCallRuntimeError::StateMismatch),
            Self::CopyStructBoolField { declaration, owner, field } => {
                if !frame.copy_struct_scopes.iter().rev().flatten()
                    .any(|(id, name)| id == declaration && name == owner) {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                }
                match structs.field(owner, field).map_err(|_|
                    PrivateScalarCallRuntimeError::StateMismatch,
                )? {
                    crate::mixed_struct_runtime::Observed::Bool(value) => Ok(value),
                    _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
                }
            }
        }
    }
}

/// 已核准的字串所有權回傳來源，必須經 Checker 與唯一持有者驗證。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarStrReturn {
    Literal(String),
    Move { source: String, declaration: DeclarationId },
}

/// 只接受原 Lexer 字串常值、明確 .move() 或 Checker 真 str 函式來源。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarStrInitializer {
    Method(PrivateScalarSelfRead),
    Literal(String),
    Move { source: String, declaration: DeclarationId },
    /// 在同一顯式工作棧完成 ReturnNamed／OwnedUtf8 交接後才提交本宣告。
    Call { name: String, arguments: Vec<PrivateScalarCallArgument> },
}
impl PrivateScalarStrInitializer {
    fn collect_calls<'a>(&'a self, out: &mut Vec<&'a str>) {
        if let Self::Method(site) = self {
            site.collect_calls(out);
        }
        if let Self::Call { name, arguments } = self {
            out.push(name);
            for arg in arguments { arg.collect_calls(out); }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarBranchAction {
    /// 原 Parser 同一 if 葉分支的多筆已核准語句，保留真正執行順序。
    Sequence(Vec<PrivateScalarBranchAction>),
    /// 原 Checker 區塊內的每一筆局部宣告，離開本區塊必須依真建立順序逆序離域。
    Scoped(Vec<PrivateScalarBranchAction>),
    DeclareInt { declaration: DeclarationId, value: PrivateScalarReturnExpr },
    DeclareEmptyInt { declaration: DeclarationId },
    DeclareBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    DeclareEmptyBool { declaration: DeclarationId },
    /// 原 Checker 唯一 str 擁有者；由真 OwnedStrRuntime 配置／逆序釋放。
    DeclareStr {
        declaration: DeclarationId, name: String, mutable: bool,
        initializer: PrivateScalarStrInitializer,
    },
    /// str 詞彙持有者存在，但尚無 ResourceId／UTF-8 緩衝區。
    DeclareEmptyStr { declaration: DeclarationId, name: String, mutable: bool },
    DeclareCopyStruct {
        declaration: DeclarationId, name: String, plan: PrivateScalarCopyStructPlan,
    },
    MoveCopyStruct {
        declaration: DeclarationId, name: String,
        source: DeclarationId, source_name: String,
    },
    /// 真 Parser 巢狀 If，沿用相同 Checker bool 條件與作用域安全證據。
    NestedIf(Box<PrivateScalarEarlyIf>),
    Return(PrivateScalarReturnExpr),
    AssignInt { declaration: DeclarationId, value: PrivateScalarReturnExpr },
    AssignCopyStructInt {
        declaration: DeclarationId, owner: String, field: String,
        value: PrivateScalarReturnExpr,
    },
    AssignCopyStructBool {
        declaration: DeclarationId, owner: String, field: String,
        value: PrivateScalarBoolExpr,
    },
    AssignCopyStructStr {
        declaration: DeclarationId, owner: String, field: String, value: String,
    },
    AssignBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    /// 原 Checker 可寫 str 的真字面值／明確整筆 .move() 覆寫。
    AssignStr { declaration: DeclarationId, name: String, initializer: PrivateScalarStrInitializer },
    /// 同一已驗證函式圖的逐輪 bool 判斷；本體仍使用原 Checker 語句計畫。
    While { condition: PrivateScalarBoolExpr, body: Box<PrivateScalarBranchAction> },
    /// 只以原 Checker 驗證的半開 int 區間與迭代 DeclarationId 執行。
    ForRange {
        iterator: DeclarationId, lower: i64, upper: i64,
        body: Box<PrivateScalarBranchAction>,
    },
    Break,
    Continue,
}

impl PrivateScalarBranchAction {
    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Sequence(steps) | Self::Scoped(steps) => {
                for action in steps { action.collect_calls(calls); }
            }
            Self::NestedIf(plan) => plan.collect_calls(calls),
            Self::Return(expr) | Self::AssignInt { value: expr, .. }
                | Self::AssignCopyStructInt { value: expr, .. }
                | Self::DeclareInt { value: expr, .. } =>
                expr.collect_calls(calls),
            Self::While { condition, body } => {
                condition.collect_calls(calls);
                body.collect_calls(calls);
            }
            Self::ForRange { body, .. } => body.collect_calls(calls),
            Self::DeclareCopyStruct { plan, .. } => plan.collect_calls(calls),
            Self::AssignBool { value, .. } | Self::AssignCopyStructBool { value, .. }
                | Self::DeclareBool { value, .. } => value.collect_calls(calls),
            Self::DeclareStr { initializer, .. } | Self::AssignStr { initializer, .. } =>
                initializer.collect_calls(calls),
            Self::DeclareEmptyInt { .. } | Self::DeclareEmptyBool { .. }
                | Self::DeclareEmptyStr { .. } | Self::MoveCopyStruct { .. }
                | Self::AssignCopyStructStr { .. }
                | Self::Break | Self::Continue => {}
        }
    }

    fn always_returns(&self) -> bool {
        match self {
            Self::Return(_) => true,
            Self::Sequence(actions) | Self::Scoped(actions) =>
                actions.last().is_some_and(Self::always_returns),
            Self::NestedIf(plan) => plan.always_returns(),
            Self::AssignInt { .. } | Self::AssignBool { .. }
                | Self::AssignCopyStructInt { .. } | Self::AssignCopyStructBool { .. }
                | Self::AssignCopyStructStr { .. }
                | Self::AssignStr { .. } | Self::DeclareInt { .. }
                | Self::DeclareBool { .. } | Self::DeclareStr { .. }
                | Self::DeclareEmptyStr { .. } | Self::DeclareCopyStruct { .. }
                | Self::MoveCopyStruct { .. } | Self::DeclareEmptyInt { .. } | Self::DeclareEmptyBool { .. }
                | Self::While { .. } | Self::ForRange { .. }
                | Self::Break | Self::Continue => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarEarlyIf {
    condition: PrivateScalarBoolExpr,
    when_true: PrivateScalarBranchAction,
    when_false: Option<PrivateScalarEarlyElse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarEarlyElse {
    Branch(PrivateScalarBranchAction),
    ElseIf(Box<PrivateScalarEarlyIf>),
}

impl PrivateScalarEarlyIf {
    fn always_returns(&self) -> bool {
        self.when_true.always_returns()
            && match &self.when_false {
                Some(PrivateScalarEarlyElse::Branch(action)) =>
                    action.always_returns(),
                Some(PrivateScalarEarlyElse::ElseIf(next)) => next.always_returns(),
                _ => false,
            }
    }

    fn collect_calls<'a>(&'a self, out: &mut Vec<&'a str>) {
        self.condition.collect_calls(out);
        self.when_true.collect_calls(out);
        if let Some(otherwise) = &self.when_false {
            match otherwise {
                PrivateScalarEarlyElse::Branch(value) => value.collect_calls(out),
                PrivateScalarEarlyElse::ElseIf(next) => next.collect_calls(out),
            }
        }
    }
}

/// V1／F4：原 Checker 已驗證的具名純值來源，在真動態
/// 函式工作到達建構點時才由 DeclarationId 讀取最新值。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarCopyStructSeed {
    Static(ApprovedSeed),
    IntBound(DeclarationId),
    BoolBound(DeclarationId),
    /// 原 Checker 已核准的 bool 括號工作，仍於真正建構點讀取動態值。
    BoolGrouped(PrivateScalarBoolExpr),
    /// 具名純 int 欄位支援原 Checker TypedHIR 的動態加減／括號，
    /// 僅限無 Call 的純值表達式，不能於建構前重播靜態字面值。
    IntArithmetic(Box<PrivateScalarReturnExpr>),
    /// 具名 int RHS 以原 Checker 已編譯的普通函式顯式工作圖求值，
    /// 一律等返回且完成借用／資源退出後才交付 P16 真建構。
    IntCall(Box<PrivateScalarReturnExpr>),
    IntField { declaration: DeclarationId, owner: String, field: String },
    BoolField { declaration: DeclarationId, owner: String, field: String },
}
impl PrivateScalarCopyStructSeed {
    fn kind(&self) -> crate::mixed_struct_runtime::ScalarKind {
        use crate::mixed_struct_runtime::ScalarKind;
        match self {
            Self::Static(value) => value.kind(),
            Self::IntBound(_) | Self::IntArithmetic(_) | Self::IntCall(_)
                | Self::IntField { .. } => ScalarKind::Int,
            Self::BoolBound(_) | Self::BoolGrouped(_) | Self::BoolField { .. } =>
                ScalarKind::Bool,
        }
    }
    fn evaluate(
        &self, frame: &PrivateScalarActiveFrame,
        structs: &crate::mixed_struct_runtime::MixedStructRuntime,
        steps: &mut usize,
        max_steps: usize,
    ) -> Result<ApprovedSeed, PrivateScalarCallRuntimeError> {
        match self {
            Self::Static(value) => Ok(value.clone()),
            Self::IntBound(id) => frame.values.get(id).copied()
                .map(ApprovedSeed::Int)
                .ok_or(PrivateScalarCallRuntimeError::StateMismatch),
            Self::BoolBound(id) => frame.bool_values.get(id).copied()
                .map(ApprovedSeed::Bool)
                .ok_or(PrivateScalarCallRuntimeError::StateMismatch),
            Self::BoolGrouped(expr) => expr.evaluate_runtime(frame, structs)
                .map(ApprovedSeed::Bool),
            Self::IntCall(_) => Err(PrivateScalarCallRuntimeError::StateMismatch),
            Self::IntArithmetic(expr) => {
                // 此局部純值 evaluator 不另建普通函式呼叫框架。
                // 函式 Call 已在 compile 階段保守拒絕；依原 Checker
                // 編譯的算術樹以顯式工作清單求值，避免 Rust 遞迴
                // 並對每一個動態算式節點計入原 D18 StepBudget。
                enum Work<'a> {
                    Read(&'a PrivateScalarReturnExpr),
                    Apply(char),
                }
                let mut work = vec![Work::Read(expr.as_ref())];
                let mut values = Vec::<i64>::new();
                while let Some(item) = work.pop() {
                    if *steps >= max_steps {
                        return Err(PrivateScalarCallRuntimeError::StepBudget);
                    }
                    *steps += 1;
                    match item {
                        Work::Read(PrivateScalarReturnExpr::Parameter(id)) => {
                            values.push(frame.values.get(id).copied().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?);
                        }
                        Work::Read(PrivateScalarReturnExpr::Constant(value)) =>
                            values.push(*value),
                        Work::Read(PrivateScalarReturnExpr::CopyStructIntField {
                            declaration, owner, field,
                        }) => {
                            if !frame.copy_struct_scopes.iter().rev().flatten()
                                .any(|(id, known)| id == declaration && known == owner) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            let observed = structs.field(owner, field).map_err(|_|
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let crate::mixed_struct_runtime::Observed::Int(value) = observed
                            else {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            };
                            values.push(value);
                        }
                        Work::Read(PrivateScalarReturnExpr::Addition(left, right)) => {
                            work.push(Work::Apply('+'));
                            work.push(Work::Read(right));
                            work.push(Work::Read(left));
                        }
                        Work::Read(PrivateScalarReturnExpr::Subtraction(left, right)) => {
                            work.push(Work::Apply('-'));
                            work.push(Work::Read(right));
                            work.push(Work::Read(left));
                        }
                        Work::Apply(operator) => {
                            let right = values.pop().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let left = values.pop().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let result = match operator {
                                '+' => left.checked_add(right),
                                '-' => left.checked_sub(right),
                                _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                            }.ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)?;
                            values.push(result);
                        }
                        // 僅原 Checker int 欄位種子可進此純整數工作棧；
                        // 不讓已核准 bool 返回以 0/1 偽裝 int 欄位值。
                        Work::Read(PrivateScalarReturnExpr::SelfRead(_) | PrivateScalarReturnExpr::Call { .. }
                            | PrivateScalarReturnExpr::BoolValue(..)
                            | PrivateScalarReturnExpr::StrValue(..)
                            | PrivateScalarReturnExpr::StrCall { .. }
                            | PrivateScalarReturnExpr::Unreachable) =>
                            return Err(PrivateScalarCallRuntimeError::StateMismatch),
                    }
                }
                let [value] = values.as_slice() else {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                };
                Ok(ApprovedSeed::Int(*value))
            }
            Self::IntField { declaration, owner, field } => {
                if !frame.copy_struct_scopes.iter().rev().flatten()
                    .any(|(id, known)| id == declaration && known == owner) {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                }
                match structs.field(owner, field).map_err(|_|
                    PrivateScalarCallRuntimeError::StateMismatch,
                )? {
                    crate::mixed_struct_runtime::Observed::Int(value) =>
                        Ok(ApprovedSeed::Int(value)),
                    _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
                }
            }
            Self::BoolField { declaration, owner, field } => {
                if !frame.copy_struct_scopes.iter().rev().flatten()
                    .any(|(id, known)| id == declaration && known == owner) {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                }
                match structs.field(owner, field).map_err(|_|
                    PrivateScalarCallRuntimeError::StateMismatch,
                )? {
                    crate::mixed_struct_runtime::Observed::Bool(value) =>
                        Ok(ApprovedSeed::Bool(value)),
                    _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
                }
            }
        }
    }

    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        if let Self::IntCall(expr) = self {
            expr.collect_calls(calls);
        }
    }
}

/// V1／F4：原 Checker 唯一結構型別及 P16 VB16 預設／具名欄位
/// 來源工作計畫；只接通無 str 資源的 int／bool 欄位。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarCopyStructPlan {
    fields: Vec<(String, crate::mixed_struct_runtime::ScalarKind, Option<ApprovedSeed>, bool)>,
    named: Vec<(String, PrivateScalarCopyStructSeed)>,
}
impl PrivateScalarCopyStructPlan {
    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        for (_, source) in &self.named {
            source.collect_calls(calls);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarLocalStep {
    Declare(PrivateScalarLocalInit),
    DeclareEmptyInt(DeclarationId),
    DeclareEmptyBool(DeclarationId),
    Assign { declaration: DeclarationId, value: PrivateScalarReturnExpr },
    DeclareBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    DeclareStr { declaration: DeclarationId, name: String, mutable: bool, initializer: PrivateScalarStrInitializer },
    DeclareEmptyStr { declaration: DeclarationId, name: String, mutable: bool },
    DeclareCopyStruct {
        declaration: DeclarationId, name: String, plan: PrivateScalarCopyStructPlan,
    },
    MoveCopyStruct {
        declaration: DeclarationId, name: String,
        source: DeclarationId, source_name: String,
    },
    AssignBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    AssignStr { declaration: DeclarationId, name: String, initializer: PrivateScalarStrInitializer },
    AssignCopyStructInt {
        declaration: DeclarationId, owner: String, field: String,
        value: PrivateScalarReturnExpr,
    },
    AssignCopyStructBool {
        declaration: DeclarationId, owner: String, field: String,
        value: PrivateScalarBoolExpr,
    },
    AssignCopyStructStr {
        declaration: DeclarationId, owner: String, field: String, value: String,
    },
    EarlyReturn(PrivateScalarEarlyIf),
    Control(PrivateScalarBranchAction),
}

impl PrivateScalarLocalStep {
    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Declare(init) => init.value.collect_calls(calls),
            Self::Assign { value, .. }
                | Self::AssignCopyStructInt { value, .. } => value.collect_calls(calls),
            Self::EarlyReturn(plan) => plan.collect_calls(calls),
            Self::Control(action) => action.collect_calls(calls),
            Self::DeclareCopyStruct { plan, .. } => plan.collect_calls(calls),
            Self::DeclareBool { value, .. } | Self::AssignBool { value, .. } =>
                value.collect_calls(calls),
            Self::DeclareStr { initializer, .. } | Self::AssignStr { initializer, .. } =>
                initializer.collect_calls(calls),
            Self::AssignCopyStructBool { .. } | Self::AssignCopyStructStr { .. }
                | Self::DeclareEmptyInt(_) | Self::DeclareEmptyBool(_)
                | Self::DeclareEmptyStr { .. } | Self::MoveCopyStruct { .. } => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarNestedCallee {
    name: String,
    span: crate::common_ir::NodeSpan,
    parameters: Vec<DeclarationId>,
    parameter_is_bool: Vec<bool>,
    parameter_is_str: Vec<bool>,
    result_span: crate::common_ir::NodeSpan,
    result: PrivateScalarReturnExpr,
    local_inits: Vec<PrivateScalarLocalInit>,
    local_steps: Vec<PrivateScalarLocalStep>,
}

struct PreparedPrivateScalarCall {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    call_span: crate::common_ir::NodeSpan,
    callee_span: crate::common_ir::NodeSpan,
    callee_name: String,
    parameter_name: Option<String>,
    parameter_id: Option<DeclarationId>,
    parameter_return_span: crate::common_ir::NodeSpan,
    return_expr: PrivateScalarReturnExpr,
    root_local_inits: Vec<PrivateScalarLocalInit>,
    root_local_steps: Vec<PrivateScalarLocalStep>,
    owner_id: Option<DeclarationId>,
    argument: Option<i64>,
    /// 其餘參數從同一原 Checker 函式簽章依宣告順序綁定。
    additional_arguments: Vec<(DeclarationId, i64, Option<DeclarationId>)>,
    /// 已在主函式真正按順序建立的 Copyable int 詞彙持有者。
    main_owners: Vec<(DeclarationId, i64)>,
    /// 同一完整來源中原 Parser／Checker 已驗證的其餘參數函式。
    nested_callees: Vec<PrivateScalarNestedCallee>,
    /// 真 main 實參中含原 Checker int 算式／巢狀呼叫時，
    /// 共用顯式工作堆疊執行，不能先預算出假 int 結果。
    main_dynamic_call: Option<PrivateScalarReturnExpr>,
    /// main 已核准動態 int 初始化須在執行時逐筆求值，不預算函式結果。
    main_dynamic_inits: Vec<PrivateScalarLocalInit>,
    main_dynamic_steps: Vec<PrivateScalarLocalStep>,
    /// 被 main 呼叫的完整函式宣告身分，供正常及遞迴路徑使用。
    callee_parameter_ids: Vec<DeclarationId>,
    callee_parameter_is_bool: Vec<bool>,
    callee_parameter_is_str: Vec<bool>,
}

/// G18／D18-01：每次呼叫的執行環境配置，不是 voiles 的語言上限。
#[derive(Debug, Clone, Copy)]
struct PrivateScalarExecutionBudget {
    max_call_frames: usize,
    max_steps: usize,
}

impl Default for PrivateScalarExecutionBudget {
    fn default() -> Self {
        Self { max_call_frames: 128, max_steps: 100_000 }
    }
}

#[derive(Debug, Clone, Copy)]
enum PrivateScalarLocalBinding {
    Int(DeclarationId),
    Bool(DeclarationId),
    UninitializedInt(DeclarationId),
    UninitializedBool(DeclarationId),
    CopyStruct(DeclarationId),
}

struct PrivateScalarActiveFrame {
    name: String,
    values: BTreeMap<DeclarationId, i64>,
    bool_values: BTreeMap<DeclarationId, bool>,
    // 按實際寫入順序保留 int／bool，退出按逆序記錄離域。
    initialized_order: Vec<PrivateScalarLocalBinding>,
    /// 同一 OwnedStrRuntime 已進入的本函式資源詞彙作用域數。
    resource_scopes: usize,
    /// 每一詞彙層原 Checker 的真 str 宣告身分。
    str_owner_scopes: Vec<Vec<(DeclarationId, String)>>,
    /// 由 P16 MixedStructRuntime 持有真正結構值，本表只對帳來源身分。
    copy_struct_scopes: Vec<Vec<(DeclarationId, String)>>,
    borrow_tracker: crate::borrow_safety::BorrowSafety,
    incoming_borrows: Vec<(crate::borrow_safety::BorrowTicket, BorrowFact)>,
    /// 每個 frame 真 Detached 返回值尚未接管的唯一 ResourceId（LIFO）。
    pending_owned_returns: Vec<u64>,
}

/// 各已完成實參維持正式型別，不以整數佔位冒充字串。
#[derive(Clone, Copy)]
enum PrivateScalarEvaluatedArgument {
    Int(i64),
    Bool(bool),
    Borrow {
        ticket: crate::borrow_safety::BorrowTicket,
        fact: BorrowFact,
    },
    Move { source: DeclarationId },
    TemporaryStr { resource_id: u64 },
}

/// 不以 Rust 遞迴執行具名函式；每個原 Checker expression 在工作堆疊
/// 上逐項求值，呼叫時由動態框架保存真正參數值。
enum PrivateScalarTask<'a> {
    InvokeSelf(&'a PrivateScalarSelfRead),
    EvaluateMethod(&'a PrivateScalarSelfRead),
    Evaluate(&'a PrivateScalarReturnExpr, bool),
    /// 每個真實參值求值完成當下才產生事件，不在 EnterCall 補記。
    RecordArgument,
    EvaluateBoolArgument(&'a PrivateScalarBoolExpr),
    RecordBoolArgument,
    RecordStrArgument,
    CreateStrLiteralArgument(&'a str),
    BorrowArgument(&'a str, BorrowFact),
    MoveArgument(&'a str, DeclarationId),
    Apply(char),
    BindLocal(DeclarationId),
    DeclareEmptyInt(DeclarationId),
    DeclareEmptyBool(DeclarationId),
    AssignLocal(DeclarationId),
    WriteCopyStructInt(DeclarationId, &'a str, &'a str),
    WriteCopyStructBool(DeclarationId, &'a str, &'a str, Option<&'a PrivateScalarBoolExpr>),
    WriteCopyStructStr(DeclarationId, &'a str, &'a str, &'a str),
    BindBool(DeclarationId, &'a PrivateScalarBoolExpr),
    /// F2：bool 動態呼叫真正返回後才可寫入局部；尚未返回不可提前初始化。
    BindBoolFromValue(DeclarationId),
    AssignBoolFromValue(DeclarationId),
    DeclareStr(DeclarationId, &'a str, bool, &'a PrivateScalarStrInitializer),
    /// str 函式真正退出後由呼叫端用原 Checker const/var 權限接管唯一 OwnedUtf8。
    FinishStrReturnBinding(DeclarationId, &'a str, bool),
    /// F2：僅原 Checker 合法的未初始化 var str 首次接收 Returned OwnedUtf8。
    FinishStrReturnAssignment(DeclarationId, &'a str),
    /// 內層 str-return 已真退出；保留同一 OwnedUtf8，從當前外層
    /// 函式再發出真 Returned(id)，交由原 ExitCall 驗證唯一資源。
    ForwardStrReturn,
    DeclareEmptyStr(DeclarationId, &'a str, bool),
    DeclareCopyStruct(DeclarationId, &'a str, &'a PrivateScalarCopyStructPlan),
    /// 具名欄位來源逐項在當前動態 frame 求值；含 Call 使用與 return
    /// 相同的 EnterCall／ExitCall 堆疊，無隱藏宿主遞迴。
    CaptureCopyStructSeed(&'a str, &'a PrivateScalarCopyStructSeed),
    FinishCopyStruct(DeclarationId, &'a str, &'a PrivateScalarCopyStructPlan),
    MoveCopyStruct(DeclarationId, &'a str, DeclarationId, &'a str),
    AssignStr(DeclarationId, &'a str, &'a PrivateScalarStrInitializer),
    AssignBool(DeclarationId, &'a PrivateScalarBoolExpr),
    CheckEarlyReturn(&'a PrivateScalarEarlyIf),
    /// 若條件包含原 Checker 准許的 bool 函式，待真正 BoolReturned 後才選分支。
    FinishEarlyCondition(&'a PrivateScalarEarlyIf),
    /// 循環的可識別反向邊亦作為 break／continue 的最近動態目標。
    CheckWhile(&'a PrivateScalarBoolExpr, &'a PrivateScalarBranchAction),
    /// 本輪迭代器僅在所屬來源區塊有效，離域後可用同一 ID 進下一輪。
    CheckFor(&'a PrivateScalarBranchAction, DeclarationId, i64, i64),
    EndFor(&'a PrivateScalarBranchAction, DeclarationId, i64, i64),
    UnbindIterator(DeclarationId),
    EndScope(usize),
    ExecuteBranchAction(&'a PrivateScalarBranchAction),
    /// 第三欄是原 main 根返回、第四欄是已核准 async await 身分。
    EnterCall(&'a str, &'a [PrivateScalarCallArgument], bool, bool),
    ExitCall,
}

/// V7／E2：原私有執行器真正動態工作堆疊的跨 Rust 呼叫保存物件。
/// 全部來自原 Checker，持有同一 P16 OwnedStrRuntime／BorrowSafety 真身分。
/// 帶來源生命週期的 task 參考只借用既有 PreparedPrivateScalarCall，
/// 不建立自我參照的佇列、重新解析 AST 或轉移至其他執行緒。
struct PrivateScalarGraphCheckpoint<'a> {
    resources: crate::owned_str_runtime::OwnedStrRuntime,
    copy_structs: crate::mixed_struct_runtime::MixedStructRuntime,
    resource_cursor: usize,
    frames: Vec<PrivateScalarActiveFrame>,
    tasks: Vec<PrivateScalarTask<'a>>,
    values: Vec<i64>,
    /// F2：型別化返回所有權，不可轉成假 i64 ResourceId。
    pending_str_returns: Vec<crate::owned_str_runtime::OwnedUtf8>,
    completed_arguments: Vec<PrivateScalarEvaluatedArgument>,
    pending_copy_structs: Vec<Vec<(String, ApprovedSeed)>>,
    steps: usize,
    awaiting_name: String,
}

impl PreparedPrivateScalarCall {
    /// V7／E5：必須由真動態 BorrowSafety 與仍被父呼叫持有的
    /// imported BorrowFact 同時證明空集合，才准啟動已核准 await。
    /// 此守門本身不簽票據或產生虛假的 ResourceId。
    fn check_await_safe_point(
        tracker: &crate::borrow_safety::BorrowSafety,
        imported: &[(crate::borrow_safety::BorrowTicket, BorrowFact)],
    ) -> Result<(), PrivateScalarCallRuntimeError> {
        if !imported.is_empty()
            || tracker.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            ));
        }
        Ok(())
    }

    /// V7／E5：呼叫圖可能跨多個同步框架；只驗證「最內層 caller」
    /// 不能證明外層仍有效的借用已合法歸還。每一個真框架都要以
    /// 同一原 BorrowSafety／incoming BorrowFact 檢查，不自行建立票據。
    fn check_await_safe_point_chain<'a>(
        frames: impl IntoIterator<Item = (
            &'a crate::borrow_safety::BorrowSafety,
            &'a [(crate::borrow_safety::BorrowTicket, BorrowFact)],
        )>,
    ) -> Result<(), PrivateScalarCallRuntimeError> {
        let mut any_frame = false;
        for (tracker, imported) in frames {
            any_frame = true;
            Self::check_await_safe_point(tracker, imported)?;
        }
        if !any_frame {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
        Ok(())
    }

    /// 只抄錄已由真 OwnedStrRuntime 完成的 ResourceId 事件，
    /// 各個動態函式退出、局部離域、故障回滾均與 Copyable 事件同序。
    /// 呼叫期間的 BorrowTicket 屬直接呼叫者 tracker；真正被呼叫端離域時
    /// 先核對完整 Checker 借用快照並原子歸還，才可釋放任何 str 所有者。
    fn return_incoming_borrows(
        parent: &mut PrivateScalarActiveFrame,
        incoming: &[(crate::borrow_safety::BorrowTicket, BorrowFact)],
        events: &mut Vec<PrivateScalarCallEvent>,
    ) -> Result<(), PrivateScalarCallRuntimeError> {
        if incoming.is_empty() { return Ok(()); }
        let tickets = incoming.iter().map(|(ticket, _)| *ticket).collect::<Vec<_>>();
        let facts = incoming.iter().map(|(_, fact)| *fact).collect::<Vec<_>>();
        // 巢狀實參中外層 BorrowTicket 仍屬呼叫者的活躍責任；
        // 內層提前 Returned 時只能釋放它自己的 incoming 子集。
        // 由獨立的原 Checker BorrowFact／真 Ticket 一一核對後原子提交。
        parent.borrow_tracker.finish_selected_verified(&tickets, &facts)
            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
        for ticket in tickets.into_iter().rev() {
            events.push(PrivateScalarCallEvent::LoanReturned(ticket));
        }
        Ok(())
    }

    /// R05：錯誤回滾依目前最內層 frame 逐層歸還尚未真正
    /// EnterCall 的實參票據。單一 completed_arguments 可能同時
    /// 保存外層尚未 Enter 的票據，與內層已 Enter 函式的票據；
    /// 禁止在內層退出前先釋放外層來源，否則逆序清理會倒置。
    /// 僅處理本 frame 追蹤器簽發的票據，不改動其他活躍 frame。
    fn return_pending_borrows_in_frame(
        frame: &mut PrivateScalarActiveFrame,
        pending: &mut Vec<crate::borrow_safety::BorrowTicket>,
        events: &mut Vec<PrivateScalarCallEvent>,
    ) -> Result<(), PrivateScalarCallRuntimeError> {
        let mut index = 0;
        while index < pending.len() {
            let ticket = pending[index];
            if frame.borrow_tracker.check_ticket_access(
                ticket, BorrowKind::SharedRead,
            ).is_ok() {
                frame.borrow_tracker.release(ticket)
                    .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                events.push(PrivateScalarCallEvent::LoanReturned(ticket));
                pending.remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn record_owned_resources(
        resources: &crate::owned_str_runtime::OwnedStrRuntime,
        cursor: &mut usize,
        events: &mut Vec<PrivateScalarCallEvent>,
    ) {
        let current = resources.events();
        for event in current.iter().skip(*cursor) {
            events.push(PrivateScalarCallEvent::OwnedResource(event.clone()));
        }
        *cursor = current.len();
    }

    /// 在單一詞彙層同時讓 scalar str 與含 str 的 struct
    /// 交錯完成初始化，現有兩種局部離域序列尚不能證明
    /// 跨類別的全域逆序；第一批僅接通不交錯的可證子集。
    fn current_scope_has_str_struct(
        frame: &PrivateScalarActiveFrame,
        structs: &crate::mixed_struct_runtime::MixedStructRuntime,
    ) -> bool {
        frame.copy_struct_scopes.last().is_some_and(|scope| {
            scope.iter().any(|(_, name)|
                structs.has_str_resource(name).unwrap_or(true))
        })
    }

    /// F2-RS01/RS05：不得將非 Copyable str 返回偷偷映射成 int。
    /// 此處只形成原 Checker 准許的 str 常值或具名明確整筆 move；
    /// 所有權真正移出及失敗回收由動態 OwnedStrRuntime 負責。
    /// 只允許同一原 Checker str 返回型別的 Literal／Move／同步 Call。
    /// 既有同步 Call 編譯已精確驗證 Lexer 括號、函式完整簽章、
    /// 子呼叫多形參 TypedHIR 與 call_borrows，不新建所有權語意。
    fn checked_str_return_expr(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<PrivateScalarReturnExpr, PrivateScalarCallPrepareError> {
        if let Ok(returned) = Self::checked_str_return(term, bindings, verified) {
            return Ok(PrivateScalarReturnExpr::StrValue(returned));
        }
        match Self::checked_str_initializer(term, bindings, verified)? {
            PrivateScalarStrInitializer::Method(site) => Ok(PrivateScalarReturnExpr::SelfRead(site)),
            PrivateScalarStrInitializer::Call { name, arguments } =>
                Ok(PrivateScalarReturnExpr::StrCall { name, arguments }),
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "原 Checker str return 須為合法字面、明確 Move 或同步函式結果",
            )),
        }
    }

    fn checked_str_return(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<PrivateScalarStrReturn, PrivateScalarCallPrepareError> {
        if let TermKind::Group(inner) = &term.kind {
            let span = term.node_span();
            if !verified.tokens.iter().any(|token|
                token.start == span.start
                    && matches!(token.kind, TokenKind::Symbol('(')))
                || !verified.tokens.iter().any(|token|
                    token.end == span.end
                        && matches!(token.kind, TokenKind::Symbol(')'))) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "str 返回 Group 缺少原 Lexer 括號身份",
                ));
            }
            return Self::checked_str_return(inner, bindings, verified);
        }
        if let Ok(text) = private_str_literal(term, &verified.tokens) {
            return Ok(PrivateScalarStrReturn::Literal(text));
        }
        let source = PreparedPrivateMoveCall::checked_move_name(term, verified)
            .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                "str 返回只接受原 Checker 核准的字面或具名 .move()",
            ))?;
        let declaration = *bindings.get(source).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str 返回來源不在真 Checker 詞彙作用域",
            ),
        )?;
        if !verified.plan.declarations.typed_declaration(declaration)
            .is_some_and(|fact| fact.ty == "str") {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "str 返回缺少原 Checker DeclarationId 所有型別",
            ));
        }
        Ok(PrivateScalarStrReturn::Move {
            source: source.to_owned(), declaration,
        })
    }

    /// 唯有原 Checker 合法所有者來源，才可進真 OwnedStrRuntime；
    /// 不能透過文字相同就虛構所有權或將 str 視為 Copyable。
    /// 所有型別實參共用唯一左至右動態求值排程；NestedStrCall
    /// 必須等自己的真 ExitCall 後才 RecordStrArgument，不得預先建立
    /// 字串或透過宿主函式遞迴偷跑。
    fn queue_call_arguments<'a>(
        tasks: &mut Vec<PrivateScalarTask<'a>>,
        arguments: &'a [PrivateScalarCallArgument],
    ) {
        for argument in arguments.iter().rev() {
            match argument {
                PrivateScalarCallArgument::Int(expr) => {
                    tasks.push(PrivateScalarTask::RecordArgument);
                    tasks.push(PrivateScalarTask::Evaluate(expr, false));
                }
                PrivateScalarCallArgument::Bool(expr) => {
                    tasks.push(PrivateScalarTask::RecordBoolArgument);
                    tasks.push(PrivateScalarTask::EvaluateBoolArgument(expr));
                }
                PrivateScalarCallArgument::BorrowStr { owner, fact } =>
                    tasks.push(PrivateScalarTask::BorrowArgument(owner, *fact)),
                PrivateScalarCallArgument::MoveStr { owner, source } =>
                    tasks.push(PrivateScalarTask::MoveArgument(owner, *source)),
                PrivateScalarCallArgument::LiteralStr(text) =>
                    tasks.push(PrivateScalarTask::CreateStrLiteralArgument(text)),
                PrivateScalarCallArgument::NestedStrCall { name, arguments } => {
                    tasks.push(PrivateScalarTask::RecordStrArgument);
                    tasks.push(PrivateScalarTask::EnterCall(
                        name, arguments, false, false,
                    ));
                    Self::queue_call_arguments(tasks, arguments);
                }
            }
        }
    }

    /// 此來源只允許原 Checker 已核准、實際執行後 Returned 的同步 str 呼叫。
    /// 不將字面常值、非 Copyable 具名借用或其他形式偽裝成臨時所有者。
    fn checked_temporary_str_argument(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<PrivateScalarCallArgument, PrivateScalarCallPrepareError> {
        match Self::checked_str_initializer(term, bindings, verified)? {
            PrivateScalarStrInitializer::Call { name, arguments } =>
                Ok(PrivateScalarCallArgument::NestedStrCall { name, arguments }),
            PrivateScalarStrInitializer::Literal(text) =>
                Ok(PrivateScalarCallArgument::LiteralStr(text)),
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "F2 str 暫存形參僅接原 Lexer 常值或 Checker 同步 str-return",
            )),
        }
    }

    fn checked_str_initializer(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<PrivateScalarStrInitializer, PrivateScalarCallPrepareError> {
        if let Ok(text) = private_str_literal(term, &verified.tokens) {
            return Ok(PrivateScalarStrInitializer::Literal(text));
        }
        // F2：同步 str 擁有返回可使用原 Checker 已核准的 int/bool/str
        // 多個混合實參。所有 BorrowFact 對帳完畢才進真工作圖；
        // 此處只生成原始安全計畫，絕不先移動或預先借用資源。
        let mut call_term = term;
        while let TermKind::Group(inner) = &call_term.kind {
            let span = call_term.node_span();
            if !verified.tokens.iter().any(|t|
                t.start == span.start && matches!(t.kind, TokenKind::Symbol('(')))
                || !verified.tokens.iter().any(|t|
                    t.end == span.end && matches!(t.kind, TokenKind::Symbol(')'))) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "str Call Group 缺少原 Lexer 括號",
                ));
            }
            call_term = inner;
        }
        if let TermKind::MethodCall(owner, name, _) | TermKind::AwaitMethodCall(owner, name, _) = &call_term.kind {
            if let Some(site) = bindings.get(owner).and_then(|id|
                PrivateScalarSelfRead::checked(call_term.node_span(), owner, name, *id, verified)).filter(|site| site.result_type == "str") {
                return Ok(PrivateScalarStrInitializer::Method(site));
            }
        }
        let (call_name, actuals): (&str, &[Term]) = match &call_term.kind {
            TermKind::Call(call) if !call.awaited =>
                (call.name.as_str(), &call.arguments),
            TermKind::UnresolvedZeroArgs(name)
                if !verified.structs.contains_key(name) => (name.as_str(), &[]),
            _ => ("", &[]),
        };
        if !call_name.is_empty() {
            let target = verified.functions.iter().find(|fun| fun.name == call_name
                && fun.name != "main" && fun.result_ty == "str" && !fun.is_async
                && fun.params.len() == actuals.len()
                && fun.params.iter().all(|param|
                    matches!(param.ty.as_str(), "int" | "bool" | "str")))
                .ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "str Call 缺少原 Checker 同步 str 返回與完整形參簽章",
                ))?;
            let mut arguments = Vec::with_capacity(actuals.len());
            let mut observed_borrows = Vec::new();
            for (argument, parameter) in actuals.iter().zip(&target.params) {
                let planned = match parameter.ty.as_str() {
                    "int" => PrivateScalarCallArgument::Int(
                        PrivateScalarReturnExpr::from_checked_bindings(
                            argument, bindings, verified,
                        )?,
                    ),
                    "bool" => PrivateScalarCallArgument::Bool(
                        PrivateScalarBoolExpr::from_checked_call_argument(
                            argument, bindings, verified,
                        )?,
                    ),
                    "str" if matches!(argument.kind, TermKind::Move(_)) => {
                        let owner = PreparedPrivateMoveCall::checked_move_name(
                            argument, verified,
                        ).map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                            "str 返回 Move 引數缺少原 Parser/Lexer 身分",
                        ))?;
                        let source = *bindings.get(owner).ok_or(
                            PrivateScalarCallPrepareError::Unsupported(
                                "str 返回 Move 引數缺少當前 DeclarationId",
                            ),
                        )?;
                        if !verified.plan.declarations.typed_declaration(source)
                            .is_some_and(|fact| fact.ty == "str") {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "str 返回 Move 引數缺少原 Checker str 身分",
                            ));
                        }
                        PrivateScalarCallArgument::MoveStr {
                            owner: owner.to_owned(), source,
                        }
                    }
                    "str" if !matches!(&argument.kind, TermKind::Name(_)) =>
                        Self::checked_temporary_str_argument(argument, bindings, verified)?,
                    "str" => {
                        let TermKind::Name(owner) = &argument.kind else {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "str 返回借用只能使用原 Parser 具名來源",
                            ));
                        };
                        let source = *bindings.get(owner).ok_or(
                            PrivateScalarCallPrepareError::Unsupported(
                                "str 返回借用缺少目前 Checker 作用域身分",
                            ),
                        )?;
                        let record = verified.plan.declarations.typed_declaration(source)
                            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                                "str 返回借用缺少原 Checker 宣告事實",
                            ))?;
                        if record.ty != "str" || !verified.tokens.iter().any(|token|
                            token.start == argument.start && token.end == argument.end
                                && matches!(&token.kind,
                                    TokenKind::Name(found) if found == owner)) {
                            return Err(PrivateScalarCallPrepareError::Unsupported(
                                "str 返回借用缺少原 Checker/Lexer 型別與來源",
                            ));
                        }
                        let fact = BorrowFact::new(source, if record.mutable {
                            BorrowKind::ExclusiveWrite
                        } else {
                            BorrowKind::SharedRead
                        });
                        observed_borrows.push(fact);
                        PrivateScalarCallArgument::BorrowStr {
                            owner: owner.clone(), fact,
                        }
                    }
                    _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                        "str 返回引數含未核准型別",
                    )),
                };
                arguments.push(planned);
            }
            if verified.plan.declarations.call_borrows_at(call_term.node_span())
                .unwrap_or(&[]) != observed_borrows.as_slice() {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "str 返回引數 BorrowFact 與原 Checker 不一致",
                ));
            }
            return Ok(PrivateScalarStrInitializer::Call {
                name: call_name.to_owned(), arguments,
            });
        }
        let source = PreparedPrivateMoveCall::checked_move_name(term, verified)
            .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                "str 局部所有權只接受原 Lexer 常值或真 .move()",
            ))?;
        let declaration = *bindings.get(source).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str .move() 來源不在本 Checker 已核准詞彙作用域",
            ),
        )?;
        if !verified.plan.declarations.typed_declaration(declaration)
            .is_some_and(|fact| fact.ty == "str") {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "str .move() 來源缺少原 Checker str DeclarationId",
            ));
        }
        Ok(PrivateScalarStrInitializer::Move {
            source: source.to_owned(), declaration,
        })
    }

    /// V1／F4：交付原 Checker 已接受、P16 可真建構的 int／bool
    /// 與受限 str 結構局部；原 VB16 具名覆蓋及預設來源各自核對。
    /// 已經有原 Checker 核准之真字面預設的 var str，
    /// 與已核准具名覆蓋及 const str 都在 P16 共用帳上執行；
    /// 缺少預設的 var str／巢狀未知資源仍維持原保守拒絕。
    fn checked_copy_struct(
        name: &str, ty: &str, term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<PrivateScalarCopyStructPlan, PrivateScalarCallPrepareError> {
        use crate::mixed_struct_runtime::ScalarKind;
        let definition = verified.structs.get(ty).ok_or(
            PrivateScalarCallPrepareError::Unsupported("結構局部缺少原 Checker 型別"),
        )?;
        let named: &[(String, Term)] = match &term.kind {
            TermKind::Construct(found, supplied) if found == ty => supplied,
            TermKind::UnresolvedZeroArgs(found) if found == ty => &[],
            _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                "結構局部必須是原 Parser／Checker 真建立式",
            )),
        };
        let mut fields = Vec::new();
        for field in &definition.fields {
            let overridden = named.iter().any(|(key, _)| key == &field.name);
            let kind = match field.ty.as_str() {
                "int" => ScalarKind::Int,
                "bool" => ScalarKind::Bool,
                // V1／F4：原 P16 VB16 已能在同一 OwnedStrRuntime
                // 依具名先、未覆蓋預設後的順序建立真正 UTF-8。
                // var str 的存在且已核准預設和 const str 使用完全
                // 相同的 Shared Ledger；不以 mutable 與否猜資源身分。
                // 無預設又未具名初始化的可寫欄位仍待部分初始化驗收。
                "str" if field.immutable || overridden || field.default.is_some() =>
                    ScalarKind::Str,
                "str" => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "結構 var str 缺少已核准預設／具名來源，部分初始化資源待驗收",
                )),
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀資源尚未接入原 Checker／P16 共同退出帳",
                )),
            };
            let default = if overridden {
                field.default.as_ref().map(|_| ApprovedSeed::Skipped(kind))
            } else {
                field.default.as_ref().map(|seed| approved_seed(seed, verified)
                    .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                        "結構局部預設值缺少原 P16 Checker 核准來源",
                    ))).transpose()?
            };
            if default.as_ref().is_some_and(|seed| seed.kind() != kind) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "結構局部預設值和原 Checker 型別不一致",
                ));
            }
            fields.push((field.name.clone(), kind, default, field.immutable));
        }
        let mut supplied = Vec::new();
        for (field_name, term) in named {
            let (_, kind, _, _) = fields.iter().find(|(key, _, _, _)| key == field_name)
                .ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "結構局部具名欄位不在原 Checker 型別中",
                ))?;
            let seed = match &term.kind {
                TermKind::Name(source_name) => {
                    let declaration = *bindings.get(source_name).ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "結構具名來源不在原 Checker 已編譯詞彙作用域",
                        ),
                    )?;
                    let span = term.node_span();
                    let expected = match kind {
                        ScalarKind::Int => crate::common_ir::ScalarType::Int,
                        ScalarKind::Bool => crate::common_ir::ScalarType::Bool,
                        _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                            "非 Copyable 資源欄位不屬此子集",
                        )),
                    };
                    let ty_text = if *kind == ScalarKind::Int { "int" } else { "bool" };
                    if !verified.plan.declarations.typed_declaration(declaration)
                        .is_some_and(|fact| fact.ty == ty_text)
                        || verified.plan.scalar_hir.expression(span)
                            .is_none_or(|fact| fact.ty != expected)
                        || verified.plan.scalar_hir.reference(span)
                            .is_none_or(|fact|
                                fact.resolved.declaration != declaration
                                    || fact.resolved.ty != expected)
                        || !verified.tokens.iter().any(|token|
                            token.start == term.start && token.end == term.end
                                && matches!(&token.kind, TokenKind::Name(found)
                                    if found == source_name)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "結構具名來源缺少 Checker TypedHIR／DeclarationId／Lexer",
                        ));
                    }
                    if *kind == ScalarKind::Int {
                        PrivateScalarCopyStructSeed::IntBound(declaration)
                    } else {
                        PrivateScalarCopyStructSeed::BoolBound(declaration)
                    }
                }
                TermKind::FieldRead(_, _) if *kind == ScalarKind::Int => {
                    let result = PrivateScalarReturnExpr::from_checked_bindings(
                        term, bindings, verified,
                    )?;
                    let PrivateScalarReturnExpr::CopyStructIntField {
                        declaration, owner, field,
                    } = result else {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "結構具名 int 欄位缺少原 Checker 唯一純值投影",
                        ));
                    };
                    PrivateScalarCopyStructSeed::IntField {
                        declaration, owner, field,
                    }
                }
                TermKind::FieldRead(_, _) if *kind == ScalarKind::Bool => {
                    let result = PrivateScalarBoolExpr::from_checked(
                        term, bindings, verified,
                    )?;
                    let PrivateScalarBoolExpr::CopyStructBoolField {
                        declaration, owner, field,
                    } = result else {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "結構具名 bool 欄位缺少原 Checker 唯一純值投影",
                        ));
                    };
                    PrivateScalarCopyStructSeed::BoolField {
                        declaration, owner, field,
                    }
                }
                // bool Group 的原 Checker TypedHIR、原 Lexer 括號及
                // 內層 DeclarationId 已由同一 bool 編譯器逐層驗證；
                // 真值直到建構點才求，不能轉成編譯時常數。
                TermKind::Group(_) if *kind == ScalarKind::Bool => {
                    PrivateScalarCopyStructSeed::BoolGrouped(
                        PrivateScalarBoolExpr::from_checked(term, bindings, verified)?,
                    )
                }
                // 具名 int 算式與原 Checker 具名函式皆使用已簽發的
                // TypedHIR。含 Call 只能經同一動態函式堆疊實際執行，
                // 不能再在 P16 建構器中另行求值或繞過資源退出。
                TermKind::BinaryInt(_) | TermKind::Group(_)
                    | TermKind::Call(_) | TermKind::MethodCall(..) | TermKind::AwaitMethodCall(..) | TermKind::UnresolvedZeroArgs(_)
                    if *kind == ScalarKind::Int => {
                    let expression = PrivateScalarReturnExpr::from_checked_bindings(
                        term, bindings, verified,
                    )?;
                    if expression.needs_call_stack() {
                        PrivateScalarCopyStructSeed::IntCall(Box::new(expression))
                    } else {
                        PrivateScalarCopyStructSeed::IntArithmetic(Box::new(expression))
                    }
                }
                _ => PrivateScalarCopyStructSeed::Static(
                    approved_seed(term, verified).map_err(|_|
                        PrivateScalarCallPrepareError::Unsupported(
                            "結構具名值缺少原 P16 Checker 純值來源",
                        ),
                    )?,
                ),
            };
            if seed.kind() != *kind || supplied.iter().any(|(key, _)| key == field_name) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "結構局部具名型別不一致或重複",
                ));
            }
            supplied.push((field_name.clone(), seed));
        }
        // 不以字面相同的使用者變數取代原 Checker DeclarationId；
        // caller 另在真正 Declare 節點核對 id、型別與可變性。
        let _ = name;
        Ok(PrivateScalarCopyStructPlan { fields, named: supplied })
    }

    /// V1／F4：直線宣告必須有原 Checker direct_moves 的完整配對。
    /// 原 Checker 故意不把 if/for/while 的可選路徑 Move 記成已執行；
    /// 對原 Checker 完整接受的控制路徑，僅以當前區塊原 Parser Move、
    /// 同型別 DeclarationId 與動態 P16 借用／存活守門按實際路徑轉交，
    /// 不得偽造直線的 direct_moves，亦不得提前 Move 未選分支。
    fn checked_copy_struct_move(
        target_name: &str, ty: &str, mutable: bool, target: DeclarationId,
        term: &Term, bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource, control_path: bool,
    ) -> Result<(DeclarationId, String), PrivateScalarCallPrepareError> {
        let source_name = PreparedPrivateMoveCall::checked_move_name(term, verified)
            .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                "結構整筆移動缺少原 Parser/Lexer 真 .move() 來源",
            ))?;
        let source = *bindings.get(source_name).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "結構整筆移動來源不在原 Checker 詞彙作用域",
            ),
        )?;
        let definition = verified.structs.get(ty).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "整筆移動目標缺少原 Checker 結構型別",
            ),
        )?;
        // 原 Checker 允許的具名 var str 已由 checked_copy_struct
        // 強制在真建構前提供受核准的字面 UTF-8；真正持有者先經
        // 共享帳 construct 成功，Move 才可沿 P16 同一唯一資源轉交。
        // 無具名預設的 var str 仍在前序 Prepare 保守拒絕。
        if !definition.fields.iter().all(|field|
                matches!(field.ty.as_str(), "int" | "bool" | "str"))
            || source == target || source_name == target_name
            || !verified.plan.declarations.matches(target, ty, mutable)
            || !verified.plan.declarations.typed_declaration(source)
                .is_some_and(|fact| fact.ty == ty)
            || (!control_path
                && !verified.plan.declarations.direct_moves.contains(&(source, target))) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "純值結構整筆移動缺少 Checker 同型別來源／目標轉移配對",
            ));
        }
        Ok((source, source_name.to_owned()))
    }

    /// V1／F3：只接受原 Checker 完整驗證後的 int 欄位可寫實例。
    /// 不以名稱或 RHS 文字自行猜測權限，先取得原 DeclarationId
    /// 與欄位型別，再編譯唯一 TypedHIR int 工作。
    fn checked_copy_struct_int_write(
        statement: &Stmt, receiver: &str, field: &str, expression: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<(DeclarationId, PrivateScalarReturnExpr), PrivateScalarCallPrepareError> {
        let declaration = *bindings.get(receiver).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者缺少本層原 Checker DeclarationId",
            ),
        )?;
        let fact = verified.plan.declarations.typed_declaration(declaration).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者缺少原 Checker 具型別來源",
            ),
        )?;
        let definition = verified.structs.get(fact.ty).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者不是原 Checker 已核准結構型別",
            ),
        )?;
        let member = definition.field(field).ok_or(
            PrivateScalarCallPrepareError::Unsupported("欄位寫入缺少原結構欄位"),
        )?;
        if !fact.mutable || member.immutable || member.ty != "int"
            || !definition.fields.iter().all(|candidate|
                matches!(candidate.ty.as_str(), "int" | "bool" | "str"))
            || !verified.tokens.iter().any(|token|
                token.start == statement.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == receiver))
            || !verified.tokens.iter().any(|token|
                token.start > statement.start && token.end <= expression.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == field)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "結構 int 欄位寫入缺少原可寫 Checker 權限或 Lexer 欄位身分",
            ));
        }
        let value = PrivateScalarReturnExpr::from_checked_bindings(
            expression, bindings, verified,
        )?;
        Ok((declaration, value))
    }

    /// V1／F3：同一原 Checker 實例與欄位寫入權限，
    /// bool RHS 只能由已核准的 TypedHIR 與 P16 真值取得。
    fn checked_copy_struct_bool_write(
        statement: &Stmt, receiver: &str, field: &str, expression: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<(DeclarationId, PrivateScalarBoolExpr), PrivateScalarCallPrepareError> {
        let declaration = *bindings.get(receiver).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者缺少本層原 Checker DeclarationId",
            ),
        )?;
        let fact = verified.plan.declarations.typed_declaration(declaration).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者缺少原 Checker 具型別來源",
            ),
        )?;
        let definition = verified.structs.get(fact.ty).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "欄位寫入接收者不是原 Checker 已核准結構型別",
            ),
        )?;
        let member = definition.field(field).ok_or(
            PrivateScalarCallPrepareError::Unsupported("欄位寫入缺少原結構欄位"),
        )?;
        if !fact.mutable || member.immutable || member.ty != "bool"
            || !definition.fields.iter().all(|candidate|
                matches!(candidate.ty.as_str(), "int" | "bool" | "str"))
            || !verified.tokens.iter().any(|token|
                token.start == statement.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == receiver))
            || !verified.tokens.iter().any(|token|
                token.start > statement.start && token.end <= expression.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == field)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "結構 bool 欄位寫入缺少原可寫 Checker 權限或 Lexer 欄位身分",
            ));
        }
        let value = PrivateScalarBoolExpr::from_checked(
            expression, bindings, verified,
        )?;
        Ok((declaration, value))
    }

    /// 原 Checker 已核准的可寫 str 欄位，只准字面值進本次
    /// 私有替換工作，不擅自將其他持有者當成可複製的 UTF-8。
    fn checked_copy_struct_str_write(
        statement: &Stmt, receiver: &str, field: &str, expression: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<(DeclarationId, String), PrivateScalarCallPrepareError> {
        let declaration = *bindings.get(receiver).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str 欄位寫入缺少原 Checker 詞彙接收者身分",
            ),
        )?;
        let fact = verified.plan.declarations.typed_declaration(declaration).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str 欄位寫入缺少原 Checker DeclarationId",
            ),
        )?;
        let definition = verified.structs.get(fact.ty).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str 欄位寫入缺少原 Checker 結構型別",
            ),
        )?;
        let member = definition.field(field).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "str 欄位寫入缺少原 Checker 欄位",
            ),
        )?;
        if !fact.mutable || member.immutable || member.ty != "str"
            || !definition.fields.iter().all(|candidate|
                matches!(candidate.ty.as_str(), "int" | "bool" | "str"))
            || !verified.tokens.iter().any(|token|
                token.start == statement.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == receiver))
            || !verified.tokens.iter().any(|token|
                token.start > statement.start && token.end <= expression.start
                    && matches!(&token.kind, TokenKind::Name(found) if found == field)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "str 欄位寫入缺少原 Checker 可寫權限與 Lexer 身分",
            ));
        }
        let text = private_str_literal(expression, &verified.tokens).map_err(|_|
            PrivateScalarCallPrepareError::Unsupported(
                "str 欄位覆寫只接受原 Lexer／Checker 核准字面值",
            ),
        )?;
        Ok((declaration, text))
    }

    /// R02／R04：同一真 Checker 已認可的可寫 str 左值及字串右值。
    /// 不靠 runtime 名稱推論 DeclarationId；不接受隱含複製或自身移動。
    fn checked_str_assignment(
        name: &str, destination: DeclarationId, expression: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
        statement_start: usize,
    ) -> Result<PrivateScalarStrInitializer, PrivateScalarCallPrepareError> {
        if !verified.plan.declarations.typed_declaration(destination)
            .is_some_and(|fact| fact.ty == "str" && fact.mutable)
            || !verified.tokens.iter().any(|token|
                token.start == statement_start
                    && matches!(&token.kind, TokenKind::Name(found) if found == name)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "str var 覆寫缺少原 Checker 可寫宣告與 Lexer 身分",
            ));
        }
        let initializer = Self::checked_str_initializer(expression, bindings, verified)?;
        if matches!(&initializer, PrivateScalarStrInitializer::Move { declaration, .. }
            if *declaration == destination) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "str 不得從自身 .move() 覆寫",
            ));
        }
        Ok(initializer)
    }

    /// 原 Checker 每一條詞彙區塊的單一宣告仍有獨立生命週期。
    fn scoped_single_declaration(action: PrivateScalarBranchAction)
        -> PrivateScalarBranchAction {
        match action {
            PrivateScalarBranchAction::DeclareInt { .. }
                | PrivateScalarBranchAction::DeclareBool { .. }
                | PrivateScalarBranchAction::DeclareEmptyInt { .. }
                | PrivateScalarBranchAction::DeclareEmptyBool { .. }
                | PrivateScalarBranchAction::DeclareStr { .. }
                | PrivateScalarBranchAction::DeclareEmptyStr { .. }
                | PrivateScalarBranchAction::DeclareCopyStruct { .. } =>
                    PrivateScalarBranchAction::Scoped(vec![action]),
            other => other,
        }
    }

    /// 離域必須清除真 frame 中本層新增的純值，不能僅補記假事件。
    fn end_scalar_scope(
        frame: &mut PrivateScalarActiveFrame,
        baseline: usize,
        events: &mut Vec<PrivateScalarCallEvent>,
    ) -> Result<(), PrivateScalarCallRuntimeError> {
        if baseline > frame.initialized_order.len() {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
        while frame.initialized_order.len() > baseline {
            match frame.initialized_order.pop() {
                Some(PrivateScalarLocalBinding::Int(id)) => {
                    if frame.values.remove(&id).is_none() {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    }
                    events.push(PrivateScalarCallEvent::LocalScopeEnded(id));
                }
                Some(PrivateScalarLocalBinding::Bool(id)) => {
                    if frame.bool_values.remove(&id).is_none() {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    }
                    events.push(PrivateScalarCallEvent::BoolScopeEnded(id));
                }
                Some(PrivateScalarLocalBinding::UninitializedInt(id)) => {
                    if frame.values.contains_key(&id) {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    }
                    events.push(PrivateScalarCallEvent::LocalScopeEndedEmpty(id));
                }
                Some(PrivateScalarLocalBinding::UninitializedBool(id)) => {
                    if frame.bool_values.contains_key(&id) {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    }
                    events.push(PrivateScalarCallEvent::BoolScopeEndedEmpty(id));
                }
                Some(PrivateScalarLocalBinding::CopyStruct(id)) => {
                    let Some(scope) = frame.copy_struct_scopes.last_mut() else {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    };
                    if !scope.iter().any(|(known, _)| *known == id) {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    }
                    scope.retain(|(known, _)| *known != id);
                    events.push(PrivateScalarCallEvent::CopyStructScopeEnded(id));
                }
                None => return Err(PrivateScalarCallRuntimeError::StateMismatch),
            }
        }
        Ok(())
    }

    /// 本批先接通單筆直接返回的 if／else-if／else 葉分支；
    /// 分支語句來源及條件均由原 Parser、Checker 審定。
    /// 僅接通分支內一筆原 Checker 已核准的 return 或
    /// 對分支外已存在 var int/bool 的賦值；沒有未證明的作用域逃逸。
    fn compile_early_branch(
        statements: &[Stmt],
        int_bindings: &BTreeMap<String, DeclarationId>,
        bool_bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
        inside_loop: bool,
    ) -> Result<PrivateScalarBranchAction, PrivateScalarCallPrepareError> {
        let [statement] = statements else {
            if statements.is_empty() {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "if 此子集不接受缺少已核准執行證據的空分支",
                ));
            }
            let mut actions = Vec::new();
            // 這張名稱表只在目前的 Parser 區塊內前向更新；
            // 同名遮蔽不會污染外部函式或兄弟 if／else 分支。
            let mut local_int = int_bindings.clone();
            let mut local_bool = bool_bindings.clone();
            let mut has_direct_local = false;
            for (index, statement) in statements.iter().enumerate() {
                let action = Self::compile_early_branch(
                    std::slice::from_ref(statement),
                    &local_int, &local_bool, verified, inside_loop,
                )?;
                if let StmtKind::Declare(owner) = &statement.kind {
                    let id = DeclarationId(statement.start);
                    // 同一張 Checker 名稱表只做 DeclarationId 身分查找；
                    // str 來源不得被當成 int 值求出，Expr 仍由 HIR 型別拒絕。
                    local_int.insert(owner.name.clone(), id);
                    if owner.ty == "bool" || verified.structs.contains_key(&owner.ty) {
                        // bool 值或 Copyable 結構 bool 欄位共用
                        // Checker DeclarationId；Name(bool) 仍須 HIR 驗證。
                        local_bool.insert(owner.name.clone(), id);
                    } else {
                        local_bool.remove(&owner.name);
                    }
                    has_direct_local = true;
                }
                // V1／F3：原 Checker 已證明整個分支 Return，
                // 或 break／continue 已中止本層控制邊時，後續來源不再有
                // 可執行 TypedHIR。不能把它編成額外 Call／ResourceId，
                // 也不能因存在合法但不可達的語句錯拒本來可執行的函式。
                // 未完整返回的 if 分支不符合 always_returns，仍會逐項
                // 編譯剩餘語句；不把未知條件的可達來源偷略過。
                let terminates_branch = action.always_returns()
                    || matches!(
                        action, PrivateScalarBranchAction::Break
                            | PrivateScalarBranchAction::Continue
                    );
                actions.push(action);
                if terminates_branch { break; }
            }
            return Ok(if has_direct_local {
                PrivateScalarBranchAction::Scoped(actions)
            } else {
                PrivateScalarBranchAction::Sequence(actions)
            });
        };
        match &statement.kind {
            StmtKind::Declare(owner) if owner.ty == "str" => {
                let id = DeclarationId(statement.start);
                if !verified.plan.declarations.matches(id, "str", owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "區塊 str 擁有者缺少原 Checker 唯一宣告身分",
                    ));
                }
                let Some(init) = owner.initializer.as_ref() else {
                    if !owner.mutable {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "原 Checker 無初值 str 只有 var 可延遲建立持有者",
                        ));
                    }
                    return Ok(PrivateScalarBranchAction::DeclareEmptyStr {
                        declaration: id, name: owner.name.clone(),
                        mutable: owner.mutable,
                    });
                };
                let initializer = Self::checked_str_initializer(
                    init, int_bindings, verified,
                )?;
                Ok(PrivateScalarBranchAction::DeclareStr {
                    declaration: id, name: owner.name.clone(),
                    mutable: owner.mutable, initializer,
                })
            }
            StmtKind::Declare(owner) if verified.structs.contains_key(&owner.ty) => {
                let id = DeclarationId(statement.start);
                if !verified.plan.declarations.matches(id, &owner.ty, owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "分支結構宣告缺少原 Checker DeclarationId／型別",
                    ));
                }
                let term = owner.initializer.as_ref().ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "分支 Copyable 結構需要原 Checker 完整建立式",
                    ),
                )?;
                if matches!(&term.kind, TermKind::Move(_)) {
                    let (source, source_name) = Self::checked_copy_struct_move(
                        &owner.name, &owner.ty, owner.mutable, id,
                        term, int_bindings, verified, true,
                    )?;
                    Ok(PrivateScalarBranchAction::MoveCopyStruct {
                        declaration: id, name: owner.name.clone(), source, source_name,
                    })
                } else {
                    let plan = Self::checked_copy_struct(
                        &owner.name, &owner.ty, term, int_bindings, verified,
                    )?;
                    Ok(PrivateScalarBranchAction::DeclareCopyStruct {
                        declaration: id, name: owner.name.clone(), plan,
                    })
                }
            }
            StmtKind::Declare(owner) if matches!(owner.ty.as_str(), "int" | "bool") => {
                let id = DeclarationId(statement.start);
                if !verified.plan.declarations.matches(id, &owner.ty, owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "區塊局部宣告缺少原 Checker 可寫性／型別／DeclarationId",
                    ));
                }
                let Some(init) = owner.initializer.as_ref() else {
                    if !owner.mutable {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "未初始化宣告只允許原 Checker 核准的 var",
                        ));
                    }
                    return Ok(if owner.ty == "bool" {
                        PrivateScalarBranchAction::DeclareEmptyBool { declaration: id }
                    } else {
                        PrivateScalarBranchAction::DeclareEmptyInt { declaration: id }
                    });
                };
                if owner.ty == "bool" {
                    Ok(PrivateScalarBranchAction::DeclareBool {
                        declaration: id,
                        value: PrivateScalarBoolExpr::from_checked_call_argument(
                            init, int_bindings, verified,
                        )?,
                    })
                } else {
                    Ok(PrivateScalarBranchAction::DeclareInt {
                        declaration: id,
                        value: PrivateScalarReturnExpr::from_checked_bindings(
                            init, int_bindings, verified,
                        )?,
                    })
                }
            }
            StmtKind::If { .. } =>
                Ok(PrivateScalarBranchAction::NestedIf(Box::new(
                    Self::compile_early_if(
                        statement, int_bindings, bool_bindings, verified, inside_loop,
                    )?,
                ))),
            StmtKind::While { condition, body } => {
                let guard = PrivateScalarBoolExpr::from_checked(
                    condition, bool_bindings, verified,
                )?;
                let steps = if matches!(guard, PrivateScalarBoolExpr::Constant(false))
                    || body.statements.is_empty() {
                    PrivateScalarBranchAction::Sequence(Vec::new())
                } else {
                    Self::compile_early_branch(
                        &body.statements, int_bindings, bool_bindings, verified, true,
                    )?
                };
                Ok(PrivateScalarBranchAction::While {
                    condition: guard,
                    body: Box::new(Self::scoped_single_declaration(steps)),
                })
            }
            StmtKind::ForRange { iterator, iterator_start, lower, upper, body } => {
                let identity = DeclarationId(*iterator_start);
                if !verified.plan.declarations.matches(identity, "int", false) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "for 迭代器缺少原 Checker const int 宣告身份",
                    ));
                }
                let start = crate::syntax_shared::integer_i64(lower).map_err(|_|
                    PrivateScalarCallPrepareError::Unsupported(
                        "for 半開左端點缺少合法 i64 來源",
                    ),
                )?;
                let end = crate::syntax_shared::integer_i64(upper).map_err(|_|
                    PrivateScalarCallPrepareError::Unsupported(
                        "for 半開右端點缺少合法 i64 來源",
                    ),
                )?;
                // 空區間原 Checker 不必替不可達本體簽發 HIR。
                let compiled = if start >= end || body.statements.is_empty() {
                    PrivateScalarBranchAction::Sequence(Vec::new())
                } else {
                    let mut scoped = int_bindings.clone();
                    scoped.insert(iterator.clone(), identity);
                    Self::compile_early_branch(
                        &body.statements, &scoped, bool_bindings, verified, true,
                    )?
                };
                Ok(PrivateScalarBranchAction::ForRange {
                    iterator: identity, lower: start, upper: end,
                    body: Box::new(Self::scoped_single_declaration(compiled)),
                })
            }
            StmtKind::Break if inside_loop => Ok(PrivateScalarBranchAction::Break),
            StmtKind::Continue if inside_loop => Ok(PrivateScalarBranchAction::Continue),
            StmtKind::Return(ret) => {
                // F2：原 Checker 已核准的提前 return 保持原型別：
                // int/bool 走共同 TypedHIR，str 擁有返回只接受原 Lexer
                // 字面值或具名明確 .move() 且原 Checker 已證安全。
                // ScalarType 僅有 Copyable int/bool；不能將 str
                // 冒充純值 HIR，也不能以 ResourceId 轉成假 i64。
                let span = ret.expression.node_span();
                let result = if verified.plan.scalar_hir.expression(span)
                    .is_some_and(|fact| fact.ty == crate::common_ir::ScalarType::Bool) {
                    PrivateScalarReturnExpr::BoolValue(Box::new(
                        PrivateScalarBoolExpr::from_checked_call_argument(
                            &ret.expression, int_bindings, verified,
                        )?,
                    ))
                } else if let Ok(returned) = Self::checked_str_return_expr(
                    &ret.expression, int_bindings, verified,
                ) {
                    // 原 Checker 已確認整筆返回來源；不可由編譯器
                    // 擅自把未通過型別驗證的 Call-return 接入資源帳。
                    returned
                } else {
                    PrivateScalarReturnExpr::from_checked_bindings(
                        &ret.expression, int_bindings, verified,
                    )?
                };
                Ok(PrivateScalarBranchAction::Return(result))
            },
            StmtKind::FieldAssign { receiver, field, value } => {
                if verified.plan.declarations.typed_declaration(
                    *int_bindings.get(receiver).ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "欄位更新缺少原 Checker 詞彙接收者身分",
                        ),
                    )?,
                ).and_then(|fact| verified.structs.get(fact.ty))
                    .and_then(|definition| definition.field(field))
                    .is_some_and(|member| member.ty == "bool") {
                    let (declaration, value) = Self::checked_copy_struct_bool_write(
                        statement, receiver, field, value, bool_bindings, verified,
                    )?;
                    Ok(PrivateScalarBranchAction::AssignCopyStructBool {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    })
                } else if verified.plan.declarations.typed_declaration(
                    *int_bindings.get(receiver).expect("原 Checker 已驗證 receiver"),
                ).and_then(|fact| verified.structs.get(fact.ty))
                    .and_then(|definition| definition.field(field))
                    .is_some_and(|member| member.ty == "str") {
                    let (declaration, value) = Self::checked_copy_struct_str_write(
                        statement, receiver, field, value, int_bindings, verified,
                    )?;
                    Ok(PrivateScalarBranchAction::AssignCopyStructStr {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    })
                } else {
                    let (declaration, value) = Self::checked_copy_struct_int_write(
                        statement, receiver, field, value, int_bindings, verified,
                    )?;
                    Ok(PrivateScalarBranchAction::AssignCopyStructInt {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    })
                }
            }
            StmtKind::Assign(assign) => {
                let id = *int_bindings.get(&assign.name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "if 純值賦值目標缺少原 Checker 已驗證 DeclarationId",
                    ),
                )?;
                let fact = verified.plan.declarations.typed_declaration(id).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "if 賦值缺少原 Checker 變數宣告",
                    ),
                )?;
                if fact.ty == "str" {
                    let initializer = Self::checked_str_assignment(
                        &assign.name, id, &assign.expression, int_bindings,
                        verified, statement.start,
                    )?;
                    return Ok(PrivateScalarBranchAction::AssignStr {
                        declaration: id, name: assign.name.clone(), initializer,
                    });
                }
                if !fact.mutable || (fact.ty != "int" && fact.ty != "bool")
                    || verified.plan.scalar_hir.assignment(statement.node_span())
                        .is_none_or(|record|
                            record.resolved.declaration != id
                                || record.resolved.ty !=
                                    (if fact.ty == "bool" {
                                        crate::common_ir::ScalarType::Bool
                                    } else {
                                        crate::common_ir::ScalarType::Int
                                    }))
                    || !verified.tokens.iter().any(|token|
                        token.start == statement.start
                            && matches!(&token.kind, TokenKind::Name(name)
                                if name == &assign.name)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "if 更新缺少真可寫 Checker HIR、Lexer 或型別來源",
                    ));
                }
                if fact.ty == "bool" {
                    if bool_bindings.get(&assign.name) != Some(&id) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "if bool 更新目標不是原 Matcher 同一 bool 宣告",
                        ));
                    }
                    Ok(PrivateScalarBranchAction::AssignBool {
                        declaration: id,
                        value: PrivateScalarBoolExpr::from_checked_call_argument(
                            &assign.expression, int_bindings, verified,
                        )?,
                    })
                } else {
                    Ok(PrivateScalarBranchAction::AssignInt {
                        declaration: id,
                        value: PrivateScalarReturnExpr::from_checked_bindings(
                            &assign.expression, int_bindings, verified,
                        )?,
                    })
                }
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "if 其餘語句待獨立控制流程與區域資源證據完成",
            )),
        }
    }

    fn compile_early_if(
        stmt: &Stmt,
        int_bindings: &BTreeMap<String, DeclarationId>,
        bool_bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
        inside_loop: bool,
    ) -> Result<PrivateScalarEarlyIf, PrivateScalarCallPrepareError> {
        let StmtKind::If { branch, no } = &stmt.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "else-if 必須保持原 Parser 真 If 節點",
            ));
        };
        let condition = PrivateScalarBoolExpr::from_checked_call_argument(
            &branch.branch.condition, int_bindings, verified,
        )?;
        let when_true = Self::scoped_single_declaration(
            Self::compile_early_branch(
                &branch.body.statements, int_bindings, bool_bindings, verified, inside_loop,
            )?,
        );
        let when_false = match no.as_ref() {
            None => None,
            Some(crate::syntax_shared::ParsedElse::Block(block)) =>
                Some(PrivateScalarEarlyElse::Branch(
                    Self::scoped_single_declaration(Self::compile_early_branch(
                        &block.statements, int_bindings, bool_bindings, verified, inside_loop,
                    )?),
                )),
            Some(crate::syntax_shared::ParsedElse::ElseIf(next)) =>
                Some(PrivateScalarEarlyElse::ElseIf(Box::new(
                    Self::compile_early_if(
                        next, int_bindings, bool_bindings, verified, inside_loop,
                    )?,
                ))),
            #[cfg(test)]
            Some(crate::syntax_shared::ParsedElse::SyntheticTest(_)) =>
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "Rust 合成 else 不是原 Parser 合法執行來源",
                )),
        };
        Ok(PrivateScalarEarlyIf { condition, when_true, when_false })
    }

    /// P18.1：從同一原 Parser／Checker 的真宣告序列，逐筆編譯
    /// 函式內 Copyable int 初始化；一律保留 DeclarationId，
    /// 不以函式名稱／變數字面名稱重新解析安全身份。
    fn prepare_function_locals(
        function: &Function, verified: &VerifiedPrivateSource,
    ) -> Result<(
        Vec<PrivateScalarLocalInit>, Vec<PrivateScalarLocalStep>,
        PrivateScalarReturnExpr, crate::common_ir::NodeSpan,
    ), PrivateScalarCallPrepareError> {
        let (last, prefix) = function.body.statements.split_last().ok_or(
            PrivateScalarCallPrepareError::Unsupported("普通函式缺少返回"),
        )?;
        let StmtKind::Return(ret) = &last.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "普通函式最後一筆必須是真 Checker 核准的 return",
            ));
        };
        // F2：bool/int 返回走同一組原 Checker 已核准的局部與 CFG
        // 編譯流程，直到最後 return 才按真 TypedHIR 選擇值型別。
        // 非 Copyable 返回的完整資源交接仍不在此 i64/bool 工作棧冒充。
        if !matches!(function.result_ty.as_str(), "int" | "bool" | "str") {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "普通函式此執行器只接通已核准 int/bool/str 返回",
            ));
        }
        let mut bindings = function.params.iter().map(|param|
            (param.name.clone(), DeclarationId(param.declaration_start))
        ).collect::<BTreeMap<_, _>>();
        let mut local_inits = Vec::new();
        let mut local_steps = Vec::new();
        let mut bool_bindings = function.params.iter()
            .filter(|param| param.ty == "bool")
            .map(|param| (param.name.clone(), DeclarationId(param.declaration_start)))
            .collect::<BTreeMap<_, _>>();
        for statement in prefix {
            if matches!(&statement.kind, StmtKind::If { .. }) {
                let branch = Self::compile_early_if(
                    statement, &bindings, &bool_bindings, verified, false,
                )?;
                // V1／F3：原 Checker 已證整個頂層 if／else-if／else
                // 都返回時，後續同層來源沒有執行邊；不再編譯
                // 無 TypedHIR 的不可達局部與不支援的結構呼叫。
                // 未證全返回的分支仍完整保留後續動態工作。
                let all_paths_return = branch.always_returns();
                local_steps.push(PrivateScalarLocalStep::EarlyReturn(branch));
                if all_paths_return { break; }
                continue;
            }
            if matches!(&statement.kind,
                StmtKind::While { .. } | StmtKind::ForRange { .. }) {
                let action = Self::compile_early_branch(
                    std::slice::from_ref(statement), &bindings, &bool_bindings,
                    verified, false,
                )?;
                local_steps.push(PrivateScalarLocalStep::Control(action));
                continue;
            }
            if let StmtKind::FieldAssign { receiver, field, value } = &statement.kind {
                if verified.plan.declarations.typed_declaration(
                    *bindings.get(receiver).ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "欄位更新缺少原 Checker 詞彙接收者身分",
                        ),
                    )?,
                ).and_then(|fact| verified.structs.get(fact.ty))
                    .and_then(|definition| definition.field(field))
                    .is_some_and(|member| member.ty == "bool") {
                    let (declaration, value) = Self::checked_copy_struct_bool_write(
                        statement, receiver, field, value, &bool_bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::AssignCopyStructBool {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    });
                } else if verified.plan.declarations.typed_declaration(
                    *bindings.get(receiver).expect("原 Checker 已驗證 receiver"),
                ).and_then(|fact| verified.structs.get(fact.ty))
                    .and_then(|definition| definition.field(field))
                    .is_some_and(|member| member.ty == "str") {
                    let (declaration, value) = Self::checked_copy_struct_str_write(
                        statement, receiver, field, value, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::AssignCopyStructStr {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    });
                } else {
                    let (declaration, value) = Self::checked_copy_struct_int_write(
                        statement, receiver, field, value, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::AssignCopyStructInt {
                        declaration, owner: receiver.clone(), field: field.clone(), value,
                    });
                }
                continue;
            }
            if let StmtKind::Assign(assign) = &statement.kind {
                if let Some(id) = bool_bindings.get(&assign.name).copied() {
                    if !verified.plan.declarations.typed_declaration(id)
                        .is_some_and(|fact| fact.ty == "bool" && fact.mutable)
                        || verified.plan.scalar_hir.assignment(statement.node_span())
                            .is_none_or(|fact|
                                fact.resolved.declaration != id
                                    || fact.resolved.ty != crate::common_ir::ScalarType::Bool)
                        || !verified.tokens.iter().any(|token|
                            token.start == statement.start
                                && matches!(&token.kind,
                                    TokenKind::Name(found) if found == &assign.name)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "bool var 賦值缺少原 Checker 可寫身分與 Token",
                        ));
                    }
                    let value = PrivateScalarBoolExpr::from_checked_call_argument(
                        &assign.expression, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::AssignBool {
                        declaration: id, value,
                    });
                    continue;
                }
                if let Some(id) = bindings.get(&assign.name).copied() {
                    if verified.plan.declarations.typed_declaration(id)
                        .is_some_and(|fact| fact.ty == "str") {
                        let initializer = Self::checked_str_assignment(
                            &assign.name, id, &assign.expression, &bindings,
                            verified, statement.start,
                        )?;
                        local_steps.push(PrivateScalarLocalStep::AssignStr {
                            declaration: id, name: assign.name.clone(), initializer,
                        });
                        continue;
                    }
                }
                let id = *bindings.get(&assign.name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "int 賦值目標不是已驗證的本函式宣告或參數",
                    ),
                )?;
                if !verified.plan.declarations.typed_declaration(id)
                    .is_some_and(|fact| fact.ty == "int" && fact.mutable)
                    || verified.plan.scalar_hir.assignment(statement.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != id
                                || fact.resolved.ty != crate::common_ir::ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == statement.start
                            && matches!(&token.kind,
                                TokenKind::Name(found) if found == &assign.name))
                {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "原 Checker int var 賦值缺少完整可寫 HIR／Token",
                    ));
                }
                let value = PrivateScalarReturnExpr::from_checked_bindings(
                    &assign.expression, &bindings, verified,
                )?;
                local_steps.push(PrivateScalarLocalStep::Assign {
                    declaration: id, value,
                });
                continue;
            }
            let StmtKind::Declare(owner) = &statement.kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "函式前置語句只接通真 Copyable int 宣告與已核准 var 賦值",
                ));
            };
            let declaration = DeclarationId(statement.start);
            if verified.structs.contains_key(&owner.ty) {
                if !verified.plan.declarations.matches(declaration, &owner.ty, owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式結構局部缺少原 Checker DeclarationId／型別",
                    ));
                }
                let term = owner.initializer.as_ref().ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "函式 Copyable 結構需要原 Checker 完整建立式",
                    ),
                )?;
                if matches!(&term.kind, TermKind::Move(_)) {
                    let (source, source_name) = Self::checked_copy_struct_move(
                        &owner.name, &owner.ty, owner.mutable, declaration,
                        term, &bindings, verified, false,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::MoveCopyStruct {
                        declaration, name: owner.name.clone(), source, source_name,
                    });
                } else {
                    let plan = Self::checked_copy_struct(
                        &owner.name, &owner.ty, term, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::DeclareCopyStruct {
                        declaration, name: owner.name.clone(), plan,
                    });
                }
                bindings.insert(owner.name.clone(), declaration);
                // 保留 struct 來源於 bool 詞彙索引，只有原 Checker
                // 已核准 bool FieldRead 才能從它取得布林值。
                bool_bindings.insert(owner.name.clone(), declaration);
                continue;
            }
            if owner.ty == "str" {
                if !verified.plan.declarations.matches(declaration, "str", owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式 str 所有者缺少原 Checker 唯一身分",
                    ));
                }
                if let Some(expr) = owner.initializer.as_ref() {
                    let initializer = Self::checked_str_initializer(
                        expr, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::DeclareStr {
                        declaration, name: owner.name.clone(),
                        mutable: owner.mutable, initializer,
                    });
                } else if owner.mutable {
                    local_steps.push(PrivateScalarLocalStep::DeclareEmptyStr {
                        declaration, name: owner.name.clone(), mutable: owner.mutable,
                    });
                } else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "尚無原 Checker const str 延遲初始化權限",
                    ));
                }
                bindings.insert(owner.name.clone(), declaration);
                bool_bindings.remove(&owner.name);
                continue;
            }
            if owner.ty == "bool" {
                if !verified.plan.declarations.matches(declaration, "bool", owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 區域宣告缺少原 Checker 唯一宣告身分",
                    ));
                }
                if let Some(initializer) = owner.initializer.as_ref() {
                    let value = PrivateScalarBoolExpr::from_checked_call_argument(
                        initializer, &bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::DeclareBool {
                        declaration, value,
                    });
                } else if owner.mutable {
                    local_steps.push(PrivateScalarLocalStep::DeclareEmptyBool(declaration));
                } else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "原 Checker 未初始化的 const bool 不可交付",
                    ));
                }
                bool_bindings.insert(owner.name.clone(), declaration);
                // 共用原 Checker DeclarationId 名稱表同時供正式函式
                // bool 實參使用；型別仍由 Reference HIR 唯一核對，
                // 不會將 bool 當成 int 或重新解析來源。
                bindings.insert(owner.name.clone(), declaration);
                continue;
            }
            if owner.ty != "int"
                || !verified.plan.declarations.matches(declaration, "int", owner.mutable)
            {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "函式內 int 區域宣告缺少 Checker 唯一宣告身分",
                ));
            }
            if let Some(initializer) = owner.initializer.as_ref() {
                let value = PrivateScalarReturnExpr::from_checked_bindings(
                    initializer, &bindings, verified,
                )?;
                let init = PrivateScalarLocalInit { declaration, value };
                local_steps.push(PrivateScalarLocalStep::Declare(init.clone()));
                local_inits.push(init);
            } else if owner.mutable {
                local_steps.push(PrivateScalarLocalStep::DeclareEmptyInt(declaration));
            } else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "原 Checker 未初始化 const int 不可交付",
                ));
            }
            bindings.insert(owner.name.clone(), declaration);
        }
        let return_span = ret.expression.node_span();
        // 原 Checker 在所有 if／else-if／else 葉分支皆 Return 時，
        // 合法不為後續不可達的原始 Return 建立 Expression HIR。
        // 僅這個可由已驗證完整分支證明的情況，保留不可達標記，
        // 若執行器錯誤落入該路徑，必須受控 StateMismatch。
        let every_branch_returns = matches!(
            local_steps.last(),
            Some(PrivateScalarLocalStep::EarlyReturn(plan)) if plan.always_returns()
        );
        let result = if every_branch_returns
            && verified.plan.scalar_hir.expression(return_span).is_none() {
            PrivateScalarReturnExpr::Unreachable
        } else if function.result_ty == "str" {
            Self::checked_str_return_expr(&ret.expression, &bindings, verified)?
        } else if function.result_ty == "bool" {
            // 尾端與提前 return 同一來源事實；bool 值只經
            // EvaluateBoolArgument 真返回，不混入 int 運算。
            PrivateScalarReturnExpr::BoolValue(Box::new(
                PrivateScalarBoolExpr::from_checked_call_argument(
                    &ret.expression, &bindings, verified,
                )?,
            ))
        } else {
            PrivateScalarReturnExpr::from_checked_bindings(
                &ret.expression, &bindings, verified,
            )?
        };
        Ok((local_inits, local_steps, result, return_span))
    }

    /// 顯式 expression 工作堆疊＋真函式框架。合法遞迴亦走同一路徑，
    /// 預算耗盡時從最內層逐一退出；禁止依賴宿主 Rust 呼叫堆疊。
    fn evaluate_graph<'a>(
        &'a self,
        expr: &'a PrivateScalarReturnExpr,
        initial_frame: &BTreeMap<DeclarationId, i64>,
        events: &mut Vec<PrivateScalarCallEvent>,
        resource_events: &mut Vec<crate::owned_str_runtime::ResourceEvent>,
        budget: PrivateScalarExecutionBudget,
        fail_after_root_binding: bool,
        initial_local_steps: &'a [PrivateScalarLocalStep],
        await_scheduler: Option<&mut dyn FnMut(
            &str, &[crate::owned_str_runtime::ResourceEvent],
        )>,
    ) -> Result<i64, PrivateScalarCallRuntimeError> {
        let mut checkpoint = None;
        self.evaluate_graph_driver(
            expr, initial_frame, events, resource_events, budget,
            fail_after_root_binding, initial_local_steps, await_scheduler,
            &mut checkpoint, false, false,
        )?.ok_or(PrivateScalarCallRuntimeError::StateMismatch)
    }

    /// 在完整原執行器的同一條動態執行路徑上封存與恢復。
    /// allow_pause=false 維持原本同步結果／事件；true 只在原
    /// Checked await 即將 EnterCall 時把「目前全部真堆疊」交回呼叫者。
    fn evaluate_graph_driver<'a>(
        &'a self,
        expr: &'a PrivateScalarReturnExpr,
        initial_frame: &BTreeMap<DeclarationId, i64>,
        events: &mut Vec<PrivateScalarCallEvent>,
        resource_events: &mut Vec<crate::owned_str_runtime::ResourceEvent>,
        budget: PrivateScalarExecutionBudget,
        fail_after_root_binding: bool,
        initial_local_steps: &'a [PrivateScalarLocalStep],
        mut await_scheduler: Option<&mut dyn FnMut(
            &str, &[crate::owned_str_runtime::ResourceEvent],
        )>,
        checkpoint: &mut Option<PrivateScalarGraphCheckpoint<'a>>,
        allow_pause: bool,
        // E3：僅能在已真正暫停且全框架無活借用的合法
        // await 安全點直接停止工作，交由同一真出口反序清理。
        cancel_at_checkpoint: bool,
    ) -> Result<Option<i64>, PrivateScalarCallRuntimeError> {
        if cancel_at_checkpoint && checkpoint.is_none() {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
        let restored = checkpoint.take();
        let mut resuming_current_await = restored.is_some();
        let snapshot = restored.unwrap_or_else(|| {
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        // P16 MixedStructRuntime 的真建構與詞彙離域；本批拒絕任何 str 欄位。
        let mut copy_structs = crate::mixed_struct_runtime::MixedStructRuntime::new();
        let mut resource_cursor = 0usize;
        resources.enter_scope();
        copy_structs.enter_scope();
        let mut frames = vec![PrivateScalarActiveFrame {
            name: self.callee_name.clone(), values: initial_frame.clone(),
            bool_values: BTreeMap::new(),
            initialized_order: Vec::new(),
            resource_scopes: 1,
            str_owner_scopes: vec![Vec::new()],
            copy_struct_scopes: vec![Vec::new()],
            borrow_tracker: crate::borrow_safety::BorrowSafety::new(),
            incoming_borrows: Vec::new(),
            pending_owned_returns: Vec::new(),
        }];
        let mut tasks = vec![PrivateScalarTask::Evaluate(expr, true)];
        // 原 Checker 語句順序在 LIFO 上反向推入，執行時保持
        // 宣告、賦值與返回的真實相對次序，不提前求值後續 RHS。
        for step in initial_local_steps.iter().rev() {
            match step {
                PrivateScalarLocalStep::Declare(init) => {
                    tasks.push(PrivateScalarTask::BindLocal(init.declaration));
                    tasks.push(PrivateScalarTask::Evaluate(&init.value, false));
                }
                PrivateScalarLocalStep::Assign { declaration, value } => {
                    tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                }
                PrivateScalarLocalStep::AssignCopyStructInt {
                    declaration, owner, field, value,
                } => {
                    tasks.push(PrivateScalarTask::WriteCopyStructInt(
                        *declaration, owner, field,
                    ));
                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                }
                PrivateScalarLocalStep::AssignCopyStructBool {
                    declaration, owner, field, value,
                } => tasks.push(PrivateScalarTask::WriteCopyStructBool(
                    *declaration, owner, field, Some(value),
                )),
                PrivateScalarLocalStep::AssignCopyStructStr {
                    declaration, owner, field, value,
                } => tasks.push(PrivateScalarTask::WriteCopyStructStr(
                    *declaration, owner, field, value,
                )),
                PrivateScalarLocalStep::DeclareEmptyInt(declaration) =>
                    tasks.push(PrivateScalarTask::DeclareEmptyInt(*declaration)),
                PrivateScalarLocalStep::DeclareEmptyBool(declaration) =>
                    tasks.push(PrivateScalarTask::DeclareEmptyBool(*declaration)),
                PrivateScalarLocalStep::DeclareBool { declaration, value } =>
                    tasks.push(PrivateScalarTask::BindBool(*declaration, value)),
                PrivateScalarLocalStep::AssignBool { declaration, value } =>
                    tasks.push(PrivateScalarTask::AssignBool(*declaration, value)),
                PrivateScalarLocalStep::DeclareStr {
                    declaration, name, mutable, initializer,
                } => tasks.push(PrivateScalarTask::DeclareStr(
                    *declaration, name, *mutable, initializer,
                )),
                PrivateScalarLocalStep::DeclareEmptyStr {
                    declaration, name, mutable,
                } => tasks.push(PrivateScalarTask::DeclareEmptyStr(
                    *declaration, name, *mutable,
                )),
                PrivateScalarLocalStep::DeclareCopyStruct {
                    declaration, name, plan,
                } => tasks.push(PrivateScalarTask::DeclareCopyStruct(
                    *declaration, name, plan,
                )),
                PrivateScalarLocalStep::MoveCopyStruct {
                    declaration, name, source, source_name,
                } => tasks.push(PrivateScalarTask::MoveCopyStruct(
                    *declaration, name, *source, source_name,
                )),
                PrivateScalarLocalStep::AssignStr {
                    declaration, name, initializer,
                } => tasks.push(PrivateScalarTask::AssignStr(*declaration, name, initializer)),
                PrivateScalarLocalStep::EarlyReturn(plan) =>
                    tasks.push(PrivateScalarTask::CheckEarlyReturn(plan)),
                PrivateScalarLocalStep::Control(action) =>
                    tasks.push(PrivateScalarTask::ExecuteBranchAction(action)),
            }
        }
        let mut values = Vec::<i64>::new();
        let mut pending_str_returns = Vec::<crate::owned_str_runtime::OwnedUtf8>::new();
        let mut completed_arguments = Vec::<PrivateScalarEvaluatedArgument>::new();
        // 只有 Copyable 欄位的暫存具名快照；不持有 UTF-8 ResourceId，
        // 函式失敗時丟棄快照不會產生已建構結構或需釋放的資源。
        // 內層呼叫又建構結構時以同一 LIFO 棧保證順序。
        let mut pending_copy_structs = Vec::<Vec<(String, ApprovedSeed)>>::new();
        let mut steps = 0usize;
        PrivateScalarGraphCheckpoint {
            resources, copy_structs, resource_cursor, frames, tasks, values,
            pending_str_returns, completed_arguments, pending_copy_structs,
            steps, awaiting_name: String::new(),
        }
        });
        let PrivateScalarGraphCheckpoint {
            mut resources, mut copy_structs, mut resource_cursor,
            mut frames, mut tasks, mut values, mut pending_str_returns,
            mut completed_arguments,
            mut pending_copy_structs, mut steps, awaiting_name: _,
        } = snapshot;
        let mut paused_await = None::<String>;
        // 取消不執行下一條 voiles 指令；此處 Ok(None) 只表示
        // 「沒有更多使用者工作」，清理守門以 cancel_at_checkpoint
        // 另行觸發，並不冒充任何正常回傳 int／執行期錯誤。
        let result = if cancel_at_checkpoint { Ok(None) } else {
        (|| -> Result<Option<i64>, PrivateScalarCallRuntimeError> {
            'graph: while let Some(task) = tasks.pop() {
                // 記錄已完成的實參不是另一個 voiles 運算步驟；
                // 即使下一運算會耗盡預算，仍保留先前已真求值的事件。
                if !matches!(&task, PrivateScalarTask::RecordArgument
                    | PrivateScalarTask::RecordBoolArgument) {
                    if steps >= budget.max_steps {
                        return Err(PrivateScalarCallRuntimeError::StepBudget);
                    }
                    steps += 1;
                }
                match task {
                    PrivateScalarTask::EvaluateMethod(site) => {
                        tasks.push(PrivateScalarTask::InvokeSelf(site));
                        for (_, arg) in site.arguments.iter().rev() {
                            tasks.push(match arg {
                                PrivateScalarCallArgument::Int(expr) => PrivateScalarTask::Evaluate(expr, false),
                                PrivateScalarCallArgument::Bool(expr) => PrivateScalarTask::EvaluateBoolArgument(expr),
                                PrivateScalarCallArgument::LiteralStr(text) => PrivateScalarTask::CreateStrLiteralArgument(text),
                                PrivateScalarCallArgument::MoveStr { owner, source } => PrivateScalarTask::MoveArgument(owner, *source),
                                PrivateScalarCallArgument::BorrowStr { owner, fact } =>
                                    PrivateScalarTask::BorrowArgument(owner, *fact),
                                _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                            });
                        }
                    }
                    PrivateScalarTask::InvokeSelf(site) => {
                        if frames.len() >= budget.max_call_frames {
                            return Err(PrivateScalarCallRuntimeError::CallBudget);
                        }
                        if site.awaited {
                            Self::check_await_safe_point_chain(frames.iter().map(|frame|
                                (&frame.borrow_tracker, frame.incoming_borrows.as_slice())))?;
                            if allow_pause && !resuming_current_await {
                                tasks.push(PrivateScalarTask::InvokeSelf(site));
                                steps -= 1;
                                paused_await = Some(site.name.clone());
                                break 'graph;
                            }
                            resuming_current_await = false;
                            if let Some(schedule) = await_scheduler.as_deref_mut() {
                                schedule(&site.name, resources.events());
                            }
                            events.push(PrivateScalarCallEvent::MethodAwaitSafePoint(site.call_span));
                        }
                        let helper_frame_available = frames.len() + 1 < budget.max_call_frames;
                        let frame = frames.last_mut().ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                        if !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(id, name)| *id == site.loan.source && name == &site.owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let mut parameters = BTreeMap::new();
                        let mut argument_tickets = Vec::new();
                        let mut moves = Vec::new();
                        for (parameter, arg) in site.arguments.iter().rev() {
                            if let PrivateScalarCallArgument::BorrowStr { fact, .. } = arg {
                                let index = completed_arguments.len().checked_sub(argument_tickets.len() + moves.len() + 1)
                                    .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                let PrivateScalarEvaluatedArgument::Borrow { ticket, fact: actual } = completed_arguments[index]
                                    else { return Err(PrivateScalarCallRuntimeError::StateMismatch); };
                                if actual != *fact { return Err(PrivateScalarCallRuntimeError::StateMismatch); }
                                frame.borrow_tracker.check_ticket_access(ticket, fact.kind)
                                    .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                                argument_tickets.push(ticket);
                            } else if let PrivateScalarCallArgument::MoveStr { owner, source } = arg {
                                let index = completed_arguments.len().checked_sub(argument_tickets.len() + moves.len() + 1)
                                    .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                if !matches!(completed_arguments[index], PrivateScalarEvaluatedArgument::Move { source: id } if id == *source) {
                                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                }
                                frame.borrow_tracker.check_direct_access(*source).map_err(PrivateScalarCallRuntimeError::Borrow)?;
                                moves.push((crate::owned_str_runtime::StrArgumentSource::Named(owner), Some(*source), *parameter));
                            } else if matches!(arg, PrivateScalarCallArgument::LiteralStr(_)) {
                                let index = completed_arguments.len().checked_sub(argument_tickets.len() + moves.len() + 1)
                                    .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                let PrivateScalarEvaluatedArgument::TemporaryStr { resource_id } = completed_arguments[index]
                                    else { return Err(PrivateScalarCallRuntimeError::StateMismatch); };
                                moves.push((crate::owned_str_runtime::StrArgumentSource::Temporary(resource_id), None, *parameter));
                            } else {
                                let value = values.pop().ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                parameters.insert(*parameter, value);
                            }
                        }
                        moves.reverse();
                        argument_tickets.reverse();
                        let functions = &self.verified.structs;
                        let owner_type = self.verified.plan.declarations.typed_declaration(site.loan.source)
                            .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?.ty;
                        let method = functions.get(owner_type).and_then(|s| s.methods_named(&site.name)
                            .find(|m| m.start == site.method_start)).ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                        let names: Vec<_> = moves.iter().map(|(owner, _, param)| {
                            let target = method.params.iter().find(|p| p.declaration_start == param.0)
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            Ok((*owner, target.name.as_str()))
                        }).collect::<Result<_, PrivateScalarCallRuntimeError>>()?;

                        let temps: Vec<_> = moves.iter().filter_map(|(s,_,_)| match s {
                            crate::owned_str_runtime::StrArgumentSource::Temporary(id) => Some(*id), _ => None,
                        }).collect();
                        if !frame.pending_owned_returns.ends_with(&temps) { return Err(PrivateScalarCallRuntimeError::StateMismatch); }
                        let tracker = &mut frame.borrow_tracker;
                        let ticket = tracker.acquire(site.loan.source, site.loan.kind)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        let mut owned_arguments = Vec::new();
                        let mut helpers = Vec::new();
                        let mut branches = Vec::new();
                        let mut method_locals = Vec::new();
                        let base = resources.events().len();
                        let mut trace = vec![PrivateSharedCallExitEvent::LoanAcquired(ticket),
                            PrivateSharedCallExitEvent::FunctionEntered];
                        let read = tracker.check_ticket_access(ticket, site.loan.kind)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)
                            .and_then(|owner| {
                                if owner != site.loan.source { return Err(PrivateScalarCallRuntimeError::StateMismatch); }
                                let ids = if names.is_empty() { Vec::new() } else {
                                    let ids = resources.enter_with_moved_arguments(&names, &mut pending_str_returns)
                                        .map_err(PrivateScalarCallRuntimeError::Resource)?;
                                    frame.pending_owned_returns.truncate(frame.pending_owned_returns.len() - temps.len());
                                    ids
                                };
                                owned_arguments.extend(moves.iter().zip(ids).map(|((_, source, param), id)| (*source,*param,id)));
                                let result = site.run_body(parameters, &mut copy_structs, &mut PrivateMethodExecution {
                                    resources: &mut resources, steps: &mut steps, limit: budget.max_steps, helper_frame_available, trace: &mut trace, helpers: &mut helpers, branches: &mut branches, locals: &mut method_locals,
                                });
                                let result = result.and_then(|value| match value {
                                    PrivateMethodValue::Int(n) => Ok(PrivateMethodValue::Int(n)),
                                    PrivateMethodValue::Bool(b) => Ok(PrivateMethodValue::Bool(b)),
                                    PrivateMethodValue::Str(source) => {
                                        let owned = match source {
                                            PrivateMethodReturnedStr::Source(PrivateScalarStrReturn::Move { declaration, .. }) => {
                                                let index = moves.iter().position(|(_,_,target)| target == declaration)
                                                    .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                                resources.return_named(names[index].1).map_err(PrivateScalarCallRuntimeError::Resource)?
                                            }
                                            PrivateMethodReturnedStr::Detached(value) => {
                                                resources.mark_detached_return(&value);
                                                value
                                            }
                                            PrivateMethodReturnedStr::Source(PrivateScalarStrReturn::Literal(text)) => {
                                                let value = resources.create_detached(text).map_err(PrivateScalarCallRuntimeError::Resource)?;
                                                resources.mark_detached_return(&value);
                                                value
                                            }
                                        };
                                        let rid = owned.resource_id();
                                        frame.pending_owned_returns.push(rid);
                                        pending_str_returns.push(owned);
                                        Ok(PrivateMethodValue::Str(rid))
                                    }
                                });
                                if !names.is_empty() { resources.leave_scope().map_err(PrivateScalarCallRuntimeError::Resource)?; }
                                result
                            });
                        if read.is_ok() { trace.push(PrivateSharedCallExitEvent::Returned); }
                        else if !trace.contains(&PrivateSharedCallExitEvent::UnwindStarted) {
                            trace.push(PrivateSharedCallExitEvent::UnwindStarted);
                        }
                        trace.extend(method_locals.into_iter().rev().map(PrivateSharedCallExitEvent::LocalScopeEnded));
                        tracker.release(ticket)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        trace.push(PrivateSharedCallExitEvent::LoanReturned(ticket));
                        for arg in argument_tickets.iter().rev() {
                            tracker.release(*arg).map_err(PrivateScalarCallRuntimeError::Borrow)?;
                            trace.push(PrivateSharedCallExitEvent::LoanReturned(*arg));
                        }
                        trace.push(PrivateSharedCallExitEvent::FunctionExited { returned: read.is_ok() });
                        completed_arguments.truncate(completed_arguments.len() - argument_tickets.len() - moves.len());
                        let ownership_events = resources.events()[base..].to_vec();
                        resource_cursor = resources.events().len();
                        events.push(PrivateScalarCallEvent::SelfRead { helpers, branches, owned_arguments, ownership_events, site: site.clone(), ticket, result: read, trace,
                            argument_tickets: argument_tickets.clone() });
                        events.extend(argument_tickets.into_iter().rev().map(PrivateScalarCallEvent::LoanReturned));
                        match read? {
                            PrivateMethodValue::Int(n) => values.push(n),
                            PrivateMethodValue::Bool(b) => values.push(i64::from(b)),
                            PrivateMethodValue::Str(_) => {}
                        }
                    }
                    PrivateScalarTask::Evaluate(term, is_root_expression) => match term {
                        PrivateScalarReturnExpr::Parameter(id) => {
                            let value = frames.last()
                                .and_then(|frame| frame.values.get(id).copied())
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            values.push(value);
                        }
                        PrivateScalarReturnExpr::Constant(value) => values.push(*value),
                        PrivateScalarReturnExpr::BoolValue(expr) => {
                            tasks.push(PrivateScalarTask::EvaluateBoolArgument(expr));
                        }
                        PrivateScalarReturnExpr::StrCall { name, arguments } => {
                            tasks.push(PrivateScalarTask::ForwardStrReturn);
                            tasks.push(PrivateScalarTask::EnterCall(
                                name, arguments, false, false,
                            ));
                            Self::queue_call_arguments(&mut tasks, arguments);
                        }
                        PrivateScalarReturnExpr::StrValue(source) => {
                            let owned = match source {
                                PrivateScalarStrReturn::Literal(text) => {
                                    let owned = resources.create_detached(text).map_err(
                                        PrivateScalarCallRuntimeError::Resource,
                                    )?;
                                    resources.mark_detached_return(&owned);
                                    owned
                                }
                                PrivateScalarStrReturn::Move { source, declaration } => {
                                    let frame = frames.last().ok_or(
                                        PrivateScalarCallRuntimeError::StateMismatch,
                                    )?;
                                    if !frame.str_owner_scopes.iter().rev().flatten()
                                        .any(|(id, name)| id == declaration && name == source) {
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                    }
                                    frame.borrow_tracker.check_direct_access(*declaration)
                                        .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                                    resources.return_named(source).map_err(
                                        PrivateScalarCallRuntimeError::Resource,
                                    )?
                                }
                            };
                            let id = owned.resource_id();
                            frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?.pending_owned_returns.push(id);
                            pending_str_returns.push(owned);
                            Self::record_owned_resources(&resources, &mut resource_cursor, events);
                        }
                        PrivateScalarReturnExpr::SelfRead(site) => {
                            if site.result_type == "str" { tasks.push(PrivateScalarTask::ForwardStrReturn); }
                            tasks.push(PrivateScalarTask::EvaluateMethod(site));
                        }
                        PrivateScalarReturnExpr::CopyStructIntField {
                            declaration, owner, field,
                        } => {
                            let frame = frames.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            if !frame.copy_struct_scopes.iter().rev().flatten()
                                .any(|(id, known)| id == declaration && known == owner) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            let observed = copy_structs.field(owner, field).map_err(|_|
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let crate::mixed_struct_runtime::Observed::Int(number) = observed
                            else {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            };
                            values.push(number);
                        }
                        PrivateScalarReturnExpr::Unreachable =>
                            return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        PrivateScalarReturnExpr::Addition(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('+'));
                            tasks.push(PrivateScalarTask::Evaluate(right, false));
                            tasks.push(PrivateScalarTask::Evaluate(left, false));
                        }
                        PrivateScalarReturnExpr::Subtraction(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('-'));
                            tasks.push(PrivateScalarTask::Evaluate(right, false));
                            tasks.push(PrivateScalarTask::Evaluate(left, false));
                        }
                        PrivateScalarReturnExpr::Call {
                            name, arguments, awaited,
                        } => {
                            // 所有子呼叫實參在父框架從左到右求值；
                            // 其中任何一項錯誤時，尚未 Enter 的函式
                            // 不得產生假的進入或返回事件。
                            tasks.push(PrivateScalarTask::EnterCall(
                                name, arguments, is_root_expression, *awaited,
                            ));
                            Self::queue_call_arguments(&mut tasks, arguments);
                        }
                    },
                    PrivateScalarTask::RecordArgument => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        events.push(PrivateScalarCallEvent::ArgumentEvaluated(value));
                        completed_arguments.push(PrivateScalarEvaluatedArgument::Int(value));
                    }
                    PrivateScalarTask::EvaluateBoolArgument(expr) => {
                        if let PrivateScalarBoolExpr::Method(site) = expr {
                            tasks.push(PrivateScalarTask::EvaluateMethod(site));
                        } else if let PrivateScalarBoolExpr::Call { name, arguments } = expr {
                            // 真 bool 子呼叫使用同一 LIFO EnterCall／ExitCall，
                            // 不在 Rust 內遞迴求值或預先編碼假 int 結果。
                            tasks.push(PrivateScalarTask::EnterCall(
                                name, arguments, false, false,
                            ));
                            Self::queue_call_arguments(&mut tasks, arguments);
                        } else {
                            let frame = frames.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let value = expr.evaluate_runtime(frame, &copy_structs)?;
                            // 0/1 只作為 Bool 型別已核准後的臨時工作棧編碼。
                            values.push(i64::from(value));
                        }
                    }
                    PrivateScalarTask::RecordBoolArgument => {
                        let encoded = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !matches!(encoded, 0 | 1) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        events.push(PrivateScalarCallEvent::BoolArgumentEvaluated(encoded == 1));
                        completed_arguments.push(
                            PrivateScalarEvaluatedArgument::Bool(encoded == 1),
                        );
                    }
                    PrivateScalarTask::CreateStrLiteralArgument(text) => {
                        let owned = resources.create_detached(text).map_err(
                            PrivateScalarCallRuntimeError::Resource,
                        )?;
                        let resource_id = owned.resource_id();
                        frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.pending_owned_returns.push(resource_id);
                        pending_str_returns.push(owned);
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                        events.push(
                            PrivateScalarCallEvent::StrLiteralTemporaryCreated(resource_id),
                        );
                        completed_arguments.push(
                            PrivateScalarEvaluatedArgument::TemporaryStr { resource_id },
                        );
                    }
                    PrivateScalarTask::ForwardStrReturn => {
                        let returned = pending_str_returns.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let resource_id = returned.resource_id();
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.pending_owned_returns.last() != Some(&resource_id)
                            || frame.str_owner_scopes.iter().flatten().any(|(_, name)|
                                resources.resource_id(name).ok() == Some(resource_id)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 子函式已發出第一個 Returned；在當前函式真正
                        // 到達 return 時再記錄同一 ID 的 Returned，不搬動
                        // OwnedUtf8，清理責任仍交由此幀 ExitCall／回滾。
                        resources.mark_detached_return(returned);
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                    }
                    PrivateScalarTask::RecordStrArgument => {
                        let resource_id = pending_str_returns.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.resource_id();
                        if frames.last().is_none_or(|frame|
                            frame.pending_owned_returns.last() != Some(&resource_id)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        completed_arguments.push(
                            PrivateScalarEvaluatedArgument::TemporaryStr { resource_id },
                        );
                    }
                    PrivateScalarTask::BorrowArgument(owner, fact) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let active = frame.str_owner_scopes.iter().rev()
                            .flat_map(|scope| scope.iter()).any(|(id, name)|
                                *id == fact.source && name == owner);
                        if !active {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 先確認資源仍有效，再取得本次動態借用真票據。
                        resources.read(owner)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        let resource_id = resources.resource_id(owner)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        let ticket = frame.borrow_tracker.acquire(fact.source, fact.kind)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        frame.borrow_tracker.check_ticket_access(
                            ticket, BorrowKind::SharedRead,
                        ).map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        events.push(PrivateScalarCallEvent::LoanAcquired(ticket));
                        events.push(PrivateScalarCallEvent::LoanResourceLinked(
                            ticket, resource_id,
                        ));
                        completed_arguments.push(
                            PrivateScalarEvaluatedArgument::Borrow { ticket, fact },
                        );
                    }
                    PrivateScalarTask::MoveArgument(owner, source) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !frame.str_owner_scopes.iter().rev().flatten()
                            .any(|(id, name)| *id == source && name == owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(source)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        resources.read(owner)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        // 已驗證真來源；到被呼叫框架建立時才原子搬移，
                        // 後續實參失敗則仍由呼叫者清理唯一 UTF-8。
                        completed_arguments.push(
                            PrivateScalarEvaluatedArgument::Move { source },
                        );
                    }
                    PrivateScalarTask::Apply(operator) => {
                        let right = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let left = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match operator {
                            '+' => left.checked_add(right),
                            '-' => left.checked_sub(right),
                            _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        }.ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)?;
                        values.push(value);
                    }
                    PrivateScalarTask::DeclareEmptyInt(id) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.values.contains_key(&id)
                            || frame.initialized_order.iter().any(|entry| matches!(entry,
                                PrivateScalarLocalBinding::Int(old)
                                    | PrivateScalarLocalBinding::UninitializedInt(old)
                                    if *old == id
                            )) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(
                            PrivateScalarLocalBinding::UninitializedInt(id),
                        );
                        events.push(PrivateScalarCallEvent::LocalDeclaredEmpty(id));
                    }
                    PrivateScalarTask::DeclareEmptyBool(id) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.bool_values.contains_key(&id)
                            || frame.initialized_order.iter().any(|entry| matches!(entry,
                                PrivateScalarLocalBinding::Bool(old)
                                    | PrivateScalarLocalBinding::UninitializedBool(old)
                                    if *old == id
                            )) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(
                            PrivateScalarLocalBinding::UninitializedBool(id),
                        );
                        events.push(PrivateScalarCallEvent::BoolDeclaredEmpty(id));
                    }
                    PrivateScalarTask::BindLocal(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        // 一筆區域宣告在單次動態框架內只可初始化一次；
                        // 同名遮蔽仍由原 Checker 決定不同 DeclarationId。
                        if frame.values.insert(id, value).is_some() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(PrivateScalarLocalBinding::Int(id));
                        events.push(PrivateScalarCallEvent::LocalInitialized(id, value));
                    }
                    PrivateScalarTask::WriteCopyStructBool(id, owner, field, expr) => {
                        if let Some(call @ PrivateScalarBoolExpr::Method(_)) = expr {
                            tasks.push(PrivateScalarTask::WriteCopyStructBool(id, owner, field, None));
                            tasks.push(PrivateScalarTask::EvaluateBoolArgument(call));
                            continue;
                        }
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(id)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        let value = match expr {
                            Some(expr) => expr.evaluate_runtime(frame, &copy_structs)?,
                            None => match values.pop() {
                                Some(0) => false, Some(1) => true,
                                _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                            },
                        };
                        copy_structs.write_bool_field(owner, field, value, true).map_err(|_|
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        events.push(PrivateScalarCallEvent::CopyStructBoolFieldAssigned(
                            id, field.to_owned(), value,
                        ));
                    }
                    PrivateScalarTask::WriteCopyStructStr(id, owner, field, text) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(id)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        // 同一 P16 真持有者對原 Checker var str 欄位先配置
                        // 新 UTF-8，再以原子交換釋放舊值；任何前置拒絕
                        // 都不應建立 ResourceId 或錯動原持有者。
                        copy_structs.write_str_field_with_shared_ledger(
                            &mut resources, owner, field, text, true,
                        ).map_err(|_| PrivateScalarCallRuntimeError::StateMismatch)?;
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                        let resource_id = copy_structs.field_resource_ids(owner)
                            .map_err(|_| PrivateScalarCallRuntimeError::StateMismatch)?
                            .into_iter().find_map(|(name, id)|
                                (name == field).then_some(id)
                            ).ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                        // 必須在舊 ResourceId 真釋放後才綁定新身分。
                        events.push(PrivateScalarCallEvent::CopyStructResourceOwnerBound {
                            declaration: id, field: field.to_owned(), resource_id,
                        });
                        events.push(PrivateScalarCallEvent::CopyStructStrFieldAssigned(
                            id, field.to_owned(), resource_id,
                        ));
                    }
                    PrivateScalarTask::WriteCopyStructInt(id, owner, field) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(id)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        copy_structs.write_int_field(owner, field, value, true).map_err(|_|
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        events.push(PrivateScalarCallEvent::CopyStructIntFieldAssigned(
                            id, field.to_owned(), value,
                        ));
                    }
                    PrivateScalarTask::AssignLocal(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if let Some(slot) = frame.values.get_mut(&id) {
                            *slot = value;
                            events.push(PrivateScalarCallEvent::LocalAssigned(id, value));
                        } else {
                            let pending = frame.initialized_order.iter_mut()
                                .find(|entry| matches!(entry,
                                    PrivateScalarLocalBinding::UninitializedInt(old)
                                        if *old == id
                                )).ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            *pending = PrivateScalarLocalBinding::Int(id);
                            if frame.values.insert(id, value).is_some() {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            events.push(PrivateScalarCallEvent::LocalInitialized(id, value));
                        }
                    }
                    PrivateScalarTask::BindBool(id, expr) => {
                        if matches!(expr, PrivateScalarBoolExpr::Call { .. } | PrivateScalarBoolExpr::Method(_)) {
                            // 先完整執行真 bool 子框架，返回前不可提前宣告。
                            tasks.push(PrivateScalarTask::BindBoolFromValue(id));
                            tasks.push(PrivateScalarTask::EvaluateBoolArgument(expr));
                        } else {
                            let frame = frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let value = expr.evaluate_runtime(frame, &copy_structs)?;
                            if frame.bool_values.insert(id, value).is_some() {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            frame.initialized_order.push(PrivateScalarLocalBinding::Bool(id));
                            events.push(PrivateScalarCallEvent::BoolInitialized(id, value));
                        }
                    }
                    PrivateScalarTask::BindBoolFromValue(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !matches!(value, 0 | 1) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.bool_values.insert(id, value == 1).is_some() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(PrivateScalarLocalBinding::Bool(id));
                        events.push(PrivateScalarCallEvent::BoolInitialized(id, value == 1));
                    }
                    PrivateScalarTask::MoveCopyStruct(id, name, source, source_name) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if source == id || !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == source && active == source_name)
                            || frame.copy_struct_scopes.last().is_none_or(|scope|
                                scope.iter().any(|(known, active)| *known == id || active == name)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(source)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        // P16 對真持有的完整實例移動；舊 Slot 原位失效，
                        // 目標接管唯一值，不重建欄位、不生成假 ResourceId。
                        copy_structs.move_whole_with_shared_ledger(
                            &mut resources, source_name, name,
                        ).map_err(|_| PrivateScalarCallRuntimeError::StateMismatch)?;
                        if !copy_structs.events().is_empty() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.copy_struct_scopes.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.push((id, name.to_owned()));
                        frame.initialized_order.push(PrivateScalarLocalBinding::CopyStruct(id));
                        // 唯一身分先綁目標，再由同一 P16 ledger 真 Moved
                        // 立即逐筆對帳；不得人工合成或跳過資源事件。
                        events.push(PrivateScalarCallEvent::CopyStructMovedBound {
                            source, target: id,
                        });
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                    }
                    PrivateScalarTask::DeclareCopyStruct(id, name, plan) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.initialized_order.iter().any(|entry| matches!(
                            entry, PrivateScalarLocalBinding::CopyStruct(found) if *found == id
                        )) || frame.copy_struct_scopes.last().is_none_or(|scope|
                            scope.iter().any(|(_, known)| known == name)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 真來源具名欄位依原 Parser 書寫順序求值；
                        // 若某欄含普通函式呼叫，就在同一顯式任務棧
                        // 上實際 Enter/Exit，所有成功後才允許 P16 建構。
                        pending_copy_structs.push(Vec::new());
                        tasks.push(PrivateScalarTask::FinishCopyStruct(id, name, plan));
                        for (field, seed) in plan.named.iter().rev() {
                            tasks.push(PrivateScalarTask::CaptureCopyStructSeed(field, seed));
                            if let PrivateScalarCopyStructSeed::IntCall(expr) = seed {
                                tasks.push(PrivateScalarTask::Evaluate(expr, false));
                            }
                        }
                    }
                    PrivateScalarTask::CaptureCopyStructSeed(field, seed) => {
                        let value = if matches!(seed, PrivateScalarCopyStructSeed::IntCall(_)) {
                            ApprovedSeed::Int(values.pop().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?)
                        } else {
                            let frame = frames.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            seed.evaluate(frame, &copy_structs, &mut steps, budget.max_steps)?
                        };
                        pending_copy_structs.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.push((field.to_owned(), value));
                    }
                    PrivateScalarTask::FinishCopyStruct(id, name, plan) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.initialized_order.iter().any(|entry| matches!(
                            entry, PrivateScalarLocalBinding::CopyStruct(found) if *found == id
                        )) || frame.copy_struct_scopes.last().is_none_or(|scope|
                            scope.iter().any(|(_, known)| known == name)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 含真 UTF-8 的結構與普通 str 在同一詞彙層交錯時，
                        // 目前不具跨類別 LIFO 證明，必須在真建構前拒絕。
                        if plan.fields.iter().any(|(_, kind, _, _)|
                            *kind == crate::mixed_struct_runtime::ScalarKind::Str)
                            && frame.str_owner_scopes.last().is_none_or(|scope|
                                !scope.is_empty()) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let named_values = pending_copy_structs.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if named_values.len() != plan.named.len()
                            || !named_values.iter().zip(&plan.named)
                                .all(|((actual, value), (expected, seed))|
                                    actual == expected && value.kind() == seed.kind()) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 不另造評估模型：原 P16 VB16 名稱覆蓋及預設
                        // 仍由同一個正式 construct 按已核准次序執行。
                        let fields = plan.fields.iter().map(|(field, kind, default, immutable)|
                            crate::mixed_struct_runtime::MixedFieldSpec {
                                name: field, kind: *kind,
                                default: default.as_ref().map(ApprovedSeed::runtime),
                                immutable: *immutable,
                            }
                        ).collect::<Vec<_>>();
                        let named = named_values.iter().map(|(field, value)|
                            crate::mixed_struct_runtime::NamedMixed {
                                field, value: value.runtime(),
                            }
                        ).collect::<Vec<_>>();
                        let built = copy_structs.construct_with_shared_ledger(
                            &mut resources, name, &fields, &named, None,
                        );
                        // 即使原 P16 真具名／預設求值中途溢位，
                        // Created→Released 也要從同一資源帳真實留下。
                        Self::record_owned_resources(&resources, &mut resource_cursor, events);
                        built.map_err(|error| match error {
                            crate::mixed_struct_runtime::MixedError::EvaluationFailed(_) =>
                                PrivateScalarCallRuntimeError::IntegerOverflow,
                            _ => PrivateScalarCallRuntimeError::StateMismatch,
                        })?;
                        if !copy_structs.events().is_empty() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.copy_struct_scopes.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.push((id, name.to_owned()));
                        frame.initialized_order.push(PrivateScalarLocalBinding::CopyStruct(id));
                        events.push(PrivateScalarCallEvent::CopyStructConstructed(id));
                        // 只取 P16 真建構後唯一 OwnedUtf8 欄位的 ResourceId，
                        // 不以文字或模擬配置補造 Created／Moved／Released。
                        for (field, resource_id) in copy_structs.field_resource_ids(name)
                            .map_err(|_| PrivateScalarCallRuntimeError::StateMismatch)?
                        {
                            events.push(PrivateScalarCallEvent::CopyStructResourceOwnerBound {
                                declaration: id, field, resource_id,
                            });
                        }
                    }
                    PrivateScalarTask::DeclareEmptyStr(id, name, mutable) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if Self::current_scope_has_str_struct(frame, &copy_structs) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 詞彙持有者可先存在，但沒有 UTF-8 資源。
                        // 真值只會在後續原 Checker 授權的 assign 建立。
                        resources.declare_empty(name, mutable).map_err(
                            PrivateScalarCallRuntimeError::Resource,
                        )?;
                        frame.str_owner_scopes.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.push((id, name.to_owned()));
                    }
                    PrivateScalarTask::DeclareStr(id, name, mutable, initializer) => {
                        if matches!(initializer, PrivateScalarStrInitializer::Call { .. } | PrivateScalarStrInitializer::Method(_)) {
                            if Self::current_scope_has_str_struct(
                                frames.last().ok_or(
                                    PrivateScalarCallRuntimeError::StateMismatch,
                                )?, &copy_structs,
                            ) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            tasks.push(PrivateScalarTask::FinishStrReturnBinding(
                                id, name, mutable,
                            ));
                            match initializer {
                                PrivateScalarStrInitializer::Call { name: callee, arguments } => {
                                    tasks.push(PrivateScalarTask::EnterCall(callee, arguments, false, false));
                                    Self::queue_call_arguments(&mut tasks, arguments);
                                }
                                PrivateScalarStrInitializer::Method(site) => tasks.push(PrivateScalarTask::EvaluateMethod(site)),
                                _ => unreachable!("已驗證的字串呼叫"),
                            }
                            continue;
                        }
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        // 一個詞彙層中，先建含 const str 的 P16 結構，
                        // 再以真 Lexer 字面值建立普通 str，必由同一帳本
                        // 先釋放後建立的 str，再逆序釋放較早的結構。
                        // 此窄化安全子集不包含從外層取得既存 ResourceId
                        // 的 Move；它的建立先後不等於本層綁定先後。
                        if Self::current_scope_has_str_struct(frame, &copy_structs)
                            && !matches!(initializer, PrivateScalarStrInitializer::Literal(_)) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if let PrivateScalarStrInitializer::Move {
                            source, declaration,
                        } = initializer {
                            if !frame.str_owner_scopes.iter().rev().flatten()
                                .any(|(known, active)| known == declaration && active == source) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            frame.borrow_tracker.check_direct_access(*declaration)
                                .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        }
                        match initializer {
                            PrivateScalarStrInitializer::Literal(text) =>
                                resources.declare(name, mutable, text),
                            PrivateScalarStrInitializer::Move { source, .. } =>
                                resources.move_named_as(source, name, mutable),
                            PrivateScalarStrInitializer::Method(_) | PrivateScalarStrInitializer::Call { .. } =>
                                return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        }.map_err(PrivateScalarCallRuntimeError::Resource)?;
                        frame.str_owner_scopes.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?.push((id, name.to_owned()));
                        Self::record_owned_resources(&resources, &mut resource_cursor, events);
                        let resource_id = resources.resource_id(name)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        events.push(PrivateScalarCallEvent::ResourceOwnerBound(
                            id, resource_id,
                        ));
                    }
                    PrivateScalarTask::FinishStrReturnBinding(id, name, mutable) => {
                        let returned = pending_str_returns.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let resource_id = returned.resource_id();
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.pending_owned_returns.pop() != Some(resource_id) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if frame.str_owner_scopes.last().is_none_or(|scope|
                            scope.iter().any(|(known, active)| known == &id || active == name)) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if let Err((error, returned)) = resources.accept_return_as(
                            name, returned, mutable,
                        ) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::Resource(error));
                        }
                        frame.str_owner_scopes.last_mut().expect("原動態 str 作用域")
                            .push((id, name.to_owned()));
                        events.push(PrivateScalarCallEvent::StrReturnAccepted(id, resource_id));
                    }
                    PrivateScalarTask::FinishStrReturnAssignment(id, name) => {
                        let returned = pending_str_returns.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let resource_id = returned.resource_id();
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.pending_owned_returns.pop() != Some(resource_id) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if !frame.str_owner_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == name) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if let Err(error) = frame.borrow_tracker.check_direct_access(id) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::Borrow(error));
                        }
                        // P16 原 Checker 的已初始化 var str 不准直接以
                        // 普通函式 str-return 覆寫；目前僅合法首次賦值。
                        // 即使假造 Checked Source 也不能越過此動態防線。
                        if resources.resource_id(name).is_ok() {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if let Err((error, returned)) = resources.replace_return_as(name, returned) {
                            resources.release_detached(returned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            return Err(PrivateScalarCallRuntimeError::Resource(error));
                        }
                        // 原 Checker 首次賦值沒有舊資源可釋放；
                        // 同一新 ResourceId 只能由當前 DeclarationId 接管一次。
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                        events.push(PrivateScalarCallEvent::StrReturnAccepted(id, resource_id));
                    }
                    PrivateScalarTask::AssignStr(id, name, initializer) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        // 若本層已有較早完成的結構字串，只能從真正
                        // 新建立的 UTF-8 字面值替換；移入外層舊資源
                        // 尚無跨資源種類全域退出次序證明，維持拒絕。
                        if Self::current_scope_has_str_struct(frame, &copy_structs)
                            && matches!(initializer, PrivateScalarStrInitializer::Move { .. }) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if !frame.str_owner_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == name) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(id)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        if let PrivateScalarStrInitializer::Move { source, declaration } = initializer {
                            if *declaration == id || !frame.str_owner_scopes.iter().rev().flatten()
                                .any(|(known, active)| known == declaration && active == source) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            frame.borrow_tracker.check_direct_access(*declaration)
                                .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        }
                        if matches!(initializer, PrivateScalarStrInitializer::Call { .. } | PrivateScalarStrInitializer::Method(_)) {
                            // 現有 P16 含 str 結構與普通 str 同層交錯的全域
                            // 退出序尚未完整證明，不能藉此呼叫拓寬既有界線。
                            if Self::current_scope_has_str_struct(frame, &copy_structs) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            tasks.push(PrivateScalarTask::FinishStrReturnAssignment(id, name));
                            match initializer {
                                PrivateScalarStrInitializer::Call { name: callee, arguments } => {
                                    tasks.push(PrivateScalarTask::EnterCall(callee, arguments, false, false));
                                    Self::queue_call_arguments(&mut tasks, arguments);
                                }
                                PrivateScalarStrInitializer::Method(site) => tasks.push(PrivateScalarTask::EvaluateMethod(site)),
                                _ => unreachable!("已驗證的字串呼叫"),
                            }
                            continue;
                        }
                        match initializer {
                            PrivateScalarStrInitializer::Literal(text) =>
                                resources.assign(name, text),
                            PrivateScalarStrInitializer::Move { source, .. } =>
                                resources.replace_from(source, name),
                            PrivateScalarStrInitializer::Method(_) | PrivateScalarStrInitializer::Call { .. } =>
                                return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        }.map_err(PrivateScalarCallRuntimeError::Resource)?;
                        Self::record_owned_resources(&resources, &mut resource_cursor, events);
                        let resource_id = resources.resource_id(name)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        events.push(PrivateScalarCallEvent::ResourceOwnerBound(id, resource_id));
                    }
                    PrivateScalarTask::AssignBool(id, expr) => {
                        if matches!(expr, PrivateScalarBoolExpr::Call { .. } | PrivateScalarBoolExpr::Method(_)) {
                            tasks.push(PrivateScalarTask::AssignBoolFromValue(id));
                            tasks.push(PrivateScalarTask::EvaluateBoolArgument(expr));
                        } else {
                            let frame = frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let value = expr.evaluate_runtime(frame, &copy_structs)?;
                            if let Some(slot) = frame.bool_values.get_mut(&id) {
                                *slot = value;
                                events.push(PrivateScalarCallEvent::BoolAssigned(id, value));
                            } else {
                                let pending = frame.initialized_order.iter_mut()
                                    .find(|entry| matches!(entry,
                                        PrivateScalarLocalBinding::UninitializedBool(old)
                                            if *old == id
                                    )).ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                *pending = PrivateScalarLocalBinding::Bool(id);
                                if frame.bool_values.insert(id, value).is_some() {
                                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                }
                                events.push(PrivateScalarCallEvent::BoolInitialized(id, value));
                            }
                        }
                    }
                    PrivateScalarTask::AssignBoolFromValue(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !matches!(value, 0 | 1) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if let Some(slot) = frame.bool_values.get_mut(&id) {
                            *slot = value == 1;
                            events.push(PrivateScalarCallEvent::BoolAssigned(id, value == 1));
                        } else {
                            let pending = frame.initialized_order.iter_mut()
                                .find(|entry| matches!(entry,
                                    PrivateScalarLocalBinding::UninitializedBool(old)
                                        if *old == id
                                )).ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            *pending = PrivateScalarLocalBinding::Bool(id);
                            if frame.bool_values.insert(id, value == 1).is_some() {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            events.push(PrivateScalarCallEvent::BoolInitialized(id, value == 1));
                        }
                    }
                    PrivateScalarTask::CheckEarlyReturn(branch) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let mut selected = None;
                        let mut current = branch;
                        loop {
                            if matches!(&current.condition, PrivateScalarBoolExpr::Call { .. } | PrivateScalarBoolExpr::Method(_)) {
                                // 子函式必須先在相同的動態棧 Enter/Exit、簽真 BoolReturned，
                                // 才能將該返回值用作分支判斷；不得預先決定控制邊。
                                tasks.push(PrivateScalarTask::FinishEarlyCondition(current));
                                tasks.push(PrivateScalarTask::EvaluateBoolArgument(
                                    &current.condition,
                                ));
                                break;
                            }
                            let condition =
                                current.condition.evaluate_runtime(frame, &copy_structs)?;
                            events.push(PrivateScalarCallEvent::ConditionalChecked(condition));
                            if condition {
                                selected = Some(&current.when_true);
                                break;
                            }
                            match current.when_false.as_ref() {
                                Some(PrivateScalarEarlyElse::Branch(value)) => {
                                    selected = Some(value);
                                    break;
                                }
                                Some(PrivateScalarEarlyElse::ElseIf(next)) => {
                                    current = next;
                                }
                                None => break,
                            }
                        }
                        if let Some(selected) = selected {
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(selected));
                        }
                    }
                    PrivateScalarTask::FinishEarlyCondition(branch) => {
                        let encoded = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !matches!(encoded, 0 | 1) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let selected = encoded == 1;
                        events.push(PrivateScalarCallEvent::ConditionalChecked(selected));
                        if selected {
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(&branch.when_true));
                        } else {
                            match branch.when_false.as_ref() {
                                Some(PrivateScalarEarlyElse::Branch(action)) =>
                                    tasks.push(PrivateScalarTask::ExecuteBranchAction(action)),
                                Some(PrivateScalarEarlyElse::ElseIf(next)) =>
                                    tasks.push(PrivateScalarTask::CheckEarlyReturn(next)),
                                None => {}
                            }
                        }
                    }
                    PrivateScalarTask::CheckWhile(condition, body) => {
                        // P15：while 條件只能是原 Checker 准許的無資源副作用純值。
                        // 此處絕不偷開未核准的 bool 函式呼叫路徑。
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = condition.evaluate_runtime(frame, &copy_structs)?;
                        events.push(PrivateScalarCallEvent::ConditionalChecked(value));
                        if value {
                            tasks.push(PrivateScalarTask::CheckWhile(condition, body));
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(body));
                        }
                    }
                    PrivateScalarTask::CheckFor(body, id, next, upper) => {
                        if next < upper {
                            let frame = frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            if frame.values.insert(id, next).is_some() {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            frame.initialized_order.push(PrivateScalarLocalBinding::Int(id));
                            events.push(PrivateScalarCallEvent::LocalInitialized(id, next));
                            let after = next.checked_add(1).ok_or(
                                PrivateScalarCallRuntimeError::IntegerOverflow,
                            )?;
                            tasks.push(PrivateScalarTask::EndFor(body, id, after, upper));
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(body));
                        }
                    }
                    PrivateScalarTask::EndFor(body, id, next, upper) => {
                        // 先撤銷當輪的唯一定義，再開始下一輪，避免同一 ID 重複綁定。
                        tasks.push(PrivateScalarTask::CheckFor(body, id, next, upper));
                        tasks.push(PrivateScalarTask::UnbindIterator(id));
                    }
                    PrivateScalarTask::UnbindIterator(id) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if frame.values.remove(&id).is_none()
                            || !matches!(frame.initialized_order.pop(),
                                Some(PrivateScalarLocalBinding::Int(found)) if found == id)
                        {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        events.push(PrivateScalarCallEvent::LocalScopeEnded(id));
                    }
                    PrivateScalarTask::EndScope(baseline) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        Self::end_scalar_scope(frame, baseline, events)?;
                        if frame.resource_scopes <= 1 {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.resource_scopes -= 1;
                        if frame.str_owner_scopes.len() <= 1 {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.str_owner_scopes.pop();
                        if frame.copy_struct_scopes.len() <= 1 {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.copy_struct_scopes.pop();
                        resources.leave_scope().map_err(PrivateScalarCallRuntimeError::Resource)?;
                        copy_structs.leave_scope_with_shared_ledger(&mut resources).map_err(|_|
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        Self::record_owned_resources(&resources, &mut resource_cursor, events);
                    }
                    PrivateScalarTask::ExecuteBranchAction(action) => match action {
                        PrivateScalarBranchAction::While { condition, body } => {
                            tasks.push(PrivateScalarTask::CheckWhile(condition, body));
                        }
                        PrivateScalarBranchAction::ForRange { iterator, lower, upper, body } => {
                            tasks.push(PrivateScalarTask::CheckFor(
                                body, *iterator, *lower, *upper,
                            ));
                        }
                        PrivateScalarBranchAction::Break | PrivateScalarBranchAction::Continue => {
                            // 僅捨棄本次迭代尚未到達的工作，不裁掉父函式。
                            // 最近的 CheckWhile／EndFor 是動態最內層循環，
                            // break 只退出該層，continue 仍必須完成當輪離域。
                            let is_break = matches!(action, PrivateScalarBranchAction::Break);
                            loop {
                                match tasks.last() {
                                    Some(PrivateScalarTask::CheckWhile(..)) => {
                                        if is_break { tasks.pop(); }
                                        break;
                                    }
                                    Some(PrivateScalarTask::EndFor(_, id, _, _)) => {
                                        let id = *id;
                                        if is_break {
                                            tasks.pop();
                                            tasks.push(PrivateScalarTask::UnbindIterator(id));
                                        }
                                        break;
                                    }
                                    Some(PrivateScalarTask::EndScope(baseline)) => {
                                        let baseline = *baseline;
                                        let frame = frames.last_mut().ok_or(
                                            PrivateScalarCallRuntimeError::StateMismatch,
                                        )?;
                                        Self::end_scalar_scope(frame, baseline, events)?;
                                        if frame.resource_scopes <= 1 {
                                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                        }
                                        frame.resource_scopes -= 1;
                                        if frame.str_owner_scopes.len() <= 1 {
                                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                        }
                                        frame.str_owner_scopes.pop();
                                        if frame.copy_struct_scopes.len() <= 1 {
                                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                        }
                                        frame.copy_struct_scopes.pop();
                                        resources.leave_scope().map_err(
                                            PrivateScalarCallRuntimeError::Resource,
                                        )?;
                                        copy_structs.leave_scope_with_shared_ledger(&mut resources).map_err(|_|
                                            PrivateScalarCallRuntimeError::StateMismatch,
                                        )?;
                                        Self::record_owned_resources(
                                            &resources, &mut resource_cursor, events,
                                        );
                                        tasks.pop();
                                    }
                                    None | Some(PrivateScalarTask::ExitCall) =>
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch),
                                    _ => { tasks.pop(); }
                                }
                            }
                        }
                        PrivateScalarBranchAction::NestedIf(plan) => {
                            // 只執行已選真分支中的內層 if；
                            // bool 條件在前面更新完的真動態框架重新讀取。
                            tasks.push(PrivateScalarTask::CheckEarlyReturn(plan));
                        }
                        PrivateScalarBranchAction::Scoped(actions) => {
                            let frame = frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let baseline = frame.initialized_order.len();
                            frame.resource_scopes += 1;
                            frame.str_owner_scopes.push(Vec::new());
                            frame.copy_struct_scopes.push(Vec::new());
                            resources.enter_scope();
                            copy_structs.enter_scope();
                            tasks.push(PrivateScalarTask::EndScope(baseline));
                            for action in actions.iter().rev() {
                                tasks.push(PrivateScalarTask::ExecuteBranchAction(action));
                            }
                        }
                        PrivateScalarBranchAction::DeclareInt { declaration, value } => {
                            tasks.push(PrivateScalarTask::BindLocal(*declaration));
                            tasks.push(PrivateScalarTask::Evaluate(value, false));
                        }
                        PrivateScalarBranchAction::DeclareBool { declaration, value } => {
                            tasks.push(PrivateScalarTask::BindBool(*declaration, value));
                        }
                        PrivateScalarBranchAction::DeclareEmptyInt { declaration } =>
                            tasks.push(PrivateScalarTask::DeclareEmptyInt(*declaration)),
                        PrivateScalarBranchAction::DeclareEmptyBool { declaration } =>
                            tasks.push(PrivateScalarTask::DeclareEmptyBool(*declaration)),
                        PrivateScalarBranchAction::DeclareStr {
                            declaration, name, mutable, initializer,
                        } => {
                            tasks.push(PrivateScalarTask::DeclareStr(
                                *declaration, name, *mutable, initializer,
                            ));
                        }
                        PrivateScalarBranchAction::DeclareEmptyStr {
                            declaration, name, mutable,
                        } => tasks.push(PrivateScalarTask::DeclareEmptyStr(
                            *declaration, name, *mutable,
                        )),
                        PrivateScalarBranchAction::DeclareCopyStruct {
                            declaration, name, plan,
                        } => tasks.push(PrivateScalarTask::DeclareCopyStruct(
                            *declaration, name, plan,
                        )),
                        PrivateScalarBranchAction::MoveCopyStruct {
                            declaration, name, source, source_name,
                        } => tasks.push(PrivateScalarTask::MoveCopyStruct(
                            *declaration, name, *source, source_name,
                        )),
                        PrivateScalarBranchAction::AssignStr {
                            declaration, name, initializer,
                        } => tasks.push(PrivateScalarTask::AssignStr(
                            *declaration, name, initializer,
                        )),
                        PrivateScalarBranchAction::Sequence(actions) => {
                            // 原 Parser 分支先後次序必須保持；工作堆疊
                            // LIFO 以反序放入，未選中的分支不排任何工作。
                            for action in actions.iter().rev() {
                                tasks.push(PrivateScalarTask::ExecuteBranchAction(action));
                            }
                        }
                        PrivateScalarBranchAction::Return(returned) => {
                            // 提前 return 只裁掉目前函式尚未執行的語句，
                            // 保留目前 ExitCall 與呼叫者外層工作。
                            while !tasks.is_empty()
                                && !matches!(tasks.last(),
                                    Some(PrivateScalarTask::ExitCall)) {
                                tasks.pop();
                            }
                            tasks.push(PrivateScalarTask::Evaluate(returned, false));
                        }
                        PrivateScalarBranchAction::AssignInt { declaration, value } => {
                            tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                            tasks.push(PrivateScalarTask::Evaluate(value, false));
                        }
                        PrivateScalarBranchAction::AssignCopyStructInt {
                            declaration, owner, field, value,
                        } => {
                            tasks.push(PrivateScalarTask::WriteCopyStructInt(
                                *declaration, owner, field,
                            ));
                            tasks.push(PrivateScalarTask::Evaluate(value, false));
                        }
                        PrivateScalarBranchAction::AssignCopyStructBool {
                            declaration, owner, field, value,
                        } => tasks.push(PrivateScalarTask::WriteCopyStructBool(
                            *declaration, owner, field, Some(value),
                        )),
                        PrivateScalarBranchAction::AssignCopyStructStr {
                            declaration, owner, field, value,
                        } => tasks.push(PrivateScalarTask::WriteCopyStructStr(
                            *declaration, owner, field, value,
                        )),
                        PrivateScalarBranchAction::AssignBool { declaration, value } => {
                            tasks.push(PrivateScalarTask::AssignBool(*declaration, value));
                        }
                    },
                    PrivateScalarTask::EnterCall(
                        name, argument_plans, is_root_call, awaited,
                    ) => {
                        // 執行期仍以原 Checker 的唯一函式宣告核對
                        // async 身分；不能靠工作計畫的一個布林值
                        // 自行把同步函式變成可等待工作。
                        if self.verified.functions.iter()
                            .find(|fun| fun.name == name)
                            .is_none_or(|fun| fun.is_async != awaited) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        // 只有原 Checker 完整簽章能決定實參正式型別與借用位置。
                        let (parameter_ids, parameter_is_bool, parameter_is_str,
                            result_expr, local_steps): (
                            Vec<DeclarationId>, Vec<bool>, Vec<bool>,
                            &PrivateScalarReturnExpr, &Vec<PrivateScalarLocalStep>,
                        ) = if name == self.callee_name {
                            (
                                self.callee_parameter_ids.clone(),
                                self.callee_parameter_is_bool.clone(),
                                self.callee_parameter_is_str.clone(),
                                &self.return_expr, &self.root_local_steps,
                            )
                        } else {
                            let callee = self.nested_callees.iter()
                                .find(|fun| fun.name == name)
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            (
                                callee.parameters.clone(), callee.parameter_is_bool.clone(),
                                callee.parameter_is_str.clone(),
                                &callee.result, &callee.local_steps,
                            )
                        };
                        let argc = argument_plans.len();
                        if parameter_ids.len() != argc
                            || parameter_is_bool.len() != argc
                            || parameter_is_str.len() != argc
                            || completed_arguments.len() < argc {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if frames.len() >= budget.max_call_frames {
                            return Err(PrivateScalarCallRuntimeError::CallBudget);
                        }
                        let evaluated = completed_arguments[
                            completed_arguments.len() - argc..
                        ].to_vec();
                        let mut locals = BTreeMap::new();
                        let mut bool_locals = BTreeMap::new();
                        let mut incoming_borrows = Vec::new();
                        let mut pending_moves = Vec::new();
                        let mut pending_temporaries = Vec::new();
                        let mut bound_events = Vec::new();
                        for (index, (plan, actual)) in argument_plans.iter()
                            .zip(evaluated).enumerate() {
                            let id = parameter_ids[index];
                            match (parameter_is_str[index], parameter_is_bool[index],
                                plan, actual) {
                                (true, false,
                                    PrivateScalarCallArgument::BorrowStr { fact, .. },
                                    PrivateScalarEvaluatedArgument::Borrow {
                                        ticket, fact: observed,
                                    }) if *fact == observed && ticket.fact() == *fact => {
                                    incoming_borrows.push((ticket, *fact));
                                    bound_events.push(
                                        PrivateScalarCallEvent::BorrowParameterBound(id, *fact),
                                    );
                                }
                                (true, false,
                                    PrivateScalarCallArgument::MoveStr { owner, source },
                                    PrivateScalarEvaluatedArgument::Move { source: observed })
                                    if *source == observed => {
                                    let actual = self.verified.functions.iter()
                                        .find(|fun| fun.name == name)
                                        .and_then(|fun| fun.params.get(index))
                                        .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                    if DeclarationId(actual.declaration_start) != id
                                        || actual.ty != "str" {
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                    }
                                    pending_moves.push((
                                        owner.clone(), *source, id, actual.name.clone(),
                                    ));
                                }
                                (true, false,
                                    PrivateScalarCallArgument::NestedStrCall { .. }
                                        | PrivateScalarCallArgument::LiteralStr(_),
                                    PrivateScalarEvaluatedArgument::TemporaryStr { resource_id }) => {
                                    let actual = self.verified.functions.iter()
                                        .find(|fun| fun.name == name)
                                        .and_then(|fun| fun.params.get(index))
                                        .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                    if DeclarationId(actual.declaration_start) != id
                                        || actual.ty != "str" {
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                    }
                                    pending_temporaries.push((
                                        resource_id, id, actual.name.clone(),
                                    ));
                                }
                                (false, true, PrivateScalarCallArgument::Bool(_),
                                    PrivateScalarEvaluatedArgument::Bool(value)) => {
                                    if bool_locals.insert(id, value).is_some() {
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                    }
                                    bound_events.push(
                                        PrivateScalarCallEvent::BoolParameterBound(id, value),
                                    );
                                }
                                (false, false, PrivateScalarCallArgument::Int(_),
                                    PrivateScalarEvaluatedArgument::Int(value)) => {
                                    if locals.insert(id, value).is_some() {
                                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                    }
                                    bound_events.push(
                                        PrivateScalarCallEvent::ParameterBound(id, value),
                                    );
                                }
                                _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                            }
                        }
                        // 被呼叫者不擁有來源；真票據仍留在直接呼叫者 tracker。
                        for (ticket, fact) in &incoming_borrows {
                            let parent = frames.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            if parent.borrow_tracker.check_ticket_access(
                                *ticket, BorrowKind::SharedRead,
                            ).map_err(PrivateScalarCallRuntimeError::Borrow)? != fact.source {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                        }
                        // 真 Move 原始來源必屬目前 caller 的活擁有者；
                        // 先核對所有來源與借用衝突，再移交，避免部分改動。
                        let mut distinct_sources = std::collections::BTreeSet::new();
                        for (owner, source, _, _) in &pending_moves {
                            let parent = frames.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            if !distinct_sources.insert(*source)
                                || !parent.str_owner_scopes.iter().rev().flatten()
                                    .any(|(id, name)| id == source && name == owner) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            parent.borrow_tracker.check_direct_access(*source)
                                .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                            resources.read(owner)
                                .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        }
                        // 多個 str-return 暫存可同時存活：各個唯一 ResourceId
                        // 仍由同一 caller 持有，不依字串內容或假身分推定。
                        let mut distinct_temporary_ids = std::collections::BTreeSet::new();
                        for (resource_id, _, _) in &pending_temporaries {
                            if !distinct_temporary_ids.insert(*resource_id)
                                || !pending_str_returns.iter().any(|owned|
                                    owned.resource_id() == *resource_id)
                                || frames.last().is_none_or(|frame|
                                    !frame.pending_owned_returns.contains(resource_id)) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                        }
                        if awaited {
                            // 真安全點：普通框架上的原 BorrowSafety
                            // 票據、及上一層傳入但尚未歸還的有效借用，
                            // 任一仍活躍即不得開始 async 函式。
                            // 先拒絕再消耗 completed_arguments，保留
                            // 正式 R05 錯誤退出可歸還的票據來源。
                            Self::check_await_safe_point_chain(frames.iter().map(
                                |frame| (&frame.borrow_tracker,
                                    frame.incoming_borrows.as_slice()),
                            ))?;
                            // 尚未動到 P16 真資源／callee frame 或消耗本次實參。
                            // 完整保存顯式工作堆疊、計數、值棧、真票據及
                            // OwnedStrRuntime；下一次 poll 從同一 EnterCall 恢復。
                            if allow_pause && !resuming_current_await {
                                tasks.push(PrivateScalarTask::EnterCall(
                                    name, argument_plans, is_root_call, awaited,
                                ));
                                steps -= 1; // 重開時才對本個真 EnterCall 計步
                                paused_await = Some(name.to_owned());
                                break 'graph;
                            }
                            resuming_current_await = false;
                        }
                        completed_arguments.truncate(completed_arguments.len() - argc);
                        if awaited {
                            events.push(PrivateScalarCallEvent::AwaitSafePoint(name.to_owned()));
                            // 真來源的 await、安全借用守門及所有實參皆已完成，
                            // 子函式尚未 Enter；在此點協作執行另一核准工作。
                            // 觀測到的 ResourceEvent 唯一出自當下 P16 真帳。
                            if let Some(schedule) = await_scheduler.as_deref_mut() {
                                schedule(name, resources.events());
                            }
                        }
                        events.push(PrivateScalarCallEvent::FunctionEntered(name.to_owned()));
                        events.extend(bound_events);
                        resources.enter_scope();
                        copy_structs.enter_scope();
                        frames.push(PrivateScalarActiveFrame {
                            name: name.to_owned(), values: locals,
                            bool_values: bool_locals,
                            initialized_order: Vec::new(),
                            resource_scopes: 1,
                            str_owner_scopes: vec![Vec::new()],
                            copy_struct_scopes: vec![Vec::new()],
                            borrow_tracker: crate::borrow_safety::BorrowSafety::new(),
                            incoming_borrows,
                            pending_owned_returns: Vec::new(),
                        });
                        // 僅以真 OwnedStrRuntime 的同一 ID 移動，不重新配置
                        // 字串；在新框架中以原 Checker 正式參數 ID 接管。
                        for (owner, source, parameter, target) in pending_moves {
                            let resource_id = resources.resource_id(&owner)
                                .map_err(PrivateScalarCallRuntimeError::Resource)?;
                            resources.move_named_as(&owner, &target, false)
                                .map_err(PrivateScalarCallRuntimeError::Resource)?;
                            events.push(PrivateScalarCallEvent::ResourceTransferred {
                                source, target: parameter, resource_id,
                            });
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?.str_owner_scopes[0].push((parameter, target));
                            events.push(PrivateScalarCallEvent::ResourceOwnerBound(
                                parameter, resource_id,
                            ));
                        }
                        // 原 Checker 巢狀 Returned 臨時值直接交付下個 str 形參。
                        // 同一 ResourceId 只轉移一次，不能合成來源 DeclarationId。
                        for (resource_id, parameter, target) in pending_temporaries {
                            let parent_index = frames.len().checked_sub(2).ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            let owned_index = pending_str_returns.iter()
                                .position(|owned| owned.resource_id() == resource_id)
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            let holder_index = frames[parent_index].pending_owned_returns.iter()
                                .position(|id| *id == resource_id)
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            let returned = pending_str_returns.remove(owned_index);
                            if let Err((error, returned)) =
                                resources.accept_temporary_argument_as(&target, returned) {
                                pending_str_returns.push(returned);
                                return Err(PrivateScalarCallRuntimeError::Resource(error));
                            }
                            frames[parent_index].pending_owned_returns.remove(holder_index);
                            events.push(PrivateScalarCallEvent::StrTemporaryTransferred {
                                target: parameter, resource_id,
                            });
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                            frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?.str_owner_scopes[0].push((parameter, target));
                            events.push(PrivateScalarCallEvent::ResourceOwnerBound(
                                parameter, resource_id,
                            ));
                        }
                        // 只有源自 main return 最外層的真 Call 才允許
                        // 故障注入；先求值的同名子呼叫絕不可誤作根框架。
                        if fail_after_root_binding && is_root_call {
                            return Err(PrivateScalarCallRuntimeError::InjectedFailure);
                        }
                        tasks.push(PrivateScalarTask::ExitCall);
                        tasks.push(PrivateScalarTask::Evaluate(result_expr, false));
                        for step in local_steps.iter().rev() {
                            match step {
                                PrivateScalarLocalStep::Declare(init) => {
                                    tasks.push(PrivateScalarTask::BindLocal(init.declaration));
                                    tasks.push(PrivateScalarTask::Evaluate(&init.value, false));
                                }
                                PrivateScalarLocalStep::Assign { declaration, value } => {
                                    tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                                }
                                PrivateScalarLocalStep::AssignCopyStructInt {
                                    declaration, owner, field, value,
                                } => {
                                    tasks.push(PrivateScalarTask::WriteCopyStructInt(
                                        *declaration, owner, field,
                                    ));
                                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                                }
                                PrivateScalarLocalStep::AssignCopyStructBool {
                                    declaration, owner, field, value,
                                } => tasks.push(PrivateScalarTask::WriteCopyStructBool(
                                    *declaration, owner, field, Some(value),
                                )),
                                PrivateScalarLocalStep::AssignCopyStructStr {
                                    declaration, owner, field, value,
                                } => tasks.push(PrivateScalarTask::WriteCopyStructStr(
                                    *declaration, owner, field, value,
                                )),
                                PrivateScalarLocalStep::DeclareEmptyInt(declaration) =>
                                    tasks.push(PrivateScalarTask::DeclareEmptyInt(*declaration)),
                                PrivateScalarLocalStep::DeclareEmptyBool(declaration) =>
                                    tasks.push(PrivateScalarTask::DeclareEmptyBool(*declaration)),
                                PrivateScalarLocalStep::DeclareBool { declaration, value } =>
                                    tasks.push(PrivateScalarTask::BindBool(*declaration, value)),
                                PrivateScalarLocalStep::AssignBool { declaration, value } =>
                                    tasks.push(PrivateScalarTask::AssignBool(*declaration, value)),
                                PrivateScalarLocalStep::DeclareStr {
                                    declaration, name, mutable, initializer,
                                } => tasks.push(PrivateScalarTask::DeclareStr(
                                    *declaration, name, *mutable, initializer,
                                )),
                                PrivateScalarLocalStep::DeclareEmptyStr {
                                    declaration, name, mutable,
                                } => tasks.push(PrivateScalarTask::DeclareEmptyStr(
                                    *declaration, name, *mutable,
                                )),
                                PrivateScalarLocalStep::DeclareCopyStruct {
                                    declaration, name, plan,
                                } => tasks.push(PrivateScalarTask::DeclareCopyStruct(
                                    *declaration, name, plan,
                                )),
                                PrivateScalarLocalStep::MoveCopyStruct {
                                    declaration, name, source, source_name,
                                } => tasks.push(PrivateScalarTask::MoveCopyStruct(
                                    *declaration, name, *source, source_name,
                                )),
                                PrivateScalarLocalStep::AssignStr {
                                    declaration, name, initializer,
                                } => tasks.push(PrivateScalarTask::AssignStr(
                                    *declaration, name, initializer,
                                )),
                                PrivateScalarLocalStep::EarlyReturn(plan) =>
                                    tasks.push(PrivateScalarTask::CheckEarlyReturn(plan)),
                                PrivateScalarLocalStep::Control(action) =>
                                    tasks.push(PrivateScalarTask::ExecuteBranchAction(action)),
                            }
                        }
                    }
                    PrivateScalarTask::ExitCall => {
                        if frames.len() <= 1 {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let returning = self.verified.functions.iter()
                            .find(|fun| fun.name == frames.last()
                                .map(|frame| frame.name.as_str()).unwrap_or(""))
                            .map(|fun| fun.result_ty.as_str())
                            .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                        let value = if returning == "str" {
                            None
                        } else {
                            Some(values.pop().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?)
                        };
                        let mut frame = frames.pop().expect("真子框架仍存在");
                        let mut str_return_id = None;
                        if returning == "str" {
                            let resource_id = pending_str_returns.last().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?.resource_id();
                            if frame.pending_owned_returns.pop() != Some(resource_id) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            str_return_id = Some(resource_id);
                            events.push(PrivateScalarCallEvent::StrReturned(resource_id));
                        } else if returning == "bool" {
                            let value = value.ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                            if !matches!(value, 0 | 1) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            events.push(PrivateScalarCallEvent::BoolReturned(value == 1));
                        } else {
                            events.push(PrivateScalarCallEvent::Returned(value.ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?));
                        }
                        let parent = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        Self::return_incoming_borrows(
                            parent, &frame.incoming_borrows, events,
                        )?;
                        // F2：已 Returned、尚待作為外層 str 實參交付的
                        // Detached OwnedUtf8 必須在「本 frame」離域前反序
                        // 釋放，不能留到 root 再無憑據地補發 Released。
                        for id in frame.pending_owned_returns.drain(..).rev() {
                            if pending_str_returns.last()
                                .map(|owned| owned.resource_id()) != Some(id) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            let owned = pending_str_returns.pop().expect("F2 暫存 LIFO");
                            resources.release_detached(owned);
                            Self::record_owned_resources(
                                &resources, &mut resource_cursor, events,
                            );
                        }
                        for entry in frame.initialized_order.iter().rev() {
                            match entry {
                                PrivateScalarLocalBinding::Int(id) =>
                                    events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                                PrivateScalarLocalBinding::Bool(id) =>
                                    events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                                PrivateScalarLocalBinding::UninitializedInt(id) =>
                                    events.push(PrivateScalarCallEvent::LocalScopeEndedEmpty(*id)),
                                PrivateScalarLocalBinding::UninitializedBool(id) =>
                                    events.push(PrivateScalarCallEvent::BoolScopeEndedEmpty(*id)),
                                PrivateScalarLocalBinding::CopyStruct(id) =>
                                    events.push(PrivateScalarCallEvent::CopyStructScopeEnded(*id)),
                            }
                        }
                        frame.values.clear();
                        if frame.copy_struct_scopes.len() != frame.resource_scopes {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        for _ in 0..frame.resource_scopes {
                            resources.leave_scope().map_err(
                                PrivateScalarCallRuntimeError::Resource,
                            )?;
                            copy_structs.leave_scope_with_shared_ledger(&mut resources).map_err(|_|
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?;
                        }
                        Self::record_owned_resources(
                            &resources, &mut resource_cursor, events,
                        );
                        events.push(PrivateScalarCallEvent::FunctionExited {
                            name: frame.name, returned: true,
                        });
                        if let Some(id) = str_return_id {
                            frames.last_mut().ok_or(
                                PrivateScalarCallRuntimeError::StateMismatch,
                            )?.pending_owned_returns.push(id);
                        }
                        if let Some(value) = value { values.push(value); }
                    }
                }
            }
            if paused_await.is_some() {
                return Ok(None);
            }
            if frames.len() != 1 || values.len() != 1
                || !pending_str_returns.is_empty()
                || !completed_arguments.is_empty()
                || !pending_copy_structs.is_empty() {
                return Err(PrivateScalarCallRuntimeError::StateMismatch);
            }
            Ok(Some(values[0]))
        })()
        };
        // 真正處於合法 await 邊時不觸發原工作圖的終止清理。
        // 不回滾／重放任何已經產生的 ResourceEvent、函式進入或求值。
        if let Some(awaiting_name) = paused_await {
            *checkpoint = Some(PrivateScalarGraphCheckpoint {
                resources, copy_structs, resource_cursor, frames, tasks, values,
                pending_str_returns, completed_arguments, pending_copy_structs,
                steps, awaiting_name,
            });
            return Ok(None);
        }
        // 回滾借用帳須延續到根框架的最後資源清理階段；
        // 成功路徑維持空清單，不改動既有真函式返回語意。
        let mut pending_loans = Vec::new();
        let must_unwind = cancel_at_checkpoint || result.is_err();
        if must_unwind {
            events.push(PrivateScalarCallEvent::UnwindStarted);
            // 先保留 pending 票據的逆序清單；它們可能分屬多個
            // 已進入的呼叫者框架。外層 Call 尚未 Enter 時，不得
            // 在內層失敗 frame 結束之前錯誤先還外層票據。
            pending_loans = completed_arguments.iter().rev()
                .filter_map(|arg| match arg {
                    PrivateScalarEvaluatedArgument::Borrow { ticket, .. } => Some(*ticket),
                    _ => None,
                }).collect::<Vec<_>>();
            // 執行期預算／溢位／狀態錯誤皆不得偽裝成 return。
            // 每層先還本層尚未 Enter 的參數借用，再歸還該
            // 已進入 frame 的 incoming 票據，最後才釋放資源。
            while frames.len() > 1 {
                Self::return_pending_borrows_in_frame(
                    frames.last_mut().expect("存在內層執行框架"),
                    &mut pending_loans, events,
                )?;
                let mut frame = frames.pop().expect("已進入的子框架");
                let parent = frames.last_mut().ok_or(
                    PrivateScalarCallRuntimeError::StateMismatch,
                )?;
                Self::return_incoming_borrows(
                    parent, &frame.incoming_borrows, events,
                )?;
                // F2：callee 已 Returned、但 caller 尚未完成接管時，
                // 故障只能由「原持有該 Detached 暫存的本 frame」
                // 記錄 Released。禁止所有函式退出後才在 root 補釋放，
                // 否則 Created/Returned 的 ResourceId 會跨幀錯綁。
                // 此處先返還 incoming 借用，再逐幀 LIFO 釋放暫存，
                // 不建立新 ResourceId，也不將暫存當具名 owner。
                for id in frame.pending_owned_returns.drain(..).rev() {
                    let Some(index) = pending_str_returns.iter().position(|owned|
                        owned.resource_id() == id
                    ) else {
                        return Err(PrivateScalarCallRuntimeError::StateMismatch);
                    };
                    let returned = pending_str_returns.remove(index);
                    resources.release_detached(returned);
                    Self::record_owned_resources(
                        &resources, &mut resource_cursor, events,
                    );
                }
                for entry in frame.initialized_order.iter().rev() {
                    match entry {
                        PrivateScalarLocalBinding::Int(id) =>
                            events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                        PrivateScalarLocalBinding::Bool(id) =>
                            events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                        PrivateScalarLocalBinding::UninitializedInt(id) =>
                            events.push(PrivateScalarCallEvent::LocalScopeEndedEmpty(*id)),
                        PrivateScalarLocalBinding::UninitializedBool(id) =>
                            events.push(PrivateScalarCallEvent::BoolScopeEndedEmpty(*id)),
                        PrivateScalarLocalBinding::CopyStruct(id) =>
                            events.push(PrivateScalarCallEvent::CopyStructScopeEnded(*id)),
                    }
                }
                frame.values.clear();
                if frame.copy_struct_scopes.len() != frame.resource_scopes {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                }
                for _ in 0..frame.resource_scopes {
                    resources.leave_scope().map_err(
                        PrivateScalarCallRuntimeError::Resource,
                    )?;
                    copy_structs.leave_scope_with_shared_ledger(&mut resources).map_err(|_|
                        PrivateScalarCallRuntimeError::StateMismatch,
                    )?;
                }
                Self::record_owned_resources(
                    &resources, &mut resource_cursor, events,
                );
                events.push(PrivateScalarCallEvent::FunctionExited {
                    name: frame.name, returned: false,
                });
            }
        }
        // 失敗時若 str 返回已脫離子幀而尚未被主程式接收，
        // 呼叫者必須以真 P16 ledger 釋放；Rust Drop 不算資源事件。
        for owned in pending_str_returns.drain(..).rev() {
            resources.release_detached(owned);
            Self::record_owned_resources(&resources, &mut resource_cursor, events);
        }
        // 直接 main→被呼叫者的真框架保存在最底層，亦必須在
        // 正常返回、注入故障或預算耗盡後按完成順序逆序離域。
        if let Some(root) = frames.first_mut() {
            if must_unwind {
                // 所有內層框架已正式失敗退出，才可歸還根框架
                // 尚未進入外層 Call 的票據；不得默默遺漏偽造來源。
                Self::return_pending_borrows_in_frame(
                    root, &mut pending_loans, events,
                )?;
                if !pending_loans.is_empty() {
                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                }
                completed_arguments.clear();
            }
            for entry in root.initialized_order.iter().rev() {
                match entry {
                    PrivateScalarLocalBinding::Int(id) =>
                        events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                    PrivateScalarLocalBinding::Bool(id) =>
                        events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                    PrivateScalarLocalBinding::UninitializedInt(id) =>
                        events.push(PrivateScalarCallEvent::LocalScopeEndedEmpty(*id)),
                    PrivateScalarLocalBinding::UninitializedBool(id) =>
                        events.push(PrivateScalarCallEvent::BoolScopeEndedEmpty(*id)),
                    PrivateScalarLocalBinding::CopyStruct(id) =>
                        events.push(PrivateScalarCallEvent::CopyStructScopeEnded(*id)),
                }
            }
            root.initialized_order.clear();
            root.values.clear();
            root.borrow_tracker.verify_static_snapshot(
                std::iter::empty::<BorrowFact>(),
            ).map_err(PrivateScalarCallRuntimeError::Borrow)?;
            if root.copy_struct_scopes.len() != root.resource_scopes {
                return Err(PrivateScalarCallRuntimeError::StateMismatch);
            }
            for _ in 0..root.resource_scopes {
                resources.leave_scope().map_err(
                    PrivateScalarCallRuntimeError::Resource,
                )?;
                copy_structs.leave_scope_with_shared_ledger(&mut resources).map_err(|_|
                    PrivateScalarCallRuntimeError::StateMismatch,
                )?;
            }
        }
        Self::record_owned_resources(
            &resources, &mut resource_cursor, events,
        );
        // copy_structs 保留由 P16 真建構、真清理的獨立持有者；
        // 任何非預期字串資源事件表示超出已證無資源子集，立即拒絕。
        if !copy_structs.events().is_empty()
            || !copy_structs.finish().map_err(|_|
                PrivateScalarCallRuntimeError::StateMismatch,
            )?.is_empty() {
            return Err(PrivateScalarCallRuntimeError::StateMismatch);
        }
        let emitted = resources.finish().map_err(
            PrivateScalarCallRuntimeError::Resource,
        )?;
        // finish() 可能另行釋放根 Scope 資源；必須在真正完成後記入帳。
        for resource in emitted.iter().skip(resource_cursor) {
            events.push(PrivateScalarCallEvent::OwnedResource(resource.clone()));
        }
        resource_events.extend(emitted);
        // E3 暫停取消：已完成 P16／HC14 真清理，呼叫者可確認
        // checkpoint 為 None 且回傳 Ok(None)；不能流出偽造 RuntimeFailure。
        if cancel_at_checkpoint { Ok(None) } else { result }
    }

    fn prepare(text: &str) -> Result<Self, PrivateScalarCallPrepareError> {
        use crate::common_ir::{ScalarType, NodeSpan};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateScalarCallPrepareError::Check)?;
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported("缺少 main"))?;
        let last = main.body.statements.last().ok_or(
            PrivateScalarCallPrepareError::Unsupported("main 缺少最後 return"),
        )?;
        let StmtKind::Return(ret) = &last.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後語句須真正返回一個具名函式呼叫",
            ));
        };
        // V1：main 的括號 return 同樣是原 Checker 已核准的 int 表達式。
        // 逐層核對真 TypedHIR、Lexer 左右括號，保留最內層的原 AST
        // 才能將零／多參數函式交給同一個動態呼叫框架，不自行猜測語法。
        let mut return_call = &ret.expression;
        while let TermKind::Group(inner) = &return_call.kind {
            let span = return_call.node_span();
            if verified.plan.scalar_hir.expression(span)
                .is_none_or(|fact| fact.ty != ScalarType::Int)
                || !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                || !verified.tokens.iter().any(|token|
                    token.end == span.end
                        && matches!(token.kind, TokenKind::Symbol(')'))) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 括號返回缺少真 Checker int HIR 與原 Lexer 詞彙",
                ));
            }
            return_call = inner;
        }
        // A2/F02：零參數 name() 仍以原完整函式宣告分類，
        // 非函式或結構建構式不得混入此受限純值執行器。
        // V1：main 前置已核准 int 初始化也可能包含唯一真函式呼叫；
        // 即使最後只是 return 先前取得的 Copyable 結果，仍可由
        // 原 Parser AST 找到同一已核准函式圖，不猜測字串名稱。
        // R01／R03：若原 Checker 未為 main 尾端建立 int Expression HIR，
        // 只在完整 main 控制圖已證明「所有葉分支都提前 Return」時，
        // 才可排除尾端不可達呼叫。單靠缺 HIR 絕不能擅自跳過來源。
        // 真合法的前置／分支函式呼叫仍由原 Parser AST 完整收集。
        // V1／F3：main 的入口函式也以 Checker 已編譯工作計畫
        // 而非第二條 Parser AST 掃描選擇。先取 return 表達式，
        // 再取原順序的前置控制／宣告工作；所有葉分支返回後
        // 的來源不會在這裡成為入口，更不會提前執行。
        let (_, main_graph_steps, main_graph_return, _) =
            Self::prepare_function_locals(main, &verified)?;
        let terminal_unreachable = verified.plan.scalar_hir
            .expression(return_call.node_span()).is_none()
            && matches!(&main_graph_return, PrivateScalarReturnExpr::Unreachable);
        let mut main_names = Vec::new();
        main_graph_return.collect_calls(&mut main_names);
        for step in &main_graph_steps {
            step.collect_calls(&mut main_names);
        }
        let callee = match main_names.first() {
            Some(name) => verified.functions.iter().find(|fun| fun.name == *name && fun.name != "main")
                .ok_or(PrivateScalarCallPrepareError::Unsupported("main 呼叫缺少原 Checker 具名函式宣告"))?,
            // 方法結果可直接返回，也可先綁入 main 的局部槽或參與算式。
            // 根框架依原 Checker 的呼叫種類與來源範圍選擇。
            None if verified.plan.declarations.call_borrows.range(main.start..main.end)
                .any(|(_, call)| call.is_method) => main,
            None => return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 已編譯可達工作計畫缺少真具名函式或方法呼叫")),
        };
        let source_name = callee.name.as_str();
        if main.result_ty != "int" || !main.params.is_empty()
            || !matches!(callee.result_ty.as_str(), "int" | "bool" | "str")
            || callee.params.iter().any(|param|
                !matches!(param.ty.as_str(), "int" | "bool" | "str"))
            || callee.body.statements.is_empty()
            || main.body.statements.is_empty() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "只接通已核准 Copyable int 完整簽章與單一返回式",
            ));
        }
        // 對函式簽章每一個 Parameter，不以名稱猜測或忽略後續參數；
        // 原 P16 TypedMember 及 Checker DeclarationId 皆須逐項相符。
        for param in &callee.params {
            let id = DeclarationId(param.declaration_start);
            let member = crate::common_ir::VerifiedTypedMember::checked_private(
                text, crate::common_ir::NodeRole::Parameter,
                param.declaration_span, &param.name, param.name_span,
                &param.ty, param.type_span, false,
            ).ok_or(PrivateScalarCallPrepareError::Unsupported(
                "多參數缺少原 Parser／Lexer 的名稱與 int 型別身分",
            ))?;
            if !verified.plan.declarations.matches(id, &param.ty, false)
                || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多參數缺少原 Checker const int 宣告及 P16 型別來源事實",
                ));
            }
        }
        let param = callee.params.first();
        let (root_local_inits, root_local_steps, return_expr, param_return_span) =
            Self::prepare_function_locals(callee, &verified)?;
        // R01／V1：從 main 已編譯工作計畫的真正候選呼叫出發，
        // 遞迴收集已核准的函式依賴；不走原 AST 第二次猜圖。
        let mut reachable = std::collections::BTreeSet::<String>::new();
        let mut pending = Vec::<String>::new();
        // 原 main 的同一已編譯候選同時決定入口和傳遞可達圖，
        // 任何零迭代／終止後綴都不能額外建立函式框架。
        for name in main_names {
            if reachable.insert(name.to_owned()) {
                pending.push(name.to_owned());
            }
        }
        // V1／F3：main 來源目錄已由完整 Checker 驗證。自第一層
        // 普通函式開始，不再第二次走訪原 AST 猜哪些子呼叫可達，
        // 而由原 Checker TypedHIR 所編譯的同一動態工作計畫取得
        // return、宣告、賦值、分支及逐輪控制的 Call 依賴。
        // 零迭代、已證全分支提前返回及無條件中止都已在計畫中
        // 被適法排除；未知條件的兩個合法分支仍完整保留。
        while let Some(name) = pending.pop() {
            let Some(function) = verified.functions.iter()
                .find(|fun| fun.name == name) else {
                // 無來源宣告的目標留給下方正式 graph 完整性檢查拒絕。
                continue;
            };
            if function.name == "main" {
                // main 是既有 root，普通函式簽章不得以 main 假造。
                continue;
            }
            // 「暫時編譯的 callee 來源」僅在本區塊借用，
            // 立即保存為自有名稱後才擴張圖；不可保留落在
            // 已離開作用域的 ReturnExpr／LocalStep 上的 &str。
            let nested_names: Vec<String> = if function.name == callee.name {
                let mut calls = Vec::new();
                return_expr.collect_calls(&mut calls);
                for step in &root_local_steps {
                    step.collect_calls(&mut calls);
                }
                calls.into_iter().map(str::to_owned).collect()
            } else {
                // 只讓原 Checker/TypedHIR 通過的返回和語句出現在
                // 工作計畫；任何未接通的真正可達合法形狀仍在此
                // 受控 Unsupported，不可偷略過或擅自創造語意。
                if !matches!(function.result_ty.as_str(), "int" | "bool" | "str")
                    || function.params.iter().any(|param|
                        !matches!(param.ty.as_str(), "int" | "bool" | "str"))
                    || function.body.statements.is_empty() {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "可達子函式缺少已核准 int 完整簽章",
                    ));
                }
                let (_, steps, result, _) =
                    Self::prepare_function_locals(function, &verified)?;
                let mut calls = Vec::new();
                result.collect_calls(&mut calls);
                for step in &steps {
                    step.collect_calls(&mut calls);
                }
                calls.into_iter().map(str::to_owned).collect()
            };
            for nested in nested_names {
                if reachable.insert(nested.clone()) {
                    pending.push(nested);
                }
            }
        }
        let mut nested_callees = Vec::new();
        for function in verified.functions.iter()
            .filter(|fun| fun.name != "main" && fun.name != callee.name
                && reachable.contains(&fun.name)) {
            if !matches!(function.result_ty.as_str(), "int" | "bool" | "str")
                || function.params.iter().any(|param|
                    !matches!(param.ty.as_str(), "int" | "bool" | "str"))
                || function.body.statements.is_empty() {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多層函式須為已核准單 return 的純 int 完整簽章",
                ));
            }
            let mut parameter_ids = Vec::new();
            let mut parameter_is_bool = Vec::new();
            let mut parameter_is_str = Vec::new();
            for param in &function.params {
                let id = DeclarationId(param.declaration_start);
                let member = crate::common_ir::VerifiedTypedMember::checked_private(
                    text, crate::common_ir::NodeRole::Parameter,
                    param.declaration_span, &param.name, param.name_span,
                    &param.ty, param.type_span, false,
                ).ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀函式參數缺少原 Parser/Lexer 型別身分",
                ))?;
                if !verified.plan.declarations.matches(id, &param.ty, false)
                    || verified.plan.members.get(&member.identity) != Some(&member) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀函式參數缺少原 Checker DeclarationId／P16 TypedMember",
                    ));
                }
                parameter_ids.push(id);
                parameter_is_bool.push(param.ty == "bool");
                parameter_is_str.push(param.ty == "str");
            }
            let (local_inits, local_steps, result, result_span) =
                Self::prepare_function_locals(function, &verified)?;
            nested_callees.push(PrivateScalarNestedCallee {
                name: function.name.clone(),
                span: NodeSpan::new(function.start, function.end),
                parameters: parameter_ids,
                parameter_is_bool, parameter_is_str,
                result_span,
                result,
                local_inits,
                local_steps,
            });
        }
        // 每個可達或不可達的已編譯函式 Call 必須在同一份
        // Checker 真來源中有完整目標；允許已核准直接／互相遞迴。
        // 是否真正執行由來源控制流決定，安全耗盡由 runtime 預算決定。
        let mut graph: Vec<(&str, &PrivateScalarReturnExpr)> =
            vec![(&callee.name, &return_expr)];
        graph.extend(nested_callees.iter().map(|fun| (fun.name.as_str(), &fun.result)));
        for (name, expression) in &graph {
            let mut calls = Vec::new();
            expression.collect_calls(&mut calls);
            let steps = if *name == callee.name {
                &root_local_steps
            } else {
                &nested_callees.iter().find(|fun| fun.name == *name)
                    .expect("已收錄的巢狀真函式").local_steps
            };
            for step in steps {
                step.collect_calls(&mut calls);
            }
            if calls.iter().any(|called|
                !graph.iter().any(|(candidate, _)| candidate == called)) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀具名呼叫缺少原 Checker 真函式宣告或完整簽章",
                ));
            }
        }

        // main 的所有前置語句須真為已核准的 Copyable int 常值初始化。
        // 正常詞彙遮蔽／任意賦值另待 P18.2，不把尚未執行的語句跳過。
        let (last_statement, main_prefix) = main.body.statements.split_last()
            .expect("已確認 main 非空");
        let mut owners = BTreeMap::<String, (DeclarationId, i64)>::new();
        let mut main_owners = Vec::new();
        // 非字面／名稱的合法 main int 初始化可能含算術、真函式
        // 及溢位，不可在 Prepare 階段求值，也不能省略整筆初始化。
        let needs_main_local_runtime = main_prefix.iter().any(|statement| {
            if matches!(&statement.kind,
                StmtKind::Assign(_) | StmtKind::If { .. }
                    | StmtKind::While { .. } | StmtKind::ForRange { .. }) {
                return true;
            }
            if let StmtKind::Declare(owner) = &statement.kind {
                if owner.ty != "int" || owner.initializer.is_none() {
                    return true;
                }
            }
            if let StmtKind::Declare(owner) = &statement.kind {
                if let Some(initializer) = &owner.initializer {
                    let mut inner = initializer;
                    while let TermKind::Group(next) = &inner.kind { inner = next; }
                    return !matches!(&inner.kind,
                        TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                        | TermKind::Name(_));
                }
            }
            false
        }) || PrivateScalarReturnExpr::first_source_call(return_call).is_none();
        if !needs_main_local_runtime {
        for statement in main_prefix {
            let StmtKind::Declare(owner) = &statement.kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多參數 main 前置語句僅接受真正 int 初始化宣告",
                ));
            };
            let id = DeclarationId(statement.start);
            let Some(init) = &owner.initializer else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main int 詞彙持有者尚未初始化",
                ));
            };
            // Group 常值仍須逐層經原 Lexer／Checker 證明為 int；
            // 只移除無副作用括號，不把任意式子在 prepare 階段預求值。
            let mut actual_init = init;
            while let TermKind::Group(inner) = &actual_init.kind {
                let span = actual_init.node_span();
                if verified.plan.scalar_hir.expression(span)
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "main Group 初始化缺少真 Checker int HIR 或括號詞彙",
                    ));
                }
                actual_init = inner;
            }
            if owner.ty != "int"
                || !verified.plan.declarations.matches(id, "int", owner.mutable)
                || verified.plan.scalar_hir.expression(actual_init.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                || owners.contains_key(&owner.name) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 具名 int 初始值缺少唯一 Checker／TypedHIR 宣告身分",
                ));
            }
            let value = match &actual_init.kind {
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                    if !verified.tokens.iter().any(|token|
                        token.start == actual_init.start && token.end == actual_init.end
                            && matches!(&token.kind, TokenKind::Integer(found) if found == raw)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "main int 字面初始化缺少原 Lexer Token",
                        ));
                    }
                    crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateScalarCallPrepareError::Unsupported(
                            "main int 詞彙初始化值超出 i64",
                        ),
                    )?
                }
                TermKind::Name(name) => {
                    // 已核准 Copyable int 可由同一 main 先前已初始化持有者取值，
                    // 不產生 Move／BorrowTicket。原 Checker Reference HIR 的
                    // DeclarationId 和 Lexer token 都必須逐一相符。
                    let (source_id, value) = owners.get(name).copied().ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "main Copyable int 初始化來源不是先前已驗證的持有者",
                        ),
                    )?;
                    if verified.plan.scalar_hir.reference(actual_init.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != source_id
                                || fact.resolved.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == actual_init.start
                                && token.end == actual_init.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == name)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "main Copyable int 初始化來源缺少原 Checker Reference HIR",
                        ));
                    }
                    value
                }
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 此子集只接通已核准 Copyable int 字面或具名初始化",
                )),
            };
            owners.insert(owner.name.clone(), (id, value));
            main_owners.push((id, value));
        }
        }

        let StmtKind::Return(_) = &last_statement.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後一筆必須是真普通函式呼叫 return",
            ));
        };
        let (call_name, call_arguments, direct_call): (&str, &[Term], bool) =
            if terminal_unreachable {
                // 此處仍必經下方 main 動態計畫的 Unreachable 身分
                // 核對，不將已證不可達的尾端 Call 實參交給執行器。
                (source_name, &[], false)
            } else { match &return_call.kind {
                TermKind::Call(call) =>
                    (call.name.as_str(), call.arguments.as_slice(), true),
                TermKind::UnresolvedZeroArgs(name) => (name.as_str(), &[], true),
                // main 可 return 原 Checker 已核准的多函式 int 算式，
                // 但仍不得把純算術當成已有函式動態證據。
                // R01：前置語句已含由原 Checker 核准的真普通函式時，
                // 最後的整數常值 return 同樣合法；不得因尾端
                // 沒有 Call 就捨棄前置呼叫、錯拒整個函式。
                // 常值本身仍僅由 Checked TypedHIR 編譯，不在
                // Prepare 階段執行或虛構任何返回事件。
                TermKind::BinaryInt(_) | TermKind::Name(_) | TermKind::MethodCall(..) | TermKind::AwaitMethodCall(..) | TermKind::FieldRead(..)
                    | TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                    if needs_main_local_runtime => (source_name, &[], false),
                TermKind::BinaryInt(_) => (source_name, &[], false),
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 最後一筆須為已核准 int 函式或函式複合返回式",
                )),
            }};
        if direct_call
            && (call_name != callee.name || call_arguments.len() != callee.params.len()) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "原 Parser 呼叫名稱／參數個數與真 Checker 函式簽章不符",
            ));
        }
        let call_span = return_call.node_span();
        // R03：原 Checker 對所有 if／else 葉分支都已返回的
        // 末端不可達 return，合法不簽發 Expression HIR。
        // 先只禁止「已有但型別非 int」的 HIR；對完全缺少
        // HIR 的情況延後到編好 main 真控制圖後，必須精確
        // 證明計畫為 Unreachable，不能將缺失默認成合法。
        let trailing_hir_absent = verified.plan.scalar_hir.expression(call_span).is_none();
        if verified.plan.scalar_hir.expression(call_span)
            .is_some_and(|fact| fact.ty != ScalarType::Int)
            || (direct_call && !callee.params.iter().any(|param| param.ty == "str")
                && verified.plan.declarations.call_borrows_at(call_span)
                    .is_some_and(|facts| !facts.is_empty())) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "呼叫的 int TypedHIR 型別不符或 Copyable 實參遭誤記為借用",
            ));
        }
        // 已核准的 main 動態 int 表達式與巢狀具名函式呼叫不能在
        // Prepare 階段預先求值。完整原 Checker HIR 先建立唯一
        // 表達式工作計畫，執行時才在 main 的真 int 持有者框架求值。
        let needs_dynamic = needs_main_local_runtime || !direct_call
            // V7 已由原 Checker 證明直接 await 的單一 Call
            // 也必須走同一動態 main 工作圖；不得偷用舊式同步
            // 字面 int 快路而遺失原 await 來源與延遲啟動身分。
            || matches!(&return_call.kind, TermKind::Call(call) if call.awaited)
            || callee.params.iter().any(|param|
                matches!(param.ty.as_str(), "bool" | "str"))
            || call_arguments.iter().any(|argument| {
            let mut inner = argument;
            while let TermKind::Group(nested) = &inner.kind {
                inner = nested;
            }
            !matches!(&inner.kind,
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                | TermKind::Name(_))
        });
        let mut main_dynamic_inits = Vec::new();
        let mut main_dynamic_steps = Vec::new();
        let main_dynamic_call = if needs_main_local_runtime {
            let (inits, steps, result, _) =
                Self::prepare_function_locals(main, &verified)?;
            main_dynamic_inits = inits;
            main_dynamic_steps = steps;
            Some(result)
        } else if needs_dynamic {
            let bindings = owners.iter().map(|(name, (id, _))|
                (name.clone(), *id)
            ).collect::<BTreeMap<_, _>>();
            if direct_call && !callee.params.iter().any(|p| p.ty == "str") {
                let arguments = call_arguments.iter().zip(&callee.params).map(
                    |(argument, parameter)| {
                        if parameter.ty == "bool" {
                            Ok(PrivateScalarCallArgument::Bool(
                                PrivateScalarBoolExpr::from_checked_call_argument(
                                    argument, &bindings, &verified,
                                )?,
                            ))
                        } else {
                            Ok(PrivateScalarCallArgument::Int(
                                PrivateScalarReturnExpr::from_checked_bindings(
                                    argument, &bindings, &verified,
                                )?,
                            ))
                        }
                    }
                ).collect::<Result<Vec<_>, PrivateScalarCallPrepareError>>()?;
                Some(PrivateScalarReturnExpr::Call {
                    name: callee.name.clone(), arguments,
                    awaited: matches!(&return_call.kind,
                        TermKind::Call(call) if call.awaited),
                })
            } else {
                Some(PrivateScalarReturnExpr::from_checked_bindings(
                    return_call, &bindings, &verified,
                )?)
            }
        } else {
            None
        };
        // 沒有 Expression HIR 只可能是原 Checker 全分支已提前
        // return 的真正不可達結尾；任何其他缺證來源仍立即拒絕。
        if trailing_hir_absent
            && !matches!(&main_dynamic_call, Some(PrivateScalarReturnExpr::Unreachable)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 可達返回式缺少原 Checker int TypedHIR",
            ));
        }
        let mut all_arguments = Vec::new();
        if main_dynamic_call.is_none() {
        for (arg, parameter) in call_arguments.iter().zip(&callee.params) {
            // 已核准的 Group 由原 Parser/TypedHIR 提供完整來源，
            // 不額外解釋未核准的表達式或改變 Copyable 權限。
            let mut actual = arg;
            while let TermKind::Group(inner) = &actual.kind {
                if verified.plan.scalar_hir.expression(actual.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == actual.start
                            && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == actual.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "Group 引數缺少已核准的同源 int 型別與括號詞彙",
                    ));
                }
                actual = inner;
            }
            let arg = actual;
            let (value, owner_id) = match &arg.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(raw),
                ) => {
                    if verified.plan.scalar_hir.expression(arg.node_span())
                        .is_none_or(|fact| fact.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == arg.start && token.end == arg.end
                                && matches!(&token.kind,
                                    TokenKind::Integer(found) if found == raw)
                        ) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "參數 int 常值缺少原 Lexer／Checker TypeHIR",
                        ));
                    }
                    (crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateScalarCallPrepareError::Unsupported(
                            "函式 int 實參超出 i64",
                        ),
                    )?, None)
                }
                TermKind::Name(name) => {
                    let (id, value) = owners.get(name).copied().ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "具名 Copyable int 實參不是原 main 已初始化所有者",
                        ),
                    )?;
                    if verified.plan.scalar_hir.reference(arg.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != id
                            || fact.resolved.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == arg.start && token.end == arg.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == name)
                        ) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "多參數實參未繫結原 Checker DeclarationId／Reference HIR",
                        ));
                    }
                    (value, Some(id))
                }
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "Copyable int 參數僅接通已驗證字面或具名來源",
                )),
            };
            all_arguments.push((
                DeclarationId(parameter.declaration_start), value, owner_id,
            ));
        }
        }
        // 前置宣告已逐筆由原 Checker、Lexer 與 int TypedHIR 驗證為
        // 無資源的純整數常值初始化；即使未被用作實參，也不能
        // 將合法宣告誤判成「未執行語句」。其身分與值仍納入防偽。
        let (parameter_id, argument, owner_id) = match all_arguments.first().copied() {
            Some((id, value, owner)) => (Some(id), Some(value), owner),
            None => (None, None, None),
        };
        let additional_arguments = all_arguments.iter().skip(1).copied().collect();
        let callee_span = NodeSpan::new(callee.start, callee.end);
        let callee_name = callee.name.clone();
        let parameter_name = param.map(|param| param.name.clone());
        let callee_parameter_ids = callee.params.iter().map(|param|
            DeclarationId(param.declaration_start)
        ).collect();
        let callee_parameter_is_bool = callee.params.iter()
            .map(|param| param.ty == "bool").collect();
        let callee_parameter_is_str = callee.params.iter()
            .map(|param| param.ty == "str").collect();
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, call_span, callee_span, callee_name,
            parameter_name, parameter_id,
            parameter_return_span: param_return_span, return_expr,
            root_local_inits, root_local_steps, owner_id, argument,
            additional_arguments, main_owners, nested_callees,
            main_dynamic_call, main_dynamic_inits, main_dynamic_steps,
            callee_parameter_ids, callee_parameter_is_bool,
            callee_parameter_is_str,
        })
    }

    fn matches_source(&self, source: &str) -> bool {
        let Ok(fresh) = Self::prepare(source) else { return false; };
        self.original == fresh.original
            && self.call_span == fresh.call_span
            && self.callee_span == fresh.callee_span
            && self.callee_name == fresh.callee_name
            && self.parameter_name == fresh.parameter_name
            && self.parameter_id == fresh.parameter_id
            && self.parameter_return_span == fresh.parameter_return_span
            && self.return_expr == fresh.return_expr
            && self.root_local_inits == fresh.root_local_inits
            && self.root_local_steps == fresh.root_local_steps
            && self.owner_id == fresh.owner_id
            && self.argument == fresh.argument
            && self.additional_arguments == fresh.additional_arguments
            && self.main_owners == fresh.main_owners
            && self.nested_callees == fresh.nested_callees
            && self.main_dynamic_call == fresh.main_dynamic_call
            && self.main_dynamic_inits == fresh.main_dynamic_inits
            && self.main_dynamic_steps == fresh.main_dynamic_steps
            && self.callee_parameter_ids == fresh.callee_parameter_ids
            && self.callee_parameter_is_bool == fresh.callee_parameter_is_bool
            && self.callee_parameter_is_str == fresh.callee_parameter_is_str
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
            && self.verified.plan.declarations.by_source
                == fresh.verified.plan.declarations.by_source
            && self.verified.plan.function_exits == fresh.verified.plan.function_exits
            && self.verified.plan.control_edges == fresh.verified.plan.control_edges
    }

    /// T3／V7：普通函式延遲等待與真正 execute() 共用完全相同
    /// 的原 Checker／Lexer／TypedHIR 封存來源預檢。只做唯讀
    /// 驗證，不得在 suspend／cancel 前假執行任何普通函式。
    fn checked_source_still_intact(&self) -> bool {
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return false;
        };
        self.original.as_ref() == self.verified.source.bytes.as_slice()
            && verified_private_source_tokens(
                source, &self.verified.tokens,
                &self.verified.structs, &self.verified.functions,
            )
            && self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            )
            && self.matches_source(source)
    }

    fn execute(&self) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(PrivateScalarExecutionBudget::default(), false)
    }

    fn execute_with_budget(&self, budget: PrivateScalarExecutionBudget)
        -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(budget, false)
    }

    /// 測試故障僅為真參數框架清理驗證，不授權 voiles except 語意。
    fn execute_with_fault(&self, fail_after_binding: bool)
        -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(
            PrivateScalarExecutionBudget::default(), fail_after_binding,
        )
    }

    fn audit_outcome(
        outcome: PrivateScalarCallOutcome,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        if outcome.has_balanced_frame_trace() {
            Ok(outcome)
        } else {
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        }
    }

    /// V1／F1：無論新 main 工作圖或既有單一直接 int 呼叫，
    /// 最外層真 Returned 值必須等於原 Checker 已核准直接呼叫的結果。
    /// 一般「main 返回 f()+g()」或另有算術運算的複合返回不適用：
    /// 子函式各自 Returned 的值本來就不等於 main 的算術結果。
    fn audit_graph_outcome(
        &self, outcome: PrivateScalarCallOutcome,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        let outcome = Self::audit_outcome(outcome)?;
        // V7：把真正動態安全點與同一個原 Checker 函式簽章
        // 交叉核對；任何 async Enter 沒有等待點、普通同步 Enter
        // 冒出等待點，或等待點與下一函式身分不相符都拒絕。
        let mut checked_method_sites = BTreeMap::new();
        let mut pending_await: Option<&str> = None;
        for event in &outcome.events {
            match event {
                PrivateScalarCallEvent::SelfRead { site, .. } => {
                    if pending_await.is_some() {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                    let checked = checked_method_sites.entry(site.call_span).or_insert_with(||
                        PrivateScalarSelfRead::checked(site.call_span, &site.owner, &site.name,
                            site.loan.source, &self.verified));
                    if checked.as_ref() != Some(site) {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                }
                PrivateScalarCallEvent::AwaitSafePoint(name) => {
                    if pending_await.is_some()
                        || !self.verified.functions.iter()
                            .any(|fun| fun.name == *name && fun.is_async) {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                    pending_await = Some(name);
                }
                PrivateScalarCallEvent::FunctionEntered(name) => {
                    let Some(target) = self.verified.functions.iter()
                        .find(|fun| fun.name == *name) else {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    };
                    if target.is_async != (pending_await == Some(name.as_str())) {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                    pending_await = None;
                }
                _ if pending_await.is_some() =>
                    return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch),
                _ => {}
            }
        }
        if pending_await.is_some() {
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        // F2：同一真進退事件另核對原 Checker 所宣告的返回型別。
        // R05 既有平衡守門不辨識 int/bool 的靜態簽章，故不得讓
        // 一筆型別錯誤但數量平衡的 Returned／BoolReturned 假冒成功。
        let mut return_types = Vec::<&str>::new();
        for event in &outcome.events {
            match event {
                PrivateScalarCallEvent::FunctionEntered(name) => {
                    let Some(function) = self.verified.functions.iter()
                        .find(|fun| fun.name == *name) else {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    };
                    return_types.push(function.result_ty.as_str());
                }
                PrivateScalarCallEvent::Returned(_) => {
                    if return_types.last() != Some(&"int") {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                }
                PrivateScalarCallEvent::BoolReturned(_) => {
                    if return_types.last() != Some(&"bool") {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                }
                PrivateScalarCallEvent::StrReturned(_) => {
                    if return_types.last() != Some(&"str") {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                }
                PrivateScalarCallEvent::FunctionExited { .. } => {
                    if return_types.pop().is_none() {
                        return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
                    }
                }
                _ => {}
            }
        }
        if !return_types.is_empty() {
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        // None 是先前已核准「直接 main 呼叫、純 int 實參」的
        // 獨立快路；其成功時也由相同的原 Checker 根 Call 決定結果。
        // Some(Addition/Subtraction/Unreachable/Parameter) 則不能直接
        // 把最後子呼叫 Returned 與 main 複合結果視為相等。
        if self.main_dynamic_call.is_some()
            && !matches!(&self.main_dynamic_call,
                Some(PrivateScalarReturnExpr::Call { .. })) {
            return Ok(outcome);
        }
        let Ok(expected) = outcome.result else { return Ok(outcome); };
        let mut depth = 0usize;
        let mut last_direct_return = None;
        for event in &outcome.events {
            match event {
                PrivateScalarCallEvent::FunctionEntered(_) => {
                    depth += 1;
                }
                PrivateScalarCallEvent::Returned(value) if depth == 1 => {
                    // main 前置工作可先有其他頂層呼叫；最後一個
                    // 才是原 Checker 已簽發的「直接返回 Call」。
                    last_direct_return = Some(*value);
                }
                PrivateScalarCallEvent::FunctionExited { .. } => {
                    depth = depth.checked_sub(1).ok_or(
                        PrivateScalarCallPrepareError::RuntimeTraceMismatch,
                    )?;
                }
                _ => {}
            }
        }
        if depth != 0 || last_direct_return != Some(expected) {
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        Ok(outcome)
    }

    fn execute_with_budget_and_fault(
        &self, budget: PrivateScalarExecutionBudget, fail_after_binding: bool,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault_scheduler(budget, fail_after_binding, None)
    }

    /// E2-A 內部同執行緒協作切換：原私有 Checker 的真 await 到達時
    /// 才呼叫排程者。不是預先掃描來源或在工作準備階段合成事件。
    fn execute_with_await_scheduler(
        &self,
        scheduler: &mut dyn FnMut(
            &str, &[crate::owned_str_runtime::ResourceEvent],
        ),
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault_scheduler(
            PrivateScalarExecutionBudget::default(), false, Some(scheduler),
        )
    }

    fn execute_with_budget_and_fault_scheduler(
        &self, budget: PrivateScalarExecutionBudget, fail_after_binding: bool,
        mut await_scheduler: Option<&mut dyn FnMut(
            &str, &[crate::owned_str_runtime::ResourceEvent],
        )>,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        // 以與 V7 延遲普通函式 suspend/cancel/wake 完全相同
        // 的單一權威再驗原封存 Lexer／AST／Checker 計畫；
        // 必須在建立任何真正 BorrowTicket／ResourceId 之前。
        if !self.checked_source_still_intact() {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        }
        // 即使原 Checker 合法遞迴，零執行預算也不得製造假的函式進入。
        if budget.max_call_frames == 0 || budget.max_steps == 0 {
            return Self::audit_outcome(PrivateScalarCallOutcome {
                result: Err(if budget.max_call_frames == 0 {
                    PrivateScalarCallRuntimeError::CallBudget
                } else {
                    PrivateScalarCallRuntimeError::StepBudget
                }),
                events: Vec::new(), resource_events: Vec::new(), parameter_slots_after_exit: 0,
            });
        }
        // 已核准純值事件與真 UTF-8 ResourceId 兩張帳簿互不冒充。
        let mut resource_events = Vec::new();
        let mut events = Vec::new();
        if let Some(dynamic_call) = &self.main_dynamic_call {
            // 純 int 複合返回可能有數個兄弟 Call，沒有可供
            //「根函式綁定後」故障注入的唯一目標；此測試介面
            // 不得假造單一根框架或默默忽略故障要求。
            if fail_after_binding
                && !matches!(dynamic_call, PrivateScalarReturnExpr::Call { .. }) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 複合 int 返回沒有唯一故障注入根呼叫",
                ));
            }
            let main_frame: BTreeMap<DeclarationId, i64> =
                self.main_owners.iter().copied().collect();
            // 這個額外的 frame 只代表執行器的 main 來源持有者；
            // 不是函式呼叫，不能占用 D18-01 的函式框架額度。
            let adjusted_budget = PrivateScalarExecutionBudget {
                max_call_frames: budget.max_call_frames.saturating_add(1),
                max_steps: budget.max_steps,
            };
            let result = self.evaluate_graph(
                dynamic_call, &main_frame, &mut events, &mut resource_events,
                adjusted_budget, fail_after_binding, &self.main_dynamic_steps,
                await_scheduler.take(),
            );
            return self.audit_graph_outcome(PrivateScalarCallOutcome {
                result, events, resource_events, parameter_slots_after_exit: 0,
            });
        }
        if let Some(argument) = self.argument {
            events.push(PrivateScalarCallEvent::ArgumentEvaluated(argument));
        }
        for (_, argument, _) in &self.additional_arguments {
            events.push(PrivateScalarCallEvent::ArgumentEvaluated(*argument));
        }
        events.push(PrivateScalarCallEvent::FunctionEntered(self.callee_name.clone()));
        let mut frame = BTreeMap::<DeclarationId, i64>::new();
        let mut result = Ok(());
        for (id, argument) in self.parameter_id.zip(self.argument).into_iter()
            .chain(self.additional_arguments.iter().map(|(id, value, _)| (*id, *value))) {
            if frame.insert(id, argument).is_some() {
                result = Err(PrivateScalarCallRuntimeError::StateMismatch);
                break;
            }
            events.push(PrivateScalarCallEvent::ParameterBound(id, argument));
        }
        let result = result.and_then(|()| {
            if fail_after_binding {
                Err(PrivateScalarCallRuntimeError::InjectedFailure)
            } else {
                self.evaluate_graph(&self.return_expr, &frame, &mut events,
                    &mut resource_events, budget, false, &self.root_local_steps,
                    await_scheduler.take())
            }
        });
        if let Ok(value) = result {
            events.push(PrivateScalarCallEvent::Returned(value));
        }

        // 部分純值回歸走舊的直接參數框架／故障注入分支；
        // 其已真正進入執行，必須先補正式回滾起點，不能混同
        // 零預算時完全沒有動態事件的預檢。
        if result.is_err() && !events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::UnwindStarted)
        ) {
            events.push(PrivateScalarCallEvent::UnwindStarted);
        }
        // 經正常 Return 或錯誤分支都要離開此呼叫的參數作用域。
        frame.clear();
        events.push(PrivateScalarCallEvent::FunctionExited {
            name: self.callee_name.clone(), returned: result.is_ok(),
        });
        self.audit_graph_outcome(PrivateScalarCallOutcome {
            result, events, resource_events, parameter_slots_after_exit: frame.len(),
        })
    }
}


include!("scalar_cooperative_queue.rs");
include!("scalar_cooperative_queue_tests.rs");
include!("scalar_deferred_await.rs");
include!("scalar_deferred_await_tests.rs");
include!("scalar_call_tests.rs");
include!("scalar_cross_runtime_tests.rs");
