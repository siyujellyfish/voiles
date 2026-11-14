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

#[derive(Debug)]
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
        let mut created_ids = std::collections::HashSet::<u64>::new();
        // 僅實際成功 Returned 的子框架，才可以在正式回滾之前
        // 歸還其直接呼叫者的票據；尚未入框架的失敗實參須先回滾。
        let mut returned_imported_loans =
            Vec::<crate::borrow_safety::BorrowTicket>::new();
        for (index, event) in self.events.iter().enumerate() {
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
        let shared_exit_events = self.events.iter().filter_map(|event| match event {
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
        }).collect::<Vec<_>>();
        frames.len() == 1 && awaited_entry.is_none()
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
enum PrivateScalarReturnExpr {
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
            Self::CopyStructIntField { .. } | Self::Call { .. }
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
            Self::Call { .. } => Err(PrivateScalarCallRuntimeError::StateMismatch),
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
    Literal(String),
    Move { source: String, declaration: DeclarationId },
    /// 在同一顯式工作棧完成 ReturnNamed／OwnedUtf8 交接後才提交本宣告。
    Call { name: String, arguments: Vec<PrivateScalarCallArgument> },
}
impl PrivateScalarStrInitializer {
    fn collect_calls<'a>(&'a self, out: &mut Vec<&'a str>) {
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
                        Work::Read(PrivateScalarReturnExpr::Call { .. }
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
    WriteCopyStructBool(DeclarationId, &'a str, &'a str, &'a PrivateScalarBoolExpr),
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
                    | TermKind::Call(_) | TermKind::UnresolvedZeroArgs(_)
                    if *kind == ScalarKind::Int => {
                    let expression = PrivateScalarReturnExpr::from_checked_bindings(
                        term, bindings, verified,
                    )?;
                    let mut calls = Vec::new();
                    expression.collect_calls(&mut calls);
                    if calls.is_empty() {
                        PrivateScalarCopyStructSeed::IntArithmetic(Box::new(expression))
                    } else {
                        PrivateScalarCopyStructSeed::IntCall(Box::new(expression))
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
                    *declaration, owner, field, value,
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
                        if let PrivateScalarBoolExpr::Call { name, arguments } = expr {
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
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !frame.copy_struct_scopes.iter().rev().flatten()
                            .any(|(known, active)| *known == id && active == owner) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.borrow_tracker.check_direct_access(id)
                            .map_err(PrivateScalarCallRuntimeError::Borrow)?;
                        let value = expr.evaluate_runtime(frame, &copy_structs)?;
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
                        if matches!(expr, PrivateScalarBoolExpr::Call { .. }) {
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
                        if let PrivateScalarStrInitializer::Call { name: callee, arguments } =
                            initializer {
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
                            tasks.push(PrivateScalarTask::EnterCall(
                                callee, arguments, false, false,
                            ));
                            // LIFO：依原 Parser 實參來源左至右真正求值，
                            // 後序算術或借用錯誤時尚未 EnterCall，不得提交 Move。
                            Self::queue_call_arguments(&mut tasks, arguments);
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
                            PrivateScalarStrInitializer::Call { .. } =>
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
                        if let PrivateScalarStrInitializer::Call { name: callee, arguments } =
                            initializer {
                            // 現有 P16 含 str 結構與普通 str 同層交錯的全域
                            // 退出序尚未完整證明，不能藉此呼叫拓寬既有界線。
                            if Self::current_scope_has_str_struct(frame, &copy_structs) {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            tasks.push(PrivateScalarTask::FinishStrReturnAssignment(id, name));
                            tasks.push(PrivateScalarTask::EnterCall(
                                callee, arguments, false, false,
                            ));
                            // 真正求值完成前不得轉交 .move()；參數借用票據
                            // 由同一 EnterCall/ExitCall 對帳並於失敗時反序歸還。
                            Self::queue_call_arguments(&mut tasks, arguments);
                            continue;
                        }
                        match initializer {
                            PrivateScalarStrInitializer::Literal(text) =>
                                resources.assign(name, text),
                            PrivateScalarStrInitializer::Move { source, .. } =>
                                resources.replace_from(source, name),
                            PrivateScalarStrInitializer::Call { .. } =>
                                return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        }.map_err(PrivateScalarCallRuntimeError::Resource)?;
                        Self::record_owned_resources(&resources, &mut resource_cursor, events);
                        let resource_id = resources.resource_id(name)
                            .map_err(PrivateScalarCallRuntimeError::Resource)?;
                        events.push(PrivateScalarCallEvent::ResourceOwnerBound(id, resource_id));
                    }
                    PrivateScalarTask::AssignBool(id, expr) => {
                        if matches!(expr, PrivateScalarBoolExpr::Call { .. }) {
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
                            if matches!(&current.condition, PrivateScalarBoolExpr::Call { .. }) {
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
                            *declaration, owner, field, value,
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
                                    *declaration, owner, field, value,
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
        // 此私有切片只限制已核准的純 int／單 return 執行形狀，
        // 不再以固定函式數量或呼叫圖是否成環，拒絕合法 Checker 來源。
        // R01：其他已經由原 Checker 完整核准、但未被 main 建構或
        // 呼叫的 struct 宣告，不是本次純 int 呼叫圖的動態工作。
        // 不能只因來源含結構定義就拒絕合法的普通函式。
        // 結構建構、self 方法及資源欄位仍不在此 evaluator 的
        // 工作計畫內；若真的由 main 使用，後續真 TypedHIR 會拒絕。
        if verified.functions.len() < 2 {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "Copyable int 真執行須具備 main 及已核准的普通函式",
            ));
        }
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
        let source_name = main_names.first().copied().ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "main 已編譯可達工作計畫缺少真具名函式呼叫",
            ),
        )?;
        let callee = verified.functions.iter()
            .find(|fun| fun.name == source_name && fun.name != "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "main 呼叫缺少原 Checker 具名函式宣告",
            ))?;
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
                TermKind::BinaryInt(_) | TermKind::Name(_)
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
            || (!callee.params.iter().any(|param| param.ty == "str")
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
            if direct_call {
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
        let mut pending_await: Option<&str> = None;
        for event in &outcome.events {
            match event {
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
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            ) || !self.matches_source(source) {
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


/// V7 E2：真正原私有 Checker 工作圖「已經開始執行」的動態續行。
/// 一次 poll 只能直到下一個真 await 安全點或原圖正式結果。
/// 借用已驗證的原計畫，避免跨呼叫自我參照與私有來源重編譯／重跑。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateScalarGraphPoll {
    Suspended,
    Completed,
}

/// E3 真取消證據：事件唯一來源是原 evaluate_graph_driver 中
/// HC14 票據歸還、P16 OwnedStrRuntime 真 scope 退出與原函式幀退出。
#[derive(Debug)]
struct PrivateScalarGraphCancellation {
    events: Vec<PrivateScalarCallEvent>,
    resource_events: Vec<crate::owned_str_runtime::ResourceEvent>,
}

struct PrivateScalarGraphContinuation<'a> {
    plan: &'a PreparedPrivateScalarCall,
    // D18-01：屬於此真 CheckedTask 的環境預算，從第一次 poll 到
    // 任意第 N 次 wake 不可重新配發或重置動態 step／frame 門檻。
    budget: PrivateScalarExecutionBudget,
    checkpoint: Option<PrivateScalarGraphCheckpoint<'a>>,
    events: Vec<PrivateScalarCallEvent>,
    resource_events: Vec<crate::owned_str_runtime::ResourceEvent>,
    outcome: Option<PrivateScalarCallOutcome>,
    status: PrivateDeferredAwaitStatus,
}

impl<'a> PrivateScalarGraphContinuation<'a> {
    /// 除了一般 main 動態圖，既有真 Checker 的「直接 main→函式」
    /// Copyable int 快路也可在原函式內部 await 時產生續行。
    /// 僅接通原 fastpath 真實參數框架；bool／str 等未經同一路徑
    /// 完整參數繫結的形式仍保守拒絕，不猜測新語意。
    fn root_for_graph(
        plan: &'a PreparedPrivateScalarCall,
        budget: PrivateScalarExecutionBudget,
    ) -> Result<(
        &'a PrivateScalarReturnExpr,
        &'a [PrivateScalarLocalStep],
        BTreeMap<DeclarationId, i64>,
        PrivateScalarExecutionBudget,
        bool,
    ), PrivateScalarCallPrepareError> {
        if let Some(expr) = plan.main_dynamic_call.as_ref() {
            return Ok((
                expr, plan.main_dynamic_steps.as_slice(),
                plan.main_owners.iter().copied().collect(),
                PrivateScalarExecutionBudget {
                    max_call_frames: budget.max_call_frames.saturating_add(1),
                    max_steps: budget.max_steps,
                }, false,
            ));
        }
        if plan.callee_parameter_is_bool.iter().any(|value| *value)
            || plan.callee_parameter_is_str.iter().any(|value| *value)
            || !plan.main_dynamic_steps.is_empty()
            || !plan.main_dynamic_inits.is_empty()
            || plan.callee_parameter_ids.len()
                != plan.parameter_id.zip(plan.argument).into_iter().count()
                    + plan.additional_arguments.len() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "私有快路續行限原 Checker 已核准的直接 Copyable int 呼叫",
            ));
        }
        let mut frame = BTreeMap::new();
        for (id, value) in plan.parameter_id.zip(plan.argument).into_iter()
            .chain(plan.additional_arguments.iter().map(
                |(id, value, _)| (*id, *value),
            )) {
            if frame.insert(id, value).is_some() {
                return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
            }
        }
        Ok((&plan.return_expr, plan.root_local_steps.as_slice(), frame, budget, true))
    }

    fn new(plan: &'a PreparedPrivateScalarCall)
        -> Result<Self, PrivateScalarCallPrepareError> {
        Self::new_with_budget(plan, PrivateScalarExecutionBudget::default())
    }

    /// D18-01：預算是每份執行環境的單次配置，不是 voiles
    /// 固定遞迴或循環語言上限。零額度在任何資源及工作建立前
    /// 拒絕，不得藉由 wake 補發新的單輪額度。
    fn new_with_budget(
        plan: &'a PreparedPrivateScalarCall,
        budget: PrivateScalarExecutionBudget,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        if budget.max_call_frames == 0 || budget.max_steps == 0 {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "V7 原 CheckedTask 須有正執行環境預算",
            ));
        }
        Self::root_for_graph(plan, budget)?;
        Ok(Self {
            plan, budget, checkpoint: None, events: Vec::new(),
            resource_events: Vec::new(), outcome: None,
            status: PrivateDeferredAwaitStatus::Prepared,
        })
    }

    fn poll(&mut self) -> Result<PrivateScalarGraphPoll, PrivateScalarCallPrepareError> {
        if !matches!(self.status,
            PrivateDeferredAwaitStatus::Prepared | PrivateDeferredAwaitStatus::Suspended) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "已結束的私有工作不得重複 poll",
            ));
        }
        // 每次真恢復均與原 Checker 同源內容與函式／控制圖對帳，
        // 不能繞過原 PreparedPrivateScalarCall::execute 的防竄改守門。
        let source = std::str::from_utf8(&self.plan.verified.source.bytes)
            .map_err(|_| PrivateScalarCallPrepareError::SourceChanged)?;
        if self.plan.original.as_ref() != self.plan.verified.source.bytes.as_slice()
            || !self.plan.verified.plan.is_consistent(
                &self.plan.verified.structs, &self.plan.verified.functions, source,
            ) || !self.plan.matches_source(source) {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        }
        let (expr, steps, initial_frame, budget, direct_fastpath) =
            Self::root_for_graph(self.plan, self.budget)?;
        if direct_fastpath && self.status == PrivateDeferredAwaitStatus::Prepared {
            if let Some(argument) = self.plan.argument {
                self.events.push(PrivateScalarCallEvent::ArgumentEvaluated(argument));
            }
            for (_, argument, _) in &self.plan.additional_arguments {
                self.events.push(PrivateScalarCallEvent::ArgumentEvaluated(*argument));
            }
            self.events.push(PrivateScalarCallEvent::FunctionEntered(
                self.plan.callee_name.clone(),
            ));
            for id in &self.plan.callee_parameter_ids {
                let value = initial_frame.get(id).copied().ok_or(
                    PrivateScalarCallPrepareError::RuntimeTraceMismatch,
                )?;
                self.events.push(PrivateScalarCallEvent::ParameterBound(*id, value));
            }
        }
        self.status = PrivateDeferredAwaitStatus::Running;
        let result = self.plan.evaluate_graph_driver(
            expr, &initial_frame, &mut self.events, &mut self.resource_events,
            budget, false, steps,
            None, &mut self.checkpoint, true, false,
        );
        if matches!(&result, Ok(None)) {
            if self.checkpoint.is_none() {
                return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
            }
            self.status = PrivateDeferredAwaitStatus::Suspended;
            return Ok(PrivateScalarGraphPoll::Suspended);
        }
        if self.checkpoint.is_some() {
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        let result = match result {
            Ok(Some(value)) => Ok(value),
            Err(error) => Err(error),
            Ok(None) => unreachable!("前方已處理真暫停"),
        };
        if direct_fastpath {
            if let Ok(value) = result {
                self.events.push(PrivateScalarCallEvent::Returned(value));
            } else if !self.events.iter().any(|event| matches!(
                event, PrivateScalarCallEvent::UnwindStarted,
            )) {
                self.events.push(PrivateScalarCallEvent::UnwindStarted);
            }
            self.events.push(PrivateScalarCallEvent::FunctionExited {
                name: self.plan.callee_name.clone(), returned: result.is_ok(),
            });
        }
        let outcome = self.plan.audit_graph_outcome(PrivateScalarCallOutcome {
            result, events: std::mem::take(&mut self.events),
            resource_events: std::mem::take(&mut self.resource_events),
            parameter_slots_after_exit: 0,
        })?;
        self.status = if outcome.result.is_ok() {
            PrivateDeferredAwaitStatus::Succeeded
        } else {
            PrivateDeferredAwaitStatus::Failed
        };
        self.outcome = Some(outcome);
        Ok(PrivateScalarGraphPoll::Completed)
    }

    /// E3：只能取消「已真進入執行而現在停在合法 await」的工作。
    /// 不再次執行任何已完成運算或 EnterCall；同一 HC14／P16
    /// 執行器從最內層向外真正歸還票據、釋放原 ResourceId。
    /// 原計畫即使之後遭竄改仍需允許既有資源退場，故此處
    /// 不重新執行來源／不以重驗原始文字阻塞必要清理。
    fn cancel_suspended(
        &mut self,
    ) -> Result<PrivateScalarGraphCancellation, PrivateScalarCallPrepareError> {
        if self.status != PrivateDeferredAwaitStatus::Suspended {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "真動態取消僅允許已停在合法 await 安全點",
            ));
        }
        let saved = self.checkpoint.as_ref().ok_or(
            PrivateScalarCallPrepareError::RuntimeTraceMismatch,
        )?;
        PreparedPrivateScalarCall::check_await_safe_point_chain(
            saved.frames.iter().map(|frame|
                (&frame.borrow_tracker, frame.incoming_borrows.as_slice())),
        ).map_err(|_| PrivateScalarCallPrepareError::RuntimeTraceMismatch)?;
        let (expr, steps, initial_frame, budget, direct_fastpath) =
            Self::root_for_graph(self.plan, self.budget)?;
        self.status = PrivateDeferredAwaitStatus::Running;
        let cleanup = self.plan.evaluate_graph_driver(
            expr, &initial_frame, &mut self.events, &mut self.resource_events,
            budget, false, steps,
            None, &mut self.checkpoint, false, true,
        );
        if direct_fastpath && matches!(cleanup, Ok(None)) {
            self.events.push(PrivateScalarCallEvent::FunctionExited {
                name: self.plan.callee_name.clone(), returned: false,
            });
        }
        // 若真正 P16／HC14 退出失敗，禁止報稱取消成功；
        // 已消耗工作不能恢復執行，保守標記 Failed 等待正式處置。
        if !matches!(cleanup, Ok(None)) || self.checkpoint.is_some() {
            self.status = PrivateDeferredAwaitStatus::Failed;
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        // 以既有 R05/SharedExit 對帳逆序 unwind，但結果僅供
        // 內部稽核；使用者不會看到任何合成 StateMismatch runtime。
        let evidence = PrivateScalarCallOutcome {
            result: Err(PrivateScalarCallRuntimeError::StateMismatch),
            events: std::mem::take(&mut self.events),
            resource_events: std::mem::take(&mut self.resource_events),
            parameter_slots_after_exit: 0,
        };
        if !evidence.has_balanced_frame_trace() {
            self.status = PrivateDeferredAwaitStatus::Failed;
            return Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch);
        }
        self.status = PrivateDeferredAwaitStatus::Cancelled;
        Ok(PrivateScalarGraphCancellation {
            events: evidence.events,
            resource_events: evidence.resource_events,
        })
    }

    fn pending_await(&self) -> Option<&str> {
        self.checkpoint.as_ref().map(|saved| saved.awaiting_name.as_str())
    }

    fn retained_frames(&self) -> usize {
        self.checkpoint.as_ref().map_or(0, |saved| saved.frames.len())
    }

    fn retained_resources(&self) -> &[crate::owned_str_runtime::ResourceEvent] {
        self.checkpoint.as_ref().map_or(&[], |saved| saved.resources.events())
    }

    fn result(&self) -> Option<&PrivateScalarCallOutcome> { self.outcome.as_ref() }
}

/// V7 E2／E3：已核准私有 Checker 圖的單執行緒協作工作佇列。
/// 此批不在背景啟動任何工作，亦不仿造受控 await 的語法結果。
/// 保存的是原 PrivateScalarGraphContinuation 與其同一套真
/// PrivateScalarTask／PrivateScalarActiveFrame／P16／HC14 checkpoint。
/// 來源計畫由呼叫者保留所有權，排程器在 Rust 生命週期內借用，
/// 不使用 unsafe 自我參照；完全持有式工作登錄仍須後續工程。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateScalarCooperativeError {
    UnknownTask,
    UnknownProgram,
    WrongState,
    IdExhausted,
    /// 原 Checker 或原 R05/SharedExit 的核對拒絕不是排程成功。
    Engine,
}

/// V7 A1/E2：擁有**原私有 Checker 真準備計畫**的來源登錄。
/// 唯一真 Parser／Checker／TypedHIR 工作物件保存在 Box，地址在
/// BTreeMap 擴容時保持穩定。排程器僅以 Rust 借用生命週期持有
/// 其續行，不能在仍有工作時失效或置換來源；不靠 unsafe／Rc
/// 自我參照，也不藉此啟用 E1 尚未核准的背景工作。
#[derive(Default)]
struct PrivateScalarCheckedProgramCatalog {
    programs: BTreeMap<u64, Box<PreparedPrivateScalarCall>>,
    next_id: u64,
}

impl PrivateScalarCheckedProgramCatalog {
    fn new() -> Self { Self::default() }

    fn prepare(
        &mut self, source: &str,
    ) -> Result<u64, PrivateScalarCallPrepareError> {
        // 靜態拒絕不能消耗新的工作身分。此處保留原
        // PreparedPrivateScalarCall::prepare 的唯一 Checker 來源。
        let plan = PreparedPrivateScalarCall::prepare(source)?;
        // 真執行圖的直接快路／複合 main 形狀亦必須可由
        // 同一 GraphContinuation 執行。不能只要 Parser 成功
        // 就允許佇列保存尚無可恢復執行位置的工作。
        let _ = PrivateScalarGraphContinuation::new(&plan)?;
        let id = self.next_id.checked_add(1).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "私有 Checker 計畫登錄 ID 已用盡，不能回繞",
            ),
        )?;
        self.next_id = id;
        self.programs.insert(id, Box::new(plan));
        Ok(id)
    }

    fn plan(&self, id: u64) -> Option<&PreparedPrivateScalarCall> {
        self.programs.get(&id).map(Box::as_ref)
    }

    fn len(&self) -> usize { self.programs.len() }
}

struct PrivateScalarCooperativeJob<'a> {
    continuation: PrivateScalarGraphContinuation<'a>,
    cancellation: Option<PrivateScalarGraphCancellation>,
}

 
/// E3：只有已正式終止並具有原 R05／P16 證據的工作才能移出 FIFO。
/// 移交完整動態結果／取消事件；絕不可在 Drop 時默默丟棄真資源帳。
#[derive(Debug)]
enum PrivateScalarRetiredJob {
    Completed(PrivateScalarCallOutcome),
    Cancelled(Option<PrivateScalarGraphCancellation>),
}

/// 排程的「準備／可執行／已抵達 await／可喚醒」不等於
/// 開放背景工作。每次 drive_one 僅消耗佇列頭的**一個**
/// 原 Checker 計畫，至下一個真 await 或原函式正式完成。
/// E2：每份真正 wake 的可追溯全域 poll 時間位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrivateScalarWakeReceipt {
    id: u64,
    after_drive_count: usize,
}

/// E2：唯一真 submit／wake 可簽發 FIFO 佇列憑據；
/// 授權歷史不隨 poll／cancel／退休而清除。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateScalarReadyIssueKind {
    Submitted,
    Woken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrivateScalarReadyIssue {
    id: u64,
    kind: PrivateScalarReadyIssueKind,
}

#[derive(Default)]
struct PrivateScalarCooperativeQueue<'a> {
    jobs: BTreeMap<u64, PrivateScalarCooperativeJob<'a>>,
    ready: std::collections::VecDeque<u64>,
    next_id: u64,
    drive_order: Vec<(u64, PrivateScalarGraphPoll)>,
    /// V7-E2：只有真正通過 wake 安全點才可取得重新 poll 的 FIFO token。
    /// 原始 poll 與 wake 歷史分開記錄，避免手動插入 ready 假裝合法喚醒。
    wake_order: Vec<PrivateScalarWakeReceipt>,
    /// 防止多工作已簽發 ready token 被調換順序：保留每筆入列
    /// 的唯一授權次序，驗證時只讀、不重新創建任何動態工作。
    ready_issues: Vec<PrivateScalarReadyIssue>,
    cancellation_order: Vec<u64>,
}

impl<'a> PrivateScalarCooperativeQueue<'a> {
    fn new() -> Self { Self::default() }

    fn submit(
        &mut self,
        plan: &'a PreparedPrivateScalarCall,
    ) -> Result<u64, PrivateScalarCooperativeError> {
        self.submit_with_budget(plan, PrivateScalarExecutionBudget::default())
    }

    /// 同一核准來源可建立不同真動態工作，各有獨立的 D18-01
    /// 環境預算。排程／喚醒不替任何工作重置其累積 steps。
    fn submit_with_budget(
        &mut self,
        plan: &'a PreparedPrivateScalarCall,
        budget: PrivateScalarExecutionBudget,
    ) -> Result<u64, PrivateScalarCooperativeError> {
        self.verify_ready_fifo()?;
        // 先在原 Checker 圖上真正建構可續行入口。
        // 來源無法核准時不消耗 ID、不加入任何工作。
        let continuation = PrivateScalarGraphContinuation::new_with_budget(plan, budget)
            .map_err(|_| PrivateScalarCooperativeError::Engine)?;
        let id = self.next_id.checked_add(1)
            .ok_or(PrivateScalarCooperativeError::IdExhausted)?;
        self.next_id = id;
        self.jobs.insert(id, PrivateScalarCooperativeJob {
            continuation, cancellation: None,
        });
        self.ready.push_back(id);
        self.ready_issues.push(PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Submitted,
        });
        Ok(id)
    }

    /// 用原 Checker 來源登錄 ID 建立真動態工作。登錄在本批
    /// 持有來源與核准執行圖，真正工作由 FIFO 驅動；相同
    /// 來源可為不同工作建立獨立 checkpoint／BorrowSafety。
    /// 未知登錄 ID 必在配發工作 ID 之前拒絕。
    fn submit_program(
        &mut self,
        catalog: &'a PrivateScalarCheckedProgramCatalog,
        program_id: u64,
    ) -> Result<u64, PrivateScalarCooperativeError> {
        self.submit_program_with_budget(
            catalog, program_id, PrivateScalarExecutionBudget::default(),
        )
    }

    fn submit_program_with_budget(
        &mut self,
        catalog: &'a PrivateScalarCheckedProgramCatalog,
        program_id: u64,
        budget: PrivateScalarExecutionBudget,
    ) -> Result<u64, PrivateScalarCooperativeError> {
        let plan = catalog.plan(program_id)
            .ok_or(PrivateScalarCooperativeError::UnknownProgram)?;
        self.submit_with_budget(plan, budget)
    }

    fn status(&self, id: u64) -> Option<PrivateDeferredAwaitStatus> {
        self.jobs.get(&id).map(|job| job.continuation.status)
    }

    fn pending_await(&self, id: u64) -> Option<&str> {
        self.jobs.get(&id).and_then(|job| job.continuation.pending_await())
    }

    fn ready_ids(&self) -> Vec<u64> {
        self.ready.iter().copied().collect()
    }

    fn drive_order(&self) -> &[(u64, PrivateScalarGraphPoll)] {
        &self.drive_order
    }

    fn cancellation_order(&self) -> &[u64] {
        &self.cancellation_order
    }

    fn wake_order(&self) -> Vec<u64> {
        self.wake_order.iter().map(|receipt| receipt.id).collect()
    }

    fn outcome(&self, id: u64) -> Option<&PrivateScalarCallOutcome> {
        self.jobs.get(&id).and_then(|job| job.continuation.result())
    }

    fn cancellation(&self, id: u64) -> Option<&PrivateScalarGraphCancellation> {
        self.jobs.get(&id).and_then(|job| job.cancellation.as_ref())
    }

    /// 暫停後到實際重新交付工作之間，原真借用帳可能出現新票據。
    /// E5 不能只憑最初 suspend 成功便讓工作消耗原唯一 checkpoint；
    /// 拒絕必先於佇列 pop_front 和原 evaluate_graph_driver 狀態變更。
    fn verify_wakeup_safety(
        job: &PrivateScalarCooperativeJob<'a>,
    ) -> Result<(), PrivateScalarCooperativeError> {
        if job.continuation.status != PrivateDeferredAwaitStatus::Suspended {
            return Ok(());
        }
        let saved = job.continuation.checkpoint.as_ref()
            .ok_or(PrivateScalarCooperativeError::Engine)?;
        PreparedPrivateScalarCall::check_await_safe_point_chain(
            saved.frames.iter().map(|frame| (
                &frame.borrow_tracker, frame.incoming_borrows.as_slice(),
            )),
        ).map_err(|_| PrivateScalarCooperativeError::Engine)
    }

    fn retained_resources(
        &self, id: u64,
    ) -> Option<&[crate::owned_str_runtime::ResourceEvent]> {
        self.jobs.get(&id).map(|job| job.continuation.retained_resources())
    }

    /// E2-A：同步驅動下一個已列入佇列的原 Checker 工作。
    /// 真 yield 後，除非呼叫者明確 wake，該工作不再次入佇列；
    /// 下一個 drive_one 可以先執行其他已驗證工作。
    /// 真 Checker SourceChanged 等預執行拒絕時保持佇列不變，
    /// 讓呼叫者可選擇安全取消，不能消耗／重排其他工作。
    fn drive_one(
        &mut self,
    ) -> Result<Option<(u64, PrivateScalarGraphPoll)>, PrivateScalarCooperativeError> {
        let Some(id) = self.ready.front().copied() else {
            return Ok(None);
        };
        // T4：真正 consume FIFO 之前，Prepared／Suspended 的
        // 整份原 poll／取消／終態／checkpoint 證據須與已驗收的
        // cancel／shutdown 相同；不可只在退休時才發現偽造。
        // 包含原 E5：wake 後增加的 BorrowTicket 仍會先被拒絕，
        // 且失敗前零 pop_front／零資源框架變更。
        self.verify_cancellable_evidence(id)?;
        self.verify_ready_fifo()?;
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        let poll = job.continuation.poll();
        let poll = match poll {
            Ok(poll) => poll,
            Err(_) => {
                // 只有原工作在尚未執行任何新動態步驟時，
                // 保留 ready 供明確取消；已進入而稽核失敗
                // 的工作不能被重排成可再次啟動。
                if !matches!(job.continuation.status,
                    PrivateDeferredAwaitStatus::Prepared
                    | PrivateDeferredAwaitStatus::Suspended) {
                    job.continuation.status = PrivateDeferredAwaitStatus::Failed;
                    self.ready.pop_front();
                }
                return Err(PrivateScalarCooperativeError::Engine);
            }
        };
        self.ready.pop_front();
        self.drive_order.push((id, poll));
        Ok(Some((id, poll)))
    }

    /// 只有真正停在原 Checker await 的工作才能進入可執行隊列。
    /// 重複 wake 不能建立第二個待消耗 token。
    fn wake(&mut self, id: u64) -> Result<(), PrivateScalarCooperativeError> {
        let job = self.jobs.get(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        if job.continuation.status != PrivateDeferredAwaitStatus::Suspended
            || job.continuation.pending_await().is_none()
            || self.ready.contains(&id) {
            return Err(PrivateScalarCooperativeError::WrongState);
        }
        // 真醒來之前也必須由原 FIFO 證明這份工作曾經
        // Suspended 且從未 Completed／Cancelled；不改原
        // 後續 wake 僅新增一個 ready token 的語意。
        self.verify_cancellable_evidence(id)?;
        self.verify_ready_fifo()?;
        self.ready.push_back(id);
        self.ready_issues.push(PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Woken,
        });
        // 只有成功完成 E5／FIFO 歷史預檢後才記錄此份喚醒授權；
        // 拒絕的重複／偽造 wake 不可獲得未來的再次 poll 資格。
        self.wake_order.push(PrivateScalarWakeReceipt {
            id, after_drive_count: self.drive_order.len(),
        });
        Ok(())
    }

    /// T4／E2：原始 ready 必須保留所有真 submit／wake 簽發次序。
    /// 不能只核對各工作 wake 次數或時間，讓多份合法 token
    /// 被手動互換而仍可 drive；全批取消也須在任何 P16/HC14
    /// 退出前預檢此條序列。此稽核不創造新狀態與借用責任。
    fn verify_ready_fifo(
        &self,
    ) -> Result<(), PrivateScalarCooperativeError> {
        let mut last_submitted = 0u64;
        let mut submitted = std::collections::BTreeSet::new();
        let mut expected_wakes = Vec::new();
        for issue in &self.ready_issues {
            match issue.kind {
                PrivateScalarReadyIssueKind::Submitted => {
                    let expected = last_submitted.checked_add(1)
                        .ok_or(PrivateScalarCooperativeError::Engine)?;
                    if issue.id != expected || issue.id > self.next_id
                        || !submitted.insert(issue.id) {
                        return Err(PrivateScalarCooperativeError::Engine);
                    }
                    last_submitted = issue.id;
                }
                PrivateScalarReadyIssueKind::Woken => {
                    if !submitted.contains(&issue.id) {
                        return Err(PrivateScalarCooperativeError::Engine);
                    }
                    expected_wakes.push(issue.id);
                }
            }
        }
        // 第一次入列只能 submit，後續新 token 必須有且只有
        // 成功 wake 的原時間憑據；不能補一份偽造的 ready
        // 事件以抵銷順序的竄改。
        if expected_wakes.iter().copied().ne(
            self.wake_order.iter().map(|receipt| receipt.id)
        ) || self.jobs.keys().any(|id| !submitted.contains(id)) {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        let mut last_issue = None::<usize>;
        for id in &self.ready {
            let job = self.jobs.get(id)
                .ok_or(PrivateScalarCooperativeError::Engine)?;
            let (position, issue) = self.ready_issues.iter().enumerate().rev()
                .find(|(_, issue)| issue.id == *id)
                .ok_or(PrivateScalarCooperativeError::Engine)?;
            if last_issue.is_some_and(|previous| position <= previous) {
                return Err(PrivateScalarCooperativeError::Engine);
            }
            match (job.continuation.status, issue.kind) {
                (PrivateDeferredAwaitStatus::Prepared,
                    PrivateScalarReadyIssueKind::Submitted) |
                (PrivateDeferredAwaitStatus::Suspended,
                    PrivateScalarReadyIssueKind::Woken) => {}
                _ => return Err(PrivateScalarCooperativeError::Engine),
            }
            last_issue = Some(position);
        }
        Ok(())
    }

    /// T4／E2：原 wake 不只核對數量，還需核對相鄰 poll 的真時序。
    /// 用全域 FIFO drive_order 索引查核，完全唯讀且不簽發新票據。
    fn verify_wake_chronology(
        &self, id: u64,
    ) -> Result<(), PrivateScalarCooperativeError> {
        // 真 wake 的全域 poll 時間只能不變或向前；不同工作可在
        // 兩個 poll 之間依次醒來，不能用重排歷史冒充 FIFO。
        if self.wake_order.iter().any(|receipt|
            receipt.after_drive_count > self.drive_order.len()
        ) || self.wake_order.windows(2).any(|pair|
            pair[0].after_drive_count > pair[1].after_drive_count
        ) {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        let polls = self.drive_order.iter().enumerate()
            .filter_map(|(index, (job_id, _))| (*job_id == id).then_some(index))
            .collect::<Vec<_>>();
        let wakes = self.wake_order.iter()
            .filter(|receipt| receipt.id == id)
            .collect::<Vec<_>>();
        for (index, receipt) in wakes.iter().enumerate() {
            let previous = polls.get(index)
                .ok_or(PrivateScalarCooperativeError::Engine)?;
            if receipt.after_drive_count <= *previous
                || polls.get(index + 1).is_some_and(|next|
                    receipt.after_drive_count > *next
                ) {
                return Err(PrivateScalarCooperativeError::Engine);
            }
        }
        Ok(())
    }

    /// T4／E3：Prepared／Suspended 都須有可由原 FIFO 真實生成的
    /// 待取消工作證據；不能僅信可被竄改的 status 標籤。
    /// 此為唯讀預檢：不 poll、不取消、不移交 P16 事件，
    /// 供單筆 cancel 與 shutdown 的整批前置驗證共同使用。
    fn verify_cancellable_evidence(
        &self, id: u64,
    ) -> Result<(), PrivateScalarCooperativeError> {
        let job = self.jobs.get(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        let continuation = &job.continuation;
        // 已終止工作再取消仍依原 V7 E3 API 契約回
        // WrongState，不被原取消歷史守門錯換為 Engine；
        // 只有*宣稱仍待取消*的工作才驗其歷史是否偽造。
        if !matches!(continuation.status,
            PrivateDeferredAwaitStatus::Prepared
            | PrivateDeferredAwaitStatus::Suspended
        ) {
            return Err(PrivateScalarCooperativeError::WrongState);
        }
        let polls = self.drive_order.iter()
            .filter_map(|(logged_id, poll)| (*logged_id == id).then_some(*poll))
            .collect::<Vec<_>>();
        let cancellations = self.cancellation_order.iter()
            .filter(|logged_id| **logged_id == id).count();
        self.verify_wake_chronology(id)?;
        let wakes = self.wake_order.iter()
            .filter(|receipt| receipt.id == id).count();
        let ready_count = self.ready.iter().filter(|candidate| **candidate == id).count();
        if cancellations != 0 || job.cancellation.is_some()
            || continuation.outcome.is_some() || ready_count > 1 {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        match continuation.status {
            PrivateDeferredAwaitStatus::Prepared => {
                // 第一次 poll 前絕不可能擁有 checkpoint、任何
                // 執行中事件或歷史完成／取消。來源核對拒絕
                // 也不允許把曾經啟動的工作退回 Prepared。
                if !polls.is_empty() || continuation.checkpoint.is_some()
                    || !continuation.events.is_empty()
                    || !continuation.resource_events.is_empty()
                    || ready_count != 1 || wakes != 0 {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
            }
            PrivateDeferredAwaitStatus::Suspended => {
                // 可重複合法 wake／poll，故可有多個 Suspended；
                // 但完成過、從未被真驅動過、沒有 checkpoint，
                // 或取消後又偽造暫停均不可被當成安全點。
                if polls.is_empty()
                    || polls.iter().any(|poll|
                        *poll != PrivateScalarGraphPoll::Suspended)
                    || continuation.checkpoint.is_none()
                    || continuation.pending_await().is_none()
                    // 真首次 Suspended 沒有 wake；每次完成一筆
                    // Suspended 後，下一筆 poll 必須有且只有
                    // 一次已通過 wake() 的新授權。ready=1 表示
                    // 最新 wake 尚未被 poll，ready=0 則已消耗。
                    || polls.len().checked_sub(1)
                        .and_then(|previous| previous.checked_add(ready_count))
                        != Some(wakes) {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
                Self::verify_wakeup_safety(job)?;
            }
            _ => return Err(PrivateScalarCooperativeError::WrongState),
        }
        Ok(())
    }

    /// E3-A：已暫停的真執行幀，走原 R05/P16/HC14 取消；
    /// 尚未啟動的工作只消耗 Prepared 計畫，零假資源事件。
    /// 任何安全檢查失敗都保留真 checkpoint 和原排程位置。
    fn cancel(&mut self, id: u64) -> Result<(), PrivateScalarCooperativeError> {
        self.verify_cancellable_evidence(id)?;
        self.verify_ready_fifo()?;
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        match job.continuation.status {
            PrivateDeferredAwaitStatus::Prepared => {
                job.continuation.status = PrivateDeferredAwaitStatus::Cancelled;
            }
            PrivateDeferredAwaitStatus::Suspended => {
                let evidence = job.continuation.cancel_suspended()
                    .map_err(|_| PrivateScalarCooperativeError::Engine)?;
                job.cancellation = Some(evidence);
            }
            _ => return Err(PrivateScalarCooperativeError::WrongState),
        }
        // 只有正式清理成功／未啟動才從可執行佇列移除；
        // 即使 wake 已加入 ready，取消仍能以相同工作身分處理。
        self.ready.retain(|candidate| *candidate != id);
        self.cancellation_order.push(id);
        Ok(())
    }

    /// V7-E3：顯式移交已終止工作的原執行結果及取消證據。
    /// 長期存在的 FIFO 不應永久保存每份已結束的真動態框架；
    /// 但未完成工作、仍在 ready 的工作及沒有 R05 證據的 Failed
    /// 都不得為了回收記憶體而遺失 BorrowTicket／ResourceId 責任。
    /// 已簽發的工作 ID 絕不重用；歷史排程與取消順序保留。
    /// E3／T4：對任意已終結工作僅檢查原 FIFO 與 R05 真
    /// 雙帳，不移交或消耗結果。shutdown 的整批預檢與
    /// retire_terminal 必須共用相同守門，防止安全條件分叉。
    fn verify_terminal_evidence(
        &self, id: u64,
    ) -> Result<PrivateDeferredAwaitStatus, PrivateScalarCooperativeError> {
        let job = self.jobs.get(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        let status = job.continuation.status;
        if matches!(status,
            PrivateDeferredAwaitStatus::Prepared
            | PrivateDeferredAwaitStatus::Suspended
            | PrivateDeferredAwaitStatus::Running
        ) {
            return Err(PrivateScalarCooperativeError::WrongState);
        }
        // 在移出 jobs 前完成全部守門，任何失敗都必須零修改。
        if self.ready.contains(&id)
            || job.continuation.checkpoint.is_some()
            || !job.continuation.events.is_empty()
            || !job.continuation.resource_events.is_empty() {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        // E3／T4：取消前是否曾真正 poll 過，是 FIFO 自己不可
        // 重排的驅動歷史，而不是可由終態標籤或 resource 數量
        // 推測。已開始執行後安全取消必有原 R05 退出證據；
        // 從未啟動的 Prepared 取消則必須完全沒有該證據。
        // T4／E2：同一原工作 ID 的整條 FIFO 真驅動歷史，
        // 除完成前的合法 await Suspended 外，只能出現唯一
        // 且位於末尾的 Completed。僅檢查最後一次 poll，
        // 會讓「完成後又暫停」與「重複 Completed」的偽造
        // 歷史冒充正常終態，錯誤退休後更失去 R05 來源。
        // 真工作只可由 drive_one 簽發這些事件；未知工作
        // 狀態／尚未進入過的 Prepared 不得偽造 Completed。
        let drive_history = self.drive_order.iter()
            .filter_map(|(logged_id, poll)| (*logged_id == id).then_some(*poll))
            .collect::<Vec<_>>();
        if let Some(completed_at) = drive_history.iter().position(|poll|
            *poll == PrivateScalarGraphPoll::Completed
        ) {
            if completed_at + 1 != drive_history.len()
                || drive_history.iter().filter(|poll|
                    **poll == PrivateScalarGraphPoll::Completed
                ).count() != 1 {
                return Err(PrivateScalarCooperativeError::Engine);
            }
        }
        let last_drive = drive_history.last().copied();
        let cancelled_count = self.cancellation_order.iter()
            .filter(|logged_id| **logged_id == id).count();
        self.verify_wake_chronology(id)?;
        let wake_count = self.wake_order.iter()
            .filter(|receipt| receipt.id == id).count();
        match status {
            PrivateDeferredAwaitStatus::Succeeded
            | PrivateDeferredAwaitStatus::Failed => {
                if last_drive != Some(PrivateScalarGraphPoll::Completed)
                    || cancelled_count != 0
                    // 第一筆 poll 由 submit 的 Prepared token 啟動，
                    // 其後每筆 Suspended→poll 都必須有真 wake。
                    || drive_history.len().checked_sub(1) != Some(wake_count) {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
                let outcome = job.continuation.outcome.as_ref()
                    .ok_or(PrivateScalarCooperativeError::Engine)?;
                if outcome.result.is_ok()
                        != (status == PrivateDeferredAwaitStatus::Succeeded)
                    || job.cancellation.is_some()
                    || !outcome.has_balanced_frame_trace()
                    || !crate::owned_str_runtime::event_trace_balanced(
                        &outcome.resource_events,
                    ) {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
            }
            PrivateDeferredAwaitStatus::Cancelled => {
                if job.continuation.outcome.is_some() || cancelled_count != 1 {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
                match (last_drive, job.cancellation.as_ref()) {
                    (None, None) => {
                        // 未真正進入過的 Prepared 取消，沒有任何
                        // 真函式／P16／HC14 事件可移交，也不能有 wake。
                        if wake_count != 0 {
                            return Err(PrivateScalarCooperativeError::Engine);
                        }
                    }
                    (Some(PrivateScalarGraphPoll::Suspended), Some(evidence)) => {
                        // 可以在尚未 wake 或已 wake 且尚未再次 poll
                        // 的安全點取消；不可有無法對帳的額外喚醒。
                        let before_last = drive_history.len().checked_sub(1)
                            .ok_or(PrivateScalarCooperativeError::Engine)?;
                        if wake_count != before_last
                            && wake_count != drive_history.len() {
                            return Err(PrivateScalarCooperativeError::Engine);
                        }
                        // 曾執行的 Suspended 必須真正逐框架退出。
                        // 不只確認 ResourceId Created/Released 張數，
                        // 還需拒絕 Unwind／FunctionExited／借票錯序
                        // 或刪除後冒充乾淨取消的來源偽造。
                        let audited = PrivateScalarCallOutcome {
                            result: Err(PrivateScalarCallRuntimeError::StateMismatch),
                            events: evidence.events.clone(),
                            resource_events: evidence.resource_events.clone(),
                            parameter_slots_after_exit: 0,
                        };
                        if !audited.has_balanced_frame_trace()
                            || !crate::owned_str_runtime::event_trace_balanced(
                                &evidence.resource_events,
                            ) {
                            return Err(PrivateScalarCooperativeError::Engine);
                        }
                    }
                    _ => return Err(PrivateScalarCooperativeError::Engine),
                }
            }
            _ => unreachable!("未完成狀態已於退休守門拒絕"),
        }
        Ok(status)
    }

    fn retire_terminal(
        &mut self, id: u64,
    ) -> Result<PrivateScalarRetiredJob, PrivateScalarCooperativeError> {
        let status = self.verify_terminal_evidence(id)?;
        self.verify_ready_fifo()?;
        let mut job = self.jobs.remove(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        match status {
            PrivateDeferredAwaitStatus::Succeeded
            | PrivateDeferredAwaitStatus::Failed => Ok(
                PrivateScalarRetiredJob::Completed(
                    job.continuation.outcome.take()
                        .expect("前方已證明完成結果確實存在"),
                ),
            ),
            PrivateDeferredAwaitStatus::Cancelled => Ok(
                PrivateScalarRetiredJob::Cancelled(job.cancellation.take()),
            ),
            _ => unreachable!("未完成工作不得退休"),
        }
    }

    /// E3-A：由呼叫端明確關閉整批工作；不在背景或 Drop 時
    /// 偷偷執行 async 子函式。先對**所有**待取消工作檢查
    /// 真 HC14 票據／合法暫停點，再按逆登錄順序逐一使用
    /// 已驗收的 cancel() 原 P16/HC14 退出。失敗預檢時
    /// 必須全體零狀態變更，保留每份真框架和 FIFO。
    ///
    /// 若在清理階段出現 R05/SharedExit 稽核失敗，原 cancel
    /// 會保守 Failed，不能謊報全部關閉或冒充原 Runtime 返回；
    /// 完全原子性的跨獨立 Runtime 故障回滾未經本批授權。
    fn shutdown(&mut self) -> Result<Vec<u64>, PrivateScalarCooperativeError> {
        self.verify_ready_fifo()?;
        let mut pending = Vec::new();
        for (&id, job) in self.jobs.iter().rev() {
            match job.continuation.status {
                PrivateDeferredAwaitStatus::Prepared
                | PrivateDeferredAwaitStatus::Suspended => {
                    // 在對任何工作做真正 P16/HC14 取消前，
                    // 逐份核對原 FIFO 未完成狀態與每筆歷史。
                    // 任一偽造工作不容許導致其他工作部分取消。
                    self.verify_cancellable_evidence(id)?;
                    pending.push(id);
                }
                PrivateDeferredAwaitStatus::Running =>
                    return Err(PrivateScalarCooperativeError::WrongState),
                PrivateDeferredAwaitStatus::Succeeded
                | PrivateDeferredAwaitStatus::Failed
                | PrivateDeferredAwaitStatus::Cancelled => {
                    // T4：整批關閉也必須先真核對每個已終結
                    // 工作本身的原 FIFO／取消歷史與 R05/P16
                    // 雙帳。不能先取消仍待命的另一份工作後，
                    // 才發現這份偽造的終態根本不能安全退休。
                    self.verify_terminal_evidence(id)?;
                }
            }
        }
        for id in &pending {
            self.cancel(*id)?;
        }
        Ok(pending)
    }
}



#[cfg(test)]
mod g18_v7_terminal_task_retirement_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"終態資源帳不可丟棄🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )).expect("必須來自原 Parser／Checker 核准的真正資源來源")
    }

    #[test]
    fn g18_v7_e3_完成與預算故障退休必移交R05原結果且工作身分不回繞() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let success = queue.submit(&plan).unwrap();
        let failed = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((success, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((failed, PrivateScalarGraphPoll::Completed)));
        queue.wake(success).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((success, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.jobs.len(), 2);

        let PrivateScalarRetiredJob::Completed(failure) =
            queue.retire_terminal(failed).unwrap() else {
                panic!("受控故障必保留真錯誤帳");
            };
        assert_eq!(failure.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(failure.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &failure.resource_events,
        ));
        let PrivateScalarRetiredJob::Completed(ok) =
            queue.retire_terminal(success).unwrap() else {
                panic!("正常完成必保留真結果");
            };
        assert_eq!(ok.result, Ok(11));
        assert!(ok.has_balanced_frame_trace());
        assert!(matches!(ok.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.status(success), None);
        assert!(matches!(queue.retire_terminal(success),
            Err(PrivateScalarCooperativeError::UnknownTask)));
        assert_eq!(queue.drive_order().len(), 3);
        let fresh = queue.submit(&plan).unwrap();
        assert_eq!(fresh, failed + 1);
        assert_eq!(queue.shutdown().unwrap(), [fresh]);
    }

    #[test]
    fn g18_v7_e3_準備與暫停不得退休取消後真ResourceId證據隨工作移交() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let suspended = queue.submit(&plan).unwrap();
        let prepared = queue.submit(&plan).unwrap();
        assert!(matches!(queue.retire_terminal(prepared),
            Err(PrivateScalarCooperativeError::WrongState)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((suspended, PrivateScalarGraphPoll::Suspended)));
        assert!(matches!(queue.retire_terminal(suspended),
            Err(PrivateScalarCooperativeError::WrongState)));
        assert_eq!(queue.ready_ids(), [prepared]);
        queue.cancel(suspended).unwrap();
        queue.cancel(prepared).unwrap();

        let PrivateScalarRetiredJob::Cancelled(Some(cancelled)) =
            queue.retire_terminal(suspended).unwrap() else {
                panic!("真已執行框架的取消必交付原資源事件");
            };
        assert!(matches!(cancelled.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert!(!cancelled.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(matches!(queue.retire_terminal(prepared).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        assert!(queue.ready_ids().is_empty());
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.cancellation_order(), &[suspended, prepared]);
        assert_eq!(queue.drive_one().unwrap(), None);
    }

    #[test]
    fn g18_v7_e3_未有原R05驗證的Failed不得被退休以掩蓋失敗() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        // 模擬引擎在正式結果產生之前即被阻斷：
        // Failed 標籤不得冒充已產生成功／失敗退出帳。
        queue.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Failed;
        assert!(matches!(queue.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(queue.ready_ids(), [id]);
        assert_eq!(queue.jobs.len(), 1);
        assert!(queue.outcome(id).is_none());
    }
}

#[cfg(test)]
mod g18_v7_cumulative_budget_tests {
    use super::*;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("必須是原 Parser／Checker 核准來源，不藉新測試放寬文法")
    }

    fn owned_source() -> &'static str {
        concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"同一預算真持有者🪼\"\n",
            "return await ready(7)\n",
            "}\n",
            "fn main()->int{ return wrapper() }\n",
        )
    }

    #[test]
    fn g18_v7_d1801_原真已執行工作跨await總步數不因wake重置() {
        let plan = checked(owned_source());
        let mut reference = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let consumed = reference.checkpoint.as_ref().unwrap().steps;
        assert!(consumed > 0);
        // 在原預先測得的「抵達 await 前」真正已執行步數上，
        // 僅再允許一個 EnterCall；後續子函式真正步驟必受控拒絕。
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 128, max_steps: consumed + 1,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(
            queue.jobs[&id].continuation.checkpoint.as_ref().unwrap().steps,
            consumed,
        );
        let before = queue.retained_resources(id).unwrap().to_vec();
        assert!(matches!(before.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_d1801_兩個真正await的第二個安全點與第三次poll仍共用總步數() {
        let plan = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{\n",
            "var hold:str = \"多安全點同一預算🪼\"\n",
            "return await first()+await second()\n",
            "}\n",
        ));
        let mut reference = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let first_steps = reference.checkpoint.as_ref().unwrap().steps;
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let second_steps = reference.checkpoint.as_ref().unwrap().steps;
        assert!(second_steps > first_steps);

        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 128, max_steps: second_steps + 1,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(queue.pending_await(id), Some("second"));
        assert_eq!(
            queue.jobs[&id].continuation.checkpoint.as_ref().unwrap().steps,
            second_steps,
        );
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(outcome.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.resource_events));
        assert_eq!(outcome.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "first",
        )).count(), 1);
        assert_eq!(queue.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Completed),
        ]);
    }

    #[test]
    fn g18_v7_d1801_同一原Checker來源兩份FIFO工作各自獨立預算與資源帳() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(owned_source()).unwrap();
        let plan = catalog.plan(program).unwrap();
        let mut reference = PrivateScalarGraphContinuation::new(plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let steps = reference.checkpoint.as_ref().unwrap().steps;

        let mut queue = PrivateScalarCooperativeQueue::new();
        let limited = queue.submit_program_with_budget(
            &catalog, program, PrivateScalarExecutionBudget {
                max_call_frames: 128, max_steps: steps + 1,
            },
        ).unwrap();
        let normal = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((limited, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((normal, PrivateScalarGraphPoll::Suspended)));
        queue.wake(limited).unwrap();
        queue.wake(normal).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((limited, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((normal, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(limited).unwrap().result,
            Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(queue.outcome(normal).unwrap().result, Ok(8));
        for id in [limited, normal] {
            let result = queue.outcome(id).unwrap();
            assert!(result.has_balanced_frame_trace());
            assert!(matches!(result.resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(x),
                crate::owned_str_runtime::ResourceEvent::Released(y),
            ] if x == y));
        }
        assert_eq!(queue.ready_ids(), []);
    }

    #[test]
    fn g18_v7_d1801_零環境預算與未知登錄身分不消耗FIFO工作ID() {
        let plan = checked(owned_source());
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(owned_source()).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        let before = queue.ready_ids();
        for budget in [
            PrivateScalarExecutionBudget { max_call_frames: 0, max_steps: 100 },
            PrivateScalarExecutionBudget { max_call_frames: 100, max_steps: 0 },
        ] {
            assert_eq!(queue.submit_with_budget(&plan, budget),
                Err(PrivateScalarCooperativeError::Engine));
            assert_eq!(queue.ready_ids(), before);
            assert_eq!(queue.jobs.len(), 1);
        }
        assert_eq!(queue.submit_program_with_budget(
            &catalog, program + 500, PrivateScalarExecutionBudget::default(),
        ), Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.ready_ids(), before);
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.cancel(id).unwrap();
        assert_eq!(queue.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
        ]);
    }

    #[test]
    fn g18_v7_d1801_函式框架環境預算限制在真正await前拒絕且釋放父資源() {
        let plan = checked(owned_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        assert_eq!(queue.pending_await(id), None);
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(x),
            crate::owned_str_runtime::ResourceEvent::Released(y),
        ] if x == y));
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
    }
}

#[cfg(test)]
mod g18_v7_loop_body_await_checkpoint_tests {
    use super::*;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("必須為原 Lexer／Parser／Checker 已核准的原有 await／迴圈組合")
    }

    fn for_source() -> &'static str {
        concat!(
            "async fn tick(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var total:int = 0\n",
            "for i in 0..3{\n",
            "var local:str = \"每輪真資源🪼\"\n",
            "total = total+await tick(i)\n",
            "}\n",
            "return total\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        )
    }

    #[test]
    fn g18_v7_e2_原Checker_for迴圈本體三次真await逐輪checkpoint而不重複建立資源() {
        let plan = checked(for_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        for iteration in 0..3 {
            assert_eq!(queue.drive_one().unwrap(), Some((
                id, PrivateScalarGraphPoll::Suspended,
            )));
            assert_eq!(queue.pending_await(id), Some("tick"));
            let events = queue.retained_resources(id).unwrap();
            assert_eq!(events.iter().filter(|e| matches!(
                e, crate::owned_str_runtime::ResourceEvent::Created(_),
            )).count(), iteration + 1);
            assert_eq!(events.iter().filter(|e| matches!(
                e, crate::owned_str_runtime::ResourceEvent::Released(_),
            )).count(), iteration);
            assert_eq!(queue.drive_one().unwrap(), None);
            queue.wake(id).unwrap();
        }
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 3);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "tick",
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "worker",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_for真第二輪await取消只清理當前局部而不重建前輪() {
        let plan = checked(for_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        let before = queue.retained_resources(id).unwrap();
        assert_eq!(before.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 2);
        assert_eq!(before.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 1);
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        let evidence = queue.cancellation(id).unwrap();
        assert!(crate::owned_str_runtime::event_trace_balanced(&evidence.resource_events));
        assert_eq!(evidence.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 2);
        assert_eq!(evidence.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 2);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "tick",
        )).count(), 1);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "tick",
        )).count(), 1);
        assert!(queue.drive_order().iter().all(|(task, _)| *task == id));
    }

    #[test]
    fn g18_v7_e2_原Checker_while本體await後continue真局部資源不重建() {
        let plan = checked(concat!(
            "async fn next(n:int)->int{ return n+2 }\n",
            "fn worker()->int{\n",
            "var total:int = 4\n",
            "var active:bool = true\n",
            "while(active){\n",
            "var local:str = \"while真輪🪼\"\n",
            "active = false\n",
            "total = await next(total)\n",
            "continue\n",
            "}\n",
            "return total\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(queue.pending_await(id), Some("next"));
        assert!(matches!(queue.retained_resources(id), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "next",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_原Checker_while本體await被取消後不執行後續continue或再迭代() {
        let plan = checked(concat!(
            "async fn next()->int{ return 5 }\n",
            "fn worker()->int{\n",
            "var active:bool = true\n",
            "while(active){\n",
            "var local:str = \"未退出的真局部🪼\"\n",
            "active = false\n",
            "var computed:int = await next()\n",
            "continue\n",
            "}\n",
            "return 7\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        let original = queue.retained_resources(id).unwrap().to_vec();
        assert!(matches!(original.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        let cancellation = queue.cancellation(id).unwrap();
        assert!(matches!(cancellation.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert!(!cancellation.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "next",
        )));
        assert_eq!(queue.drive_one().unwrap(), None);
        assert!(queue.outcome(id).is_none());
    }
}

#[cfg(test)]
mod g18_v7_cooperative_shutdown_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("原 Parser／Checker 真正核准的 V7 來源")
    }

    fn owned_parent_source() -> &'static str {
        concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"真正已建立的持有者🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )
    }

    #[test]
    fn g18_v7_e3_三工作安全關閉須逆登錄順序取消且各自真資源歸零() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&plan).unwrap();
        let c = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.ready_ids(), [c]);
        assert!(queue.retained_resources(a).unwrap().iter().any(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )));
        assert!(queue.retained_resources(b).unwrap().iter().any(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )));
        assert_eq!(queue.shutdown().unwrap(), [c, b, a]);
        assert_eq!(queue.cancellation_order(), &[c, b, a]);
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.drive_order(), &[
            (a, PrivateScalarGraphPoll::Suspended),
            (b, PrivateScalarGraphPoll::Suspended),
        ]);
        assert_eq!(queue.drive_one().unwrap(), None);
        for id in [a, b] {
            assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
            assert!(queue.outcome(id).is_none());
            assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(first),
                crate::owned_str_runtime::ResourceEvent::Released(last),
            ] if first == last));
        }
        assert!(queue.cancellation(c).is_none());
        assert_eq!(queue.shutdown().unwrap(), []);
        assert_eq!(queue.cancellation_order(), &[c, b, a]);
    }

    #[test]
    fn g18_v7_e3_任何待關閉工作仍持真BorrowTicket整批預檢必零狀態變更() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [a]);
        let before_a = queue.retained_resources(a).unwrap().to_vec();
        let before_b = queue.retained_resources(b).unwrap().to_vec();
        let tracker = &mut queue.jobs.get_mut(&b).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0].borrow_tracker;
        let ticket = tracker.acquire(
            DeclarationId(1881), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [a]);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(a).unwrap(), before_a.as_slice());
        assert_eq!(queue.retained_resources(b).unwrap(), before_b.as_slice());
        queue.jobs.get_mut(&b).unwrap().continuation.checkpoint.as_mut()
            .unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [b, a]);
        assert_eq!(queue.ready_ids(), []);
        for id in [a, b] {
            assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(x),
                crate::owned_str_runtime::ResourceEvent::Released(y),
            ] if x == y));
        }
    }

    #[test]
    fn g18_v7_e3_真遞迴兩層UTF8框架停在await後關閉須內層優先釋放() {
        let plan = checked(concat!(
            "async fn terminal()->int{ return 17 }\n",
            "fn descend(flag:bool)->int{\n",
            "var own:str = \"遞迴每層🪼\"\n",
            "if(flag){\nreturn descend(false)\n}\n",
            "return await terminal()\n}\n",
            "fn main()->int{ return descend(true)+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("terminal"));
        assert_eq!(queue.jobs.get(&id).unwrap().continuation.retained_frames(), 3);
        let retained = queue.retained_resources(id).unwrap();
        let [crate::owned_str_runtime::ResourceEvent::Created(outer),
             crate::owned_str_runtime::ResourceEvent::Created(inner)] = retained else {
            panic!("真正兩層遞迴持有者應已建立且尚未釋放");
        };
        assert_ne!(outer, inner);
        let (outer, inner) = (*outer, *inner);
        assert_eq!(queue.shutdown().unwrap(), [id]);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.cancellation(id).unwrap().resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(outer),
            crate::owned_str_runtime::ResourceEvent::Created(inner),
            crate::owned_str_runtime::ResourceEvent::Released(inner),
            crate::owned_str_runtime::ResourceEvent::Released(outer),
        ]);
        let evidence = queue.cancellation(id).unwrap();
        assert!(!evidence.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "terminal"
        )));
        assert_eq!(evidence.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false,
            } if name == "descend"
        )).count(), 2);
    }

    #[test]
    fn g18_v7_e2_三次for真UTF8迭代已清理後才允許await暫停並安全關閉() {
        let plan = checked(concat!(
            "async fn done(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var root:str = \"仍持有的根🪼\"\n",
            "var total:int = 0\n",
            "for i in 0..3{\n",
            "var local:str = \"每輪已退出🪼\"\n",
            "total = total+i\n",
            "}\n",
            "return await done(total)\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("done"));
        let state = queue.retained_resources(id).unwrap();
        assert_eq!(state.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 4);
        assert_eq!(state.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 3);
        assert_eq!(queue.shutdown().unwrap(), [id]);
        let events = &queue.cancellation(id).unwrap().resource_events;
        assert_eq!(events.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 4);
        assert_eq!(events.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 4);
        assert!(crate::owned_str_runtime::event_trace_balanced(events));
        assert!(!queue.cancellation(id).unwrap().events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name)
                if name == "done")));
    }

    #[test]
    fn g18_v7_e2_while真動態迴圈清理後等待由同一工作恢復不重跑迭代() {
        let plan = checked(concat!(
            "async fn done(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var root:str = \"while根🪼\"\n",
            "var active:bool = true\n",
            "var total:int = 4\n",
            "while(active){\n",
            "var local:str = \"while本輪🪼\"\n",
            "total = total+2\n",
            "active = false\n",
            "continue\n",
            "}\n",
            "return await done(total)\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("done"));
        let before = queue.retained_resources(id).unwrap();
        assert_eq!(before.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 2);
        assert_eq!(before.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 1);
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "done"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "worker"
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_關閉保留已完成工作真Outcome只取消其餘Ready或Suspended() {
        let sync = checked(concat!(
            "fn plain()->int{ return 8 }\n",
            "fn main()->int{ return plain()+0 }\n",
        ));
        let async_plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let done = queue.submit(&sync).unwrap();
        let suspended = queue.submit(&async_plan).unwrap();
        let not_started = queue.submit(&async_plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((done, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(), Some((suspended, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.ready_ids(), [not_started]);
        assert_eq!(queue.shutdown().unwrap(), [not_started, suspended]);
        assert_eq!(queue.status(done), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(queue.outcome(done).unwrap().result, Ok(8));
        assert!(queue.outcome(done).unwrap().has_balanced_frame_trace());
        assert!(queue.cancellation(done).is_none());
        assert_eq!(queue.status(not_started), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.status(suspended), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.cancellation(not_started).is_none());
        assert!(matches!(queue.cancellation(suspended).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(x),
            crate::owned_str_runtime::ResourceEvent::Released(y),
        ] if x == y));
        assert!(queue.ready_ids().is_empty());
    }

    #[test]
    fn g18_v7_e3_尚有Running工作關閉保守拒絕且保持整批FIFO() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let first = queue.submit(&plan).unwrap();
        let second = queue.submit(&plan).unwrap();
        queue.jobs.get_mut(&second).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Running;
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.ready_ids(), [first, second]);
        assert!(queue.cancellation_order().is_empty());
        queue.jobs.get_mut(&second).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        assert_eq!(queue.shutdown().unwrap(), [second, first]);
        assert!(queue.ready_ids().is_empty());
    }

    #[test]
    fn g18_v7_e3_尚未開始的三份來源關閉不得產生任何真Runtime事件() {
        let source = checked(concat!(
            "async fn ready()->int{\n",
            "var resource:str = \"未執行不得建立\"\n",
            "return 14\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&source).unwrap();
        let b = queue.submit(&source).unwrap();
        let c = queue.submit(&source).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [c, b, a]);
        assert!(queue.drive_order().is_empty());
        for id in [a, b, c] {
            assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
            assert!(queue.cancellation(id).is_none());
            assert!(queue.outcome(id).is_none());
            assert!(queue.retained_resources(id).unwrap().is_empty());
        }
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.shutdown().unwrap(), []);
    }
}

#[cfg(test)]
mod g18_v7_checked_program_catalog_tests {
    use super::*;

    fn source(n: i64) -> String {
        format!(
            "async fn ready()->int{{ return {n} }}\n\
             fn main()->int{{ return await ready()+0 }}\n"
        )
    }

    #[test]
    fn g18_v7_a1_原Checker來源由登錄持有且FIFO續行引用同一真計畫身分() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(&source(17)).unwrap();
        assert_eq!(program, 1);
        assert_eq!(catalog.len(), 1);
        let checked_ptr = catalog.plan(program).unwrap() as *const _;
        let mut queue = PrivateScalarCooperativeQueue::new();
        let task = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(task, 1);
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
        assert_eq!(queue.status(task), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(task), Some("ready"));
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
        queue.wake(task).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(task).unwrap().result, Ok(17));
        assert!(queue.outcome(task).unwrap().has_balanced_frame_trace());
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
    }

    #[test]
    fn g18_v7_a1_靜態非法來源零身分消耗且原Checker合法bool動態圖完整保留() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let invalid = concat!(
            "async fn ready()->int{ return true }\n",
            "fn main()->int{ return await ready()+0 }\n",
        );
        assert!(matches!(catalog.prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        assert_eq!(catalog.len(), 0);
        // 原 Checker 合法的 bool 參數呼叫已經採真 main_dynamic_call；
        // 不可因為另有 int-only 直接快路的保守界線，誤拒完整動態圖。
        let bool_graph = concat!(
            "fn choose(flag:bool)->int{\n",
            "if(flag){return 4}\nreturn 7\n}\n",
            "fn main()->int{ return choose(true) }\n",
        );
        let flag_id = catalog.prepare(bool_graph).unwrap();
        assert_eq!(flag_id, 1);
        assert!(catalog.plan(flag_id).unwrap().main_dynamic_call.is_some());
        let second = catalog.prepare(&source(6)).unwrap();
        assert_eq!(second, 2);
        assert_eq!(catalog.len(), 2);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let flag_task = queue.submit_program(&catalog, flag_id).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((flag_task, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(flag_task).unwrap().result, Ok(4));
        assert!(queue.outcome(flag_task).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e2_未知來源登錄ID不得消耗FIFO工作身分或其他已核准佇列() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(&source(8)).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        assert_eq!(queue.submit_program(&catalog, 500),
            Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.ready_ids(), []);
        let task = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(task, 1);
        assert_eq!(queue.submit_program(&catalog, 500),
            Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.ready_ids(), [task]);
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(task).unwrap();
        assert_eq!(queue.status(task), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn g18_v7_e2_同一份原Checker登錄來源可產生兩份相互獨立的真續行() {
        let source = concat!(
            "async fn ready()->int{ return 33 }\n",
            "fn wrapper()->int{\n",
            "var own:str = \"同源各有真Runtime🪼\"\n",
            "return await ready()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        );
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(source).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit_program(&catalog, program).unwrap();
        let b = queue.submit_program(&catalog, program).unwrap();
        assert_ne!(a, b);
        assert!(std::ptr::eq(
            queue.jobs.get(&a).unwrap().continuation.plan,
            queue.jobs.get(&b).unwrap().continuation.plan,
        ));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert!(matches!(queue.retained_resources(a), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        assert!(matches!(queue.retained_resources(b), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        queue.cancel(a).unwrap();
        queue.wake(b).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.outcome(a).is_none());
        assert_eq!(queue.outcome(b).unwrap().result, Ok(33));
        assert!(matches!(queue.cancellation(a).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(matches!(queue.outcome(b).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(queue.outcome(b).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e3_登錄來源真溢位及取消分別沿原Runtime清理不偽造成功() {
        let failed = concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn main()->int{\n",
            "var own:str = \"原溢位也清理🪼\"\n",
            "return await over(9223372036854775807)+0\n}\n",
        );
        let cancelled = concat!(
            "async fn no()->int{ return 9 }\n",
            "fn main()->int{\n",
            "var own:str = \"取消前還在🪼\"\n",
            "return await no()+0\n}\n",
        );
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let failure_program = catalog.prepare(failed).unwrap();
        let cancel_program = catalog.prepare(cancelled).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit_program(&catalog, failure_program).unwrap();
        let b = queue.submit_program(&catalog, cancel_program).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        let outcome = queue.outcome(a).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(outcome.has_balanced_frame_trace());
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(matches!(queue.cancellation(b).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn g18_v7_a1_來源登錄身分耗盡時原計畫不回繞也不污染先前工作() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let original = catalog.prepare(&source(4)).unwrap();
        assert_eq!(original, 1);
        catalog.next_id = u64::MAX;
        assert!(matches!(catalog.prepare(&source(8)),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(catalog.len(), 1);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_program(&catalog, original).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(4));
    }

    #[test]
    fn g18_v7_a1_排程器完成後釋放借用原登錄仍能增加下一份真來源() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let one = catalog.prepare(&source(5)).unwrap();
        {
            let mut queue = PrivateScalarCooperativeQueue::new();
            let id = queue.submit_program(&catalog, one).unwrap();
            assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
            queue.wake(id).unwrap();
            assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
            assert_eq!(queue.outcome(id).unwrap().result, Ok(5));
        }
        // Rust 編譯期保證：仍被續行借用時 catalog 不可變更；
        // 所有相關工作自然結束／離域後才重新取得唯一可變存取。
        let two = catalog.prepare(&source(12)).unwrap();
        assert_eq!(two, 2);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_program(&catalog, two).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(12));
    }
}

#[cfg(test)]
mod g18_v7_cooperative_graph_queue_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(text: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(text)
            .expect("必須使用原 Parser／Checker 真正核准的私有來源")
    }

    #[test]
    fn g18_v7_e2_真動態工作三份FIFO交錯後依明確喚醒次序完成() {
        let left = checked(concat!(
            "async fn left()->int{ return 7 }\n",
            "fn wrapper()->int{\n",
            "var parent:str = \"佇列內原資源🪼\"\n",
            "return await left()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let right = checked(concat!(
            "async fn right()->int{ return 11 }\n",
            "fn main()->int{ return await right()+0 }\n",
        ));
        let sync = checked(concat!(
            "fn simple()->int{ return 19 }\n",
            "fn main()->int{ return simple()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&left).unwrap();
        let b = queue.submit(&right).unwrap();
        let c = queue.submit(&sync).unwrap();
        assert_eq!((a, b, c), (1, 2, 3));
        assert_eq!(queue.ready_ids(), [a, b, c]);
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(queue.drive_order().is_empty());
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("left"));
        assert!(matches!(queue.retained_resources(a), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((c, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(c).unwrap().result, Ok(19));
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.drive_one().unwrap(), None);
        assert!(queue.outcome(a).is_none());
        queue.wake(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [b, a]);
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(11));
        let finished = queue.outcome(a).unwrap();
        assert_eq!(finished.result, Ok(7));
        assert!(finished.has_balanced_frame_trace());
        assert!(matches!(finished.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.drive_order(), &[
            (a, PrivateScalarGraphPoll::Suspended),
            (b, PrivateScalarGraphPoll::Suspended),
            (c, PrivateScalarGraphPoll::Completed),
            (b, PrivateScalarGraphPoll::Completed),
            (a, PrivateScalarGraphPoll::Completed),
        ]);
    }

    #[test]
    fn g18_v7_e2_多await工作在不同真指令位置反覆yield且其他工作可插入() {
        let multi = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{ return await first()+await second() }\n",
        ));
        let other = checked(concat!(
            "async fn other()->int{ return 40 }\n",
            "fn main()->int{ return await other()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&multi).unwrap();
        let b = queue.submit(&other).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("first"));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("second"));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        queue.wake(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(40));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(a).unwrap().result, Ok(5));
        let awaited = queue.outcome(a).unwrap().events.iter().filter_map(|event|
            match event {
                PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name.as_str()),
                _ => None,
            }
        ).collect::<Vec<_>>();
        assert_eq!(awaited, ["first", "second"]);
        assert!(queue.outcome(a).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e3_已醒來尚待執行的真父框架取消會移除FIFO並真釋放資源() {
        let active = checked(concat!(
            "async fn never()->int{ return 8 }\n",
            "fn wrapper()->int{\n",
            "var own:str = \"原工作唯一持有🪼\"\n",
            "return await never()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let next = checked(concat!(
            "fn ready()->int{ return 12 }\n",
            "fn main()->int{ return ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&active).unwrap();
        let b = queue.submit(&next).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [b, a]);
        queue.cancel(a).unwrap();
        assert_eq!(queue.ready_ids(), [b]);
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.outcome(a).is_none());
        assert_eq!(queue.cancellation_order(), &[a]);
        let evidence = queue.cancellation(a).unwrap();
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(!evidence.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "never",
        )));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(12));
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.cancel(a), Err(PrivateScalarCooperativeError::WrongState));
    }

    #[test]
    fn g18_v7_e5_喚醒前及已排隊後新增真祖先票據都不得偷消耗checkpoint() {
        let plan = checked(concat!(
            "async fn work()->int{ return 5 }\n",
            "fn main()->int{\n",
            "var hold:str = \"不許跨等待借用🪼\"\n",
            "return await work()+0\n}\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        let before = queue.retained_resources(id).unwrap().to_vec();
        let source = DeclarationId(974);
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(queue.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert!(queue.ready_ids().is_empty());
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), before.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        queue.wake(id).unwrap();
        assert_eq!(queue.ready_ids(), [id]);
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(queue.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [id]);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), before.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(5));
        assert!(matches!(queue.outcome(id).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
    }

    #[test]
    fn g18_v7_e3_取消前真祖先票據未還需保留已喚醒FIFO與原ResourceId() {
        let plan = checked(concat!(
            "async fn async_fn()->int{ return 4 }\n",
            "fn main()->int{\nvar own:str = \"待取消🪼\"\n",
            "return await async_fn()+0\n}\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        let original = queue.retained_resources(id).unwrap().to_vec();
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(DeclarationId(975), BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(queue.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [id]);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), original.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        queue.cancel(id).unwrap();
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.cancellation_order(), &[id]);
        assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
    }

    #[test]
    fn g18_v7_e2_普通同步工作不得醒來且重複喚醒不得重複入FIFO() {
        let plan = checked(concat!(
            "async fn target()->int{ return 9 }\n",
            "fn main()->int{ return await target()+0 }\n",
        ));
        let sync = checked(concat!(
            "fn plain()->int{ return 3 }\n",
            "fn main()->int{ return plain()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&sync).unwrap();
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.wake(b), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.ready_ids(), [b, a]);
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.wake(b), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(a).unwrap().result, Ok(9));
        assert_eq!(queue.drive_order().len(), 3);
    }

    #[test]
    fn g18_v7_e3_未啟動工作取消必零真動態事件且無未知工作副作用() {
        let plan = checked(concat!(
            "async fn ready()->int{\nvar never:str = \"不得建構\"\n",
            "return 9\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        assert_eq!(queue.wake(812), Err(PrivateScalarCooperativeError::UnknownTask));
        assert_eq!(queue.cancel(812), Err(PrivateScalarCooperativeError::UnknownTask));
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.ready_ids(), [id]);
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.ready_ids(), []);
        assert!(queue.cancellation(id).is_none());
        assert!(queue.outcome(id).is_none());
        assert!(queue.retained_resources(id).unwrap().is_empty());
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.drive_order(), &[]);
        assert_eq!(queue.cancellation_order(), &[id]);
    }

    #[test]
    fn g18_v7_e2_工作身分耗盡必拒絕且不污染原待執行FIFO() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 1 }\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let valid = queue.submit(&plan).unwrap();
        queue.next_id = u64::MAX;
        assert_eq!(queue.submit(&plan), Err(PrivateScalarCooperativeError::IdExhausted));
        assert_eq!(queue.ready_ids(), [valid]);
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.drive_order(), &[]);
        assert_eq!(queue.drive_one().unwrap(), Some((valid, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(valid).unwrap();
        assert_eq!(queue.status(valid), Some(PrivateDeferredAwaitStatus::Cancelled));
    }
}

// G18／V7-A1：同一私有 Checker 已核准的直接 await 來源延遲工作。
// 此處跨 Rust API 呼叫真正保留「未開始執行」的工作計畫；執行中
// 函式的顯式 Task/Frame 堆疊尚未成為可持久化協程，不能宣稱 A2 Closed。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateDeferredAwaitStatus {
    Prepared,
    Suspended,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateDeferredAwaitError {
    UnknownTask,
    WrongState,
    AlreadyFinished,
    Cancelled,
    SourceChanged,
}

struct PrivateDeferredAwaitJob {
    plan: Option<PreparedPrivateScalarCall>,
    status: PrivateDeferredAwaitStatus,
    outcome: Option<PrivateScalarCallOutcome>,
}

/// 只管理本次已核准的私有直接 await；每一個工作有唯一遞增 ID。
/// 原始 TypedHIR、函式圖及 UTF-8 原始來源保存在 PreparedPrivateScalarCall。
/// 此佇列不讀取新語法、不自行簽發 BorrowFact 或 ResourceId。
#[derive(Default)]
struct PrivateCheckedDeferredAwaitQueue {
    jobs: BTreeMap<u64, PrivateDeferredAwaitJob>,
    next_id: u64,
    execution_order: Vec<u64>,
}

impl PrivateCheckedDeferredAwaitQueue {
    fn new() -> Self { Self::default() }

    fn prepare(&mut self, source: &str)
        -> Result<u64, PrivateScalarCallPrepareError> {
        let plan = PreparedPrivateScalarCall::prepare(source)?;
        // E1：延遲啟動前必須確定 main 的唯一真正可達語句是
        // 直接 await return。若存在先前宣告、控制流程或複合
        // expression，沒有保存已執行動態框架的證據，一律拒絕。
        // 不能以「預先執行到 await 再重跑來源」模擬 suspend。
        let direct_await = matches!(
            plan.main_dynamic_call.as_ref(),
            Some(PrivateScalarReturnExpr::Call { awaited: true, .. })
        );
        let main = plan.verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "私有延遲工作缺少原 Checker main 宣告",
            ))?;
        if !direct_await || main.body.statements.len() != 1
            || !plan.main_dynamic_steps.is_empty()
            || !plan.main_dynamic_inits.is_empty()
            || !plan.main_owners.is_empty() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "只有原 Checker 證明 main 無前置作用的直接 await 可延遲",
            ));
        }
        let id = self.next_id.checked_add(1)
            .expect("內部私有延遲工作 ID 已用盡，禁止回繞");
        self.next_id = id;
        self.jobs.insert(id, PrivateDeferredAwaitJob {
            plan: Some(plan), status: PrivateDeferredAwaitStatus::Prepared,
            outcome: None,
        });
        Ok(id)
    }

    fn status(&self, id: u64) -> Option<PrivateDeferredAwaitStatus> {
        self.jobs.get(&id).map(|job| job.status)
    }

    fn execution_order(&self) -> &[u64] { &self.execution_order }

    /// 明確建立可跨 Rust 呼叫保存的安全點；目前尚未開始
    /// main 真執行，沒有任何資源／借用／控制事件可被偽造。
    fn suspend(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if job.status != PrivateDeferredAwaitStatus::Prepared {
            return Err(PrivateDeferredAwaitError::WrongState);
        }
        job.status = PrivateDeferredAwaitStatus::Suspended;
        Ok(())
    }

    /// E3：取消 Prepared 或 Suspended 但尚未啟動的工作；
    /// 不建立函式框架、不執行原來源，也不記入 Released 假事件。
    fn cancel(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if !matches!(job.status,
            PrivateDeferredAwaitStatus::Prepared | PrivateDeferredAwaitStatus::Suspended) {
            return Err(PrivateDeferredAwaitError::AlreadyFinished);
        }
        job.plan = None;
        job.status = PrivateDeferredAwaitStatus::Cancelled;
        Ok(())
    }

    /// 唯一已保留的原 Checker 工作在喚醒時才由真私有 Runtime 執行。
    /// 成功、受控溢位／預算等動態錯誤均保留原事件與清理結果；
    /// SourceChanged 在任何動態執行前拒絕，不消耗工作或偽造事件。
    fn wake(&mut self, id: u64)
        -> Result<&PrivateScalarCallOutcome, PrivateDeferredAwaitError> {
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        match job.status {
            PrivateDeferredAwaitStatus::Suspended => {}
            PrivateDeferredAwaitStatus::Cancelled =>
                return Err(PrivateDeferredAwaitError::Cancelled),
            _ => return Err(PrivateDeferredAwaitError::WrongState),
        }
        job.status = PrivateDeferredAwaitStatus::Running;
        let outcome = match job.plan.as_ref()
            .expect("Suspended 工作仍有唯一原 Checker 工作計畫").execute() {
            Ok(outcome) => outcome,
            Err(_) => {
                job.status = PrivateDeferredAwaitStatus::Suspended;
                return Err(PrivateDeferredAwaitError::SourceChanged);
            }
        };
        job.status = if outcome.result.is_ok() {
            PrivateDeferredAwaitStatus::Succeeded
        } else {
            PrivateDeferredAwaitStatus::Failed
        };
        job.plan = None;
        job.outcome = Some(outcome);
        self.execution_order.push(id);
        Ok(job.outcome.as_ref().expect("只保存真私有執行器結果"))
    }
}






#[cfg(test)]
mod g18_v7_fastpath_graph_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e2_直接main呼叫原私有wrapper真UTF8幀可跨呼叫暫停恢復() {
        let plan = checked(concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"直接快路原資源🪼\"\n",
            "return await ready(6)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        assert!(plan.main_dynamic_call.is_none());
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("ready"));
        assert_eq!(live.retained_frames(), 1);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper"
        )).count(), 1);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "wrapper",
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_e2_快路多int實參沿原Checker宣告順序綁定並恢復await() {
        let plan = checked(concat!(
            "async fn increase(n:int)->int{ return n+1 }\n",
            "fn wrapper(first:int,second:int)->int{\n",
            "return await increase(first+second)\n}\n",
            "fn main()->int{ return wrapper(2,3) }\n",
        ));
        assert!(plan.main_dynamic_call.is_none());
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let bound = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::ParameterBound(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        // 真內層 async increase(n) 亦會新增自己的合法 ParameterBound，
        // 只取原 wrapper 的前兩筆對帳，不可把真正第三筆誤判成重複。
        assert_eq!(bound[..plan.callee_parameter_ids.len()],
            plan.callee_parameter_ids.iter().copied().zip([2,3]).collect::<Vec<_>>());
    }

    #[test]
    fn g18_v7_e3_直接main快路父資源暫停取消走原真scope退場() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 9 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"取消快路🪼\"\n",
            "return await ready()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(live.result().is_none());
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert!(live.result().is_none());
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "wrapper",
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready",
        )));
    }

    #[test]
    fn g18_v7_e4_直接main快路await後原運算溢位不生成成功Returned() {
        let plan = checked(concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"溢位仍須退場🪼\"\n",
            "return await over(9223372036854775807)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "wrapper",
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }
}

#[cfg(test)]
mod g18_v7_real_graph_cancel_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e3_真父子函式已建兩份UTF8資源後取消須逆序真釋放() {
        let plan = checked(concat!(
            "async fn target()->int{ return 4 }\n",
            "fn inner()->int{\n",
            "var child:str = \"內🪼\"\n",
            "return await target()\n}\n",
            "fn outer()->int{\n",
            "var parent:str = \"外🪼\"\n",
            "return inner()\n}\n",
            "fn main()->int{ return outer()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("target"));
        assert_eq!(live.retained_frames(), 3);
        let (parent, child) = match live.retained_resources() {
            [crate::owned_str_runtime::ResourceEvent::Created(parent),
             crate::owned_str_runtime::ResourceEvent::Created(child)] => (*parent, *child),
            _ => panic!("原父子已執行真 UTF-8 必各持唯一 ResourceId"),
        };
        assert_ne!(parent, child);
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert!(live.result().is_none());
        assert_eq!(live.retained_frames(), 0);
        assert_eq!(live.pending_await(), None);
        assert_eq!(evidence.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(parent),
            crate::owned_str_runtime::ResourceEvent::Created(child),
            crate::owned_str_runtime::ResourceEvent::Released(child),
            crate::owned_str_runtime::ResourceEvent::Released(parent),
        ]);
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
                | PrivateScalarCallEvent::Returned(_),
        )));
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_v7_e3_第一次真await已完成第二次暫停取消不得重執行第一個() {
        let plan = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{ return await first()+await second() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("second"));
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "first"
        )).count(), 1);
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "second"
        )));
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "first"
        )).count(), 1);
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "second"
        )));
        assert!(evidence.resource_events.is_empty());
    }

    #[test]
    fn g18_v7_e3_尚未執行與已完成的真工作不能冒稱安全點取消() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 8 }\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        live.cancel_suspended().unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(matches!(live.poll(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        let mut completed = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(completed.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(completed.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert!(matches!(completed.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(completed.result().unwrap().result, Ok(8));
    }

    #[test]
    fn g18_v7_e3_暫停後意外新增真BorrowTicket時取消拒絕並保留原框架() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 4 }\n",
            "fn main()->int{\n",
            "var own:str = \"不能提前釋放🪼\"\n",
            "return await ready()+0\n}\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let original = live.retained_resources().to_vec();
        let source = DeclarationId(987);
        let ticket = live.checkpoint.as_mut().unwrap()
            .frames[0].borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Suspended);
        assert_eq!(live.retained_resources(), original.as_slice());
        assert_eq!(live.retained_frames(), 1);
        live.checkpoint.as_mut().unwrap()
            .frames[0].borrow_tracker.release(ticket).unwrap();
        let evidence = live.cancel_suspended().unwrap();
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
    }

    #[test]
    fn g18_v7_e3_暫停取消只留下真退場事件不釋放未啟動async子函式資源() {
        let plan = checked(concat!(
            "async fn ready()->int{\n",
            "var never:str = \"不得建構\"\n",
            "return 6\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(live.retained_resources().is_empty());
        let evidence = live.cancel_suspended().unwrap();
        assert!(evidence.resource_events.is_empty());
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
        assert!(live.result().is_none());
    }
}

#[cfg(test)]
mod g18_v7_real_graph_checkpoint_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e2_原私有函式真UTF8資源與顯式工作棧跨Rust呼叫暫停後原點恢復() {
        let plan = checked(concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"父框架已執行🪼\"\n",
            "return await ready(6)\n}\n",
            "fn main()->int{ return wrapper()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("ready"));
        assert_eq!(live.retained_frames(), 2);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert!(live.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper",
        )));
        assert!(!live.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
        assert!(live.result().is_none());

        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready"
        )).count(), 1);
        assert_eq!(live.retained_frames(), 0);
        assert_eq!(live.pending_await(), None);
        assert!(plan.audit_graph_outcome(PrivateScalarCallOutcome {
            result: out.result.clone(), events: out.events.clone(),
            resource_events: out.resource_events.clone(),
            parameter_slots_after_exit: out.parameter_slots_after_exit,
        }).is_ok());
    }

    #[test]
    fn g18_v7_e2_原Checker兩個真await可跨三次呼叫從不同指令位置接續() {
        let plan = checked(concat!(
            "async fn left(n:int)->int{ return n+1 }\n",
            "async fn right(n:int)->int{ return n+2 }\n",
            "fn main()->int{ return await left(2)+await right(3) }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("left"));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("right"));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let awaited = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(awaited, ["left", "right"]);
        assert!(matches!(live.poll(),
            Err(PrivateScalarCallPrepareError::Unsupported(
                "已結束的私有工作不得重複 poll",
            )),
        ));
    }

    #[test]
    fn g18_v7_e2_私有真框架暫停後能先執行另一工作再喚醒原工作() {
        let first = checked(concat!(
            "async fn ready()->int{ return 13 }\n",
            "fn main()->int{ return await ready() }\n",
        ));
        let second = checked(concat!(
            "async fn other()->int{ return 21 }\n",
            "fn main()->int{ return await other() }\n",
        ));
        let mut one = PrivateScalarGraphContinuation::new(&first).unwrap();
        let mut two = PrivateScalarGraphContinuation::new(&second).unwrap();
        assert_eq!(one.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(two.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(two.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(two.result().unwrap().result, Ok(21));
        assert!(one.result().is_none());
        assert_eq!(one.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(one.result().unwrap().result, Ok(13));
        assert!(one.result().unwrap().has_balanced_frame_trace());
        assert!(two.result().unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e2_真私有await暫停後溢位仍只清理原P16資源不虛構返回() {
        let plan = checked(concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn main()->int{\n",
            "var hold:str = \"原框架錯誤也須釋放🪼\"\n",
            "return await over(9223372036854775807)\n}\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_e2_同步原Checker工作不產生虛假的可暫停await() {
        let plan = checked(concat!(
            "fn forward(n:int)->int{ return n+4 }\n",
            "fn main()->int{ return forward(2)+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(live.result().unwrap().result, Ok(6));
        assert_eq!(live.pending_await(), None);
        assert!(!live.result().unwrap().events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
    }

    #[test]
    fn g18_v7_e2_原Checker未授權跨await借用不能建立動態續行() {
        let source = concat!(
            "async fn ready()->int{ return 4 }\n",
            "fn wrapper(source:str)->int{ return await ready() }\n",
            "fn main()->int{\n",
            "var source:str = \"仍有效🪼\"\n",
            "return wrapper(source)\n}\n",
        );
        match PreparedPrivateScalarCall::prepare(source) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 必須先拒絕，不能建立真續行"),
        }
    }
}

#[cfg(test)]
mod g18_v7_ancestor_borrow_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    #[test]
    fn g18_v7_e5_外層真BorrowTicket未還不能因內層無票據而進await() {
        let mut parent = crate::borrow_safety::BorrowSafety::new();
        let child = crate::borrow_safety::BorrowSafety::new();
        let source = DeclarationId(501);
        let ticket = parent.acquire(source, BorrowKind::SharedRead).unwrap();
        let empty: &[(crate::borrow_safety::BorrowTicket, BorrowFact)] = &[];
        // 原先只檢查最內層 child 將錯誤放行；新的全框架安全點必拒。
        assert!(PreparedPrivateScalarCall::check_await_safe_point(&child, empty).is_ok());
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain([
                (&parent, empty), (&child, empty),
            ]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )),
        );
        assert_eq!(parent.check_ticket_access(ticket, BorrowKind::SharedRead), Ok(source));
        parent.release(ticket).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point_chain([
            (&parent, empty), (&child, empty),
        ]).is_ok());
    }

    #[test]
    fn g18_v7_e5_外層傳入仍有效的BorrowFact不能跨過內層await() {
        let parent = crate::borrow_safety::BorrowSafety::new();
        let child = crate::borrow_safety::BorrowSafety::new();
        let mut authority = crate::borrow_safety::BorrowSafety::new();
        let fact = BorrowFact::new(DeclarationId(502), BorrowKind::SharedRead);
        let ticket = authority.acquire(fact.source, fact.kind).unwrap();
        let incoming = [(ticket, fact)];
        let empty: &[(crate::borrow_safety::BorrowTicket, BorrowFact)] = &[];
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain([
                (&parent, incoming.as_slice()), (&child, empty),
            ]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )),
        );
        authority.finish_selected_verified(&[ticket], &[fact]).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point_chain([
            (&parent, empty), (&child, empty),
        ]).is_ok());
    }

    #[test]
    fn g18_v7_e5_空函式鏈不能被當成已驗證await安全點() {
        let empty: [(&crate::borrow_safety::BorrowSafety,
            &[(crate::borrow_safety::BorrowTicket, BorrowFact)]); 0] = [];
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain(empty),
            Err(PrivateScalarCallRuntimeError::StateMismatch),
        );
    }

    #[test]
    fn g18_v7_e5_真Checker三層函式皆無借用可正常直接await() {
        let source = concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn inner(n:int)->int{ return await ready(n) }\n",
            "fn outer(n:int)->int{ return inner(n) }\n",
            "fn main()->int{ return outer(8) }\n",
        );
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert!(outcome.has_balanced_frame_trace());
        assert_eq!(outcome.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready",
        )).count(), 1);
        assert!(plan.audit_graph_outcome(outcome).is_ok());
    }

    #[test]
    fn g18_v7_e5_原Checker仍須拒絕未證歸還的跨await字串借用() {
        let source = concat!(
            "async fn ready()->int{ return 6 }\n",
            "fn inner(source:str)->int{ return await ready() }\n",
            "fn outer(source:str)->int{ return inner(source) }\n",
            "fn main()->int{\n",
            "var source:str = \"有效借用🪼\"\n",
            "return outer(source)\n}\n",
        );
        match PreparedPrivateScalarCall::prepare(source) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 必須在動態工作圖建立前拒絕借用跨 await"),
        }
    }
}

#[cfg(test)]
mod g18_v7_private_deferred_await_tests {
    use super::*;

    fn src(value: i64) -> String {
        format!(
            "async fn ready()->int{{ return {value} }}\n\
             fn main()->int{{ return await ready() }}\n"
        )
    }

    #[test]
    fn g18_v7_a1_私有Checker工作可跨呼叫保存直到真喚醒才執行() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(23)).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(queue.execution_order().is_empty());
        assert!(queue.jobs[&id].outcome.is_none());
        queue.suspend(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(queue.jobs[&id].plan.is_some());
        assert!(queue.execution_order().is_empty());
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Ok(23));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready",
        )));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(queue.execution_order(), &[id]);
        assert!(queue.jobs[&id].plan.is_none());
    }

    #[test]
    fn g18_v7_a1_兩個私有Checked工作只依真喚醒順序執行() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let first = queue.prepare(&src(1)).unwrap();
        let second = queue.prepare(&src(2)).unwrap();
        queue.suspend(first).unwrap();
        queue.suspend(second).unwrap();
        assert_eq!(queue.wake(second).unwrap().result, Ok(2));
        assert_eq!(queue.status(first), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.wake(first).unwrap().result, Ok(1));
        assert_eq!(queue.execution_order(), &[second, first]);
    }

    #[test]
    fn g18_v7_a1_取消已暫停私有工作不執行任何函式事件() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(12)).unwrap();
        queue.suspend(id).unwrap();
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.jobs[&id].plan.is_none());
        assert!(queue.jobs[&id].outcome.is_none());
        assert!(queue.execution_order().is_empty());
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::Cancelled));
        assert_eq!(queue.cancel(id), Err(PrivateDeferredAwaitError::AlreadyFinished));
    }

    #[test]
    fn g18_v7_a1_真正私有async局部UTF8只在喚醒後建構並唯一釋放() {
        let source = concat!(
            "async fn ready()->int{\n",
            "var hold:str = \"真正🪼UTF-8\"\n",
            "return 27\n}\n",
            "fn main()->int{ return await ready() }\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(source).unwrap();
        queue.suspend(id).unwrap();
        assert!(queue.jobs[&id].outcome.is_none());
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Ok(27));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_a2_私有直接等待真運算溢位後錯誤與資源單次退出() {
        let source = concat!(
            "async fn overflow()->int{\n",
            "var hold:str = \"出錯也要清理🪼\"\n",
            "return 9223372036854775807+1\n}\n",
            "fn main()->int{ return await overflow() }\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(source).unwrap();
        queue.suspend(id).unwrap();
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(queue.execution_order(), &[id]);
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
    }

    #[test]
    fn g18_v7_a1_有前置main資源不得假裝成已保存的等待框架() {
        let source = concat!(
            "async fn ready()->int{ return 3 }\n",
            "fn main()->int{\n",
            "var hold:str = \"不准偷偷跳過\"\n",
            "return await ready()\n}\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        assert!(matches!(queue.prepare(source),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert!(queue.execution_order().is_empty());
    }

    #[test]
    fn g18_v7_a1_複合await與普通同步呼叫不能冒充直接等待() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let addition = concat!(
            "async fn ready()->int{ return 3 }\n",
            "fn main()->int{ return await ready()+1 }\n",
        );
        assert!(matches!(queue.prepare(addition),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        let sync = concat!(
            "fn plain()->int{ return 7 }\n",
            "fn main()->int{ return plain() }\n",
        );
        assert!(matches!(queue.prepare(sync),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn g18_v7_a1_靜態非法來源不得建立工作或消耗工作身分() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let invalid = concat!(
            "async fn ready()->int{ return true }\n",
            "fn main()->int{ return await ready() }\n",
        );
        assert!(matches!(queue.prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let id = queue.prepare(&src(7)).unwrap();
        assert_eq!(id, 1);
        assert!(queue.execution_order().is_empty());
        assert_eq!(queue.suspend(100), Err(PrivateDeferredAwaitError::UnknownTask));
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
        queue.cancel(id).unwrap();
    }

    #[test]
    fn g18_v7_a1_已封存私有來源竄改不能執行而可安全取消() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(17)).unwrap();
        queue.suspend(id).unwrap();
        queue.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().original[0] = b'!';
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(queue.execution_order().is_empty());
        assert!(queue.jobs[&id].outcome.is_none());
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn g18_v7_a1_真工作不允許重複暫停或重複喚醒() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(9)).unwrap();
        queue.suspend(id).unwrap();
        assert_eq!(queue.suspend(id), Err(PrivateDeferredAwaitError::WrongState));
        assert_eq!(queue.wake(id).unwrap().result, Ok(9));
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
        assert_eq!(queue.cancel(id), Err(PrivateDeferredAwaitError::AlreadyFinished));
        assert_eq!(queue.execution_order(), &[id]);
    }
}

#[cfg(test)]
mod g18_private_scalar_call_tests {
    use super::*;
    use crate::common_ir::NodeSpan;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src)
            .expect("真私有 Checker 已核准 Copyable int 參數應能進入動態框架")
    }

    fn literal_src() -> &'static str {
        "fn main()->int{\nreturn forward(7)\n}\nfn forward(value:int)->int{\nreturn value\n}\n"
    }

    // E5：真動態 BorrowSafety 安全點只在原 Checker 的
    // await Call 實際到達後簽發，並與具名 async 函式 EnterCall 對帳。
    #[test]
    fn g18_v7_e5_直接await確實留下與真async框架相鄰的安全點() {
        let src = "async fn ready(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await ready(6)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let marker = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready"
        )).expect("真正到達 await 才產生安全點");
        assert!(matches!(out.events.get(marker+1),
            Some(PrivateScalarCallEvent::FunctionEntered(name)) if name == "ready"));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_偽造刪除async等待事件不得冒充同步進框架() {
        let src = "async fn ready()->int{\nreturn 7\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        let mut out = plan.execute().unwrap();
        out.events.retain(|e| !matches!(e, PrivateScalarCallEvent::AwaitSafePoint(_)));
        // 舊的純框架平衡表只看 enter/exit，須另用原 Checker
        // async 來源身分對帳防止等待事件整筆消失。
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v7_e5_等待點改綁別的函式或重播都必須拒絕() {
        let src = "async fn ready()->int{\nreturn 7\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        let mut forged = plan.execute().unwrap();
        let mark = forged.events.iter_mut().find_map(|e| match e {
            PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name),
            _ => None,
        }).unwrap();
        *mark = "another".to_string();
        assert!(!forged.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(forged),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let mut replayed = plan.execute().unwrap();
        let pos = replayed.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )).unwrap();
        replayed.events.insert(pos, PrivateScalarCallEvent::AwaitSafePoint(
            "ready".to_string(),
        ));
        assert!(!replayed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e5_原同步普通函式不得假造await安全點() {
        let src = "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(7)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_跨函式借用來源由原Checker拒絕且真票據守門同樣拒絕() {
        let src = "async fn ready()->int{\nreturn 4\n}\nfn wrapper(source:str)->int{\nreturn await ready()\n}\nfn main()->int{\nvar source:str = \"真資源🪼\"\nreturn wrapper(source)\n}\n";
        // Checker 在 wrapper 獨立檢查階段已看見未證明歸還的
        // str 參數借用，故不能假裝此來源可進入真執行器。
        match PreparedPrivateScalarCall::prepare(src) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 須比 runtime 更早拒絕跨等待借用"),
        }
        let fact = BorrowFact::new(DeclarationId(803), BorrowKind::SharedRead);
        let mut tracker = crate::borrow_safety::BorrowSafety::new();
        let ticket = tracker.acquire(fact.source, fact.kind).unwrap();
        assert_eq!(PreparedPrivateScalarCall::check_await_safe_point(&tracker, &[]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )));
        // 另一種真正來源：本框架 tracker 即使沒有借用，
        // 上一個動態框架傳入而尚未歸還的票據仍不可越過 await。
        let empty = crate::borrow_safety::BorrowSafety::new();
        assert_eq!(PreparedPrivateScalarCall::check_await_safe_point(
            &empty, &[(ticket, fact)],
        ), Err(PrivateScalarCallRuntimeError::Borrow(
            crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
        )));
        tracker.release(ticket).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point(
            &tracker, &[],
        ).is_ok());
    }

    #[test]
    fn g18_v7_e5_真正str_move直接await不冒充借用且資源唯一清理() {
        let src = "async fn consume(value:str)->int{\nreturn 4\n}\nfn main()->int{\nvar source:str = \"真移交🪼\"\nreturn await consume(source.move())\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "consume"
        )).count(), 1);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 1);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 1);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_尚未能進入async函式的框架預算錯誤不可虛構安全點() {
        let src = "async fn ready()->int{\nreturn 4\n}\nfn wrapper()->int{\nreturn await ready()\n}\nfn main()->int{\nreturn wrapper()\n}\n";
        let plan = checked(src);
        let out = plan.execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 500,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )));
    }


    // V7／E2-A：來源真的執行到 await 前，協作排程才可暫用
    // 同一 Rust 執行緒；父函式 P16 資源與真框架一直由原工作持有。
    #[test]
    fn g18_v7_e2_真私有await在內層函式中讓出且父框架資源仍存活() {
        let main = checked(concat!(
            "async fn target(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\nvar hold:str = \"原工作🪼\"\nreturn await target(6)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let other = checked(concat!(
            "async fn side()->int{ return 9 }\n",
            "fn main()->int{ return await side() }\n",
        ));
        let mut visited = Vec::<(String, Vec<crate::owned_str_runtime::ResourceEvent>)>::new();
        let mut scheduler = |name: &str, resources: &[crate::owned_str_runtime::ResourceEvent]| {
            visited.push((name.to_owned(), resources.to_vec()));
            let finished = other.execute().unwrap();
            assert_eq!(finished.result, Ok(9));
            assert!(finished.has_balanced_frame_trace());
        };
        let out = main.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].0, "target");
        assert!(matches!(visited[0].1.as_slice(),
            [crate::owned_str_runtime::ResourceEvent::Created(_)]));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "target"
        )).count(), 1);
        assert!(main.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_雙真正await只按動態左右順序發出協作切換() {
        let plan = checked(concat!(
            "async fn first(n:int)->int{ return n+1 }\n",
            "async fn second(n:int)->int{ return n+2 }\n",
            "fn main()->int{ return await first(2)+await second(3) }\n",
        ));
        let mut order = Vec::<String>::new();
        let mut scheduler = |name: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            order.push(name.to_owned());
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(order, ["first", "second"]);
        assert!(out.has_balanced_frame_trace());
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_未達等待點的框架預算耗盡絕不執行排程者() {
        let plan = checked(concat!(
            "async fn inner()->int{ return 3 }\n",
            "fn wrapper()->int{ return await inner() }\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut count = 0usize;
        let mut scheduler = |_: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            count += 1;
        };
        let out = plan.execute_with_budget_and_fault_scheduler(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 200 },
            false, Some(&mut scheduler),
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(count, 0);
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
    }

    #[test]
    fn g18_v7_e2_普通同步呼叫不得觸發等待排程() {
        let plan = checked(concat!(
            "fn plain(n:int)->int{ return n+1 }\n",
            "fn main()->int{ return plain(5) }\n",
        ));
        let mut count = 0usize;
        let mut scheduler = |_: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            count += 1;
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(count, 0);
        assert!(out.has_balanced_frame_trace());
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_真await讓出後原執行溢位仍逐框架清理且不虛構返回() {
        let plan = checked(concat!(
            "async fn overflow(n:int)->int{ return n+1 }\n",
            "fn main()->int{\nvar hold:str = \"留存🪼\"\n",
            "return await overflow(9223372036854775807)\n}\n",
        ));
        let mut seen = Vec::new();
        let mut scheduler = |name: &str, events: &[crate::owned_str_runtime::ResourceEvent]| {
            seen.push((name.to_owned(), events.to_vec()));
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "overflow");
        assert!(matches!(seen[0].1.as_slice(),
            [crate::owned_str_runtime::ResourceEvent::Created(_)]));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::UnwindStarted)));
        assert!(out.events.iter().all(|e|
            !matches!(e, PrivateScalarCallEvent::Returned(_))));
    }


    // V7／A1：原 Checker 已接受的直接 await，不在準備時執行；
    // 只在原 int 工作圖真正走到 await Call 才逐幀進入並退出。
    #[test]
    fn g18_v7_a1_原私有async直接await真int參數進框架返回() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(4)\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { awaited: true, .. })));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(4)
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "increase"
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(5)
        )));
        assert_eq!(out.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_v7_a1_真正零參數async直接await不能走舊同步快路() {
        let src = "async fn ready()->int{\nreturn 8\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { awaited: true, .. })));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )).count(), 1);
    }

    #[test]
    fn g18_v7_a1_兩個直接await按原運算式順序串行執行() {
        let src = "async fn left(n:int)->int{\nreturn n+1\n}\nasync fn right(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nreturn await left(2)+await right(3)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["left", "right"]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v7_a1_直接await內層溢位必受控回滾不偽造正常返回() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(9223372036854775807)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, returned: false }
            if name == "increase"
        )));
    }

    #[test]
    fn g18_v7_a1_竄改已編譯await身分即使來源相同仍拒絕執行() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(4)\n}\n";
        let mut plan = checked(src);
        let Some(PrivateScalarReturnExpr::Call { awaited, .. }) = &mut plan.main_dynamic_call
            else { panic!("必須是已核准的直接等待工作計畫") };
        assert!(*awaited);
        *awaited = false;
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F2／R07：不變更 Checker 型別決策，只要求同框架
    // int、bool、str 借用／Move 都使用唯一的原 DeclarationId。
    #[test]
    fn g18_v1_f2_混合int_bool參數合法且遞迴重入各有獨立身分() {
        let src = "fn alpha(n:int,flag:bool)->int{\nif(flag){return beta(n,false)}\nreturn n\n}\nfn beta(n:int,flag:bool)->int{\nreturn alpha(n,flag)\n}\nfn main()->int{\nreturn alpha(7,true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
                | PrivateScalarCallEvent::BoolParameterBound(..)
        )).count(), 6);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f2_整數參數重複綁定即使原結果正確也拒絕() {
        let src = "fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(9)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).expect("原 Checker 的唯一整數參數");
        let forged = out.events[index].clone();
        out.events.insert(index + 1, forged);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f2_布林參數重播與冒充int同身分一律拒絕() {
        let src = "fn pick(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n\n}\nfn main()->int{\nreturn pick(true,4)\n}\n";
        let baseline = checked(src).execute().unwrap();
        assert_eq!(baseline.result, Ok(5));
        assert!(baseline.has_balanced_frame_trace());
        let index = baseline.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::BoolParameterBound(..)
        )).expect("原 Checker bool 參數");
        let mut replay = checked(src).execute().unwrap();
        replay.events.insert(index + 1, replay.events[index].clone());
        assert!(!replay.has_balanced_frame_trace());
        let mut spoof = baseline;
        let id = match spoof.events[index] {
            PrivateScalarCallEvent::BoolParameterBound(id, _) => id,
            _ => unreachable!("已確認 bool 參數"),
        };
        spoof.events.insert(index + 1,
            PrivateScalarCallEvent::ParameterBound(id, 8));
        assert!(!spoof.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f2_借用str參數不可偽造已存在的int宣告身分() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"真借用🪼\"\nreturn inspect(owner,3)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let int_id = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::ParameterBound(id, _) => Some(*id),
            _ => None,
        }).expect("真整數參數");
        let borrow = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::BorrowParameterBound(id, _) => Some(id),
            _ => None,
        }).expect("真 str 借用參數");
        *borrow = int_id;
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f2_跨框架str移交不可重綁另一個int參數身分() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owner:str = \"真移交🪼\"\nreturn inspect(owner.move(),4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let int_id = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::ParameterBound(id, _) => Some(*id),
            _ => None,
        }).expect("真整數參數");
        for event in &mut out.events {
            if let PrivateScalarCallEvent::ResourceTransferred { target, .. } = event {
                *target = int_id;
            }
        }
        let transferred = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ResourceTransferred { .. }
        )).expect("真正跨框架移交");
        let owned = out.events.iter_mut().skip(transferred + 1).find_map(|event| match event {
            PrivateScalarCallEvent::ResourceOwnerBound(id, _) => Some(id),
            _ => None,
        }).expect("移交後真正子框架資源綁定");
        *owned = int_id;
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    // V1／F1：原 Checker 直接 Call、子呼叫及 main 的真正
    // int 返回值必須與最外層 Returned 事件相符，複合式不能誤判。
    #[test]
    fn g18_v1_f1_main直接呼叫的真返回值與最外層事件一致() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let prepared = checked(src);
        assert!(matches!(&prepared.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { .. })));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_偽造最外層Returned數值即使框架平衡也拒絕() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let prepared = checked(src);
        let mut out = prepared.execute().unwrap();
        let event = out.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("原 Checker 的直接返回事件");
        if let PrivateScalarCallEvent::Returned(value) = event {
            *value += 100;
        }
        // 原退出帳只核對有無返回，未曾交叉核對真正 main 結果。
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_多層返回只核對最外層不得誤用內層值() {
        let src = "fn leaf(n:int)->int{\nreturn n+1\n}\nfn wrap(n:int,flag:bool)->int{\nif(flag){return leaf(n)+2}\nreturn n\n}\nfn main()->int{\nreturn wrap(3,true)\n}\n";
        let prepared = checked(src);
        let baseline = prepared.execute().unwrap();
        assert_eq!(baseline.result, Ok(6));
        assert!(prepared.audit_graph_outcome(baseline).is_ok());
        let mut forged = prepared.execute().unwrap();
        let last = forged.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("外層 wrap 的真返回");
        if let PrivateScalarCallEvent::Returned(value) = last {
            *value += 1;
        }
        assert!(forged.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(forged),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_main兄弟呼叫複合算術不得誤把子返回當主返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)+choose(false,3)\n}\n";
        let prepared = checked(src);
        assert!(matches!(&prepared.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Addition(_, _))));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int直接呼叫沿同一原Checker結果真值稽核() {
        let prepared = checked(literal_src());
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int直接返回被改寫即使事件平衡也必拒絕() {
        let prepared = checked(literal_src());
        assert!(prepared.main_dynamic_call.is_none());
        let mut out = prepared.execute().unwrap();
        let event = out.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("原動態函式返回");
        if let PrivateScalarCallEvent::Returned(value) = event {
            *value = 777;
        }
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_舊式純int多層函式返回不得把子函式結果當主返回() {
        let src = "fn leaf(n:int)->int{\nreturn n+1\n}\nfn wrap(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn wrap(4)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [5, 7]);
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int溢位無返回且仍正常回滾() {
        let src = "fn forward(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn forward(9223372036854775807)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序() {
        let prepared = checked(literal_src());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            PrivateScalarCallEvent::ArgumentEvaluated(7),
            PrivateScalarCallEvent::FunctionEntered("forward".into()),
            PrivateScalarCallEvent::ParameterBound(prepared.parameter_id.expect("已驗證唯一 int 參數"), 7),
            PrivateScalarCallEvent::Returned(7),
            PrivateScalarCallEvent::FunctionExited {
                name: "forward".into(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nconst n:int = 8\nreturn identity(n)\n}\n";
        let prepared = checked(src);
        assert!(prepared.owner_id.is_some());
        assert_ne!(prepared.owner_id, prepared.parameter_id);
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nvar owner:int = 42\nreturn identity(owner)\n}\n";
        let prepared = checked(src);
        for _ in 0..3 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(42));
            assert_eq!(result.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_c_真正參數框架中途故障必清空且不冒充成功返回() {
        let prepared = checked(literal_src());
        let failure = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failure.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failure.parameter_slots_after_exit, 0);
        assert!(!failure.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failure.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "forward".into(), returned: false,
        }));
        let success = prepared.execute().unwrap();
        assert_eq!(success.result, Ok(7));
        assert_eq!(success.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_錯誤_copyable_int_實參型別及引數個數由原_checker_拒絕() {
        for src in [
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(true)\n}\n",
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(1,2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_)),), "{src}");
        }
    }

    #[test]
    fn g18_a_具名未初始化_int_無法假裝有函式實參值() {
        let src = "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int\nreturn forward(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_a_竄改已驗證參數_declarationid_和返回值必須零執行拒絕() {
        let mut prepared = checked(literal_src());
        prepared.parameter_id = Some(DeclarationId(0));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.argument = Some(999);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_竄改主函式呼叫跨度或原來源位元組不得建立任何參數框架() {
        let mut prepared = checked(literal_src());
        prepared.call_span = NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // G18 P18.1 第二批：函式內的算術不是靜態算好的常值，
    // 每個運算元都須從當次合法 Copyable int 參數槽取值。
    #[test]
    fn g18_a_真函式_copyable_int_參數參與加法且回傳動態值() {
        let src = "fn step(value:int)->int{\nreturn value+3\n}\nfn main()->int{\nreturn step(9)\n}\n";
        let plan = checked(src);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(result.events.contains(&PrivateScalarCallEvent::Returned(12)));
    }

    #[test]
    fn g18_a_真函式_copyable_int_參數多次引用每次都取同一實際值() {
        let src = "fn twice(value:int)->int{\nreturn value+value\n}\nfn main()->int{\nreturn twice(11)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(22));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算() {
        let src = "fn mix(value:int)->int{\nreturn (value+2)-(value-1)\n}\nfn main()->int{\nreturn mix(10)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(3));
    }

    #[test]
    fn g18_a_具名_copyable_int_實參傳入函式後參與真減法() {
        let src = "fn minus(value:int)->int{\nreturn value-3\n}\nfn main()->int{\nconst owner:int = 10\nreturn minus(owner)\n}\n";
        let plan = checked(src);
        assert!(plan.owner_id.is_some());
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_函式參數真加法溢位不能假造_return_且確實離域() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(!result.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(result.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "inc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_函式參數真減法溢位亦不可留下框架或偽造成功事件() {
        let src = "fn minus(value:int)->int{\nreturn value-9223372036854775807-2\n}\nfn main()->int{\nreturn minus(0)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }

    #[test]
    fn g18_a_竄改編譯後_true_arithmetic_運算樹必在執行前拒絕() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(2)\n}\n";
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Subtraction(
            Box::new(PrivateScalarReturnExpr::Parameter(prepared.parameter_id.expect("已驗證 int 參數"))),
            Box::new(PrivateScalarReturnExpr::Constant(1)),
        );
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_非法_bool_參與_int_加法與字面值不足均由原_checker_拒絕() {
        let bad_type = "fn wrong(value:int)->int{\nreturn value+true\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let bad_name = "fn wrong(value:int)->int{\nreturn value+missing\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_name),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }
    // P18.1／18.4：不是新的模型，而是原單參數真正 executor 的
    // 多 Parameter DeclarationId、TypedHIR、依序求值與退出擴充。
    fn multiple_source() -> &'static str {
        "fn combine(a:int,b:int)->int{\nreturn (a+b)-a\n}\nfn main()->int{\nreturn combine(7,9)\n}\n"
    }

    #[test]
    fn g18_a_雙_copyable_int_參數依原_checker_宣告順序逐一綁定() {
        let prepared = checked(multiple_source());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events[0], PrivateScalarCallEvent::ArgumentEvaluated(7));
        assert_eq!(outcome.events[1], PrivateScalarCallEvent::ArgumentEvaluated(9));
        assert!(matches!(outcome.events[2], PrivateScalarCallEvent::FunctionEntered(_)));
        assert_eq!(outcome.events[3], PrivateScalarCallEvent::ParameterBound(
            prepared.parameter_id.expect("已驗證 int 參數"), 7,
        ));
        assert_eq!(outcome.events[4], PrivateScalarCallEvent::ParameterBound(
            prepared.additional_arguments[0].0, 9,
        ));
        assert_ne!(prepared.parameter_id.expect("已驗證 int 參數"), prepared.additional_arguments[0].0);
    }

    #[test]
    fn g18_a_三個_int_參數在同一真框架內逐項參與原_hir_算式() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn (a-b)+c\n}\nfn main()->int{\nreturn blend(8,3,5)\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 3);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_同一個_const_int_持有者作兩個實參屬_copyable_不消耗() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst original:int = 21\nreturn sum(original,original)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.owner_id, prepared.additional_arguments[0].2);
        for _ in 0..3 {
            let outcome = prepared.execute().unwrap();
            assert_eq!(outcome.result, Ok(42));
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_多個具名_var_const_int_和常值實參按照原宣告身分傳遞() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn a+b+c\n}\nfn main()->int{\nconst first:int = 4\nvar second:int = 3\nreturn blend(first,1,second)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        assert_eq!(prepared.additional_arguments[1].2, Some(prepared.main_owners[1].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_多參數整數溢位不可回傳成功且所有動態參數清空() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(9223372036854775807,1)\n}\n";
        let prepared = checked(text);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_多參數真框架測試故障不殘留參數也不假冒_return() {
        let prepared = checked(multiple_source());
        let failed = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_a_擅改第二實參值或第二_parameter_declarationid_先驗後拒絕() {
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].1 = 100;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].0 = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.main_owners.push((DeclarationId(999), 1));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_多參數錯誤型別引數數量拒絕且未使用純值宣告合法() {
        for text in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,true)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(text),
                Err(PrivateScalarCallPrepareError::Check(_))), "{text}");
        }
        let unused = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar extra:int = 5\nreturn sum(1,2)\n}\n";
        let prepared = checked(unused);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
    }

    // P18.1／P18.3／P18.4：同一原 Checker／Parameter HIR 的真正多層具參數
    // 普通函式圖；每個呼叫邊皆有真正的進／出事件與獨立 int slot。
    fn graph_source() -> &'static str {
        "fn leaf(c:int)->int{\nreturn c+1\n}\nfn middle(a:int,b:int)->int{\nreturn leaf(a+b)\n}\nfn main()->int{\nreturn middle(3,4)\n}\n"
    }

    #[test]
    fn g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出() {
        let prepared = checked(graph_source());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 2);
        assert!(matches!(&result.events[0], PrivateScalarCallEvent::ArgumentEvaluated(3)));
        assert!(matches!(&result.events[1], PrivateScalarCallEvent::ArgumentEvaluated(4)));
        let trace = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(format!("+{name}")),
            PrivateScalarCallEvent::FunctionExited { name, returned } if *returned =>
                Some(format!("-{name}")),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(trace, ["+middle", "+leaf", "-leaf", "-middle"]);
    }

    #[test]
    fn g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值() {
        let src = "fn last(x:int)->int{\nreturn x-2\n}\nfn next(y:int)->int{\nreturn last(y+3)\n}\nfn first(z:int)->int{\nreturn next(z+4)\n}\nfn main()->int{\nreturn first(8)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "next", "last"]);
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["last", "next", "first"]);
    }

    #[test]
    fn g18_a_同一巢狀葉函式重複真呼叫各自返回且不共用參數槽() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn twice(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn twice(2,5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let values = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [3, 6, 9]);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )).count(), 2);
    }

    #[test]
    fn g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "outer"]);
    }

    #[test]
    fn g18_c_父框架故障時不可進入尚未求值的巢狀子函式() {
        let failed = checked(graph_source()).execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )));
        assert!(!failed.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failed.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "middle".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_巢狀函式來源或參數宣告身份被篡改零動態框架拒絕() {
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].parameters[0] = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].result = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.verified.plan.declarations.by_source.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出() {
        let src = "fn alpha(x:int)->int{\nreturn beta(x)\n}\nfn beta(y:int)->int{\nreturn alpha(y)\n}\nfn main()->int{\nreturn alpha(1)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["alpha", "beta", "alpha", "beta", "alpha"]);
        let exited = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exited, ["alpha", "beta", "alpha", "beta", "alpha"]);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕() {
        let bad_type = graph_source().replace("leaf(a+b)", "leaf(true)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let unknown = graph_source().replace("leaf(a+b)", "missing(a+b)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&unknown),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 8, max_steps: 500 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 8);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count(), 8);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_合法前向六層以上_copyable_int_函式真逐層返回() {
        let mut src = String::new();
        for i in 0..9 {
            let target = if i == 8 {
                "n+1".to_owned()
            } else {
                format!("f{}(n)", i+1)
            };
            src.push_str(&format!("fn f{i}(n:int)->int{{\nreturn {target}\n}}\n"));
        }
        src.push_str("fn main()->int{\nreturn f0(12)\n}\n");
        let outcome = checked(&src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 9);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 9);
    }

    #[test]
    fn g18_a_五個_copyable_int_真參數不應被內部三參數上界拒絕() {
        let src = "fn sum(a:int,b:int,c:int,d:int,e:int)->int{\nreturn a+b+c+d+e\n}\nfn main()->int{\nreturn sum(1,2,3,4,5)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(15));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ParameterBound(..))).count(), 5);
    }

    #[test]
    fn g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(9)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 200, max_steps: 8 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        let entered = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count();
        assert!(entered >= 2);
        assert_eq!(entered, exited);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_零框架或零步數預算不得偽造任何真函式進入() {
        let checked = checked(graph_source());
        for budget in [
            PrivateScalarExecutionBudget { max_call_frames: 0, max_steps: 100 },
            PrivateScalarExecutionBudget { max_call_frames: 100, max_steps: 0 },
        ] {
            let outcome = checked.execute_with_budget(budget).unwrap();
            assert!(outcome.result.is_err());
            assert!(outcome.events.is_empty());
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕() {
        let src = "fn unused(x:int)->int{\nreturn unused(x)\n}\nfn good(y:int)->int{\nreturn y+2\n}\nfn main()->int{\nreturn good(7)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 30 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(name) if name == "unused")).count(), 0);
    }

    #[test]
    fn g18_c_有界步數的兩個巢狀兄弟呼叫僅在實際出現時建立獨立框架() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn both(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn both(1,2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "leaf", "both"]);
    }


    #[test]
    fn g18_a_零參數前向葉函式與_copyable_int_父框架真返回() {
        let src = "fn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\nfn fixed()->int{\nreturn 7\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["plus", "fixed"]);
        let returned = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [7, 12]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_零參數函式重複呼叫不重用動態框架且保留真求值順序() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn add(n:int)->int{\nreturn fixed()+n+fixed()\n}\nfn main()->int{\nreturn add(5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["add", "fixed", "fixed"]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "fixed")).count(), 2);
    }

    #[test]
    fn g18_c_零參數自遞迴由真框架預算受控中止不虛構返回() {
        let src = "fn loop()->int{\nreturn loop()\n}\nfn entry(n:int)->int{\nreturn n+loop()\n}\nfn main()->int{\nreturn entry(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                returned: false, ..
            })).count(), 4);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_零參數子函式原宣告來源遭竄改必在動態執行前拒絕() {
        let src = "fn fixed()->int{\nreturn 7\n}\nfn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\n";
        let mut prepared = checked(src);
        let zero = prepared.nested_callees.iter_mut()
            .find(|f| f.name == "fixed").expect("原 Checker 的零參數函式");
        assert!(zero.parameters.is_empty());
        zero.parameters.push(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_巢狀兄弟呼叫每個實參求值完成立即紀錄且不重複() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a-b\n}\nfn outer(n:int)->int{\nreturn pair(leaf(n),leaf(n+2))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(-2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        // main 的 2、leaf(2) 的 2、pair 第一實參 3、
        // leaf(4) 的 4、pair 第二實參 5，必為原 Parser 的左至右順序。
        assert_eq!(arguments, [2, 2, 3, 4, 5]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf")).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")).count(), 1);
    }

    #[test]
    fn g18_c_第一實參完成後第二實參溢位仍保留真事件而不進入父呼叫() {
        let src = "fn first(x:int)->int{\nreturn x+1\n}\nfn bad(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a+b\n}\nfn outer(n:int)->int{\nreturn pair(first(n),bad(9223372036854775807))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 2, 3, 9223372036854775807]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 3);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited { .. })).count(), 3);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "outer")));
    }



    #[test]
    fn g18_a_main_零參數呼叫真正建立空參數框架並返回() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events, vec![
            PrivateScalarCallEvent::FunctionEntered("value".to_owned()),
            PrivateScalarCallEvent::Returned(9),
            PrivateScalarCallEvent::FunctionExited {
                name: "value".to_owned(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_main_零參數前向函式串接巢狀真返回() {
        let src = "fn first()->int{\nreturn second()\n}\nfn main()->int{\nreturn first()\n}\nfn second()->int{\nreturn 11\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "second"]);
        let returns = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [11, 11]);
    }

    #[test]
    fn g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出() {
        let src = "fn repeat()->int{\nreturn repeat()\n}\nfn main()->int{\nreturn repeat()\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 4);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_a_main_零參數來源被偽造一個參數必執行前拒絕() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let mut prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        prepared.parameter_id = Some(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.argument = Some(123);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_main_原_checker_括號常值引數真正進入雙參數框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum((5),(7))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 7]);
    }

    #[test]
    fn g18_a_main_多層括號具名_copyable_來源仍維持原宣告身分() {
        let src = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst original:int = 11\nreturn identity((((original))))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號布林與未知名稱不可充當合法_int_實參() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id((true))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id(((missing)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_數個未使用純值宣告與具名實參各自保留來源身分() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nvar input:int = 4\nvar another:int = 9\nreturn id(input)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert!(prepared.owner_id.is_some());
        for _ in 0..2 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(4));
            assert_eq!(result.parameter_slots_after_exit, 0);
            assert_eq!(result.events.iter().filter(|event| matches!(
                event, PrivateScalarCallEvent::ArgumentEvaluated(_)
            )).count(), 1);
        }
    }

    #[test]
    fn g18_a_未使用純值宣告遭竄改仍須執行前拒絕() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nreturn id(3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[0].1 = 999;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let changed = src.replace("unused:int = 8", "unused:int = 9");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&changed),
            Ok(_)));
        assert_ne!(prepared.original.as_ref(), changed.as_bytes());
    }



    #[test]
    fn g18_a_main_括號返回帶參數普通函式依原_checker_真求值() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn (((inc(7))))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(arguments, [7]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_多層括號零參數前向函式返回不虛構參數() {
        let src = "fn main()->int{\nreturn (((value())))\n}\nfn value()->int{\nreturn 23\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(23));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..))).count(), 0);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號返回的真內層呼叫跨度不得竄改() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(6)))\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.call_span = crate::common_ir::NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_括號返回錯型別與不存在函式由原_checker_拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(true)))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((missing(6)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_main_具名_copyable_int_多層括號初始化仍真傳入函式() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nconst base:int = ((((6))))\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 6);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_未使用的合法_group_int_初始化不應阻擋零參數函式() {
        let src = "fn fixed()->int{\nreturn 17\n}\nfn main()->int{\nvar unused:int = ((8))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 8);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(17));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_括號初始化為布林或缺失名稱仍由原_checker_拒絕() {
        for src in [
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((true))\nreturn id(owner)\n}\n",
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((missing))\nreturn id(owner)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_括號初始化持有者遭竄改不得建立動態框架() {
        let src = "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nvar owner:int = (((9)))\nreturn id(owner)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
        prepared.main_owners[0].1 = 90;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_已核准_copyable_int_名稱初始化串接真傳參() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst origin:int = 6\nvar copied:int = origin\nconst further:int = copied\nreturn inc(further)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert_eq!(prepared.main_owners.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [6, 6, 6]);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(6)
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_括號名稱初始化與未使用副本不消耗來源() {
        let src = "fn fixed()->int{\nreturn 13\n}\nfn main()->int{\nconst origin:int = ((9))\nvar unused:int = ((((origin))))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.main_owners[0].1, 9);
        assert_eq!(prepared.main_owners[1].1, 9);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_初始化來源錯型別或未宣告及尚未初始化仍靜態拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst ok:bool = true\nconst copy:int = ok\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst copy:int = missing\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar original:int\nconst copy:int = original\nreturn id(copy)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_被複製初始化持有者來源身分竄改必先拒絕() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst origin:int = 3\nconst copy:int = origin\nreturn id(copy)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[1].0 = DeclarationId(usize::MAX);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.main_owners[1].1 = 31;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_原_checker_算術實參於真動態框架依順序求值() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,7-4)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(6));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count(), 1);
    }

    #[test]
    fn g18_a_main_具名_copyable_算術與巢狀普通函式依真來源求值() {
        let src = "fn inc(v:int)->int{\nreturn v+1\n}\nfn combine(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{\nconst origin:int = 5\nreturn combine(inc(origin),inc(2+1))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["inc", "inc", "combine"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 6, 3, 4]);
    }

    #[test]
    fn g18_a_main_零參數葉函式作為複合實參須先真返回再綁定目標() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(fixed(),6)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["fixed", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [4, 6]);
    }

    #[test]
    fn g18_c_main_第二算術實參溢位保留前一完成事件且不可進入目標() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn sum(5+1,high+1)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [6]);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
                | PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_c_main_算術實參步數預算耗盡不會補寫未完成參數事件() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 6 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))));
    }

    #[test]
    fn g18_c_main_動態實參綁定後故障須先退出真根框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 2);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_a_main_動態算式已驗證工作計畫遭竄改必在執行前拒絕() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(99));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_動態實參非法型別或未知名稱仍須由原_checker_拒絕() {
        for src in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(true+2,3)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(2+missing,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_c_main_同名巢狀實參故障只能中斷真正外層函式框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(sum(1,2),3)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count();
        assert_eq!(enters, 2);
        let successes = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count();
        let failures = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "sum")).count();
        assert_eq!((successes, failures), (1, 1));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(3))).count(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
    }

    #[test]
    fn g18_c_main_不同名子函式先成功後故障只注入根函式綁定() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(4),2)\n}\n";
        let outcome = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned } =>
                Some((name.as_str(), *returned)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, [("inc", true), ("sum", false)]);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_動態實參的來源框架不占用唯一使用者函式預算() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_巢狀實參先返回後可合法復用唯一框架額度() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(1),3)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inc", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [1, 2, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "inc")).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count(), 1);
    }


    #[test]
    fn g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(1+2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer"]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "outer".into(), returned: false,
        }));
    }


    #[test]
    fn g18_a_main_普通函式返回值與常值相加由同一真動態工作堆疊完成() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(5)+2\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_兩個兄弟函式合成返回必照原_checker_左右順序執行() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let args = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 4]);
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [3, 5]);
    }

    #[test]
    fn g18_a_main_多層括號包含函式複合返回保持原_parser_int_來源() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn ((inc(3)+inc(4)))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
    }

    #[test]
    fn g18_c_main_子函式已真返回後最外層算術溢位不得冒充成功() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn id(high)+1\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(9223372036854775807)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "id"
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_複合返回工作計畫被竄改必在任何函式執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+3\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_複合函式返回與非法型別或未知名稱仍由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+true\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+missing\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_c_main_多兄弟複合返回不得假造單一根函式故障注入() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(8));
        assert!(matches!(prepared.execute_with_fault(true),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    #[test]
    fn g18_a_函式參數與多個純值區域初始化依_checker_順序真返回() {
        let src = "fn calc(n:int)->int{\nconst one:int = 1\nvar next:int = n+one\nreturn next+2\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 2);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_巢狀函式各自區域宣告不與父框架參數混用() {
        let src = "fn outer(n:int)->int{\nconst result:int = leaf(n+1)\nreturn result+1\n}\nfn leaf(x:int)->int{\nconst y:int = x+2\nreturn y\n}\nfn main()->int{\nreturn outer(3)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 1);
        assert_eq!(prepared.nested_callees.iter().find(|f| f.name == "leaf").unwrap().local_inits.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let names = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(names, ["outer", "leaf"]);
    }

    #[test]
    fn g18_a_真動態_main_實參進入帶區域宣告函式仍唯一返回() {
        let src = "fn calc(n:int)->int{\nconst extra:int = n+2\nreturn extra\n}\nfn main()->int{\nreturn calc(2+3)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_c_函式區域算術溢位必退出而不得虛構返回() {
        let src = "fn fail(n:int)->int{\nconst too_big:int = n+1\nreturn too_big\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "fail".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_局部整數宣告跨多個遞迴框架不得共用儲存位置() {
        let src = "fn repeat(n:int)->int{\nconst next:int = n+1\nreturn repeat(next)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {returned: false, ..})).count();
        assert_eq!((entered, exited), (4, 4));
    }

    #[test]
    fn g18_a_函式區域宣告工作計畫遭偽造必在執行前拒絕() {
        let src = "fn calc(n:int)->int{\nconst next:int = n+1\nreturn next\n}\nfn main()->int{\nreturn calc(2)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.root_local_inits[0].value = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_函式區域初始化錯誤型別與未知參照由原_checker_拒絕() {
        for src in [
            "fn calc(n:int)->int{\nconst x:int = true\nreturn n\n}\nfn main()->int{\nreturn calc(2)\n}\n",
            "fn calc(n:int)->int{\nconst x:int = missing\nreturn x\n}\nfn main()->int{\nreturn calc(2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_兩筆真區域初始化與逆序離域事件保持同源宣告身分() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nvar second:int = first+2\nreturn second\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let prepared = checked(src);
        let ids = prepared.root_local_inits.iter()
            .map(|init| init.declaration).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        let created = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        let ended = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(created, [(ids[0], 4), (ids[1], 6)]);
        assert_eq!(ended, [ids[1], ids[0]]);
        assert_eq!(out.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_第二局部初始化運算溢位只釋放已完成宣告() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nconst second:int = n+9223372036854775807\nreturn second\n}\nfn main()->int{\nreturn calc(1)\n}\n";
        let prepared = checked(src);
        let first_id = prepared.root_local_inits[0].declaration;
        let second_id = prepared.root_local_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first_id, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first_id)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second_id
        )));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "calc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域() {
        let src = "fn again(n:int)->int{\nconst local:int = n+1\nreturn again(local)\n}\nfn main()->int{\nreturn again(1)\n}\n";
        let prepared = checked(src);
        let id = prepared.root_local_inits[0].declaration;
        let out = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(actual, value) if *actual == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [2, 3, 4]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(actual) if *actual == id)
        ).count(), 3);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }


    #[test]
    fn g18_a_main_動態整數初始化應真執行後再傳入普通函式() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        assert!(prepared.main_owners.is_empty());
        let id = prepared.main_dynamic_inits[0].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_函式結果可先存入區域再用純值算術返回() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst result:int = inc(4)\nreturn result+1\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        let out = prepared.execute().unwrap();
        // inc(4) = 5，main 再加 1，真正結果為 6。
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let local = prepared.main_dynamic_inits[0].declaration;
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(local, 5)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))).count(), 1);
    }

    #[test]
    fn g18_a_main_前向零參數結果可作為初始化再跨局部複製() {
        let src = "fn main()->int{\nconst first:int = fixed()\nvar copy:int = first\nreturn inc(copy)\n}\nfn inc(n:int)->int{\nreturn n+1\n}\nfn fixed()->int{\nreturn 8\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(9));
        let inits = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(inits.len(), 2);
        assert_eq!(inits.iter().map(|(_, value)| *value).collect::<Vec<_>>(), [8, 8]);
    }

    #[test]
    fn g18_c_main_動態初始化中後筆溢位不得跳過前筆真離域() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst one:int = inc(1)\nconst too_big:int = 9223372036854775807+one\nreturn inc(one)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let first = prepared.main_dynamic_inits[0].declaration;
        let second = prepared.main_dynamic_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second
        )));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_真動態初始化計畫被篡改必在執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_inits[0].value = PrivateScalarReturnExpr::Constant(500);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_非法動態初始化型別及未宣告來源仍交由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = true+3\nreturn inc(base)\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = missing+3\nreturn inc(base)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_r01_r02_函式內多筆可寫純值賦值必依原_checker_語句次序() {
        let source = "fn calculate(n:int)->int{\nvar current:int = n+1\ncurrent = current+2\ncurrent = current+3\nreturn current\n}\nfn main()->int{\nreturn calculate(5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.root_local_inits.len(), 1);
        assert_eq!(checked.root_local_steps.len(), 3);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert_eq!(result.parameter_slots_after_exit, 0);
        let changes = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(key, value) if *key == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [8, 11]);
        assert!(result.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 6)));
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(key) if *key == id)).count(), 1);
    }

    #[test]
    fn g18_r01_r02_賦值右值真呼叫先完成後才寫入持有者() {
        let source = "fn bump(n:int)->int{\nreturn n+1\n}\nfn work(n:int)->int{\nvar value:int = n\nvalue = bump(value)\nreturn value+2\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(6));
        let returned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).unwrap();
        let assigned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(key, 4) if *key == id)).unwrap();
        assert!(returned < assigned);
    }

    #[test]
    fn g18_r01_r02_main_具名_copyable_int_多次更新後才進入普通函式() {
        let source = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar value:int = 2\nvalue = value+3\nreturn identity(value)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let id = checked.main_dynamic_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalAssigned(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_真巢狀函式可寫宣告不能跨動態框架共用() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\nreturn leaf(current)\n}\nfn leaf(x:int)->int{\nvar tmp:int = x+1\ntmp = tmp+1\nreturn tmp\n}\nfn main()->int{\nreturn work(1)\n}\n";
        let checked = checked(source);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        let writes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(id, v) => Some((*id, *v)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert_ne!(writes[0].0, writes[1].0);
        assert_eq!((writes[0].1, writes[1].1), (2, 4));
    }

    #[test]
    fn g18_r01_r02_賦值右值溢位不得補記後續賦值或假返回() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\ncurrent = current+9223372036854775807\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(actual, 4) if *actual == id)).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(..))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_const_重賦值與錯型別必由真_checker_先拒絕() {
        for source in [
            "fn work(n:int)->int{\nconst current:int = n\ncurrent = current+1\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nvar current:int = n\ncurrent = true\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nmissing = n+1\nreturn n\n}\nfn main()->int{\nreturn work(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r01_r02_已編譯賦值的_checker_身分遭竄改在執行前拒絕() {
        let source = "fn work(n:int)->int{\nvar value:int = n\nvalue = value+2\nreturn value\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::Assign { value, .. } = &mut checked.root_local_steps[1] {
            *value = PrivateScalarReturnExpr::Constant(99);
        } else {
            panic!("真 Checker 有第二筆可寫賦值");
        }
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_具名_bool_true_提前_return_不可再執行未達返回路徑() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        assert_eq!(plan.root_local_steps.len(), 2);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
    }

    #[test]
    fn g18_r03_具名_bool_false_不走提前_return_而正常執行後段() {
        let source = "fn choose(n:int)->int{\nconst flag:bool = false\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))));
    }

    #[test]
    fn g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = false\nflag = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_, true))));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
    }

    #[test]
    fn g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回() {
        let source = "fn choose(n:int)->int{\nvar first:bool = false\nvar second:bool = true\nif(first){\nreturn n+1\n}else if(second){\nreturn n+2\n}else{\nreturn n+3\n}\nreturn n+4\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        let checks = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6 | 7))));
    }

    #[test]
    fn g18_r03_巢狀子函式提前返回只退出該函式不跳過父框架返回() {
        let source = "fn leaf(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn outer(n:int)->int{\nconst result:int = leaf(n)\nreturn result+3\n}\nfn main()->int{\nreturn outer(4)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5 | 8))).count(), 2);
    }

    #[test]
    fn g18_r03_提前返回分支溢位只退出真的已初始化作用域() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nvar temp:int = n+1\nif(flag){\nreturn temp+9223372036854775807\n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolScopeEnded(_))));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(_))));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r03_未宣告_bool_及_const_bool_重綁由_checker_靜態拒絕() {
        for source in [
            "fn choose(n:int)->int{\nconst flag:bool = false\nflag = true\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(missing){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(n){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r03_篡改已編譯分支條件前須拒絕任何真框架() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        if let PrivateScalarLocalStep::EarlyReturn(branch) = &mut plan.root_local_steps[1] {
            branch.condition = PrivateScalarBoolExpr::Constant(false);
        } else {
            panic!("原 Checker 提供合法 if Return 計畫");
        }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r04_已核准_bool_true_函式實參與_int_返回共用真框架() {
        let source = "fn choose(flag:bool)->int{\nif(flag){return 4}\nreturn 7\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
        assert!(out.events.contains(&PrivateScalarCallEvent::BoolParameterBound(
            checked.callee_parameter_ids[0], true,
        )));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))));
    }

    #[test]
    fn g18_r04_已核准_bool_false_int_混合參數保持宣告順序和型別() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn main()->int{\nreturn choose(false,8)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true, false]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let order = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(false) => Some("bool"),
            PrivateScalarCallEvent::ArgumentEvaluated(8) => Some("int"),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(order, ["bool", "int"]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(..))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_r04_巢狀普通函式真_bool_參數不被誤讀成父整數槽() {
        let source = "fn inner(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn outer(flag:bool,n:int)->int{\nreturn inner(flag,n+2)\n}\nfn main()->int{\nreturn outer(true,4)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 2);
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer", "inner"]);
    }

    #[test]
    fn g18_r04_main_變動具名_bool_正值先更新再真正當實參() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+3}\nreturn n+4\n}\nfn main()->int{\nvar switched:bool = false\nswitched = true\nreturn choose(switched,5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_,true))));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
    }

    #[test]
    fn g18_r04_bool_混合參數進入後故障只退出本次真框架() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let checked = checked(source);
        let out = checked.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(_,5))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r04_bool_參數不是_int_隱含轉型_原_checker_型別優先拒絕() {
        for source in [
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(1,2)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true,false)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r04_bool_參數綁定靜態型別被竄改前必拒絕() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(6));
        checked.callee_parameter_is_bool[0] = false;
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r05_混合布林整數與巢狀提前返回動態事件帳簿必平衡() {
        let src = "fn leaf(n:int)->int{\nvar flag:bool = true\nvar x:int = n+1\nif(flag){return x}\nreturn n\n}\nfn parent(n:int,flag:bool)->int{\nvar other:int = leaf(n)\nif(flag){return other+2}\nreturn other+3\n}\nfn main()->int{\nreturn parent(3,true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionExited {returned: true,..}
        )).count(), 2);
    }

    #[test]
    fn g18_r05_動態整數溢位與真失敗退出仍保留合法平衡事件() {
        let src = "fn fail(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r05_遞迴重入相同靜態宣告身分不應誤判為重複清理() {
        let src = "fn repeat(n:int)->int{\nvar x:int = n\nx = x+1\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let out = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_偽造未建立區域的離域與錯順序均遭事件帳簿拒絕() {
        let x = DeclarationId(99);
        let y = DeclarationId(100);
        for events in [
            vec![PrivateScalarCallEvent::LocalScopeEnded(x)],
            vec![PrivateScalarCallEvent::LocalInitialized(x, 1)],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(y, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
                PrivateScalarCallEvent::LocalScopeEnded(y),
            ],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(x, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
            ],
            vec![PrivateScalarCallEvent::LocalAssigned(x, 5)],
        ] {
            let outcome = PrivateScalarCallOutcome {
                result: Ok(1), events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            };
            assert!(!outcome.has_balanced_frame_trace());
            assert!(matches!(PreparedPrivateScalarCall::audit_outcome(outcome),
                Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        }
    }

    #[test]
    fn g18_r05_虛構函式返回和錯誤退出標記不得通過一致性稽核() {
        let cases = [
            vec![PrivateScalarCallEvent::Returned(1)],
            vec![PrivateScalarCallEvent::FunctionExited {
                name: "leaf".into(), returned: false,
            }],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::FunctionExited {
                    name: "other".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: true,
                },
            ],
        ];
        for events in cases {
            assert!(!PrivateScalarCallOutcome {
                result: Ok(1), events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }

    #[test]
    fn g18_r05_布林與整數反向離域跨型別錯序不可冒充真清理() {
        let x = DeclarationId(100);
        let b = DeclarationId(200);
        let valid = vec![
            PrivateScalarCallEvent::FunctionEntered("fn".into()),
            PrivateScalarCallEvent::LocalInitialized(x, 3),
            PrivateScalarCallEvent::BoolInitialized(b, true),
            PrivateScalarCallEvent::Returned(3),
            PrivateScalarCallEvent::BoolScopeEnded(b),
            PrivateScalarCallEvent::LocalScopeEnded(x),
            PrivateScalarCallEvent::FunctionExited {
                name: "fn".into(), returned: true,
            },
        ];
        assert!(PrivateScalarCallOutcome {
            result: Ok(3), events: valid.clone(), resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut invalid = valid.clone();
        invalid.swap(4, 5);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: invalid, resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut missing = valid;
        missing.remove(4);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: missing, resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_純值事件帳簿不得將錯誤參數綁定或布林寫入當作合法框架() {
        for events in [
            vec![PrivateScalarCallEvent::BoolParameterBound(DeclarationId(1), true)],
            vec![PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), false)],
            vec![
                PrivateScalarCallEvent::FunctionEntered("f".into()),
                PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), true),
                PrivateScalarCallEvent::FunctionExited {name:"f".into(),returned:false},
            ],
        ] {
            assert!(!PrivateScalarCallOutcome {
                result: Err(PrivateScalarCallRuntimeError::StateMismatch),
                events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }


    #[test]
    fn g18_r03_if_已核准_int_可寫分支後仍執行真正尾端返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(6))).count(), 1);
    }

    #[test]
    fn g18_r03_if_false_未選中分支不得寫入而仍由原值返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\n}else if(false){\nvalue = value+100\n}else{\nvalue = value+2\n}\nreturn id(value)\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|ev| match ev {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::FunctionEntered(name) if name == "id")).count(), 1);
    }

    #[test]
    fn g18_r03_if_變更_bool_使後續獨立_if_重新讀取真動態值() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nif(true){\nflag = true\n}\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::BoolAssigned(_, true))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 2);
    }

    #[test]
    fn g18_r03_if_分支呼叫溢位不允許後續真賦值或假返回() {
        let src = "fn bump(n:int)->int{\nreturn n+1\n}\nfn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = bump(value)\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..)
                | PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r03_if_分支不能更改_const_或偷換型別_原_checker_拒絕() {
        for src in [
            "fn choose(n:int)->int{\nconst value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = false}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r03_if_分支原_checker_賦值工作計畫篡改於執行前拒絕() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::EarlyReturn(if_plan) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::AssignInt { value, .. } = &mut if_plan.when_true {
                *value = PrivateScalarReturnExpr::Constant(99);
            } else { panic!("真 Parser int 更新分支遺失"); }
        } else { panic!("真 Parser if 分支遺失"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_同一真_if_分支兩次_var_int_更新須照原_parser_先後返回() {
        for (flag, expected, updates) in [("true", 9, vec![5, 8]), ("false", 4, vec![])] {
            let src = format!(
                "fn choose(flag:bool,n:int)->int{{\nvar value:int = n\nif(flag){{\nvalue = value+2\nvalue = value+3\n}}\nreturn value+1\n}}\nfn main()->int{{\nreturn choose({},3)\n}}\n",
                flag,
            );
            let out = checked(&src).execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert!(out.has_balanced_frame_trace());
            let values = out.events.iter().filter_map(|event| match event {
                PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(values, updates);
        }
    }

    #[test]
    fn g18_r03_else_if_選中分支多筆更新不可執行其他兄弟分支() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else if(true){\nvalue = value+1\nvalue = value+2\n}else{\nvalue = value+50\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [4, 6]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
    }

    #[test]
    fn g18_r03_分支先更新_int_bool_後下個_if_必讀動態最新值() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nvar flag:bool = false\nif(true){\nvalue = value+2\nflag = true\n}\nif(flag){\nreturn value+1\n}\nreturn value+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_分支內先變更再提前_return_不得執行不可達尾端() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\nreturn value+2\n}\nreturn value+10\n}\nfn main()->int{\nreturn choose(true,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(7)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(15)
        )));
    }

    #[test]
    fn g18_r03_同分支第二更新溢位只保存第一個真更新並失敗退出() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_分支內常值錯誤賦值與真工作計畫竄改均須零額外執行() {
        let invalid = "fn choose(n:int)->int{\nconst x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let source = "fn choose(n:int)->int{\nvar x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
        if let PrivateScalarLocalStep::EarlyReturn(conditional) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::Sequence(actions) = &mut conditional.when_true {
                actions[1] = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(999),
                );
            } else { panic!("原 Parser 多筆分支序列未收錄"); }
        } else { panic!("原 Parser if 節點未收錄"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_真巢狀_if_葉提前返回保留雙層條件順序() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = true\nif(true){\nif(flag){\nreturn n+1\n}else{\nreturn n+2\n}\n}\nreturn n+3\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(6)
        )));
    }

    #[test]
    fn g18_r03_外層先更新_bool_後內層_if_須重讀最新值再寫入_int() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nvar value:int = n\nif(true){\nflag = true\nif(flag){\nvalue = value+2\n}else{\nvalue = value+99\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_外層_false_路徑內的_else_巢狀_if_只執行選中的葉() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else{\nif(false){\nvalue = value+20\n}else{\nvalue = value+3\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let changes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [7]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
    }

    #[test]
    fn g18_r03_巢狀提前_return_只退出內函式不截斷父函式算術() {
        let src = "fn leaf(n:int)->int{\nif(true){\nif(false){\nreturn n+10\n}else{\nreturn n+1\n}\n}\nreturn n+3\n}\nfn outer(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [3, 5]);
    }

    #[test]
    fn g18_r03_巢狀分支第二次_int_更新溢位真框架不補寫後續結果() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_巢狀條件非法名稱與工作計畫遭竄改必先拒絕() {
        let invalid = "fn choose(n:int)->int{\nif(true){\nif(missing){\nreturn n\n}\n}\nreturn n\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn choose(n:int)->int{\nif(true){\nif(true){\nreturn n+1\n}\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let mut prepared = checked(valid);
        assert_eq!(prepared.execute().unwrap().result, Ok(2));
        if let PrivateScalarLocalStep::EarlyReturn(plan) = &mut prepared.root_local_steps[0] {
            if let PrivateScalarBranchAction::NestedIf(inner) = &mut plan.when_true {
                inner.when_true = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(50),
                );
            } else { panic!("真巢狀 If 原來源丟失"); }
        } else { panic!("原 If 工作計畫丟失"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_普通函式_while_false_零迭代仍真正檢查且返回() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(false){\nvalue = value+1\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_逐輪重讀_bool_並僅更新到達的_int() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nvar flag:bool = true\nwhile(flag){\nvalue = value+2\nflag = false\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
    }

    #[test]
    fn g18_r03_普通函式_while_內_break_不執行後續寫入() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(true){\nvalue = value+1\nbreak\nvalue = value+20\n}\nreturn value\n}\nfn main()->int{\nreturn count(1)\n}\n";
        // 只接受原 Checker 核准的可達性，不自行放寬 break 後語句。
        match PreparedPrivateScalarCall::prepare(src) {
            Ok(plan) => {
                let out = plan.execute().unwrap();
                assert_eq!(out.result, Ok(2));
                assert!(out.has_balanced_frame_trace());
            }
            Err(PrivateScalarCallPrepareError::Check(_)) => {}
            Err(other) => panic!("原 Checker 接受但 while 執行器拒絕：{other:?}"),
        }
    }

    #[test]
    fn g18_r03_普通函式_while_步數預算耗盡不產生假返回() {
        let src = "fn count(n:int)->int{\nwhile(true){\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 30,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_真控制計畫篡改須執行前拒絕() {
        let src = "fn count(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nflag = false\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(1));
        if let PrivateScalarLocalStep::Control(PrivateScalarBranchAction::While {
            condition, ..
        }) = &mut prepared.root_local_steps[1] {
            *condition = PrivateScalarBoolExpr::Constant(false);
        } else { panic!("原 while 來源控制計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_普通函式_for_迭代器真值逐輪參與累加() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 2..5{\nvalue = value+i\n}\nreturn value\n}\nfn main()->int{\nreturn sum(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LocalInitialized(_, v) if (2..5).contains(v) =>
                Some(*v), _ => None,
        }).collect::<Vec<_>>(), [3, 2, 3, 4]);
    }

    #[test]
    fn g18_r01_普通函式_for_空半開區間不虛構本體或迭代器() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 2..2{\nvalue = value+50\n}\nreturn value\n}\nfn main()->int{\nreturn sum(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_普通函式_for_break_只結束最近一輪並先離域() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 3..9{\nvalue = value+i\nbreak\nvalue = value+100\n}\nreturn value\n}\nfn main()->int{\nreturn sum(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(_))).count(), 2);
    }

    #[test]
    fn g18_r03_普通函式_for_continue_每輪清除同一宣告身份() {
        let src = "fn sum(n:int)->int{\nfor i in 0..3{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalInitialized(..))).count(), 3);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(..))).count(), 3);
    }

    #[test]
    fn g18_r02_普通函式_巢狀_for_同名迭代器用不同宣告身份() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 1..2{\nfor i in 8..9{\nvalue = value+i\n}\n}\nreturn value\n}\nfn main()->int{\nreturn sum(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        // 外層 var value 的初值恰好也是 1；只按語意事件順序
        // 排除首個本地宣告，再核對兩個真正 for 迭代的來源身份。
        let locals = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::LocalInitialized(id, value) = event {
                Some((*id, *value))
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(locals.len(), 3);
        let iter = &locals[1..];
        assert_eq!(iter.len(), 2);
        assert_ne!(iter[0].0, iter[1].0);
        assert_eq!([iter[0].1, iter[1].1], [1, 8]);
    }

    #[test]
    fn g18_r03_普通函式_巨大合法_for_受真步數預算控制() {
        let src = "fn sum(n:int)->int{\nfor i in 0..9223372036854775807{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 32,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r03_普通函式_for_迭代身份計畫偽造必先拒絕() {
        let src = "fn sum(n:int)->int{\nfor i in 2..4{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(4));
        if let PrivateScalarLocalStep::Control(PrivateScalarBranchAction::ForRange {
            iterator, ..
        }) = &mut prepared.root_local_steps[0] {
            *iterator = DeclarationId(0);
        } else { panic!("原 for 迭代身份工作計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_分支局部_int_由真_checker_初始化並在離域前讀取() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst bonus:int = 4\nvalue = value+bonus\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let locals = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, _) => Some(*id), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(locals.len(), 2);
        let ended = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ended, [locals[1], locals[0]]);
    }

    #[test]
    fn g18_r02_分支同名遮蔽只寫入區塊內宣告_外部值不變() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvar value:int = 8\nvalue = value+1\n}\nreturn value\n}\nfn main()->int{\nreturn choose(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        let initializers = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::LocalInitialized(id, value) = event {
                Some((*id, *value))
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(initializers.len(), 2);
        assert_ne!(initializers[0].0, initializers[1].0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalAssigned(
            initializers[1].0, 9,
        )));
    }

    #[test]
    fn g18_r02_巢狀條件使用分支局部_bool_真值且離域() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst allowed:bool = true\nif(allowed){\nvalue = value+3\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolInitialized(..)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolScopeEnded(..)
        )).count(), 1);
    }

    #[test]
    fn g18_r02_while_逐輪純值局部宣告配合_continue_逆序清除() {
        let src = "fn count(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nconst step:int = 2\nflag = false\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalInitialized(_, 2)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 1);
    }

    #[test]
    fn g18_r02_for_每輪局部值及_iterator_各自離域後才下一輪() {
        let src = "fn count(n:int)->int{\nfor i in 0..3{\nconst local:int = i\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let mut active = Vec::new();
        for event in &out.events {
            match event {
                PrivateScalarCallEvent::LocalInitialized(id, _) => active.push(*id),
                PrivateScalarCallEvent::LocalScopeEnded(id) => assert_eq!(active.pop(), Some(*id)),
                _ => {}
            }
        }
        assert!(active.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalInitialized(..)
        )).count(), 6);
    }

    #[test]
    fn g18_r02_已離域局部名稱禁止逃逸且作用域計畫篡改拒絕() {
        let bad = "fn choose(n:int)->int{\nif(true){\nconst hidden:int = 5\n}\nreturn hidden\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "fn choose(n:int)->int{\nif(true){\nconst hidden:int = 5\n}\nreturn n\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        if let PrivateScalarLocalStep::EarlyReturn(branch) = &mut plan.root_local_steps[0] {
            if let PrivateScalarBranchAction::Scoped(actions) = &mut branch.when_true {
                actions[0] = PrivateScalarBranchAction::DeclareInt {
                    declaration: DeclarationId(0),
                    value: PrivateScalarReturnExpr::Constant(5),
                };
            } else { panic!("原 Checker 單局部區塊未建立離域計畫"); }
        } else { panic!("原 if 來源缺失"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_同一函式框架真_ownedstr_局部與_copyable_int_返回() {
        let src = "fn inc(n:int)->int{\nvar owned:str = \"內🪼\"\nreturn n+1\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_真_if_區塊_utf8_只在選中分支建立且退域釋放() {
        let src = "fn inc(n:int)->int{\nvar result:int = n\nif(true){\nvar owned:str = \"分支\"\nresult = result+2\n}\nif(false){\nvar never:str = \"不執行\"\n}\nreturn result\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_真_for_每輪_utf8_各有唯一_resourceid_並先釋放() {
        let src = "fn inc(n:int)->int{\nfor i in 0..3{\nvar owned:str = \"迭代🪼\"\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Released(1),
            E::Created(2), E::Released(2),
            E::Created(3), E::Released(3),
        ]);
    }

    #[test]
    fn g18_r04_真_while_continue_離域_utf8_不重用舊資源() {
        let src = "fn inc(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nvar owned:str = \"本輪\"\nflag = false\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_父子函式同時擁有真_utf8_返回後依堆疊逆序釋放() {
        let src = "fn child(n:int)->int{\nvar child_owner:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar parent_owner:str = \"父\"\nreturn child(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Created(2),
            E::Released(2), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_函式迴圈資源在步數預算耗盡仍全數真釋放() {
        let src = "fn inc(n:int)->int{\nwhile(true){\nvar owned:str = \"反覆\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 6, max_steps: 30,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.resource_events.is_empty());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &out.resource_events,
        ));
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r04_真_ownedstr_帳簿偽造及未核准隱含複製皆先拒絕() {
        let src = "fn inc(n:int)->int{\nvar owned:str = \"原文\"\nreturn n\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        out.resource_events.pop();
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let invalid = "fn inc(n:int)->int{\nvar a:str = \"原文\"\nvar b:str = a\nreturn n\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_r04_真同函式_ownedstr_明確_move_單一_resourceid_不複製() {
        let src = "fn transfer(n:int)->int{\nvar original:str = \"🪼\"\nvar target:str = original.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_同函式_if_內_move_外層所有者成空_內層只釋放一次() {
        let src = "fn transfer(n:int)->int{\nvar original:str = \"外\"\nif(true){\nvar target:str = original.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_同函式_for_逐輪真_move_每次是新_resourceid() {
        let src = "fn transfer(n:int)->int{\nfor i in 0..2{\nvar source:str = \"迭代\"\nvar target:str = source.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
            E::Created(2), E::Moved(2), E::Released(2),
        ]);
    }

    #[test]
    fn g18_r04_父函式真_move_與子函式新_ownedstr_依動態順序退出() {
        let src = "fn child(n:int)->int{\nvar local:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar outer:str = \"父\"\nvar moved:str = outer.move()\nreturn child(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Created(2),
            E::Released(2), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_同函式_if_move_提前返回後不得遺失原_ResourceId() {
        let src = "fn transfer(n:int)->int{\nif(true){\nvar source:str = \"內\"\nvar target:str = source.move()\nreturn n+1\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_不選中分支的_move_不能偽造已轉移事件() {
        let src = "fn transfer(n:int)->int{\nvar source:str = \"原本\"\nif(false){\nvar unused:str = source.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_已移動來源再次_move_原_checker_拒絕且偽造來源身分先拒絕() {
        let invalid = "fn transfer(n:int)->int{\nvar source:str = \"內\"\nvar first:str = source.move()\nvar second:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn transfer(n:int)->int{\nvar source:str = \"內\"\nvar target:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let mut prepared = checked(valid);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        if let PrivateScalarLocalStep::DeclareStr {
            initializer: PrivateScalarStrInitializer::Move { declaration, .. }, ..
        } = &mut prepared.root_local_steps[1] {
            *declaration = DeclarationId(0);
        } else { panic!("原 .move() 已核准來源工作計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_原_checker_const_str_move_同函式保留目標不可寫性() {
        let src = "fn transfer(n:int)->int{\nvar source:str = \"🪼\"\nconst target:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
        ]);
    }

    /// 不汙染 F00／P16.1 的 frozen owned_str_runtime::tests 清單；
    /// R04 新可寫性驗證收歸 G18 專用 Gate。
    #[test]
    fn g18_r04_const_move_來源所有權轉交後目標不可寫且_resourceid_只釋放一次() {
        use crate::owned_str_runtime::{OwnedStrRuntime, StrRuntimeError, ResourceEvent};
        let mut resources = OwnedStrRuntime::new();
        resources.declare("source", true, "🪼").unwrap();
        resources.move_named_as("source", "target", false).unwrap();
        assert_eq!(resources.read("source"), Err(StrRuntimeError::MovedSource));
        assert_eq!(resources.read("target"), Ok("🪼"));
        assert_eq!(resources.assign("target", "改寫"),
            Err(StrRuntimeError::ImmutableBinding));
        assert_eq!(resources.finish().unwrap(), [
            ResourceEvent::Created(1),
            ResourceEvent::Moved(1),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_父子真_utf8_資源事件必嵌入函式進退同序序列() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar inner:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar outer:str = \"父\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let paired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::OwnedResource(value) => Some(value.clone()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(paired, out.resource_events);
        let child_exit = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::FunctionExited { name, returned: true }
                if name == "leaf"
        )).unwrap();
        let child_release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(child_release < child_exit);
    }

    #[test]
    fn g18_r05_原_checker_真_move_資源編號不跨父子函式錯置() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn child(n:int)->int{\nvar child_owned:str = \"第二\"\nreturn n\n}\nfn parent(n:int)->int{\nvar source:str = \"第一\"\nconst target:str = source.move()\nreturn child(n)\n}\nfn main()->int{\nreturn parent(5)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Created(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_for_逐輪真_utf8_釋放不得延後至下一輪或函式退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nfor i in 0..3{\nvar local:str = \"輪\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(6)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Released(2),
            R::Created(3), R::Released(3),
        ]);
        let trace = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::OwnedResource(event) => Some(event.clone()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(trace, out.resource_events);
    }

    #[test]
    fn g18_r05_步數耗盡與真資源清理必先於失敗函式退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn looped(n:int)->int{\nvar persistent:str = \"持有\"\nwhile(true){\nvar local:str = \"迭代\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn looped(1)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 28,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        let exit = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::FunctionExited { name, returned: false }
                if name == "looped"
        )).unwrap();
        let last_release = out.events.iter().rposition(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).unwrap();
        assert!(last_release < exit);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r05_偽造另一份平衡_resourceid_不得繞過真事件映射() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nvar owned:str = \"原\"\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        out.resource_events.extend([R::Created(88), R::Released(88)]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_偽造提前函式退出再釋放真_resourceid_必被拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nvar owned:str = \"原\"\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, .. } if name == "count"
        )).unwrap();
        let release = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        let resource = out.events.remove(release);
        let exit_after_removal = if release < exit { exit - 1 } else { exit };
        out.events.insert(exit_after_removal + 1, resource);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r04_具名_const_str_與_int_真同框架借用票據先還才釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"來源🪼\"\nreturn inspect(owner,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acquired = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanAcquired(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let owner_released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(acquired < released && released < owner_released);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )).count(), 1);
    }

    #[test]
    fn g18_r04_具名_var_str_獨占借用與_copyable_int_真返回() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn inspect(owner,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, BorrowFact {
                kind: BorrowKind::ExclusiveWrite, ..
            })
        )));
    }

    #[test]
    fn g18_r04_父函式局部_ownedstr_真正借用給子函式並續算() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn parent(n:int)->int{\nconst owner:str = \"父\"\nreturn leaf(owner,n)+2\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r05_借用首實參後第二實參動態溢位須還真票據再清理_utf8() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn explode(n:int)->int{\nreturn n+9223372036854775807\n}\nfn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,explode(1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let loan_returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let resource_released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(loan_returned < resource_released);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )));
    }

    #[test]
    fn g18_r05_真_str_借用參數綁定後故障依原快照全數歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,3)\n}\n";
        let out = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r04_真_checker_借用來源篡改或隱含複製仍拒絕() {
        let illegal = "fn inspect(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源\"\nvar copied:str = owner\nreturn inspect(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(illegal),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "fn inspect(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
        if let Some(PrivateScalarReturnExpr::Call { arguments, .. }) =
            &mut prepared.main_dynamic_call {
            if let PrivateScalarCallArgument::BorrowStr { fact, .. } = &mut arguments[0] {
                fact.source = DeclarationId(0);
            } else { panic!("原 BorrowFact 工作計畫未保存"); }
        } else { panic!("main 原 Checker 真呼叫工作計畫缺失"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r04_真函式_str_字面值覆寫只釋放被取代資源一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"舊\"\nowner = \"新🪼\"\nreturn n\n}\nfn main()->int{\nreturn run(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).count(), 1);
    }

    #[test]
    fn g18_r04_真函式_move_覆寫舊目標與來源重新初始化不複製() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar source:str = \"甲\"\nvar target:str = \"乙\"\ntarget = source.move()\nsource = \"重建\"\nreturn n\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Released(2),
            R::Created(3), R::Released(3), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_選中_if_字串覆寫原子提交_未選分支零資源事件() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"初始\"\nif(true){\nowner = \"選中\"\n}else{\nowner = \"不得建立\"\n}\nreturn n\n}\nfn main()->int{\nreturn run(6)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r02_普通函式_for_逐輪_str_覆寫即時釋放上一個_owner() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nfor i in 0..3{\nowner = \"每輪\"\n}\nreturn n\n}\nfn main()->int{\nreturn run(7)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1),
            R::Created(3), R::Released(2), R::Created(4),
            R::Released(3), R::Released(4),
        ]);
    }

    #[test]
    fn g18_r03_函式提前返回先完成_str_覆寫再真離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"甲\"\nif(true){\nowner = \"乙\"\nreturn n+1\n}\nreturn 0\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r04_不可寫_str_隱含複製與已移動再次使用均由_checker_拒絕() {
        let invalid = [
            "fn run(n:int)->int{\nconst owner:str = \"原\"\nowner = \"新\"\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
            "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = 1\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
            "fn run(n:int)->int{\nvar source:str = \"原\"\nvar target:str = \"舊\"\ntarget = source.move()\ntarget = source.move()\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_r04_原_checker_str_move_真跨函式_唯一_utf8_交接() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar owned:str = \"所有權🪼\"\nreturn receive(owned.move(),4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|ev| matches!(ev,
            PrivateScalarCallEvent::ResourceTransferred {
                resource_id: 1, ..
            }
        )).count(), 1);
        assert!(!out.events.iter().any(|ev| matches!(ev,
            PrivateScalarCallEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn g18_r04_父函式局部_整筆_move_交給子函式同一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn parent(n:int)->int{\nconst local:str = \"父框架\"\nreturn leaf(local.move(),n)\n}\nfn main()->int{\nreturn parent(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_同次真_call_獨立_move_及_const_str_借用各守原身分() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(moved:str,borrowed:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"移動\"\nconst second:str = \"借用\"\nreturn inspect(first.move(),second,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        let ticket = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let borrowed_release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(ticket < borrowed_release);
    }

    #[test]
    fn g18_r05_跨框架_move_已完成綁定後故障由子框架釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar owned:str = \"中途失敗\"\nreturn receive(owned.move(),4)\n}\n";
        let out = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r05_第二實參溢位時_move_仍由原呼叫者唯一釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"求值失敗\"\nreturn receive(owned.move(),1+9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::FunctionEntered(name) if name == "receive"
        )));
    }

    #[test]
    fn g18_r04_跨函式_move_真來源被_cherker_禁止重複及隱含複製() {
        let invalid = [
            "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned,owned.move())\n}\n",
            "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nvar duplicate:str = owned\nreturn receive(owned.move(),1)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r05_真跨函式移動事件篡改來源或資源編號均拒絕() {
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned.move(),3)\n}\n";
        let baseline = checked(src).execute().unwrap();
        assert!(baseline.has_balanced_frame_trace());
        let mut wrong_source = checked(src).execute().unwrap();
        let event = wrong_source.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { source, .. } = event {
            *source = DeclarationId(0);
        }
        assert!(!wrong_source.has_balanced_frame_trace());
        let mut wrong_id = checked(src).execute().unwrap();
        let event = wrong_id.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { resource_id, .. } = event {
            *resource_id = 999;
        }
        assert!(!wrong_id.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r02_函式var_int無初值在原checker准許後第一次賦值才建立值槽() {
        let src = "fn calc(n:int)->int{\nvar local:int\nlocal = n+4\nreturn local\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let declared = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))).unwrap();
        let initialized = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalInitialized(_, 7))).unwrap();
        let ended = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(_))).unwrap();
        assert!(declared < initialized && initialized < ended);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r02_宣告未初始化int離域不虛構初始化值或回收資源() {
        let src = "fn calc(n:int)->int{\nvar spare:int\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(..)
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r02_函式var_bool無初值先合法賦值後用於提前return() {
        let src = "fn calc(n:int)->int{\nvar ready:bool\nready = true\nif(ready){\nreturn n+2\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolDeclaredEmpty(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, true)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r02_同作用域未初始化與已初始化局部按宣告逆序離域() {
        let src = "fn calc(n:int)->int{\nvar pending:int\nconst later:int = 6\npending = n+1\nreturn later\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        let prepared = checked(src);
        let prefix = &prepared.root_local_steps;
        let a = match &prefix[0] {
            PrivateScalarLocalStep::DeclareEmptyInt(id) => *id,
            _ => panic!("原 Checker 無初值宣告來源不存在"),
        };
        let b = match &prefix[1] {
            PrivateScalarLocalStep::Declare(init) => init.declaration,
            _ => panic!("原 Checker 常值宣告來源不存在"),
        };
        assert_eq!(values, [b, a]);
    }

    #[test]
    fn g18_r02_分支var_int尚未初始化後更新不污染外層遮蔽() {
        let src = "fn calc(n:int)->int{\nvar result:int = n\nif(true){\nvar result:int\nresult = 15\n}\nreturn result\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let declare_empty = out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count();
        assert_eq!(declare_empty, 1);
    }

    #[test]
    fn g18_r03_for_逐輪var_int先保留空槽再第一次賦值與continue() {
        let src = "fn calc(n:int)->int{\nvar total:int = 0\nfor i in 0..3{\nvar step:int\nstep = i+2\ntotal = total+step\ncontinue\n}\nreturn total+n\n}\nfn main()->int{\nreturn calc(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )).count(), 0);
    }

    #[test]
    fn g18_r03_while_continue_未初始化bool與int只執行選中路徑() {
        let src = "fn calc(n:int)->int{\nvar active:bool\nactive = true\nvar count:int = n\nwhile(active){\nvar next:int\nnext = count+4\ncount = next\nactive = false\ncontinue\n}\nreturn count\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r03_初始化右值溢位時空局部只離域不造假值() {
        let src = "fn calc(n:int)->int{\nvar pending:int\npending = n+9223372036854775807\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(..)
                | PrivateScalarCallEvent::Returned(..)
        )));
    }

    #[test]
    fn g18_r01_main_var_int無初值賦值後真傳入普通函式() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar base:int\nbase = 4\nreturn inc(base)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 4)
        )));
    }

    #[test]
    fn g18_r02_無初值int_bool未合法賦值即讀由原checker拒絕() {
        let invalid = [
            "fn calc(n:int)->int{\nvar pending:int\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n",
            "fn calc(n:int)->int{\nvar flag:bool\nif(flag){return n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r02_未初始化來源計畫篡改類型執行前零事件拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:int\npending = n+1\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.root_local_steps[0] =
            PrivateScalarLocalStep::DeclareEmptyBool(DeclarationId(0));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r05_偽造未初始化局部冒充已初始化或刪除真離域須拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:int\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let mut altered = checked(src).execute().unwrap();
        altered.events.retain(|e| !matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        ));
        assert!(!altered.has_balanced_frame_trace());
        let mut altered = checked(src).execute().unwrap();
        let at = altered.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))
        ).unwrap();
        let id = match altered.events[at] {
            PrivateScalarCallEvent::LocalDeclaredEmpty(id) => id,
            _ => unreachable!(),
        };
        altered.events.insert(at+1, PrivateScalarCallEvent::LocalInitialized(id, 99));
        assert!(!altered.has_balanced_frame_trace());
    }



    #[test]
    fn g18_r02_真str無初值單純宣告離域零ResourceId() {
        let src = "fn calc(n:int)->int{\nvar spare:str\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(..)
        )));
    }

    #[test]
    fn g18_r02_真str無初值後字面賦值只建立一次並退出釋放() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"海月\"\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r02_延遲str兩次賦值第二次原子替換釋放舊資源() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"先\"\npending = \"後\"\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Created(2), E::Released(1), E::Released(2),
        ]);
    }

    #[test]
    fn g18_r02_無初值str以已核准move首次接收原ResourceId() {
        let src = "fn calc(n:int)->int{\nvar source:str = \"唯一\"\nvar pending:str\npending = source.move()\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r03_未選分支的str空宣告與賦值不建立UTF8資源() {
        let src = "fn calc(n:int)->int{\nif(false){\nvar pending:str\npending = \"不可達\"\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r03_for_逐輪無初值str先賦值每輪唯一ResourceId清理() {
        let src = "fn calc(n:int)->int{\nfor i in 0..3{\nvar pending:str\npending = \"每輪\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Released(1),
            E::Created(2), E::Released(2),
            E::Created(3), E::Released(3),
        ]);
    }

    #[test]
    fn g18_r03_空str與真str共框架溢位僅清理確實建立的資源() {
        let src = "fn calc(n:int)->int{\nvar spare:str\nvar owned:str = \"已建\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_非法未初始化str讀取借用必由原Checker提前拒絕() {
        let invalid = [
            "fn receive(s:str,n:int)->int{return n}\nfn calc(n:int)->int{\nvar spare:str\nreturn receive(spare,n)\n}\nfn main()->int{return calc(3)}\n",
            "fn calc(n:int)->int{\nvar first:str\nvar other:str = first\nreturn n\n}\nfn main()->int{return calc(3)}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r02_延遲str來源編譯身分篡改必在動態執行前拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"唯一\"\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        plan.root_local_steps[0] = PrivateScalarLocalStep::DeclareEmptyStr {
            declaration: DeclarationId(0), name: "pending".to_string(), mutable: true,
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r03_延遲str真字面賦值後借用參數先還票再釋放資源() {
        let src = "fn receive(s:str,n:int)->int{\nreturn n+1\n}\nfn calc(n:int)->int{\nvar pending:str\npending = \"借用\"\nreturn receive(pending,n)\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let repaid = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().rposition(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(
                crate::owned_str_runtime::ResourceEvent::Released(_)
            )
        )).unwrap();
        assert!(repaid < released);
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }



    #[test]
    fn g18_r01_main_var首次賦值的普通函式來源必真執行() {
        let src = "fn increment(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar out:int\nout = increment(4)\nreturn out\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let declared = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))).unwrap();
        let entered = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "increment"
        )).unwrap();
        let initialized = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 5))).unwrap();
        assert!(declared < entered && entered < initialized);
    }

    #[test]
    fn g18_r01_main先返回葉函式後更新局部不得逆序求值() {
        let src = "fn leaf()->int{\nreturn 7\n}\nfn main()->int{\nvar value:int\nvalue = leaf()+3\nreturn value\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::Returned(7))).unwrap();
        let initialized = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 10))).unwrap();
        assert!(returned < initialized);
    }

    #[test]
    fn g18_r02_main同一var先以函式初次賦值再由函式重賦值() {
        let src = "fn a(n:int)->int{\nreturn n+2\n}\nfn b(n:int)->int{\nreturn n+3\n}\nfn main()->int{\nvar result:int\nresult = a(1)\nresult = b(result)\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) =>
                Some(name.as_str()), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["a", "b"]);
    }

    #[test]
    fn g18_r03_main函式賦值右值溢位不能假造成功初始化或第二個呼叫() {
        let src = "fn first(n:int)->int{\nreturn n+1\n}\nfn later()->int{\nreturn 3\n}\nfn main()->int{\nvar result:int\nresult = first(9223372036854775807)\nresult = later()\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "later"
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r01_main賦值來源計畫遭竄改必先拒絕不得偽造執行() {
        let src = "fn leaf()->int{\nreturn 2\n}\nfn main()->int{\nvar value:int\nvalue = leaf()\nreturn value\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(2));
        if let PrivateScalarLocalStep::Assign { value, .. }
            = &mut plan.main_dynamic_steps[1] {
            *value = PrivateScalarReturnExpr::Constant(900);
        } else { panic!("原 Checker 的 main 右值真呼叫必須保留"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_r01_main_if選中分支的真函式呼叫不需出現在尾端return() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = inc(3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc"
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_if_false未選函式零事件且else選中另一真函式() {
        let src = "fn unused()->int{\nreturn 99\n}\nfn chosen()->int{\nreturn 7\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = unused()\n}else{\nresult = chosen()\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let enters = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["chosen"]);
    }

    #[test]
    fn g18_r03_main_while逐輪重讀條件內的真函式來源() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar active:bool = true\nvar result:int = 0\nwhile(active){\nresult = inc(result)\nactive = false\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r01_main_for迭代值在真函式參數中按半開區間逐輪傳入() {
        let src = "fn add(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar result:int = 0\nfor i in 1..4{\nresult = add(result,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "add"
        )).count(), 3);
    }

    #[test]
    fn g18_r03_main巢狀分支函式溢位不得執行後續else或虛構返回() {
        let src = "fn overflow(n:int)->int{\nreturn n+1\n}\nfn skipped()->int{\nreturn 9\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = overflow(9223372036854775807)\n}else{\nresult = skipped()\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "skipped"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_main巢狀控制真函式源計畫篡改必在執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = inc(4)\n}\nreturn result\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(5));
        plan.main_dynamic_steps.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_r01_r03_main選中if內真utf8局部借用給函式後才離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"分支🪼\"\nresult = inspect(owner,4)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let borrowed = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).unwrap();
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).unwrap();
        let released = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1)))).unwrap();
        assert!(borrowed < returned && returned < released);
    }

    #[test]
    fn g18_r02_r03_main_if_false來源不建資源_else同名局部只取真分支() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar owner:str = \"不執行\"\nresult = inspect(owner,9)\n}else{\nvar owner:str = \"唯一🪼\"\nresult = inspect(owner,4)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
    }

    #[test]
    fn g18_r02_r03_main_for外層utf8每輪重借同源須逐次還票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"外層🪼\"\nvar result:int = 0\nfor i in 0..3{\nresult = result+inspect(owner,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
    }

    #[test]
    fn g18_r03_main_while內真字串借用隨continue先還票再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar active:bool = true\nvar result:int = 0\nwhile(active){\nvar owner:str = \"每輪🪼\"\nresult = inspect(owner,3)\nactive = false\ncontinue\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_))).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_if借用首實參後第二實參溢位須回滾真票據與資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"錯誤🪼\"\nresult = inspect(owner,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }



    #[test]
    fn g18_r02_main_if內真str局部move給第二局部仍僅釋放原resourceid一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"獨占🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_for每輪str先move再借用不可複製同一個owner() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nfor i in 0..3{\nvar source:str = \"每輪\"\nvar target:str\ntarget = source.move()\nresult = result+inspect(target,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
            R::Created(3), R::Moved(3), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
    }

    #[test]
    fn g18_r03_main_if先move後呼叫次實參溢位同一資源須恰好釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"錯誤\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r02_main分支中已move來源再次借用必由原checker先拒絕() {
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"一次\"\nvar target:str\ntarget = source.move()\nresult = inspect(source,3)\n}\nreturn result\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01～R03：將真 UTF-8 所有權轉移、呼叫借用與 CFG 退出放在同一
    // 原 Checker 驗證的 main 工作堆疊；不是分別執行模型後拼湊結果。
    #[test]
    fn g18_r03_main_while_break前真move借用只建立清理首輪資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nvar active:bool = true\nwhile(active){\nvar source:str = \"首輪🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,2)\nbreak\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_for_continue每輪真move與借用先還票再走回邊() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar total:int = 0\nfor i in 0..3{\nvar source:str = \"回邊🪼\"\nvar target:str\ntarget = source.move()\ntotal = total+inspect(target,i)\ncontinue\n}\nreturn total\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
            R::Created(3), R::Moved(3), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
        for id in [1, 2, 3] {
            let released = out.events.iter().position(|e| matches!(
                e, PrivateScalarCallEvent::OwnedResource(R::Released(actual)) if *actual == id
            )).unwrap();
            assert!(out.events[..released].iter().any(|e| matches!(
                e, PrivateScalarCallEvent::LoanReturned(_)
            )));
        }
    }

    #[test]
    fn g18_r03_main_if提前return真move借用且跳過後續資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn fallback(n:int)->int{\nreturn n\n}\nfn main()->int{\nif(true){\nvar source:str = \"提前🪼\"\nvar target:str\ntarget = source.move()\nreturn inspect(target,4)\n}\nreturn fallback(0)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "fallback"
        )));
    }

    #[test]
    fn g18_r01_main選中分支真字串借用嵌入第二層函式實參() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn increment(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"多層🪼\"\nresult = increment(inspect(owner,3))\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inspect", "increment"]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_if_false未選多層函式不造資源真else只借用一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn increment(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar owner:str = \"未選🪼\"\nresult = increment(inspect(owner,99))\n}else{\nvar owner:str = \"選中🪼\"\nresult = increment(inspect(owner,3))\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }


    // R01：其他合法宣告不是 main 所需的動態工作，不能強制把
    // 它的返回型別誤當 int 編譯；實際可達的所有層仍完整核對。
    #[test]
    fn g18_r01_未使用的合法bool輔助函式不得阻斷真int主呼叫() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn forward(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn forward(6)\n}\n";
        let plan = checked(src);
        assert!(plan.nested_callees.is_empty());
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
    }

    #[test]
    fn g18_r01_主函式到二層真葉函式仍收錄且跳過不相干bool宣告() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn leaf(n:int)->int{\nreturn n+1\n}\nfn relay(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn relay(5)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["leaf"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["relay", "leaf"]);
    }

    #[test]
    fn g18_r01_巢狀實參呼叫圖需收錄內外兩個int目標而略過無關函式() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn inner(n:int)->int{\nreturn n+2\n}\nfn outer(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn outer(inner(3))\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["inner"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inner", "outer"]);
    }

    #[test]
    fn g18_r01_主函式兄弟分支只執行選中函式而收錄兩者並略過無關bool() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn left(n:int)->int{\nreturn n+1\n}\nfn right(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = left(10)\n}else{\nresult = right(3)\n}\nreturn result\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["right"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name)
                if name == "left" || name == "ready"
        )));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )).count(), 1);
    }

    #[test]
    fn g18_r01_可達圖身分遭竄改及bool誤用int回傳都須拒絕() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn leaf(n:int)->int{\nreturn n+1\n}\nfn relay(n:int)->int{\nreturn leaf(n)\n}\nfn main()->int{\nreturn relay(5)\n}\n";
        let mut plan = checked(src);
        plan.nested_callees.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let wrong = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn main()->int{\nreturn ready(true)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(wrong),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01：source 內可存在不會建構或執行的合法結構宣告。
    // 此切片只執行原 Checker 已准的真普通函式圖。
    #[test]
    fn g18_r01_無關int結構宣告不應阻斷main純函式執行() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(8)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.verified.structs.len(), 1);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.resource_events.is_empty());
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r01_未使用結構的合法self方法不可冒充真函式進入() {
        let src = "struct Archive{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn step(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nreturn step(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "get"
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r01_無關結構的utf8預設欄位不得於main執行時建立資源() {
        let src = "struct Archive{\nvar title:str = \"未使用🪼\"\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(_)
        )));
    }

    #[test]
    fn g18_r02_旁觀結構宣告不影響真main分支字串借用與清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar title:str = \"旁觀\"\n}\nfn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"唯一🪼\"\nresult = inspect(owner,3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r01_非法重複結構欄位仍由原checker拒絕() {
        let src = "struct Archive{\nvar count:int = 1\nvar count:int = 2\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(2)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_r01_來源已驗證的結構清單遭竄改必在執行前拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(4)\n}\n";
        let mut plan = checked(src);
        plan.verified.structs.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R03：跨 if／else-if／while／for 的真控制邊，必須在同一
    // 原 Checker 核准工作堆疊依實際進入路徑清理 Move／BorrowTicket。
    #[test]
    fn g18_r03_main_else_if選中分支str_move借用只建立一個resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar source:str = \"未選一\"\nresult = inspect(source,100)\n}else if(true){\nvar source:str = \"選中🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,3)\n}else{\nvar source:str = \"未選二\"\nresult = inspect(source,200)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_while內for_break只清理內層move借用且外層仍繼續() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar active:bool = true\nvar total:int = 0\nwhile(active){\nfor i in 0..3{\nvar source:str = \"內層🪼\"\nvar target:str\ntarget = source.move()\ntotal = total+inspect(target,i)\nbreak\n}\nactive = false\n}\nreturn total\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_for內提前return完成真move借用且不進後續迭代() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn fallback(n:int)->int{\nreturn n\n}\nfn main()->int{\nfor i in 0..4{\nvar source:str = \"第一輪🪼\"\nvar target:str\ntarget = source.move()\nreturn inspect(target,i+5)\n}\nreturn fallback(0)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "fallback"
        )));
    }

    #[test]
    fn g18_r03_main_else_if真move後第二引數溢位須回滾票據和唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = 99\n}else if(true){\nvar source:str = \"錯誤🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }

    #[test]
    fn g18_r03_main_else_if已move來源再借用由原checker提前拒絕() {
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = 1\n}else if(true){\nvar source:str = \"單一🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(source,3)\n}\nreturn result\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01：main 尾端只是合法 int 常值，前置語句裡的函式
    // 一樣必須真進入與真返回，不能以最後的常值掩蓋它的錯誤。
    #[test]
    fn g18_r01_main前置真呼叫後尾端int常值return必執行呼叫() {
        let src = "fn calc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar ignored:int = calc(3)\nreturn 9\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "calc"
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(4)
        )));
    }

    #[test]
    fn g18_r01_main真賦值函式後尾端括號整數常值仍須執行() {
        let src = "fn calc(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar output:int\noutput = calc(5)\nreturn (11)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "calc"
        )).count(), 1);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(7)
        )));
    }

    #[test]
    fn g18_r03_main分支真str借用函式後返回常值應先歸還再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nif(true){\nvar owner:str = \"先執行🪼\"\nvar ignored:int = inspect(owner,3)\n}\nreturn 17\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(17));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main前置函式溢位不可被最後return常值掩蓋() {
        let src = "fn crash(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar unused:int = crash(9223372036854775807)\nreturn 9\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "crash"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(9)
        )));
    }

    #[test]
    fn g18_r01_main前置兩函式依序執行後才允許返回無關常值() {
        let src = "fn first(n:int)->int{\nreturn n+1\n}\nfn second(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar a:int = first(2)\nvar b:int = second(a)\nreturn 0\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        let entered: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["first", "second"]);
    }

    #[test]
    fn g18_r01_main前置呼叫尾端常值計畫遭竄改必先拒絕() {
        let src = "fn calc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar unused:int = calc(3)\nreturn 9\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        plan.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_不含真函式呼叫的main常值仍不得偽造函式證據() {
        let src = "fn unused(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn 9\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    // R03：原 Checker 不會為所有分支已返回的不可達末端 return
    // 簽發 int Expression HIR；不准因此執行假的末端返回。
    #[test]
    fn g18_r03_main_if_else全分支真返回且末端不可達常值不需假hir() {
        let src = "fn left()->int{\nreturn 2\n}\nfn right()->int{\nreturn 3\n}\nfn main()->int{\nif(true){\nreturn left()\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let plan = checked(src);
        assert!(plan.verified.plan.scalar_hir.expression(plan.call_span).is_none());
        assert!(matches!(&plan.main_dynamic_call, Some(PrivateScalarReturnExpr::Unreachable)));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "left"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
    }

    #[test]
    fn g18_r03_main_if_else假分支不進且不可達末端return零事件() {
        let src = "fn left()->int{\nreturn 5\n}\nfn right()->int{\nreturn 7\n}\nfn main()->int{\nvar use_left:bool = false\nif(use_left){\nreturn left()\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let calls: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(calls, ["right"]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
    }

    #[test]
    fn g18_r03_main全分支返回時選中str借用完成還票與資源清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn right()->int{\nreturn 999\n}\nfn main()->int{\nif(true){\nvar owner:str = \"提前🪼\"\nreturn inspect(owner,3)\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )));
    }

    #[test]
    fn g18_r03_main全分支提前返回溢位不可退回不可達常值() {
        let src = "fn crash(n:int)->int{\nreturn n+1\n}\nfn safe()->int{\nreturn 3\n}\nfn main()->int{\nif(true){\nreturn crash(9223372036854775807)\n}else{\nreturn safe()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "safe"
        )));
    }

    #[test]
    fn g18_r03_main不可達尾端計畫遭改為可執行常值須拒絕() {
        let src = "fn calc()->int{\nreturn 3\n}\nfn other()->int{\nreturn 4\n}\nfn main()->int{\nif(true){\nreturn calc()\n}else{\nreturn other()\n}\nreturn 99\n}\n";
        let mut plan = checked(src);
        assert!(matches!(plan.main_dynamic_call, Some(PrivateScalarReturnExpr::Unreachable)));
        plan.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(99));
        assert!(matches!(plan.execute(), Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R02：跨框架同名的 str 局部是不同 Checker DeclarationId；
    // 真資源核心必保留原本詞彙身分，不得因文字同名交錯釋放。
    #[test]
    fn g18_r02_父子函式同名str各自建立並依框架反向釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"子層🪼\"\nreturn n+2\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層🪼\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn mid(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(_, _)
        )).count(), 2);
    }

    #[test]
    fn g18_r02_三層move資源與兩層旁觀str不同id各自唯一清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nvar spare:str = \"葉層\"\nreturn n+1\n}\nfn mid(item:str,n:int)->int{\nvar spare:str = \"中層\"\nreturn leaf(item.move(),n)\n}\nfn main()->int{\nvar original:str = \"根🪼\"\nreturn mid(original.move(),3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Created(2),
            R::Moved(1), R::Created(3), R::Released(3),
            R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_r02_借用中的父框架str與子框架新同名局部不能互相冒充() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nvar owner:str = \"子層獨立\"\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"父層仍在借用\"\nreturn inspect(owner,7)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_子框架同名str後溢位必先釋放子層再清理父層() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"子層錯誤\"\nreturn n+9223372036854775807\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層持有\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn mid(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_重入同一葉函式的同名str每次重新配置且不污染父框架() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"每次不同id\"\nreturn n+1\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層仍持有\"\nvar first:int = leaf(n)\nvar second:int = leaf(first)\nreturn second\n}\nfn main()->int{\nreturn mid(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2),
            R::Created(3), R::Released(3), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf"
        )).count(), 2);
    }

    #[test]
    fn g18_r02_借用str參數不得因子函式可見同名文字擅自move() {
        let src = "fn leaf(item:str)->int{\nreturn 1\n}\nfn mid(item:str)->int{\nreturn leaf(item.move())\n}\nfn main()->int{\nvar owner:str = \"不可轉借\"\nreturn mid(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))
                | Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    // R01：主函式尾端若已由 Checker 完整證明不可達，未執行
    // 的其他合法函式不得被強制當作純 int 動態圖的一環。
    #[test]
    fn g18_r01_main全分支提前返回不應編譯尾端不支援的結構函式() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nif(true){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert_eq!(plan.callee_name, "chosen");
        assert!(!plan.nested_callees.iter().any(|p| p.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_r03_main_false分支真函式不被不可達結構呼叫阻塞() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nvar select:bool = false\nif(select){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert!(!plan.nested_callees.iter().any(|p| p.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let entered: Vec<_> = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["other"]);
    }


    // V1／F4：以下均由原 Parser／Checker 已核准的真來源出發，
    // 只有無 str 欄位的結構才交給 P16 MixedStructRuntime 真執行。
    // V1／R01：用與原 Checker 同源的真正 main→函式工作圖驗證
    // Copyable 結構欄位在呼叫實參、賦值、跨框架與故障退出的順序。
    // V1-F3：來源為原 Checker TypedHIR 認可的 bool 欄位；
    // 在真正的 if／while 和普通函式 bool 實參位置動態讀取 P16 持有值。
    // V1-F3：真 Parser/Checker 的 for／while 控制與 Copyable 結構
    // int/bool 欄位混合，逐輪動態求值；不可達邊不得冒充函式進入。
    #[test]
    fn g18_v1_f3_真結構bool欄位三輪for內呼叫函式有序累積() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 3\n}\nfn step(x:int)->int{\nreturn x+2\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nfor i in 0..3{\nif(meter.enabled){\ntotal = total+step(meter.count)\n}\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(15));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "step"
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f3_真結構欄位while中呼叫後break不重複執行() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 4\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nwhile(meter.enabled){\ntotal = next(meter.count)\nbreak\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_真結構欄位零次for永不進入待呼叫函式() {
        let src = "struct Meter{\nvar count:int = 4\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 8\nfor i in 0..0{\ntotal = next(meter.count)\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )));
    }

    #[test]
    fn g18_v1_f3_真結構欄位for條件continue不執行後續呼叫() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 5\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nfor i in 0..2{\nif(meter.enabled){\ncontinue\n}\ntotal = next(meter.count)\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool真欄位控制if選中及未選中不造假框架() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 5\n}\nfn yes()->int{\nreturn 7\n}\nfn no()->int{\nreturn 3\n}\nfn route()->int{\nvar note:Meter = Meter(enabled=false)\nif(note.enabled){\nreturn yes()\n}else{\nreturn no()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "yes"
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位真實參在子函式if判定() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn yes()->int{\nreturn 10\n}\nfn no()->int{\nreturn 2\n}\nfn choose(flag:bool)->int{\nif(flag){\nreturn yes()\n}\nreturn no()\n}\nfn route()->int{\nvar meter:Meter = Meter(enabled=false)\nreturn choose(meter.enabled)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::BoolArgumentEvaluated(false)
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool真欄位while每輪判定且break有序離域() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one(x:int)->int{\nreturn x\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nwhile(meter.enabled){\ntotal = total+1\nbreak\n}\nreturn one(total)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ConditionalChecked(true)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_結構bool_false_while零迭代且保留真函式圖() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one(x:int)->int{\nreturn x\n}\nfn route()->int{\nvar meter:Meter = Meter(enabled=false)\nvar total:int = 8\nwhile(meter.enabled){\ntotal = total+1\n}\nreturn one(total)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ConditionalChecked(false)
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位型別不符原Checker直接拒絕() {
        let src = "struct Meter{\nvar count:int = 9\n}\nfn yes()->int{\nreturn 7\n}\nfn route()->int{\nvar meter:Meter = Meter()\nif(meter.count){\nreturn yes()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位編譯身分竄改必須零執行拒絕() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one()->int{\nreturn 1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nif(meter.enabled){\nreturn one()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(1));
        let Some(PrivateScalarLocalStep::EarlyReturn(branch)) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::EarlyReturn(_))) else {
            panic!("原 Checker 真 if 工作圖必須存在");
        };
        branch.condition = PrivateScalarBoolExpr::CopyStructBoolField {
            declaration: DeclarationId(0),
            owner: "meter".to_owned(), field: "enabled".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F3：所有 int 欄位寫入均由真正 Checker 允許的
    // var 實例／var 欄位與 TypedHIR RHS 簽發，P16 真值更新後才留下事件。
    // V1／F3：bool 欄位只在真 P16 持有者修改後可改變
    // 同一原 Checker 合法的 if／while 動態控制判斷。
    // V1／F3：補 #660 未涵蓋的來源交叉，沿用同一 Checker/P16 真 runtime。
    #[test]
    fn g18_v1_f3_同一函式if內同名結構遮蔽更新後外層真值不變() {
        let source = "struct Meter{\nvar count:int = 1\nvar ready:bool = false\n}\nfn run()->int{\nvar note:Meter = Meter(count=3)\nif(true){\nvar note:Meter = Meter(count=9)\nnote.count = 11\nnote.ready = true\nif(note.ready){\nnote.count = note.count+1\n}\n}\nreturn note.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f3_bool欄位真來源作為右值並保持先讀後寫() {
        let src = "struct Flags{\nvar left:bool = false\nvar right:bool = true\n}\nfn route()->int{\nvar f:Flags = Flags()\nf.left = f.right\nf.right = false\nif(f.left){\nreturn 9\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 2);
    }


    #[test]
    fn g18_v1_f3_bool欄位只寫所選if葉而且可傳實參() {
        let src = "struct Flag{\nvar ready:bool = false\n}\nfn choose(value:bool)->int{\nif(value){\nreturn 5\n}\nreturn 2\n}\nfn route(flag:bool)->int{\nvar f:Flag = Flag()\nif(flag){\nf.ready = true\n}else{\nf.ready = false\n}\nreturn choose(f.ready)\n}\nfn main()->int{\nreturn route(false)+route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 2);
    }


    #[test]
    fn g18_v1_f3_bool欄位for_continue與while_break真回邊() {
        let src = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nfor i in 0..2{\nf.ready = true\ncontinue\n}\nwhile(f.ready){\nf.ready = false\nbreak\n}\nif(f.ready){\nreturn 99\n}\nreturn 4\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 3);
    }


    #[test]
    fn g18_v1_f3_bool欄位非法const及錯型別必原Checker拒絕() {
        let const_owner = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nconst f:Flag = Flag()\nf.ready = true\nreturn 0\n}\nfn main()->int{return route()}\n";
        let const_field = "struct Flag{\nconst ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nf.ready = true\nreturn 0\n}\nfn main()->int{return route()}\n";
        let wrong_type = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nf.ready = 1\nreturn 0\n}\nfn main()->int{return route()}\n";
        for src in [const_owner, const_field, wrong_type] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    // V1／F4：使用原 Checker direct_moves 的唯一轉移配對，
    // 真 P16 整筆 move 後目標才可以讀，原來源已不可讀。
    #[test]
    fn g18_v1_f4_純結構整筆move真值唯一轉交而無第二建構() {
        let src = "struct Marker{\nvar count:int = 6\nvar enabled:bool = true\n}\nfn route()->int{\nvar source:Marker = Marker()\nvar target:Marker = source.move()\nreturn target.count+1\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_純結構兩次move鏈只交付最後唯一持有者() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nvar third:Marker = second.move()\nreturn third.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f4_純結構真分支整筆move早返且完成雙局部離域() {
        let src = "struct Marker{\nvar count:int = 8\n}\nfn route(flag:bool)->int{\nvar source:Marker = Marker()\nif(flag){\nvar target:Marker = source.move()\nreturn target.count\n}\nreturn 2\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        // Checker 接受但實際沒選到的同一條控制路徑，
        // 不得提前發出 MoveBound 或少清理來源結構。
        let not_selected = src.replace("route(true)", "route(false)");
        let skipped = checked(&not_selected).execute().unwrap();
        assert_eq!(skipped.result, Ok(2));
        assert!(skipped.has_balanced_frame_trace());
        assert!(skipped.resource_events.is_empty());
        assert!(!skipped.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )));
        assert_eq!(skipped.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_move後再次移動相同原結構原Checker必拒絕() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nvar illegal:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_move動態計畫DeclarationId遭改寫必先拒絕() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
        let Some(PrivateScalarLocalStep::MoveCopyStruct { source, .. }) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::MoveCopyStruct { .. })) else {
            panic!("原 Checker 真 move 工作應存在");
        };
        *source = DeclarationId(0);
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F4：含 const str 的整筆結構移動不重新建立 UTF-8，
    // 原 P16 真 OwnedUtf8 必須保有 ResourceId 並由新持有者唯一釋放。
    // V1-F4：原 Checker 真具名 var str 完整結構以同一
    // P16 SharedLedger Move，移後仍可依既存原子欄位覆寫核准路徑更新。
    #[test]
    fn g18_v1_f4_具名varstr結構整筆move原resourceid唯一轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"未選預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"真值🪼\")\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr移動前後兩次真覆寫各id只釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"不執行預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"初始\")\nfirst.data = \"移動前\"\nvar second:Marker = first.move()\nsecond.data = \"移動後🪼\"\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Moved(2),
            R::Created(3), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 2);
    }


    // V1-F4／R07：P16 真資源欄位次序是結構定義順序，不是名稱字典序；
    // 欄位原子換值後仍必以最新 ResourceId 逐欄 Move。
    #[test]
    fn g18_v1_f4_倒序欄名替換後整筆move仍依P16欄位順序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"預設甲\"\nvar alpha:str = \"預設乙\"\nvar count:int = 8\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"甲\",alpha=\"乙\")\nold.zeta = \"新甲🪼\"\nvar next:Marker = old.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3), R::Released(1),
            R::Moved(3), R::Moved(2), R::Released(2), R::Released(3),
        ]);
    }

    #[test]
    fn g18_v1_f4_雙欄move事件即使雙帳同步調序仍須拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"甲\"\nvar alpha:str = \"乙\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"真甲\",alpha=\"真乙\")\nvar next:Marker = old.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let inline: Vec<_> = out.events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateScalarCallEvent::OwnedResource(R::Moved(_)))
                .then_some(index)).collect();
        let ledger: Vec<_> = out.resource_events.iter().enumerate()
            .filter_map(|(index, event)|
                matches!(event, R::Moved(_)).then_some(index)).collect();
        assert_eq!(inline.len(), 2);
        assert_eq!(ledger.len(), 2);
        out.events.swap(inline[0], inline[1]);
        out.resource_events.swap(ledger[0], ledger[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f4_雙欄連續兩次整筆move仍保持欄位原次序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"甲\"\nvar alpha:str = \"乙\"\nvar count:int = 6\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"真甲\",alpha=\"真乙\")\nvar middle:Marker = old.move()\nvar next:Marker = middle.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Moved(1), R::Moved(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_雙具名varstr整筆move保持雙resourceid及逆序退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar first:str = \"忽略甲\"\nvar second:str = \"忽略乙\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker(first=\"甲\",second=\"乙🪼\")\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr與conststr整筆move後僅var可覆寫() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar data:str = \"預設\"\nconst label:str = \"不可寫\"\nvar count:int = 9\n}\nfn route()->int{\nvar old:Marker = Marker(data=\"可移🪼\")\nvar new:Marker = old.move()\nnew.data = \"新值\"\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Created(3), R::Released(1), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名varstr整筆move僅真條件葉有資源移動() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 4\nvar data:str = \"預設\"\n}\nfn route(flag:bool)->int{\nvar old:Marker = Marker(data=\"真值\")\nif(flag){\nvar new:Marker = old.move()\nreturn new.count\n}\nreturn old.count\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let selected = checked(src).execute().unwrap();
        assert_eq!(selected.result, Ok(4));
        assert!(selected.has_balanced_frame_trace());
        assert_eq!(selected.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        let false_src = src.replace("route(true)", "route(false)");
        let not_selected = checked(&false_src).execute().unwrap();
        assert_eq!(not_selected.result, Ok(4));
        assert!(not_selected.has_balanced_frame_trace());
        assert_eq!(not_selected.resource_events, [R::Created(1), R::Released(1)]);
    }

    #[test]
    fn g18_v1_f4_具名varstr整筆move後再讀來源原Checker必拒絕() {
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"源值\")\nvar second:Marker = first.move()\nreturn first.count\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(PreparedPrivateScalarCall::prepare(src).is_err());
    }

    #[test]
    fn g18_v1_f4_conststr結構整筆move真資源唯一轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"真資源🪼\"\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { .. }
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_雙conststr整筆move兩個id不複製並反序退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_conststr整筆move在真條件分支才發出moved() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"分支資源\"\n}\nfn route(flag:bool)->int{\nvar first:Marker = Marker()\nif(flag){\nvar second:Marker = first.move()\nreturn second.count\n}\nreturn first.count\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        let src_false = src.replace("route(true)", "route(false)");
        let absent = checked(&src_false).execute().unwrap();
        assert_eq!(absent.result, Ok(7));
        assert!(absent.has_balanced_frame_trace());
        assert_eq!(absent.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!absent.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )));
    }

    #[test]
    fn g18_v1_f4_conststr整筆move後溢位仍真釋放唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"回滾🪼\"\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count+9223372036854775807\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_conststr整筆move資源事件竄改及刪除必拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"防偽\"\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Moved(_))
        )).expect("原共享帳已發出真 Moved");
        let original = out.events[index].clone();
        out.events[index] = PrivateScalarCallEvent::OwnedResource(R::Moved(999));
        assert!(!out.has_balanced_frame_trace());
        out.events[index] = original;
        out.events.remove(index);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_可寫str預設結構整筆move維持唯一resourceid() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Marker{\nvar count:int = 6\nvar data:str = \"安全\"\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);

    }

    #[test]
    fn g18_v1_f4_無str預設的部分初始化整筆move仍保守拒絕() {
        // 真 Checker 先拒絕未初始化 var str 整筆 Move，
        // 不能為了命中 Runtime Unsupported 降低靜態安全標準。
        let src = "struct Marker{\nvar count:int = 6\nvar data:str\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let result = PreparedPrivateScalarCall::prepare(src);
        assert!(matches!(&result, Err(PrivateScalarCallPrepareError::Check(_))),
            "原 Checker 須先拒絕不完整來源");

    }

    #[test]
    fn g18_v1_f3_bool結構欄位寫入後if重新讀取最新值() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nif(item.enabled){\nreturn 9\n}\nreturn 2\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, name, false)
                if name == "enabled"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool欄位while首輪更新後反向邊真結束() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nvar total:int = 0\nwhile(item.enabled){\ntotal = total+1\nitem.enabled = false\n}\nreturn total\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(false)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool結構欄位只有選中分支真更新() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nif(false){\nitem.enabled = false\n}else{\nitem.enabled = true\n}\nif(item.enabled){\nreturn 8\n}\nreturn 2\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, _, true)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, _, false)
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f3_bool結構寫入後溢位要正常單次離域() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 9223372036854775807+1\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool欄位const持有者或const欄位原Checker拒絕() {
        let owner = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nconst item:Flag = Flag()\nitem.enabled = false\nreturn 1\n}\nfn main()->int{\nreturn step()\n}\n";
        let field = "struct Flag{\nconst enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 1\n}\nfn main()->int{\nreturn step()\n}\n";
        for src in [owner, field] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f3_bool欄位偽造更新DeclarationId必拒絕() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 3\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let mut changed = out;
        let target = changed.events.iter_mut().find(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).expect("真 P16 bool 欄位事件");
        if let PrivateScalarCallEvent::CopyStructBoolFieldAssigned(id, _, _) = target {
            *id = DeclarationId(0);
        }
        assert!(!changed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f3_真struct_int欄位寫入後讀回更新值() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = note.count+4\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, field, 7)
                if field == "count"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_分支只寫入選中路徑的真struct欄位() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step(flag:bool)->int{\nvar note:Meter = Meter()\nif(flag){\nnote.count = 9\n}else{\nnote.count = 2\n}\nreturn note.count\n}\nfn main()->int{\nreturn step(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, field, 2)
                if field == "count"
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, _, 9)
        )));
    }

    #[test]
    fn g18_v1_f3_真struct可寫欄位在callee故障後仍須一次逆序離域() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 9\nreturn note.count+9223372036854775807\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f3_const結構或const欄位的寫入原Checker必拒絕() {
        let const_owner = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nconst note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let const_field = "struct Meter{\nconst count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        for src in [const_owner, const_field] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f3_偽造struct欄位更新來源身分必在稽核拒絕() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let mut forged = out;
        let field_event = forged.events.iter_mut().find(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(..)
        )).expect("原 P16 真欄位寫入事件");
        if let PrivateScalarCallEvent::CopyStructIntFieldAssigned(id, _, _) = field_event {
            *id = DeclarationId(0);
        }
        assert!(!forged.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f1_f3_main結構欄位作為真函式實參依動態值求出() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nvar note:Archive = Archive(count=8)\nreturn plus(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f1_f3_main結構欄位函式初始化賦值與返回有序執行() {
        let src = "struct Archive{\nvar count:int = 5\n}\nfn plus(x:int)->int{\nreturn x+2\n}\nfn main()->int{\nvar note:Archive = Archive()\nvar result:int = 0\nresult = plus(note.count)\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f1_f3_父子同名結構幀中讀欄位恢復父值不串位() {
        let src = "struct Archive{\nvar count:int = 1\n}\nfn leaf(x:int)->int{\nreturn x+1\n}\nfn mid()->int{\nvar note:Archive = Archive(count=9)\nreturn leaf(note.count)\n}\nfn main()->int{\nvar note:Archive = Archive(count=3)\nreturn mid()+leaf(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f1_f3_被呼叫者溢位先退出後清理main真結構() {
        let src = "struct Archive{\nvar count:int = 9223372036854775807\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nvar note:Archive = Archive()\nreturn plus(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    // V1／F4：原 Checker 的具名純值來源必須按 DeclarationId
    // 真正讀取動態 frame，而不是以初始字面值假造欄位。
    // V1／F4：只有原 Checker 接受的 Copyable int/bool 真欄位才能
    // 作另一結構的具名值；建構完成後不可偷偷保留來源別名。
    #[test]
    fn g18_v1_f4_具名int欄位真值快照不受來源後續更新污染() {
        let source = "struct Source{\nvar count:int = 3\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\na.count = 7\nvar b:Target = Target(count=a.count)\na.count = 9\nreturn b.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool來源欄位真快照決定後續分支() {
        let source = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar a:Flag = Flag()\na.ready = true\nvar b:Flag = Flag(ready=a.ready)\na.ready = false\nif(b.ready){\nreturn 5\n}\nreturn 2\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名雙欄位由另一真結構讀int_bool混合參數() {
        let source = "struct Source{\nvar count:int = 3\nvar ready:bool = true\n}\nstruct Target{\nvar count:int = 0\nvar ready:bool = false\n}\nfn route(flag:bool)->int{\nvar a:Source = Source()\na.count = 9\na.ready = flag\nvar b:Target = Target(count=a.count,ready=a.ready)\nif(b.ready){\nreturn b.count\n}\nreturn 1\n}\nfn main()->int{\nreturn route(true)+route(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 4);
    }

    #[test]
    fn g18_v1_f4_具名欄位來源型別不合由原Checker先拒絕() {
        let source = "struct Source{\nvar ready:bool = true\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\nvar b:Target = Target(count=a.ready)\nreturn b.count\n}\nfn main()->int{return route()}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(source),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_具名真欄位來源身分竄改執行前拒絕() {
        let source = "struct Source{\nvar count:int = 3\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\na.count = 7\nvar b:Target = Target(count=a.count)\na.count = 9\nreturn b.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
        let Some(PrivateScalarLocalStep::DeclareCopyStruct { plan: named, .. }) =
            plan.root_local_steps.iter_mut().find(|step| match step {
                PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                    plan.named.iter().any(|(name, _)| name == "count"),
                _ => false,
            }) else { panic!("已驗證真欄位來源工作應存在"); };
        named.named[0].1 = PrivateScalarCopyStructSeed::IntField {
            declaration: DeclarationId(0),
            owner: "a".to_owned(),
            field: "count".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F4：原 Checker 的具名 int 加減保留當下真局部與真結構欄位，
    // 交由已驗證 P16 建構求值；動態溢位不得留下半個資源結構。
    // V1／F1/F3/F4：具名 Copyable int 的真函式 Call 必須經
    // 同一顯式工作堆疊執行，所有 RHS 完成後 P16 才能原子建構。
    // V1／F2×F4：具名建構中的真普通函式可借用／移動 str，
    // 但 ResourceId／BorrowTicket 一律先完成原函式退出才允許 P16 建構。
    // V1／F3×F4：同一 Checker 已編譯 Call 不得因 for/while 反向邊
    // 被提前執行；每輪完整 P16 作用域、借用票據獨立反向歸還。
    #[test]
    fn g18_v1_f3_f4_具名函式for三輪建構並continue逐輪反序離域() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar sum:int = 0\nfor i in 0..3{\nvar note:Meter = Meter(count=plus(i))\nsum = sum+note.count\ncontinue\n}\nreturn sum\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f3_f4_具名函式while真回邊只執行動態首輪() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar value:int = 0\nvar active:bool = true\nwhile(active){\nvar note:Meter = Meter(count=plus(value))\nvalue = note.count\nactive = false\n}\nreturn value\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_f4_空for具名callee與P16資源均零效果() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nfor i in 0..0{\nvar note:Meter = Meter(count=plus(i))\n}\nreturn 5\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f3_f4_具名函式for每輪真借同一str但P16按輪獨立() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(item:str,x:int)->int{\nreturn x+1\n}\nfn run()->int{\nconst owner:str = \"逐輪借用🪼\"\nvar sum:int = 0\nfor i in 0..2{\nvar note:Meter = Meter(count=plus(owner,i))\nsum = sum+note.count\n}\nreturn sum\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f2_f4_具名呼叫借str真票據先歸還後建構且資源唯一() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst owner:str = \"原資料🪼\"\nvar note:Meter = Meter(count=inspect(owner,4))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).expect("callee 回票");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("具名函式完畢後才建構");
        assert!(released < constructed);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_具名呼叫真str_move先轉唯一資源且返回後建構() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn consume(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nvar owner:str = \"整筆轉交🪼\"\nvar note:Meter = Meter(count=consume(owner.move(),3))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).expect("唯一原資源在 callee 離域");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("P16 真建構");
        assert!(released < constructed);
    }

    #[test]
    fn g18_v1_f2_f4_雙str具名函式依序借還且兩個resourceid不混() {
        let src = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst a:str = \"甲🪼\"\nconst b:str = \"乙🪼\"\nvar note:Pair = Pair(first=inspect(a,2),second=inspect(b,6))\nreturn note.first+note.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_第一具名借用已成功後第二呼叫溢位仍不得建構() {
        let src = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn overflow(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nconst owner:str = \"保留🪼\"\nvar maximum:int = 9223372036854775807\nvar note:Pair = Pair(first=inspect(owner,3),second=overflow(maximum))\nreturn note.first+note.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_具名已取得str借用但子呼叫預算耗盡必先還票() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst owner:str = \"框架預算🪼\"\nvar note:Meter = Meter(count=inspect(owner,2))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 300 }
        ).unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }

    #[test]
    fn g18_v1_f4_具名int真呼叫先返回後才建構P16唯一值() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn add(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar base:int = 8\nvar note:Meter = Meter(count=add(base))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["build", "add"]);
    }

    #[test]
    fn g18_v1_f4_兩個具名int函式RHS依原書寫順序進出() {
        let source = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn double(x:int)->int{\nreturn x+x\n}\nfn build()->int{\nvar pair:Pair = Pair(first=plus(2),second=double(4))\nreturn pair.first+pair.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["build", "plus", "double"]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名加減內兩層函式真工作圖與結果() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar note:Meter = Meter(count=plus(plus(3))+2)\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名函式發生溢位時零結構提交且真逐幀回滾() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar max:int = 9223372036854775807\nvar note:Meter = Meter(count=plus(max))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_未選控制葉具名函式零執行零結構() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build(flag:bool)->int{\nif(flag){\nvar note:Meter = Meter(count=plus(7))\nreturn note.count\n}\nreturn 4\n}\nfn main()->int{\nreturn build(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f4_具名函式來源計畫遭竄改不得偷偷替換回傳() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn add(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar base:int = 8\nvar note:Meter = Meter(count=add(base))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "count")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("真具名函式來源計畫");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::IntCall(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Int(9));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_bool括號預設真值由P16建構後讀取() {
        let src = "struct Meter{\nvar enabled:bool = (true)\n}\nfn run()->int{\nvar note:Meter = Meter()\nif(note.enabled){\nreturn 10\n}\nreturn 1\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_bool雙重括號預設與具名覆蓋不錯置() {
        let src = "struct Meter{\nvar enabled:bool = ((false))\n}\nfn run()->int{\nvar first:Meter = Meter()\nvar second:Meter = Meter(enabled=true)\nif(first.enabled){\nreturn 1\n}\nif(second.enabled){\nreturn 6\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_bool括號預設錯型別原Checker拒絕() {
        let src = "struct Meter{\nvar enabled:bool = (1)\n}\nfn run()->int{\nvar note:Meter = Meter()\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_bool括號預設計畫變造不許偽造欄位() {
        let src = "struct Meter{\nvar enabled:bool = (true)\n}\nfn run()->int{\nvar note:Meter = Meter()\nif(note.enabled){\nreturn 4\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        let default = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.fields.iter_mut().find(|(field, _, _, _)| field == "enabled")
                    .map(|(_, _, seed, _)| seed),
            _ => None,
        }).expect("原 P16 bool 預設來源");
        assert!(matches!(default, Some(ApprovedSeed::Bool(true))));
        *default = Some(ApprovedSeed::Bool(false));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名bool括號讀取更新後局部真值() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar flag:bool = false\nflag = true\nvar note:Meter = Meter(enabled=(flag))\nif(note.enabled){\nreturn 9\n}\nreturn 1\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名bool雙括號跨結構讀取即時值() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar seed:Meter = Meter(enabled=false)\nseed.enabled = true\nvar note:Meter = Meter(enabled=((seed.enabled)))\nif(note.enabled){\nreturn 5\n}\nreturn 2\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool括號非法型別原Checker先拒絕() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar note:Meter = Meter(enabled=(1))\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_具名bool括號來源計畫變造執行前拒絕() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar flag:bool = true\nvar note:Meter = Meter(enabled=(flag))\nif(note.enabled){\nreturn 3\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "enabled")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("原 Checker 真具名 bool Group");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::BoolGrouped(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Bool(true));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名int多層算式讀更新後局部而非宣告時常值() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar source:int = 4\nsource = source+3\nvar note:Meter = Meter(count=(source+2))\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名int算式跨真正可寫結構欄位加減() {
        let source = "struct Meter{\nvar count:int = 1\n}\nfn compute()->int{\nvar left:Meter = Meter(count=6)\nleft.count = 9\nvar right:Meter = Meter(count=(left.count-2)+5)\nreturn right.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool與動態int算式一併交付真P16快照() {
        let source = "struct Meter{\nvar count:int = 0\nvar enabled:bool = false\n}\nfn choose(flag:bool)->int{\nvar source:int = 3\nvar note:Meter = Meter(count=source+4,enabled=flag)\nsource = 99\nif(note.enabled){\nreturn note.count\n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名動態int溢位不得建立半個結構或遺留資源() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar maximum:int = 9223372036854775807\nvar note:Meter = Meter(count=maximum+1)\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名int算式原Checker拒絕錯型別且動態計畫防偽() {
        let invalid = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar ready:bool = true\nvar note:Meter = Meter(count=ready+1)\nreturn note.count\n}\nfn main()->int{return compute()}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar source:int = 4\nsource = source+3\nvar note:Meter = Meter(count=(source+2))\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "count")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("應找到原 Checker 具名動態算式");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::IntArithmetic(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Int(7));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名int取最後動態局部值不重播宣告值() {
        let source = "struct Meter{\nvar count:int = 1\n}\nfn route()->int{\nvar n:int = 5\nn = 9\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名bool來源由真函式參數動態分支決定() {
        let source = "struct Meter{\nvar count:int = 3\nvar ready:bool = false\n}\nfn route(flag:bool)->int{\nvar note:Meter = Meter(ready=flag)\nif(note.ready){\nreturn note.count+5\n}\nreturn note.count\n}\nfn main()->int{\nreturn route(true)+route(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名原局部遮蔽後使用本層Checker身分值() {
        let source = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:int = 3\nif(true){\nvar n:int = 11\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nreturn n\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f4_具名int_bool錯型別與未初始化原Checker先拒絕() {
        let wrong_type = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:bool = true\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{return route()}\n";
        let uninitialized = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:int\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{return route()}\n";
        for src in [wrong_type, uninitialized] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f4_具名局部來源DeclarationId篡改執行前拒絕() {
        let src = "struct Meter{\nvar count:int = 1\n}\nfn route()->int{\nvar n:int = 5\nn = 9\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let Some(PrivateScalarLocalStep::DeclareCopyStruct { plan: named, .. }) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::DeclareCopyStruct { .. })) else {
            panic!("原 Checker 真具名 struct 工作不存在");
        };
        named.named[0].1 = PrivateScalarCopyStructSeed::IntBound(DeclarationId(0));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_真P16結構int欄位讀取而非硬編預設值() {
        let src = "struct Meter{\nvar count:int = 7\nvar flag:bool = true\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.count+3\n}\nfn main()->int{\nreturn getter()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名覆蓋保留真欄位來源不執行錯誤預設() {
        let src = "struct Meter{\nvar count:int = 9223372036854775807+1\n}\nfn getter()->int{\nvar note:Meter = Meter(count=9)\nreturn note.count\n}\nfn main()->int{\nreturn getter()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f4_同名Copyable結構跨兩次函式呼叫不錯用欄位值() {
        let src = "struct Meter{\nvar count:int = 1\n}\nfn first()->int{\nvar note:Meter = Meter(count=4)\nreturn note.count\n}\nfn second()->int{\nvar note:Meter = Meter(count=9)\nreturn note.count\n}\nfn main()->int{\nreturn first()+second()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(13));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_欄位錯誤型別或不存在原Checker即拒絕() {
        let invalid_type = "struct Meter{\nvar flag:bool = true\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.flag\n}\nfn main()->int{\nreturn getter()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let missing = "struct Meter{\nvar count:int = 7\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.unknown\n}\nfn main()->int{\nreturn getter()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(missing),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_真欄位編譯身分遭篡改執行前必拒絕() {
        let src = "struct Meter{\nvar count:int = 7\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.count\n}\nfn main()->int{\nreturn getter()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
        plan.return_expr = PrivateScalarReturnExpr::CopyStructIntField {
            declaration: DeclarationId(0),
            owner: "note".to_owned(), field: "count".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F4：原 Checker 核准 const str 結構的真正 UTF-8 所有權，
    // 同一 OwnedStrRuntime 帳中按建構先後配置 ResourceId。
    #[test]
    fn g18_v1_f4_同層先conststr結構後字面str按真建立反序離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"結構\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nvar tail:str = \"晚建🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層雙str結構先於普通str三資源真反序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst first:str = \"甲\"\nconst second:str = \"乙\"\nvar count:int = 5\n}\nfn run()->int{\nvar item:Archive = Archive()\nconst tail:str = \"丙\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層後建varstr兩次字面覆寫仍先於結構離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 9\nconst label:str = \"先建\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nvar tail:str = \"舊\"\ntail = \"新🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(2), R::Released(3), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層後建str借用先還票再釋放字串與結構() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 4\nconst label:str = \"較早\"\n}\nfn receive(s:str)->int{\nreturn 6\n}\nfn run()->int{\nvar item:Archive = Archive()\nconst tail:str = \"借用者\"\nreturn receive(tail)+item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).expect("BorrowTicket 真歸還");
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).expect("真資源退出");
        assert!(returned < released);
    }

    #[test]
    fn g18_v1_f4_真if局部先結構後str兩資源按所選葉釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 3\nconst label:str = \"分支\"\n}\nfn run()->int{\nif(true){\nvar item:Archive = Archive()\nvar tail:str = \"內層🪼\"\n}\nreturn 8\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    // V1-F4：原 Checker 准許的具名 var str 字面值，與不可寫
    // str 欄位使用 P16 同一真 OwnedUtf8 資源帳，不放行預設 var str。
    // V1-F4：所有替換只能在原 Checker 合法的 var str 欄位上，
    // 真共享帳先建新 OwnedUtf8 再釋放舊 ResourceId。
    #[test]
    fn g18_v1_f4_str欄位寫入事件重複插入不得假裝二次提交() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let i = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).expect("真欄位寫入事件");
        let duplicate = out.events[i].clone();
        out.events.insert(i, duplicate);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_str欄位寫入確認遺失不許冒充正常退出() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        ));
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_str欄位寫入確認早於新resource綁定必拒絕() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let i = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).expect("真寫入確認");
        let j = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { resource_id: 2, .. }
        )).expect("新資源真綁定");
        out.events.swap(i, j);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_for兩輪重用原DeclarationId仍逐輪驗真str欄位() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar total:int = 0\nfor i in 0..2{\nvar item:Archive = Archive(body=\"初值\")\nitem.body = \"覆寫🪼\"\ntotal = total+item.count\n}\nreturn total\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
            R::Created(3), R::Created(4), R::Released(3), R::Released(4),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名varstr欄位真字面覆寫建立新id釋放舊id() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 7\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"舊🪼\")\nitem.body = \"新🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(_, field, 2)
                if field == "body"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_同一varstr欄位雙次更新每id只真釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 3\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"初始\")\nitem.body = \"第一次\"\nitem.body = \"第二次🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1),
            R::Created(3), R::Released(2), R::Released(3),
        ]);
    }

    #[test]
    fn g18_v1_f4_if真分支更新varstr假分支零新資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 5\n}\nfn run(flag:bool)->int{\nvar item:Archive = Archive(body=\"原值\")\nif(flag){\nitem.body = \"選中🪼\"\n}\nreturn item.count\n}\nfn main()->int{\nreturn run(false)+run(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Created(3), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_conststr欄位寫入由原Checker拒絕不得產生資源() {
        let src = "struct Archive{\nconst label:str = \"不可寫\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive()\nitem.label = \"非法\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(PreparedPrivateScalarCall::prepare(src).is_err());
    }

    #[test]
    fn g18_v1_f4_竄改str欄位寫入後resourceid不得通過退出稽核() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        let id = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::CopyStructStrFieldAssigned(_, _, resource_id) =>
                Some(resource_id),
            _ => None,
        }).expect("原 Checker 真 str 欄位寫入身分");
        *id = 999;
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_具名varstr欄位以真utf8覆蓋未執行預設() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\nvar data:str = \"未選預設\"\n}\nfn run()->int{\nvar item:Archive = Archive(data=\"具名🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::CopyStructResourceOwnerBound { field, resource_id: 1, .. }
                if field == "data"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_雙具名varstr欄位由同一真帳逆序離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar first:str = \"預設甲\"\nvar second:str = \"預設乙\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive(first=\"甲🪼\",second=\"乙🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr和conststr交錯仍共用唯一resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst header:str = \"固定\"\nvar body:str = \"舊值\"\nvar count:int = 3\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"新值\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr結構完成前先等待int函式真返回() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"未選\"\nvar count:int = 1\n}\nfn increase(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"動態🪼\",count=increase(5))\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(6)
        )).expect("具名 int 的真函式已返回");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("P16 真建構");
        assert!(returned < constructed);
    }

    #[test]
    fn g18_v1_f4_具名varstr建立後溢位仍須單一真資源回收() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"回滾🪼\")\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
    }

    // V1/F4：可寫預設 str 與具名欄位共用唯一 P16 Shared Ledger；
    // 明確驗證原 Checker 全欄位依序求值、唯一 ResourceId 與故障退回。
    #[test]
    fn g18_v1_f4_兩個未覆蓋varstr預設依宣告次序建立並逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"甲🪼\"\nvar second:str = \"乙🪼\"\nvar count:int = 12\n}\nfn read()->int{\nvar item:Parcel = Parcel()\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_具名str覆蓋預設與另一varstr預設只建立兩筆真資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"舊預設不能執行\"\nvar second:str = \"合法預設\"\nvar count:int = 5\n}\nfn read()->int{\nvar item:Parcel = Parcel(first=\"具名替代🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_建立預設varstr後真整數溢位必回滾唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"回滾🪼\"\nvar count:int = 1\n}\nfn fail()->int{\nvar item:Parcel = Parcel()\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn fail()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_真預設str建構函式編譯計畫被刪除須零執行拒絕() {
        let src = "struct Parcel{\nvar first:str = \"合法🪼\"\nvar count:int = 3\n}\nfn read()->int{\nvar item:Parcel = Parcel()\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        plan.root_local_steps.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_未具名varstr預設由p16真建立與釋放() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar body:str = \"未授權預設\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);

    }

    #[test]
    fn g18_v1_f4_conststr具名int函式真返回後才建立資源結構() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 0\nconst label:str = \"真資源🪼\"\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar item:Archive = Archive(count=plus(6))\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let inner_return = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(7)
        )).expect("真 plus 返回");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("真 P16 結構建立");
        assert!(inner_return < constructed);
    }

    #[test]
    fn g18_v1_f4_單一結構兩個conststr欄位唯一id各自逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { .. }
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_結構欄位resourceid竄改不能通過同一退出稽核() {
        let src = "struct Archive{\nvar count:int = 3\nconst label:str = \"真身分\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let link = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::CopyStructResourceOwnerBound { resource_id, .. } =>
                Some(resource_id),
            _ => None,
        }).expect("真結構字串 ResourceId 連結");
        *link = 999;
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_conststr結構共用唯一真resourceid與逆序退出() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"繁體🪼\"\n}\nfn idle()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_兩個conststr結構唯一id按完成反序清理() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"預設\"\n}\nfn idle()->int{\nvar first:Archive = Archive(label=\"甲🪼\")\nvar second:Archive = Archive(label=\"乙🪼\")\nreturn first.count+second.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Created(2),
            crate::owned_str_runtime::ResourceEvent::Released(2),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_conststr結構建立後整數溢位應釋放真資源() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"錯誤清理🪼\"\n}\nfn idle()->int{\nvar item:Archive = Archive()\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_未選分支conststr結構零建立零清理() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"不可達🪼\"\n}\nfn chosen()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn chosen()\n}\nreturn 4\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "chosen"
        )));
    }

    #[test]
    fn g18_v1_f4_跨兩函式conststr結構各用獨立resourceid() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"重入🪼\"\n}\nfn first()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn second()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn first()+second()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
            crate::owned_str_runtime::ResourceEvent::Created(2),
            crate::owned_str_runtime::ResourceEvent::Released(2),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層普通str與含str結構交錯安全拒絕且資源回收() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"安全限制\"\n}\nfn idle()->int{\nvar title:str = \"先建立🪼\"\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StateMismatch));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f4_原v0可達純int結構局部真建構並反序離域() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["route", "idle"]);
    }

    #[test]
    fn g18_v1_f4_未選分支純結構與資源均零建構() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_f4_純結構建構後int溢位逐框架離域不留資源() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 9223372036854775807+1\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_可達普通函式真建立預設str不虛構雙帳() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"保留🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn main()->int{\nreturn idle()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);

    }

    #[test]
    fn g18_r01_main可達預設str結構函式真正執行() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn main()->int{\nreturn idle()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(91));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_r01_main不可達來源圖被手工加入未支援函式須拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nif(true){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.callee_name = "idle".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R01：已核准的普通函式亦可能在所有葉分支已返回後
    // 保留不可達尾端呼叫；不能把它當成必要動態子圖。
    #[test]
    fn g18_r01_普通函式兩葉return後不可達結構函式不污染圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 5\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.callee_name, "route");
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        assert!(matches!(prepared.return_expr, PrivateScalarReturnExpr::Unreachable));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_r03_多層普通函式選false真返回不能進不可達tail() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn first()->int{\nreturn 3\n}\nfn second()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn first()\n}else{\nreturn second()\n}\nreturn idle()\n}\nfn bridge(flag:bool)->int{\nreturn route(flag)\n}\nfn main()->int{\nreturn bridge(false)\n}\n";
        let plan = checked(src);
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["bridge", "route", "second"]);
    }

    #[test]
    fn g18_r04_普通函式提前return真str借用不可達結構零資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn fallback()->int{\nreturn 8\n}\nfn route()->int{\nif(true){\nvar owner:str = \"內🪼\"\nreturn inspect(owner,5)\n}else{\nreturn fallback()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route()\n}\n";
        let plan = checked(src);
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    // V1／F4：原 Checker 合法的零迭代本體不在執行圖內。
    // 只可以略過真正不可達的 work；有正迭代的結構局部仍須拒絕。
    // V1/F3：跨三層普通函式的子呼叫集合來自同一份
    // 原 Checker 已簽發的工作計畫，零迭代中的合法構造函式
    // 不可回流成新的子函式依賴。
    #[test]
    fn g18_v1_三層真工作計畫只收錄動態可能呼叫的函式() {
        let source = "struct Archive{\nvar count:int = 7\n}\nfn hidden()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn leaf()->int{\nreturn 9\n}\nfn mid()->int{\nfor i in 0..0{\nvar phantom:int = hidden()\n}\nreturn leaf()\n}\nfn top()->int{\nreturn mid()\n}\nfn main()->int{\nreturn top()\n}\n";
        let plan = checked(source);
        assert_eq!(plan.callee_name, "top");
        let names: Vec<_> = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect();
        assert_eq!(names, ["leaf", "mid"]);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "hidden"
        )));
    }

    #[test]
    fn g18_v1_普通函式工作計畫須保留兩個動態可能的分支呼叫() {
        let source = "fn left()->int{\nreturn 6\n}\nfn right()->int{\nreturn 8\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn 0\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let plan = checked(source);
        let names: Vec<_> = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect();
        assert_eq!(names, ["left", "right"]);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "left"
        )));
    }

    #[test]
    fn g18_v1_工作計畫閉包竄改巢狀被呼叫者不得通過來源稽核() {
        let source = "fn leaf()->int{\nreturn 9\n}\nfn mid()->int{\nreturn leaf()\n}\nfn top()->int{\nreturn mid()\n}\nfn main()->int{\nreturn top()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        plan.nested_callees.pop();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_原checker空for本體結構函式不污染可達呼叫閉包() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..0{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_原checker_false_while結構函式不污染可達呼叫閉包() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nwhile(false){\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_正迭代for真呼叫預設str結構函式() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..1{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_空迭代閉包被竄改新增可達目標須零執行拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..0{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
        prepared.nested_callees.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F3：同一原 Parser／Checker 已證的 Return 須切斷後續工作圖。
    // 不可達函式不得占用 frame／OwnedStr ResourceId，也不可把合法
    // 但未接通的結構執行路徑誤判成此選中分支的必要功能。

    // V1／F3：root main 也須由原 Checker 已編譯的控制工作圖
    // 列出可能的子呼叫；不再以原 AST 遞迴掃描推定執行可達。

    #[test]
    fn g18_v1_main空for內可達性被原Checker排除不污染呼叫圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn main()->int{\nfor i in 0..0{\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|entry| entry.name.as_str()).collect::<Vec<_>>();
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main_false_while內結構函式零建構且返回真目標() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 11\n}\nfn main()->int{\nwhile(false){\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|entry| entry.name.as_str()).collect::<Vec<_>>();
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main真迭代可達結構函式實際進框架() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn main()->int{\nfor i in 0..1{\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_頂層普通函式全葉返回後局部不可達不再錯編() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 8\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nconst skipped:int = idle()\nreturn 0\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.return_expr, PrivateScalarReturnExpr::Unreachable));
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.resource_events.is_empty());
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main全葉返回後局部不可達不再污染首入口() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 8\n}\nfn main()->int{\nif(false){\nreturn left()\n}else{\nreturn right()\n}\nconst skipped:int = idle()\nreturn 0\n}\n";
        let plan = checked(src);
        assert_eq!(plan.callee_name, "left");
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.resource_events.is_empty());
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_頂層非全葉返回仍真執行可達str結構局部() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nconst live:int = idle()\nreturn live\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_區塊已返回後結構函式不得進入工作圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\nvar skipped:int = idle()\n}\nreturn 2\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect::<Vec<_>>();
        for target in ["good"] {
            assert!(names.contains(&target), "已核准的可達呼叫不能遺失：{target}");
        }
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_巢狀全分支返回後不可達建構不得污染兄弟返回() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 5\n}\nfn right()->int{\nreturn 8\n}\nfn fallback()->int{\nreturn 12\n}\nfn route(flag:bool)->int{\nif(flag){\nif(false){\nreturn left()\n}else{\nreturn right()\n}\nvar skipped:int = idle()\n}\nreturn fallback()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect::<Vec<_>>();
        for target in ["left","right","fallback"] {
            assert!(names.contains(&target), "已核准的可達呼叫不能遺失：{target}");
        }
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_未知條件編譯所有可達str來源而只執行選中邊() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 6\n}\nfn route(flag:bool)->int{\nif(flag){\nvar value:int = idle()\nreturn value\n}\nreturn good()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, []);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));

    }

    #[test]
    fn g18_r01_原v0可達預設str結構函式真返回() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_r01_普通函式不可達閉包計畫被增添目標須在執行前拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 5\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.nested_callees.clear();
        assert!(matches!(plan.execute(), Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R05：真回傳與故障退出均按原顯式工作堆疊反向還票。
    // 平衡但錯序的事件不能被當成 Checker 已核准執行證據。
    #[test]
    fn g18_r05_雙str實參真借用必反序歸還且不提前釋放資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst left:str = \"甲🪼\"\nconst right:str = \"乙🪼\"\nreturn inspect(left,right,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn g18_r05_雙str正常返回交換兩張票據歸還次序必拒絕() {
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst left:str = \"甲\"\nconst right:str = \"乙\"\nreturn inspect(left,right,5)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let returns: Vec<_> = out.events.iter().enumerate()
            .filter_map(|(index, event)| matches!(
                event, PrivateScalarCallEvent::LoanReturned(_)
            ).then_some(index)).collect();
        assert_eq!(returns.len(), 2);
        out.events.swap(returns[0], returns[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_雙str實參後第三引數溢位反序歸還再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"第一\"\nconst second:str = \"第二\"\nreturn inspect(first,second,1+9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::UnwindStarted)));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn g18_r05_失敗回滾雙str平衡但錯序還票仍必拒絕() {
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"第一\"\nconst second:str = \"第二\"\nreturn inspect(first,second,1+9223372036854775807)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let returns: Vec<_> = out.events.iter().enumerate()
            .filter_map(|(index, event)| matches!(
                event, PrivateScalarCallEvent::LoanReturned(_)
            ).then_some(index)).collect();
        assert_eq!(returns.len(), 2);
        out.events.swap(returns[0], returns[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_跨子呼叫前後的兩張借用票據各按真返回逆序歸還() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+1\n}\nfn parent(s:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,3))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        let entered: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["leaf", "parent"]);
    }

    // G18 R05：覆蓋 BorrowSafety 的獨立子集原子交易，
    // 不擴增 F00 已凍結的 borrow_safety::tests:: 原 24 項來源。
    #[test]
    fn g18_r05_巢狀函式子集歸票不能提前解除外層活躍票據() {
        let mut tracker = BorrowSafety::new();
        let outer = DeclarationId(8041);
        let inner = DeclarationId(8042);
        let first = tracker.acquire(outer, BorrowKind::SharedRead).unwrap();
        let second = tracker.acquire(inner, BorrowKind::SharedRead).unwrap();
        tracker.finish_selected_verified(&[second], &[second.fact()]).unwrap();
        assert_eq!(
            tracker.check_direct_access(outer),
            Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed),
        );
        assert_eq!(tracker.check_direct_access(inner), Ok(()));
        assert_eq!(
            tracker.check_ticket_access(first, BorrowKind::SharedRead), Ok(outer),
        );
        tracker.finish_scope_verified(&[first], &[first.fact()]).unwrap();
        assert_eq!(tracker.check_direct_access(outer), Ok(()));
    }

    #[test]
    fn g18_r05_子集歸票靜態來源失配必原子拒絕保留所有票據() {
        let mut tracker = BorrowSafety::new();
        let first = tracker.acquire(
            DeclarationId(8061), BorrowKind::SharedRead,
        ).unwrap();
        let second = tracker.acquire(
            DeclarationId(8062), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(
            tracker.finish_selected_verified(&[second], &[first.fact()]),
            Err(crate::borrow_safety::BorrowSafetyError::SnapshotMismatch),
        );
        assert_eq!(
            tracker.finish_selected_verified(&[second], &[]),
            Err(crate::borrow_safety::BorrowSafetyError::SnapshotMismatch),
        );
        for ticket in [first, second] {
            assert_eq!(
                tracker.check_ticket_access(ticket, BorrowKind::SharedRead),
                Ok(ticket.fact().source),
            );
        }
        tracker.finish_scope_verified(
            &[first, second], &[first.fact(), second.fact()],
        ).unwrap();
    }

    #[test]
    fn g18_r05_子集重複票據不得部分歸還其餘合法借用() {
        let mut tracker = BorrowSafety::new();
        let original = tracker.acquire(
            DeclarationId(8081), BorrowKind::SharedRead,
        ).unwrap();
        let remaining = tracker.acquire(
            DeclarationId(8082), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(
            tracker.finish_selected_verified(
                &[original, original], &[original.fact(), original.fact()],
            ),
            Err(crate::borrow_safety::BorrowSafetyError::InvalidRelease),
        );
        assert_eq!(
            tracker.check_ticket_access(original, BorrowKind::SharedRead),
            Ok(original.fact().source),
        );
        assert_eq!(
            tracker.check_ticket_access(remaining, BorrowKind::SharedRead),
            Ok(remaining.fact().source),
        );
        tracker.finish_scope_verified(
            &[original, remaining], &[original.fact(), remaining.fact()],
        ).unwrap();
    }



    // R05：外層未 Enter 的借用票據不能先於正在退出的內層
    // 借用／資源返還。只有真正已進入的 frame 按由內向外順序卸責。
    #[test]
    fn g18_r05_外層待呼叫借用與內層函式溢位須先還內票再還外票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(s:str,n:int)->int{\nvar local:str = \"子框架🪼\"\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外票\"\nconst inner:str = \"內票\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "parent"
        )));
    }

    #[test]
    fn g18_r05_內層真move溢位先清理子資源外層待借用才歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外票\"\nvar inner:str = \"移動🪼\"\nreturn parent(outer,leaf(inner.move(),1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let child_exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false,
            } if name == "leaf"
        )).unwrap();
        let loan_returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(child_exit < loan_returned);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "parent"
        )));
    }

    #[test]
    fn g18_r05_巢狀回滾交換內外票據歸還順序須拒絕() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateScalarCallEvent::LoanReturned(_)).then_some(index)
        ).collect::<Vec<_>>();
        assert_eq!(returned.len(), 2);
        out.events.swap(returned[0], returned[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_不在任何框架內的偽造待歸票不得掩蓋溢位() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        // 偽造歸票即使附在正式 UnwindStarted 後也沒有真簽發來源，
        // 應由共用票據及來源雙帳同時拒絕。
        let token = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).expect("必有真內層票據");
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        out.events.insert(unwind + 1, PrivateScalarCallEvent::LoanReturned(token));
        assert!(!out.has_balanced_frame_trace());
    }


}


/// R06：跨各個原 Checker 已核准執行器的**真動態結果**整合回歸。
/// 不假設各路徑已共享同一 runtime，亦不把 int 離域等同 OwnedStr Release。
#[cfg(test)]
mod g18_r06_cross_runtime_tests {
    use super::*;
    use crate::owned_str_runtime::{self, ResourceEvent};

    fn scalar() -> &'static str {
        "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n"
    }

    fn borrow() -> &'static str {
        "fn measure(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源🪼\"\nreturn measure(owner)\n}\n"
    }

    fn move_borrow() -> &'static str {
        "fn consume(owned:str,watch:str)->int{\nvar local:str = owned.move()\nreturn 11\n}\nfn main()->int{\nvar source:str = \"唯一🪼\"\nconst observer:str = \"借用\"\nreturn consume(source.move(),observer)\n}\n"
    }

    fn method() -> &'static str {
        "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n"
    }

    fn recursive() -> &'static str {
        "fn descend(flag:bool,payload:str)->int{\nvar local:str = \"層🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root)\n}\n"
    }

    #[test]
    fn g18_r06_混合布林整數與真借用成功返回各守門須同時成立() {
        let pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(6));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap().execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert_eq!(borrowed.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
        // int/bool ScopeEvent 不會也不能憑空創造 OwnedStr 事件。
        assert!(!pure.events.is_empty());
    }

    #[test]
    fn g18_r06_同_call_整筆移動借用和同步_self_不混淆資源責任() {
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert_eq!(mixed.result, Ok(11));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert_eq!(mixed.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 2);
        let own = PreparedPrivateSelfMethod::prepare(method())
            .unwrap().execute().unwrap();
        assert_eq!(own.result, Ok(7));
        assert!(own.has_balanced_self_borrow_trace());
        assert!(own.events.is_empty());
        assert!(mixed.ticket_returned && own.ticket_returned);
    }

    #[test]
    fn g18_r06_合法遞迴真資源與普通借用在預算耗盡時皆不得假返回() {
        let plan = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap();
        let exhausted = plan.execute(PrivateRecursiveStrBudget {
            max_call_frames: 2, max_steps: 80,
        }).unwrap();
        assert_eq!(exhausted.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(exhausted.has_balanced_resource_frame_trace());
        assert!(exhausted.loans_returned);
        assert!(owned_str_runtime::event_trace_balanced(&exhausted.events));
        assert!(exhausted.frames.iter().all(|event| !matches!(
            event, PrivateRecursiveFrameEvent::Exited { returned: true, .. }
        )));
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(borrowed.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(!borrowed.call_exited);
    }

    #[test]
    fn g18_r06_不同執行器的正常失敗不得丟失已建資源或複製假返回() {
        let pure = PreparedPrivateScalarCall::prepare(
            "fn inc(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n"
        ).unwrap().execute().unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|ev| matches!(
            ev, PrivateScalarCallEvent::Returned(_)
        )));
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(!method.method_exited);
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow()).unwrap()
            .execute_with_fault(Some(3)).unwrap();
        assert_eq!(mixed.result, Err(PrivateMixedOwnershipRunError::InjectedFailure));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert!(mixed.ticket_returned);
    }

    #[test]
    fn g18_r06_跨模型偽造離域資源票據任一項必在交付稽核失敗() {
        let mut pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap()
            .execute().unwrap();
        pure.events.push(PrivateScalarCallEvent::LocalScopeEnded(DeclarationId(0)));
        assert!(!pure.has_balanced_frame_trace());
        let mut borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        borrowed.ticket_returned = false;
        assert!(!borrowed.has_balanced_borrow_resource_trace());
        let mut mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        mixed.events.clear();
        assert!(!mixed.has_balanced_mixed_resource_trace());
        let mut self_method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute().unwrap();
        self_method.events.extend([
            ResourceEvent::Created(77), ResourceEvent::Released(77),
        ]);
        assert!(!self_method.has_balanced_self_borrow_trace());
        let mut nested = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap()
            .execute(PrivateRecursiveStrBudget {
                max_call_frames: 4, max_steps: 100,
            }).unwrap();
        assert!(nested.has_balanced_resource_frame_trace());
        nested.loans_returned = false;
        assert!(!nested.has_balanced_resource_frame_trace());
    }
    #[test]
    fn g18_r06_真_while_for_局部_算術與_str_借用雙執行器結果皆須平衡() {
        let src = "fn calculate(n:int)->int{\nvar result:int = n\nvar active:bool = true\nwhile(active){\nconst step:int = 2\nresult = result+step\nactive = false\n}\nfor i in 1..3{\nconst copied:int = i\nresult = result+copied\n}\nreturn result\n}\nfn main()->int{\nreturn calculate(3)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(8));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 6);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert_eq!(borrowed.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r06_巢狀同名遮蔽與同步_self_不同資源帳簿均有真退出() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvar value:int = 9\nif(true){\nvalue = value+2\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        let ids = pure.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LocalInitialized(id, _) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.ticket_returned);
    }

    #[test]
    fn g18_r06_迴圈_break_先釋放局部與遞迴資源耗盡各守真出口() {
        let src = "fn choose(n:int)->int{\nfor i in 1..8{\nconst local:int = i\nbreak\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 2);
        let recursive = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap()
            .execute(PrivateRecursiveStrBudget {
                max_call_frames: 2, max_steps: 90,
            }).unwrap();
        assert_eq!(recursive.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(recursive.has_balanced_resource_frame_trace());
        assert!(recursive.loans_returned);
    }

    #[test]
    fn g18_r06_區塊算術溢位與_str_借用中途故障均禁止虛構返回() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst increment:int = 9223372036854775807\nvalue = value+increment\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(borrowed.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(!borrowed.call_exited);
    }

    #[test]
    fn g18_r06_純值區塊離域偽造及_move_borrow_資源偽造皆失敗() {
        let src = "fn choose(n:int)->int{\nif(true){\nconst local:int = 3\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let mut pure = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        pure.events.insert(0, PrivateScalarCallEvent::LocalScopeEnded(DeclarationId(0)));
        assert!(!pure.has_balanced_frame_trace());
        let mut mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert!(mixed.has_balanced_mixed_resource_trace());
        mixed.ticket_returned = false;
        assert!(!mixed.has_balanced_mixed_resource_trace());
    }

    #[test]
    fn g18_r06_合法迴圈中提前返回清理_另一條資源模型仍歸還票據() {
        let src = "fn choose(n:int)->int{\nfor i in 1..3{\nconst temporary:int = i\nreturn temporary+n\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(5));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )).count(), 1);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(borrowed.ticket_returned);
    }

    #[test]
    fn g18_r06_巢狀_while_預算耗盡與_self_故障無假成功返回() {
        let src = "fn choose(n:int)->int{\nwhile(true){\nconst temporary:int = 2\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute_with_budget(PrivateScalarExecutionBudget {
                max_call_frames: 5, max_steps: 32,
            }).unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(!method.method_exited);
    }

    #[test]
    fn g18_r06_靜態逃逸拒絕不得取代_true_runtime_安全交叉驗證() {
        let invalid = "fn choose(n:int)->int{\nfor i in 0..3{\nconst hidden:int = i\n}\nreturn hidden\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn choose(n:int)->int{\nfor i in 0..3{\nconst hidden:int = i\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(valid).unwrap()
            .execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert_eq!(mixed.result, Ok(11));
        assert!(mixed.has_balanced_mixed_resource_trace());
    }

    #[test]
    fn g18_r06_同一函式圖兩張_const_str_共享票據來源相同但序號不同() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst shared:str = \"共享🪼\"\nreturn inspect(shared,shared,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let tickets = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(tickets.len(), 2);
        assert_ne!(tickets[0], tickets[1]);
        assert_eq!(tickets[0].fact(), tickets[1].fact());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r06_兩個獨立_var_str_同一次真呼叫各有獨占票據() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn inspect(first,second,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let facts = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::BorrowParameterBound(_, fact) = event {
                Some(*fact)
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].kind, BorrowKind::ExclusiveWrite);
        assert_eq!(facts[1].kind, BorrowKind::ExclusiveWrite);
        assert_ne!(facts[0].source, facts[1].source);
    }

    #[test]
    fn g18_r06_同一真函式堆疊先_move_後借另一_source_資源不混淆() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar moved:str = \"搬移\"\nconst observed:str = \"觀察\"\nconst destination:str = moved.move()\nreturn inspect(observed,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).unwrap();
        assert!(returned < released);
    }

    #[test]
    fn g18_r06_for_重複同一外層_str_真借用逐輪歸還() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn count(n:int)->int{\nconst source:str = \"外\"\nvar result:int = n\nfor i in 0..2{\nresult = inspect(source,result)\n}\nreturn result\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r06_未選中的_if_真借用不簽發任何動態票據() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn choose(n:int)->int{\nconst source:str = \"外\"\nif(false){\nreturn inspect(source,n)\n}\nreturn n\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
                | PrivateScalarCallEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r05_r06_偽造借用函式已退出後才歸還票據必拒絕() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"外\"\nreturn inspect(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, .. }
                if name == "inspect"
        )).unwrap();
        let loan = out.events.remove(returned);
        let new_exit = if returned < exit { exit - 1 } else { exit };
        out.events.insert(new_exit + 1, loan);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r06_同一_while_重借外層_const_str_逐次歸還才進下一輪() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nconst source:str = \"重借🪼\"\nvar value:int = n\nvar active:bool = true\nwhile(active){\nvalue = observe(source,value)\nactive = false\n}\nreturn observe(source,value)\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acquired = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count();
        let returned = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count();
        assert_eq!((acquired, returned), (2, 2));
    }

    #[test]
    fn g18_r06_真借用已取得但子呼叫框架預算耗盡仍全數原子歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nconst source:str = \"呼叫預算\"\nreturn observe(source,n)\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute_with_budget(
            PrivateScalarExecutionBudget {max_call_frames: 1, max_steps: 100}
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        let acq = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count();
        let ret = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count();
        assert_eq!(acq, ret);
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r06_借用後第二_int_實參溢位_真票據歸還先於_resourceid_釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nconst source:str = \"溢位\"\nreturn observe(source,n+9223372036854775807)\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let loan = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(loan < released);
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
    }

    #[test]
    fn g18_r05_r06_重複歸還同一票據必由同序帳簿拒絕() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn observe(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let loan = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let duplicate = out.events[loan].clone();
        out.events.insert(loan + 1, duplicate);
        assert!(!out.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r05_r06_每張真借用票據必連至實際_utf8_resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nreturn inspect(first,second,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let linked = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanResourceLinked(ticket, id) =>
                Some((*ticket, *id)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(linked.len(), 2);
        assert_ne!(linked[0].0, linked[1].0);
        assert_eq!([linked[0].1, linked[1].1], [1, 2]);
        for (ticket, id) in linked {
            let acquired = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanAcquired(found) if found == &ticket
            )).unwrap();
            let joined = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanResourceLinked(found, linked)
                    if found == &ticket && *linked == id
            )).unwrap();
            let returned = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanReturned(found) if found == &ticket
            )).unwrap();
            assert!(acquired < joined && joined < returned);
        }
    }

    #[test]
    fn g18_r05_r06_偽造借用未歸還便先釋放真來源資源必拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn inspect(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        // 將唯一真正 Released 提前至票據取得後、被呼叫者進入前。
        // ResourceEvent 序列本身仍平衡，舊稽核只看兩套獨立帳簿會漏判。
        let release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        let event = out.events.remove(release);
        let linked = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).unwrap();
        out.events.insert(linked + 1, event);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_票據資源身分偽造重複綁定或缺失均拒絕() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn inspect(source,4)\n}\n";
        let original = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(original.has_balanced_frame_trace());
        let position = original.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, _)
        )).unwrap();

        let mut wrong_id = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(_, resource) =
            &mut wrong_id.events[position] {
            *resource = 999;
        } else { panic!("缺少原票據連結"); }
        assert!(!wrong_id.has_balanced_frame_trace());

        let mut duplicate = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        let duplicate_link = duplicate.events[position].clone();
        duplicate.events.insert(position + 1, duplicate_link);
        assert!(!duplicate.has_balanced_frame_trace());

        let mut missing = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        missing.events.remove(position);
        assert!(!missing.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r05_r06_同框架兩個存活_resourceid_借用不得錯綁他人() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"甲\"\nconst second:str = \"乙\"\nreturn inspect(first,4)\n}\n";
        let baseline = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(baseline.result, Ok(4));
        assert!(baseline.has_balanced_frame_trace());
        assert_eq!(baseline.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        // 真來源是 first / ResourceId 1。假裝其連結是同樣活躍的
        // second / ResourceId 2：僅驗證 ResourceId 存在仍會誤放行。
        let mut wrong = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        let linked = wrong.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(_, resource) =
            &mut wrong.events[linked] {
            *resource = 2;
        } else { panic!("缺少真連結"); }
        assert!(!wrong.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(wrong),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_str_覆寫後再借新_resourceid_票據須對應最新擁有者() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nvar owner:str = \"舊\"\nowner = \"新\"\nreturn inspect(owner,n)\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 2)
        )).count(), 1);
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(returned < release);
    }

    #[test]
    fn g18_r06_真_move_覆寫後借目標仍指同一原始_resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nvar source:str = \"甲\"\nvar target:str = \"乙\"\ntarget = source.move()\nreturn inspect(target,n)\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).count(), 1);
    }

    #[test]
    fn g18_r05_r06_偽造_str_覆寫後仍由舊資源認領必拒絕() {
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = \"新\"\nreturn n\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceOwnerBound(_, resource) = &mut out.events[index] {
            *resource = 1; // 舊資源早已 Released，不能被同一 DeclarationId 復活
        } else { panic!("缺少真擁有者轉移事件"); }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r06_str_覆寫後整數溢位仍釋放目前唯一擁有者() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = \"新\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::Returned(_)
        )));
    }


    #[test]
    fn g18_r06_三層真函式同一_resourceid_連續跨框架_move_僅一次清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn mid(item:str,n:int)->int{\nreturn leaf(item.move(),n)\n}\nfn main()->int{\nvar owner:str = \"三層\"\nreturn mid(owner.move(),5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_r06_函式_for_每輪新_str_跨函式_move_返回後即時釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nvar result:int = n\nfor i in 0..2{\nvar owner:str = \"逐輪\"\nresult = consume(owner.move(),result)\n}\nreturn result\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r06_同次呼叫先借再_move_不同真來源_各有正確出口() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(borrowed:str,moved:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst read:str = \"讀\"\nvar transfer:str = \"移\"\nreturn inspect(read,transfer.move(),2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(returned < released);
    }

    #[test]
    fn g18_r06_移動實參已準備但_callbudget_耗盡不得假轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn parent(n:int)->int{\nvar owner:str = \"預算\"\nreturn leaf(owner.move(),n)\n}\nfn main()->int{\nreturn parent(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 }
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
    }

    #[test]
    fn g18_r05_r06_偽造跨框架_move_目標宣告身分須拒絕() {
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned.move(),2)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let event = out.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { target, .. } = event {
            *target = DeclarationId(0);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_原_checker_VB14_借用別名缺乏重借證明不得進真子呼叫() {
        let invalid = [
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nreturn leaf(item,n)\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nreturn leaf(item,n)\n}\nfn main()->int{\nvar owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nvar value:int = n\nfor i in 0..2{\nvalue = leaf(item,value)\n}\nreturn value\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
        ];
        for src in invalid {
            let Err(PrivateScalarCallPrepareError::Check(diagnostic)) =
                PreparedPrivateScalarCall::prepare(src) else {
                panic!("VB14 必須由正式 Checker 拒絕，不能交由 runtime 猜測");
            };
            assert!(format!("{diagnostic:?}").contains("VB14"));
        }
    }

    #[test]
    fn g18_r06_原_checker_准許真持有者_move_給子函式不受_VB14_錯誤擴張() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn bridge(n:int)->int{\nvar owned:str = \"合法\"\nreturn leaf(owned.move(),n)\n}\nfn main()->int{\nreturn bridge(5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_r06_存活借用票據期間偽造同源跨框架_move_仍須拒絕() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"不可移\"\nreturn observe(owner,3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let ticket = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )).unwrap();
        let source = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(ticket.fact().source),
            _ => None,
        }).unwrap();
        // 借用函式已進入，來源 LoanResourceLinked 仍有效；
        // 即使 forged transfer 指向存在的 ResourceId，也不可認領。
        out.events.insert(ticket + 1, PrivateScalarCallEvent::ResourceTransferred {
            source, target: DeclarationId(0), resource_id: 1,
        });
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_同一真_call_雙_resourceid_整筆_move_依原參數順序交付逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙🪼\"\nreturn consume(first.move(),second.move(),5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let handoffs = out.events.iter().filter_map(|ev| match ev {
            PrivateScalarCallEvent::ResourceTransferred { resource_id, .. } =>
                Some(*resource_id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(handoffs, [1, 2]);
    }

    #[test]
    fn g18_r06_雙_move_後續_int_實參動態溢位不可提前移交任何資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn consume(first.move(),second.move(),1+9223372036854775807)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|ev| matches!(ev,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
    }

    #[test]
    fn g18_r06_雙_resourceid_跨三層函式_每層重新綁定但零複製零洩漏() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn bridge(a:str,b:str,n:int)->int{\nreturn leaf(a.move(),b.move(),n)\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn bridge(first.move(),second.move(),2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Moved(1), R::Moved(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r06_同一來源重複兩次_move_必由_checker_阻止進入真框架() {
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nreturn consume(first.move(),first.move(),2)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_r06_成功函式未真正_return_不得提前歸還其入參票據() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let before = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::Returned(3)
        )).unwrap();
        let after = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(before < after);
        let premature = out.events.remove(after);
        out.events.insert(before, premature);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_函式失敗還入參票據必先留下真逐框架回滾起點() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nconst owner:str = \"失敗\"\nreturn inspect(owner,1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let unwind = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(unwind < returned);
        out.events.remove(unwind);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_偽造成功路徑開始回滾或失敗路徑重複開始均須拒絕() {
        let valid = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"真\"\nreturn inspect(owner,2)\n}\n";
        let mut completed = PreparedPrivateScalarCall::prepare(valid).unwrap()
            .execute().unwrap();
        assert!(completed.has_balanced_frame_trace());
        completed.events.insert(0, PrivateScalarCallEvent::UnwindStarted);
        assert!(!completed.has_balanced_frame_trace());

        let fails = "fn inspect(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nconst owner:str = \"真\"\nreturn inspect(owner,1)\n}\n";
        let mut failed = PreparedPrivateScalarCall::prepare(fails).unwrap()
            .execute().unwrap();
        assert!(failed.has_balanced_frame_trace());
        let at = failed.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        failed.events.insert(at + 1, PrivateScalarCallEvent::UnwindStarted);
        assert!(!failed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_無借用的失敗回滾不可遺漏_unwindstarted() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn calculate(n:int)->int{\nvar owner:str = \"真資源\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn calculate(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let count = out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::UnwindStarted)
        ).count();
        assert_eq!(count, 1);
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        ));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_成功結果不能以未返回的函式退出事件偽造() {
        let src = "fn calculate(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn calculate(5)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::Returned(_)
        ));
        for event in &mut out.events {
            if let PrivateScalarCallEvent::FunctionExited { returned, .. } = event {
                *returned = false;
            }
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r05_r06_未進入子框架的借用失敗不能先於回滾起點還票() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nconst source:str = \"尚未進入\"\nreturn observe(source,n+9223372036854775807)\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(unwind < returned);
        // 保留所有資源事件及票據數量，只把還票移到正式失敗起點之前。
        let forged = out.events.remove(returned);
        out.events.insert(unwind, forged);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_已成功歸還的前序借用不受後續溢位回滾誤封鎖() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"先成功\"\nreturn observe(source,1)+9223372036854775807\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let success = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(1)
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        assert!(success < returned && returned < unwind);
    }


    #[test]
    fn g18_r05_r06_正式回滾後不得補造新的條件計算事件() {
        let src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        out.events.insert(at + 1, PrivateScalarCallEvent::ConditionalChecked(true));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_正式回滾後即使局部事件平衡也不能重新初始化() {
        let src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let fake = DeclarationId(0);
        out.events.insert(at + 1, PrivateScalarCallEvent::LocalInitialized(fake, 7));
        out.events.insert(at + 2, PrivateScalarCallEvent::LocalScopeEnded(fake));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r05_r06_普通函式與同步self真呼叫均須通過共用框架票據進退帳() {
        let scalar_src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"真正借用\"\nreturn observe(source,4)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(4));
        assert!(scalar.has_balanced_frame_trace());
        let self_src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(self_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r05_r06_普通函式與同步self錯誤均須真回滾且拒絕偽造退出() {
        let scalar_src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(scalar.has_balanced_frame_trace());
        scalar.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        ));
        assert!(!scalar.has_balanced_frame_trace());
        let self_src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let plan = PreparedPrivateSelfMethod::prepare(self_src).unwrap();
        let mut method = plan.execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        method.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        ));
        assert!(!method.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_r06_普通函式真借用票據不得早於返回或晚於離域() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"因果\"\nreturn observe(source,4)\n}\n";
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut early = plan.execute().unwrap();
        assert!(early.has_balanced_frame_trace());
        let position = early.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let event = early.events.remove(position);
        let returned = early.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).unwrap();
        early.events.insert(returned, event);
        assert!(!early.has_balanced_frame_trace());

        let mut late = plan.execute().unwrap();
        let position = late.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let event = late.events.remove(position);
        let exit = late.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: true, .. }
        )).unwrap();
        late.events.insert(exit + 1, event);
        assert!(!late.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r06_普通函式與同步self方法int局部均沿用原_checker_真正求值且離域() {
        let scalar_src = "fn calc(a:int)->int{\nconst local:int = a+4\nreturn local-2\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(7));
        assert!(scalar.has_balanced_frame_trace());

        let method_src = "struct Counter{\nvar count:int = 7\nfn calc(a:int)->int{\nconst local:int = a+4\nreturn local-2\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(5)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert_eq!(method.method_local_events.len(), 2);
        assert_eq!(method.method_local_slots_after_exit, 0);
        assert!(method.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r06_普通函式與同步self局部溢位均保留真失敗回滾與乾淨離域() {
        let scalar_src = "fn calc(a:int)->int{\nconst first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(scalar.has_balanced_frame_trace());
        let method_src = "struct Counter{\nvar count:int = 7\nfn calc(a:int)->int{\nconst first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(5)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.method_entered && !method.method_exited && method.ticket_returned);
        assert_eq!(method.method_local_events.len(), 2);
        assert_eq!(method.method_local_slots_after_exit, 0);
    }


    #[test]
    fn g18_r05_r06_真普通函式int局部與同步self共用離域帳阻擋假初始化錯序() {
        let src = "fn calculate(n:int)->int{\nconst temp:int = n+2\nreturn temp\n}\nfn main()->int{\nreturn calculate(4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, _)
        )).unwrap();
        let forged = out.events.remove(at);
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )).unwrap();
        out.events.insert(returned + 1, forged);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_普通函式bool局部亦納入共用真退出帳的局部來源責任() {
        let src = "fn choose(n:int)->int{\nconst flag:bool = false\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEnded(_)
        )).unwrap();
        out.events.remove(at);
        assert!(!out.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r06_同源int普通函式與方法局部只有方法真checker前綴可交付() {
        let scalar_src = "fn calc(n:int)->int{\nconst local:int = n+1\nreturn local\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(4));
        assert!(scalar.has_balanced_frame_trace());

        let self_src = "struct Sample{\nvar x:int = 1\nfn calc(n:int)->int{\nconst local:int = n+1\nreturn local\n}\n}\nfn main()->int{\nconst value:Sample = Sample()\nreturn value.calc(3)\n}\n";
        let plan = PreparedPrivateSelfMethod::prepare(self_src).unwrap();
        let mut method = plan.execute().unwrap();
        assert_eq!(method.result, Ok(4));
        assert!(method.has_balanced_self_borrow_trace());
        // 同一成功值與相同事件數量仍不能替換原 Checker
        // 簽發的方法內局部 DeclarationId 來源。
        method.expected_method_local_ids = vec![DeclarationId(0)];
        assert!(!method.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r06_普通函式與同步self方法均以真var_int更新通過共用離域帳() {
        let ordinary = "fn calc(a:int,b:int)->int{\nvar amount:int = a\namount = amount+b\nreturn amount\n}\nfn main()->int{\nreturn calc(3,4)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(ordinary)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(7));
        assert!(scalar.has_balanced_frame_trace());
        assert!(scalar.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 7)
        )));
        let method_src = "struct Sample{\nvar x:int = 1\nfn calc(a:int,b:int)->int{\nvar amount:int = a\namount = amount+b\nreturn amount\n}\n}\nfn main()->int{\nconst value:Sample = Sample()\nreturn value.calc(3,4)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalAssigned(_)
        )));
    }


}
