impl PreparedPrivateSelfMethod {
    fn matches_source(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false; };
        self.entry_bindings == fresh.entry_bindings
            && self.entry_frames == fresh.entry_frames
            && self.constructors == fresh.constructors
            && self.preceding_sites == fresh.preceding_sites
            && self.original == fresh.original
            // T3：此入口已用唯一原 Lexer／Parser／Checker 建立 fresh；
            // 必須逐筆核對**完整** Token，不只查已使用的 Name/Type
            // 角色。否則 method bool Group 原括號可能在 prepare 後
            // 遭私有層更改卻仍因值葉相同而放行。
            && self.verified.tokens == fresh.verified.tokens
            && self.struct_name == fresh.struct_name
            && self.owner == fresh.owner && self.owner_id == fresh.owner_id
            && self.owner_mutable == fresh.owner_mutable
            && self.field == fresh.field && self.field_value == fresh.field_value
            && self.field_immutable == fresh.field_immutable
            && self.additional_copy_fields == fresh.additional_copy_fields
            && self.write_value == fresh.write_value
            && self.write_parameter == fresh.write_parameter
            && self.write_checked_int == fresh.write_checked_int
            && self.direct_method_bool_write == fresh.direct_method_bool_write
            && self.return_calculation == fresh.return_calculation
            && self.method_int_locals == fresh.method_int_locals
            && self.method_helpers == fresh.method_helpers
            && self.method_branch == fresh.method_branch
            && self.method_branch_else == fresh.method_branch_else
            && self.method_branch_else_write == fresh.method_branch_else_write
            && self.method_branch_else_field_return == fresh.method_branch_else_field_return
            && self.method_branch_else_calculation == fresh.method_branch_else_calculation
            && self.method_branch_write == fresh.method_branch_write
            && self.method_branch_bool_write == fresh.method_branch_bool_write
            && self.method_branch_return == fresh.method_branch_return
            && self.method_nested_branch == fresh.method_nested_branch
            && self.method_nested_branch_parameter == fresh.method_nested_branch_parameter
            && self.method_nested_self_bool_field == fresh.method_nested_self_bool_field
            && self.method_nested_else_calculation == fresh.method_nested_else_calculation
            && self.method_nested_true_write == fresh.method_nested_true_write
            && self.method_nested_else_write == fresh.method_nested_else_write
            && self.method_branch_int_plans == fresh.method_branch_int_plans
            && self.method_branch_parameter == fresh.method_branch_parameter
            && self.method_branch_self_bool_field == fresh.method_branch_self_bool_field
            && self.method_int_assignments == fresh.method_int_assignments
            && self.method == fresh.method
            && self.awaited_method == fresh.awaited_method
            && self.method_span == fresh.method_span
            && self.call_span == fresh.call_span && self.loan == fresh.loan
            && self.argument_binding == fresh.argument_binding
            && self.additional_argument_bindings == fresh.additional_argument_bindings
            && self.method_bool_arguments == fresh.method_bool_arguments
            && self.method_bool_sources == fresh.method_bool_sources
            && self.main_bool_locals == fresh.main_bool_locals
            && self.main_bool_sources == fresh.main_bool_sources
            && self.main_bool_assignments == fresh.main_bool_assignments
            && self.main_scalar_steps == fresh.main_scalar_steps
            && self.main_int_assignments == fresh.main_int_assignments
            && self.main_owner_field_assignments == fresh.main_owner_field_assignments
            && self.main_owner_bool_field_assignments
                == fresh.main_owner_bool_field_assignments
            && self.main_int_locals == fresh.main_int_locals
            && self.main_init_plans == fresh.main_init_plans
            && self.argument_sources == fresh.argument_sources
            && self.argument_plans == fresh.argument_plans
            && self.verified.plan.method_safety == fresh.verified.plan.method_safety
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
    }

    /// T3／V7：佇列在 suspend/cancel/shutdown/prepare 之前與
    /// 真方法 execute() 共用**完全相同**的原 Checker 來源門檻。
    /// 準備期合法不代表封存計畫與 Lexer/ScalarHIR 後續未遭變造；
    /// 此唯讀預檢不建立 BorrowTicket、資源或函式事件。
    fn checked_source_still_intact(&self) -> bool {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return false;
        };
        self.original.as_ref() == self.verified.source.bytes.as_slice()
            // 原 await Token、方法標頭／Receiver／Call 及深層
            // Name/Type 必須來自同一份封存 Lexer，不能
            // 只以重建原字串的 fresh Checker 取代 Token 稽核。
            && verified_private_source_tokens(
                text, &self.verified.tokens,
                &self.verified.structs, &self.verified.functions,
            )
            && self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            )
            && self.matches_source(text)
    }

    fn execute(&self) -> Result<PrivateSelfMethodOutcome, PrivateSelfMethodPrepareError> {
        self.execute_with_fault(false)
    }

    /// 僅 Rust 測試可注入方法存活期錯誤，不新增 voiles 例外語意。
    fn execute_with_fault(&self, fail_inside_method: bool)
        -> Result<PrivateSelfMethodOutcome, PrivateSelfMethodPrepareError> {
        if !self.checked_source_still_intact() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        // 原 main 第一筆是真 owner 構造：在任何可能溢位的
        // Copyable 主函式局部／helper 之前真建立同一執行期結構。
        // FieldAssign 只透過這個已構造 owner 真提交，不把最後
        // 欄位值當成 constructor 預設值或在方法借票後補寫。
        use crate::mixed_struct_runtime::{
            MixedStructRuntime, MixedFieldSpec,
        };
        let mut slots = PrivateSelfCallerSlots::default();
        slots.events.push(PrivateSharedCallExitEvent::FunctionEntered);
        let mut waiting = Vec::new();
        for (frame, bindings) in self.entry_frames.iter().zip(&self.entry_bindings) {
            let values = private_self_bind_entry_values(bindings, &slots.integers)?;
            waiting.push((*frame, std::mem::replace(&mut slots.integers, values),
                std::mem::take(&mut slots.declarations)));
            slots.events.push(PrivateSharedCallExitEvent::FunctionEntered);
            slots.declared_integers.clear();
            for (id, _) in bindings {
                slots.declarations.push(*id);
                slots.declared_integers.insert(*id);
                slots.events.push(PrivateSharedCallExitEvent::LocalInitialized(*id));
            }
        }
        let mut runtime = MixedStructRuntime::new();
        // P16 在真 main 首句按原 Checker 宣告順序建立全部
        // Copyable int／bool 欄位，沒有新增 OwnedStr 或資源事件。
        let mut owned_fields = Vec::new();
        for constructor in &self.constructors {
            let mut specs = Vec::with_capacity(constructor.fields.len());
            for (name, seed, immutable) in &constructor.fields {
                specs.push(MixedFieldSpec {
                    name, kind: seed.kind(), default: Some(seed.runtime()), immutable: *immutable,
                });
            }
            runtime.construct(&constructor.name, &specs, &[], None)
                .map_err(PrivateSelfMethodPrepareError::Cleanup)?;
            for (name, id) in runtime.field_resource_ids(&constructor.name)
                .map_err(PrivateSelfMethodPrepareError::Cleanup)? {
                owned_fields.push((constructor.id, name, id));
            }
        }
        let mut tracker = crate::borrow_safety::BorrowSafety::new();
        let text = std::str::from_utf8(&self.original)
            .map_err(|_| PrivateSelfMethodPrepareError::SourceChanged)?;
        let mut preceding_calls = Vec::new();
        let mut completed = None;
        for binding in &self.preceding_sites {
            let site = PrivateSelfMethodSite::prepare_site(
                text, &self.verified, Some(binding.statement_index), Some(&runtime), &slots,
            )?;
            let outcome = site.execute_site(&mut runtime, &mut tracker, &mut slots, fail_inside_method)?;
            if outcome.origin.call_span != binding.call_span || outcome.origin.owner != binding.receiver {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
            match outcome.result {
                Ok(value) => {
                    if !slots.declared_integers.insert(binding.declaration)
                        || slots.integers.insert(binding.declaration, value).is_some() {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    slots.declarations.push(binding.declaration);
                    slots.events.push(PrivateSharedCallExitEvent::LocalInitialized(binding.declaration));
                }
                Err(_) => { completed = Some(outcome); break; }
            }
            preceding_calls.push(outcome);
        }
        let mut outcome = match completed {
            Some(failed) => failed,
            None if preceding_calls.is_empty() =>
                self.site.execute_site(&mut runtime, &mut tracker, &mut slots, fail_inside_method)?,
            None => {
                let site = PrivateSelfMethodSite::prepare_site(
                    text, &self.verified, None, Some(&runtime), &slots,
                )?;
                site.execute_site(&mut runtime, &mut tracker, &mut slots, fail_inside_method)?
            }
        };
        outcome.expected_preceding_spans = self.preceding_sites.iter().take(preceding_calls.len())
            .map(|binding| (binding.receiver, binding.call_span)).collect();
        outcome.preceding_calls = preceding_calls;
        use PrivateSharedCallExitEvent as E;
        if outcome.result.is_ok() {
            slots.events.push(E::Returned);
        } else if !slots.events.contains(&E::UnwindStarted) {
            slots.events.push(E::UnwindStarted);
        }
        for id in slots.declarations.into_iter().rev() {
            let initialized = slots.integers.remove(&id).is_some() || slots.booleans.remove(&id).is_some();
            slots.events.push(if initialized { E::LocalScopeEnded(id) } else { E::LocalScopeEndedEmpty(id) });
        }
        // 結構仍在建立它的普通函式內；所有 self 票據已歸還，再逆序釋放欄位。
        for constructor in &self.constructors {
            let current = runtime.field_resource_ids(&constructor.name)
                .map_err(PrivateSelfMethodPrepareError::Cleanup)?;
            if !current.iter().map(|(name, id)| (name, id)).eq(
                owned_fields.iter().filter(|(owner, _, _)| *owner == constructor.id)
                    .map(|(_, name, id)| (name, id))) {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
        }
        outcome.expected_owned_fields = self.constructors.iter().flat_map(|owner|
            owner.fields.iter().filter_map(move |(name, seed, _)|
                matches!(seed, ApprovedSeed::Str(_)).then(|| (owner.id, name.clone())))
        ).collect();
        outcome.owned_fields = owned_fields;
        outcome.events = runtime.finish().map_err(PrivateSelfMethodPrepareError::Cleanup)?;
        slots.events.push(E::FunctionExited { returned: outcome.result.is_ok() });
        outcome.entry_frames = self.entry_frames.clone();
        while let Some((frame, mut values, declarations)) = waiting.pop() {
            if outcome.result.is_ok() {
                slots.events.push(E::Returned);
            }
            for id in declarations.into_iter().rev() {
                values.remove(&id).ok_or(PrivateSelfMethodPrepareError::SourceChanged)?;
                slots.events.push(E::LocalScopeEnded(id));
            }
            slots.events.push(E::FunctionExited { returned: outcome.result.is_ok() });
            outcome.exited_callers.push((frame, outcome.result.clone()));
        }
        outcome.caller_events = slots.events;
        outcome.caller_exited = true;
        outcome.main_local_slots_after_exit = slots.integers.len() + slots.booleans.len();

        if !outcome.has_balanced_self_borrow_trace() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        Ok(outcome)
    }
}

impl PrivateSelfMethodSite {
    fn execute_site(
        &self,
        runtime: &mut crate::mixed_struct_runtime::MixedStructRuntime,
        tracker: &mut crate::borrow_safety::BorrowSafety,
        caller: &mut PrivateSelfCallerSlots,
        fail_inside_method: bool,
    ) -> Result<PrivateSelfMethodOutcome, PrivateSelfMethodPrepareError> {
        use crate::mixed_struct_runtime::Observed;
        // 在建立任何 method self BorrowTicket 之前，以 main 真宣告順序
        // 將 Copyable int 局部求值寫入自己的執行期框架。具名複製
        // 必須實際讀取先前槽位，不把 prepare 中的常值當成唯一動態證據。
        if self.main_int_locals.len() != self.main_init_plans.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        let main_values = &mut caller.integers;
        let main_bool_values = &mut caller.booleans;
        if self.main_bool_sources.len() != self.main_bool_locals.len()
            || self.main_int_locals.len() != self.main_init_plans.len()
            || self.method_bool_sources.len() != self.method_bool_arguments.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        // R05／B4：任何真函式事件之前，先只讀驗證原 Parser
        // 步驟清冊恰好涵蓋每筆宣告與賦值，且同類索引依序遞增。
        // 來源篡改不得靠跳過某個 bool 宣告或重播 int helper 偷換帳。
        let (mut next_bool, mut next_int, mut next_assign, mut next_int_assign,
            mut next_owner_field, mut next_owner_bool_field) = (0, 0, 0, 0, 0, 0);
        for step in &self.main_scalar_steps {
            match step {
                PrivateSelfMainScalarStep::BoolDeclare(index)
                | PrivateSelfMainScalarStep::BoolDeclareEmpty(index)
                    if *index == next_bool => next_bool += 1,
                PrivateSelfMainScalarStep::IntDeclare(index)
                | PrivateSelfMainScalarStep::IntDeclareEmpty(index)
                    if *index == next_int => next_int += 1,
                PrivateSelfMainScalarStep::BoolAssign(index) if *index == next_assign =>
                    next_assign += 1,
                PrivateSelfMainScalarStep::IntAssign(index)
                    if *index == next_int_assign => next_int_assign += 1,
                PrivateSelfMainScalarStep::OwnerIntFieldAssign(index)
                    if *index == next_owner_field => next_owner_field += 1,
                PrivateSelfMainScalarStep::OwnerBoolFieldAssign(index)
                    if *index == next_owner_bool_field => next_owner_bool_field += 1,
                _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
            }
        }
        if next_bool != self.main_bool_locals.len()
            || next_int != self.main_int_locals.len()
            || next_assign != self.main_bool_assignments.len()
            || next_int_assign != self.main_int_assignments.len()
            || next_owner_field != self.main_owner_field_assignments.len()
            || next_owner_bool_field != self.main_owner_bool_field_assignments.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        let mut main_initialization_error = None;
        // 與一般函式共用真 Caller 動態進退帳。純 bool 宣告與
        // 重賦不產生資源事件，int helper 在真正原語句位置才 Enter；
        // 任何溢位中斷，後方 bool 更新與方法呼叫均不得執行。
        let mut exit_events = Vec::<PrivateSharedCallExitEvent>::new();
        let mut ordinary_calls_completed = 0usize;
        let mut ordinary_calls_entered = 0usize;
        let mut branch_checked = None::<bool>;
        let mut nested_branch_checked = None::<bool>;
        let mut branch_write_applied = false;
        let mut branch_bool_write_observed = None::<(String,bool,bool)>;
        let mut branch_else_write_applied = false;
        let mut nested_write_applied = false;
        let mut nested_else_write_applied = false;
        // 已宣告、已初始化是不同狀態：Empty 宣告不可偷建
        // false/0 值槽；唯一原 Checker Assignment 首次提交才可使用。
        let declared_bool = &mut caller.declared_booleans;
        let declared_int = &mut caller.declared_integers;
        let mut pending_bool = declared_bool.iter().copied().filter(|id|
            !main_bool_values.contains_key(id)).collect::<std::collections::BTreeSet<_>>();
        let mut pending_int = declared_int.iter().copied().filter(|id|
            !main_values.contains_key(id)).collect::<std::collections::BTreeSet<_>>();
        let mut copied_events = 0;
        for step in &self.main_scalar_steps {
            use PrivateSharedCallExitEvent as E;
            let mut local_event = None;
            match *step {
                PrivateSelfMainScalarStep::OwnerBoolFieldAssign(index) => {
                    let (field, planned, source) =
                        &self.main_owner_bool_field_assignments[index];
                    let actual = match source {
                        Some(PrivateSelfBoolArgumentSource::MainLocal(id)) =>
                            *main_bool_values.get(id).ok_or(
                                PrivateSelfMethodPrepareError::SourceChanged,
                            )?,
                        Some(PrivateSelfBoolArgumentSource::OwnerField {
                            declaration, field: rhs_field,
                        }) if *declaration == self.owner_id =>
                            match runtime.field(&self.owner, rhs_field) {
                                Ok(Observed::Bool(value)) => value,
                                _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                            },
                        Some(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                        None => *planned,
                    };
                    // 先求完整 RHS，再由真 P16 做唯一原子欄位更新；
                    // 不得在 prepare 寫值或提前建立方法 BorrowTicket。
                    if actual != *planned {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    if let Err(problem) = runtime.write_bool_field(
                        &self.owner, field, actual, self.owner_mutable,
                    ) {
                        main_initialization_error =
                            Some(PrivateSelfMethodRuntimeError::Resource(problem));
                        break;
                    }
                }
                PrivateSelfMainScalarStep::OwnerIntFieldAssign(index) => {
                    let (planned, expression) = &self.main_owner_field_assignments[index];
                    // 真 P16 讀取目前已構造／可能已被前序寫入的
                    // 同一 owner；不得用 constructor 或 prepare 常值。
                    let observed = match runtime.field(&self.owner, &self.field) {
                        Ok(Observed::Int(value)) => value,
                        _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    let bound = private_self_bind_owner_int_read(
                        expression, self.owner_id, &self.owner, &self.field,
                        observed,
                    ).ok_or(PrivateSelfMethodPrepareError::SourceChanged)?;
                    if !private_self_approved_int_call_arithmetic(
                        &bound, &self.method_helpers,
                    ) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    let mut calls = Vec::new();
                    bound.collect_calls(&mut calls);
                    let executed = if !calls.is_empty() {
                        private_self_execute_arithmetic_call(
                            &bound, &main_values, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    } else {
                        bound.evaluate(&main_values)
                    };
                    let actual = match executed {
                        Ok(value) => value,
                        Err(PrivateScalarCallRuntimeError::IntegerOverflow) => {
                            main_initialization_error =
                                Some(PrivateSelfMethodRuntimeError::IntegerOverflow);
                            break;
                        }
                        Err(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    if actual != *planned {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    if let Err(problem) = runtime.write_int_field(
                        &self.owner, &self.field, actual, self.owner_mutable,
                    ) {
                        main_initialization_error =
                            Some(PrivateSelfMethodRuntimeError::Resource(problem));
                        break;
                    }
                }
                PrivateSelfMainScalarStep::BoolDeclareEmpty(index) => {
                    let (_, id, _) = &self.main_bool_locals[index];
                    if self.main_bool_sources[index].is_some()
                        || !declared_bool.insert(*id) || !pending_bool.insert(*id)
                        || main_bool_values.contains_key(id) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    caller.declarations.push(*id);
                    local_event = Some(E::LocalDeclaredEmpty(*id));
                }
                PrivateSelfMainScalarStep::IntDeclareEmpty(index) => {
                    let (_, id, _, _) = &self.main_int_locals[index];
                    if !matches!(&self.main_init_plans[index],
                        PrivateScalarReturnExpr::Unreachable)
                        || !declared_int.insert(*id) || !pending_int.insert(*id)
                        || main_values.contains_key(id) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    caller.declarations.push(*id);
                    local_event = Some(E::LocalDeclaredEmpty(*id));
                }
                PrivateSelfMainScalarStep::BoolDeclare(index) => {
                    let (_, id, planned) = &self.main_bool_locals[index];
                    // 原 Parser 真 BoolDeclare 到達時，才能取 P16
                    // 已建構 owner.bool 的當下值；不能僅取 prepare 快照。
                    let actual = match &self.main_bool_sources[index] {
                        Some(PrivateSelfBoolArgumentSource::MainLocal(source)) =>
                            *main_bool_values.get(source)
                                .ok_or(PrivateSelfMethodPrepareError::SourceChanged)?,
                        Some(PrivateSelfBoolArgumentSource::OwnerField {
                            declaration, field,
                        }) if *declaration == self.owner_id =>
                            match runtime.field(&self.owner, field) {
                                Ok(Observed::Bool(value)) => value,
                                _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                            },
                        Some(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                        None => *planned,
                    };
                    if actual != *planned || !declared_bool.insert(*id)
                        || main_bool_values.insert(*id, actual).is_some() {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    caller.declarations.push(*id);
                    local_event = Some(E::LocalInitialized(*id));
                }
                PrivateSelfMainScalarStep::BoolAssign(index) => {
                    let (target, planned, source) = &self.main_bool_assignments[index];
                    if !declared_bool.contains(target) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    // 原 Parser 真 BoolAssign 逐筆動態求值並原子提交，
                    // 未初始化 bool 只有真正抵達本次賦值才有值槽。
                    let actual = match source {
                        Some(PrivateSelfBoolArgumentSource::MainLocal(id)) =>
                            *main_bool_values.get(id)
                                .ok_or(PrivateSelfMethodPrepareError::SourceChanged)?,
                        Some(PrivateSelfBoolArgumentSource::OwnerField {
                            declaration, field,
                        }) if *declaration == self.owner_id =>
                            match runtime.field(&self.owner, field) {
                                Ok(Observed::Bool(value)) => value,
                                _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                            },
                        Some(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                        None => *planned,
                    };
                    if actual != *planned {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    let had_value = main_bool_values.insert(*target, actual).is_some();
                    let was_pending = pending_bool.remove(target);
                    if had_value == was_pending {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    local_event = Some(if was_pending { E::LocalInitialized(*target) } else { E::LocalAssigned(*target) });
                }
                PrivateSelfMainScalarStep::IntAssign(index) => {
                    let (target, planned, initializer) =
                        &self.main_int_assignments[index];
                    if !declared_int.contains(target) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    // 僅於真正抵達 IntDeclare／IntAssign 位置，
                    // 從同一 P16 owner 讀目前 int，不信任 prepare 快照。
                    let observed = match runtime.field(&self.owner, &self.field) {
                        Ok(Observed::Int(value)) => value,
                        _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    let bound = private_self_bind_owner_int_read(
                        initializer, self.owner_id, &self.owner, &self.field, observed,
                    ).ok_or(PrivateSelfMethodPrepareError::SourceChanged)?;
                    if !private_self_approved_int_call_arithmetic(
                        &bound, &self.method_helpers,
                    ) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    let mut source_calls = Vec::new();
                    bound.collect_calls(&mut source_calls);
                    let executed = if !source_calls.is_empty() {
                        private_self_execute_arithmetic_call(
                            &bound, &main_values, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    } else {
                        bound.evaluate(&main_values)
                    };
                    let actual = match executed {
                        Ok(value) => value,
                        Err(PrivateScalarCallRuntimeError::IntegerOverflow) => {
                            main_initialization_error =
                                Some(PrivateSelfMethodRuntimeError::IntegerOverflow);
                            break;
                        }
                        Err(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    if actual != *planned {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    let had_value = main_values.insert(*target, actual).is_some();
                    let was_pending = pending_int.remove(target);
                    if had_value == was_pending {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    local_event = Some(if was_pending { E::LocalInitialized(*target) } else { E::LocalAssigned(*target) });
                }
                PrivateSelfMainScalarStep::IntDeclare(index) => {
                    let (_, id, planned, _) = &self.main_int_locals[index];
                    let initializer = &self.main_init_plans[index];
                    // 僅於真正抵達 IntDeclare／IntAssign 位置，
                    // 從同一 P16 owner 讀目前 int，不信任 prepare 快照。
                    let observed = match runtime.field(&self.owner, &self.field) {
                        Ok(Observed::Int(value)) => value,
                        _ => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    let bound = private_self_bind_owner_int_read(
                        initializer, self.owner_id, &self.owner, &self.field, observed,
                    ).ok_or(PrivateSelfMethodPrepareError::SourceChanged)?;
                    if !private_self_approved_int_call_arithmetic(
                        &bound, &self.method_helpers,
                    ) {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    let mut source_calls = Vec::new();
                    bound.collect_calls(&mut source_calls);
                    let executed = if !source_calls.is_empty() {
                        private_self_execute_arithmetic_call(
                            &bound, &main_values, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    } else {
                        bound.evaluate(&main_values)
                    };
                    let actual = match executed {
                        Ok(value) => value,
                        Err(PrivateScalarCallRuntimeError::IntegerOverflow) => {
                            main_initialization_error =
                                Some(PrivateSelfMethodRuntimeError::IntegerOverflow);
                            break;
                        }
                        Err(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
                    };
                    if actual != *planned || !declared_int.insert(*id)
                        || main_values.insert(*id, actual).is_some() {
                        return Err(PrivateSelfMethodPrepareError::SourceChanged);
                    }
                    caller.declarations.push(*id);
                    local_event = Some(E::LocalInitialized(*id));
                }
            }
            caller.events.extend_from_slice(&exit_events[copied_events..]);
            copied_events = exit_events.len();
            caller.events.extend(local_event);
        }
        if self.argument_sources.len()
            != usize::from(self.argument_binding.is_some())
                + self.additional_argument_bindings.len()
            || self.argument_plans.len() != self.argument_sources.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }

        let mut tickets = Vec::new();
        let mut facts = Vec::new();
        let mut method_entered = false;
        let mut method_exited = false;
        let mut direct_bool_write_observed = None::<(String, bool, bool)>;
        let mut await_start_loan_index = None;
        let mut method_local_events = Vec::<PrivateSelfMethodLocalEvent>::new();
        let mut initialized_method_locals = Vec::<DeclarationId>::new();
        let mut parameter_frame = std::collections::BTreeMap::<DeclarationId, i64>::new();
        let mut bool_parameter_frame = std::collections::BTreeMap::<DeclarationId, bool>::new();
        let main_locals_complete = main_initialization_error.is_none();
        let result = (|| -> Result<i64, PrivateSelfMethodRuntimeError> {
            // main 複合 int 初始化在任何方法 BorrowTicket 簽發前
            // 就可能受控溢位，尚未進入的方法不能留下假進／退出。
            if let Some(error) = main_initialization_error {
                return Err(error);
            }
            // 完成原 Checker 參數的真值框架綁定；選中來源返回時
            // 僅由同一 DeclarationId 讀取此框架，不重用 owner 值。
            for ((id, prepared), source) in self.method_bool_arguments.iter().copied()
                .zip(&self.method_bool_sources) {
                // bool 實參依原 Parser 位置在借入 self 之前求值；
                // 原 main 局部讀當下真值槽，CopyStructBoolField
                // 必從同一 P16 owner 讀現值，不得以 prepare 快照
                // 冒充、偷轉 int、或另簽 BorrowTicket。
                let value = match source {
                    Some(PrivateSelfBoolArgumentSource::MainLocal(source)) =>
                        *main_bool_values.get(source).ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?,
                    Some(PrivateSelfBoolArgumentSource::OwnerField {
                        declaration, field,
                    }) if *declaration == self.owner_id =>
                        match runtime.field(&self.owner, field) {
                            Ok(Observed::Bool(value)) => value,
                            _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                        },
                    Some(_) => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                    None => prepared,
                };
                if value != prepared
                    || bool_parameter_frame.insert(id, value).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
            }
            for ((id, prepared), expression) in self.argument_binding.into_iter()
                .chain(self.additional_argument_bindings.iter().copied())
                .zip(&self.argument_plans) {
                // 真執行於方法實參位置重新讀同一 P16 owner.int；
                // 不以 prepare 快照或方法借票後的值偽造來源。
                let observed = match runtime.field(&self.owner, &self.field) {
                    Ok(Observed::Int(value)) => value,
                    _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                };
                let bound = private_self_bind_owner_int_read(
                    expression, self.owner_id, &self.owner, &self.field,
                    observed,
                ).ok_or(PrivateSelfMethodRuntimeError::WrongReceiver)?;
                if !private_self_approved_int_call_arithmetic(
                    &bound, &self.method_helpers,
                ) {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                // 按原方法引數次序真正求值；尚未完成全部引數前
                // 不會取得 self BorrowTicket 或偽造進入方法框架。
                let mut source_calls = Vec::new();
                bound.collect_calls(&mut source_calls);
                let actual = if !source_calls.is_empty() {
                    // 在借入 self 前依原 Parser 左到右真正求完整
                    // 算式，內層 Call 各自真退出；任何失敗不得
                    // 提前進入同步方法或簽發 BorrowTicket。
                    private_self_execute_arithmetic_call(
                        &bound, &main_values, &self.method_helpers,
                        &mut exit_events, &mut ordinary_calls_entered,
                        &mut ordinary_calls_completed,
                    ).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                } else {
                    bound.evaluate(&main_values).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                };
                if actual != prepared || parameter_frame.insert(id, actual).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
            }
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            if self.awaited_method {
                // V7 E1：真 main 引數全部求值之後，直到直接
                // await 才開始簽發 self 票。方法本體不在 prepare
                // 或未到達 await 前執行，不引入背景任務。
                await_start_loan_index = Some(exit_events.len());
            }
            exit_events.push(PrivateSharedCallExitEvent::LoanAcquired(ticket));
            tickets.push(ticket);
            facts.push(self.loan);
            tracker.verify_static_snapshot(facts.iter().copied())
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            match tracker.check_direct_access(self.owner_id) {
                Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed) => {}
                Err(other) => return Err(PrivateSelfMethodRuntimeError::Ticket(other)),
                Ok(()) => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
            }
            method_entered = true;
            exit_events.push(PrivateSharedCallExitEvent::FunctionEntered);
            if fail_inside_method {
                return Err(PrivateSelfMethodRuntimeError::InjectedFailure);
            }
            // 由已取得的 self 真方法框架按宣告次序執行局部。
            // 不進入任何未達作用域的局部，也不誤用 main 的值槽。
            for (id, initializer) in &self.method_int_locals {
                let calculated = if private_self_resolve_nested_argument_call(
                    initializer, &self.method_helpers,
                ).is_some() {
                    // 僅在真 self 方法框架中，對原 Checker 已核准的
                    // 直接巢狀純值 helper 逐層 Enter/Exited；若任一
                    // 子引數或本體故障，絕不寫 LocalInitialized。
                    private_self_execute_nested_argument_call(
                        initializer, &parameter_frame, &self.method_helpers,
                        &mut exit_events, &mut ordinary_calls_entered,
                        &mut ordinary_calls_completed,
                    ).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                } else {
                    initializer.evaluate(&parameter_frame).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                };
                if parameter_frame.insert(*id, calculated).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                initialized_method_locals.push(*id);
                method_local_events.push(PrivateSelfMethodLocalEvent::Initialized(
                    *id, calculated,
                ));
                exit_events.push(PrivateSharedCallExitEvent::LocalInitialized(*id));
            }
            // 只有原 Checker 正式核准的 var int 局部允許依序更新；
            // 先完成 RHS 真值計算再提交目標，故障時保留舊值
            // 直到原有 ScopeEnded 逆序清理。
            for (id, expression) in &self.method_int_assignments {
                if !initialized_method_locals.contains(id) {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                let value = if private_self_resolve_nested_argument_call(
                    expression, &self.method_helpers,
                ).is_some() {
                    // 原 var 值在完整 RHS 及所有子函式返回成功前
                    // 不變；失敗無 LocalAssigned、仍由原方法
                    // 逆序 ScopeEnded 並歸還同一張 self 票據。
                    private_self_execute_nested_argument_call(
                        expression, &parameter_frame, &self.method_helpers,
                        &mut exit_events, &mut ordinary_calls_entered,
                        &mut ordinary_calls_completed,
                    ).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                } else {
                    expression.evaluate(&parameter_frame).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                };
                if parameter_frame.insert(*id, value).is_none() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                method_local_events.push(PrivateSelfMethodLocalEvent::Assigned(
                    *id, value,
                ));
                exit_events.push(PrivateSharedCallExitEvent::LocalAssigned(*id));
            }
            // 條件只於真 self 框架進入後，從原 Checker bool 參數
            // 獨立框架或 Lexer 常值取得；支線有寫入時不能先寫後選。
            let selected_branch = if let Some((condition, _)) = self.method_branch {
                // 與一般函式 BoolValue 走同一原 Checker bool 節點；
                // self 的欄位讀取必用**當前有效 BorrowTicket**，不能
                // 用 prepare 的初始欄位快照冒稱真控制流程。
                let observed = if let Some(field) = &self.method_branch_self_bool_field {
                    if tracker.check_ticket_access(ticket, BorrowKind::SharedRead)
                        .map_err(PrivateSelfMethodRuntimeError::Ticket)? != self.owner_id {
                        return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                    }
                    match runtime.field(&self.owner, field) {
                        Ok(Observed::Bool(value)) => value,
                        _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                    }
                } else if let Some(id) = self.method_branch_parameter {
                    *bool_parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver,
                    )?
                } else { condition };
                if observed != condition {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                branch_checked = Some(observed);
                Some(observed)
            } else { None };
            // 只有外層 true 已真正抵達時才觀測內層 Checker bool，
            // 並在寫入 self 前確定唯一被選中的巢狀葉。
            if selected_branch == Some(true) {
                if let Some((condition, _)) = self.method_nested_branch {
                    let observed = if let Some(field) = &self.method_nested_self_bool_field {
                        if tracker.check_ticket_access(ticket, BorrowKind::SharedRead)
                            .map_err(PrivateSelfMethodRuntimeError::Ticket)? != self.owner_id {
                            return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                        }
                        match runtime.field(&self.owner, field) {
                            Ok(Observed::Bool(value)) => value,
                            _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                        }
                    } else if let Some(id) = self.method_nested_branch_parameter {
                        *bool_parameter_frame.get(&id).ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?
                    } else { condition };
                    if observed != condition {
                        return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                    }
                    nested_branch_checked = Some(observed);
                }
            }
            // 原 Checker 只簽發一張方法入口票據；僅選中葉可真寫。
            // 無論常值／動態 Copyable int，原 bool 控制葉真正命中
            // 後，方可由同一方法參數 frame 取得其真 DeclId 值。
            let selected_branch_write = match selected_branch {
                Some(true) if self.method_nested_branch.is_some() =>
                    if nested_branch_checked == Some(true) {
                        self.method_nested_true_write
                    } else {
                        self.method_nested_else_write
                    },
                Some(true) => self.method_branch_write,
                Some(false) => self.method_branch_else_write,
                None => None,
            };
            let branch_write_value = match selected_branch_write.map(
                |(source, _)| source
            ) {
                Some(PrivateSelfApprovedBranchWrite::Literal(value)) => Some(value),
                Some(PrivateSelfApprovedBranchWrite::Parameter(id)) => Some(
                    *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver,
                    )?,
                ),
                Some(PrivateSelfApprovedBranchWrite::CheckedIntAt(span)) => {
                    // 僅被真 if/else/巢狀 bool 選中的 NodeSpan 才能
                    // 在唯一 self ExclusiveWrite Ticket 持有期間
                    // 真正讀取方法 int frame；未選葉零算術操作。
                    let plan = self.method_branch_int_plans.get(&span)
                        .ok_or(PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    // 僅真選中葉可執行其已核准的 Call；維持原
                    // ScalarHIR 左右求值順序、SharedCallExit 進退與
                    // 內層故障 Unwind，絕不預先啟動另一葉。
                    let mut calls = Vec::new();
                    plan.collect_calls(&mut calls);
                    let result = if calls.is_empty() {
                        plan.evaluate(&parameter_frame)
                    } else {
                        private_self_execute_arithmetic_call(
                            plan, &parameter_frame, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    };
                    Some(result.map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?)
                }
                None => None,
            };
            let direct_parameter_write = self.write_parameter.map(|id|
                parameter_frame.get(&id).copied()
                    .ok_or(PrivateSelfMethodRuntimeError::WrongReceiver)
            ).transpose()?;
            let checked_arithmetic_write = self.write_checked_int.as_ref()
                .map(|expression| {
                    // 原 Checker 封存的直接 RHS 真值，仍只在
                    // 自借用入口後執行；若存在已核准 Call，與
                    // 選中分支共用 SharedCallExit 真框架及溢位
                    // Unwind，寫入一定在全部子函式成功返回之後。
                    let mut calls = Vec::new();
                    expression.collect_calls(&mut calls);
                    if calls.is_empty() {
                        expression.evaluate(&parameter_frame)
                    } else {
                        private_self_execute_arithmetic_call(
                            expression, &parameter_frame, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    }
                })
                .transpose().map_err(|reason| match reason {
                    PrivateScalarCallRuntimeError::IntegerOverflow =>
                        PrivateSelfMethodRuntimeError::IntegerOverflow,
                    _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                })?;
            if [self.write_value.is_some(), direct_parameter_write.is_some(),
                checked_arithmetic_write.is_some()].into_iter().filter(|present|
                    *present
                ).count() > 1 {
                return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
            }
            // RHS 溢位時不提交 write_int_field，原錯誤退出
            // 仍從此唯一 self 方法框架 Unwind 並真歸還票據。
            let selected_write = self.write_value.or(direct_parameter_write)
                .or(checked_arithmetic_write).or(branch_write_value);
            if let Some(value) = selected_write {
                let authorized = tracker.check_ticket_access(
                    ticket, BorrowKind::ExclusiveWrite,
                ).map_err(PrivateSelfMethodRuntimeError::Ticket)?;
                if authorized != self.owner_id {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                runtime.write_int_field(
                    &self.owner, &self.field, value, self.owner_mutable,
                ).map_err(PrivateSelfMethodRuntimeError::Resource)?;
                if selected_branch == Some(true) && self.method_branch_write.is_some() {
                    branch_write_applied = true;
                }
                if selected_branch == Some(false)
                    && self.method_branch_else_write.is_some() {
                    branch_else_write_applied = true;
                }
                if selected_branch == Some(true) && nested_branch_checked == Some(true)
                    && self.method_nested_true_write.is_some() {
                    nested_write_applied = true;
                }
                if selected_branch == Some(true) && nested_branch_checked == Some(false)
                    && self.method_nested_else_write.is_some() {
                    nested_else_write_applied = true;
                }
            }
            // M2：原 Checker 已選中的 true 葉也可在原共通
            // 控制節點寫入唯一已核准 var self.bool。未選葉不得
            // 做任何 P16 寫入；唯一 ExclusiveWrite Ticket
            // 由方法入口發行，不得為分支另行借票。
            let branch_write = self.method_branch_bool_write.as_ref()
                .filter(|_| selected_branch == Some(true))
                .map(|(field, source, _)| (field, source, None));
            let direct_write = self.direct_method_bool_write.as_ref()
                .map(|(field, source, expected)| (field, source, Some(*expected)));
            if let Some((field, source, expected)) = branch_write.or(direct_write) {
                if selected_write.is_some()
                    || tracker.check_ticket_access(ticket, BorrowKind::ExclusiveWrite)
                        .map_err(PrivateSelfMethodRuntimeError::Ticket)? != self.owner_id {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                let before = match runtime.field(&self.owner, field) {
                    Ok(Observed::Bool(value)) => value,
                    _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                };
                let actual = match source {
                    PrivateSelfBoolWriteSource::Literal(value) => *value,
                    PrivateSelfBoolWriteSource::Parameter(id) =>
                        *bool_parameter_frame.get(id).ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?,
                    PrivateSelfBoolWriteSource::SelfField(name) =>
                        match runtime.field(&self.owner, name) {
                            Ok(Observed::Bool(value)) => value,
                            _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                        },
                };
                if expected.is_some_and(|planned| actual != planned) {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                runtime.write_bool_field(
                    &self.owner, field, actual, self.owner_mutable,
                ).map_err(PrivateSelfMethodRuntimeError::Resource)?;
                let after = match runtime.field(&self.owner, field) {
                    Ok(Observed::Bool(value)) => value,
                    _ => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
                };
                if after != actual {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                let observed = Some((field.clone(), before, after));
                if branch_write.is_some() {
                    branch_bool_write_observed = observed;
                    branch_write_applied = true;
                } else {
                    direct_bool_write_observed = observed;
                }
            }
            // 真方法 self 欄位讀取須以存活 BorrowTicket 授權，不可
            // 僅因原 AST 名稱看似 self 而直接取用所有者。
            let authorized = tracker.check_ticket_access(ticket, BorrowKind::SharedRead)
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            if authorized != self.owner_id {
                return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
            }
            let Observed::Int(value) = runtime.field(&self.owner, &self.field)
                .map_err(PrivateSelfMethodRuntimeError::Resource)? else {
                return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
            };
            let selected_calculation = match selected_branch {
                Some(true) => {
                    if self.method_nested_branch.is_some() {
                        if nested_branch_checked == Some(true) {
                            self.method_branch_return.clone().ok_or(
                                PrivateSelfMethodRuntimeError::WrongReceiver,
                            )?
                        } else {
                            self.method_nested_else_calculation.clone().ok_or(
                                PrivateSelfMethodRuntimeError::WrongReceiver,
                            )?
                        }
                    } else {
                        self.method_branch_return.clone().ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?
                    }
                },
                Some(false) => {
                    if let Some(calculation) = &self.method_branch_else_calculation {
                        calculation.clone()
                    } else if let Some((literal, _)) = self.method_branch_else {
                        PrivateSelfIntReturn::Constant(literal)
                    } else {
                        self.return_calculation.clone()
                    }
                }
                None => self.return_calculation.clone(),
            };
            let calculated = match selected_calculation {
                PrivateSelfIntReturn::Field => value,
                PrivateSelfIntReturn::Constant(value) => value,
                PrivateSelfIntReturn::Add(rhs) => value.checked_add(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
                PrivateSelfIntReturn::Subtract(rhs) => value.checked_sub(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
                PrivateSelfIntReturn::Parameter(id) =>
                    *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?,
                PrivateSelfIntReturn::ParameterAdd(id, rhs) => {
                    let base = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    base.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterSubtract(id, rhs) => {
                    let base = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    base.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterPairAdd(left, right) => {
                    let lhs = *parameter_frame.get(&left).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    let rhs = *parameter_frame.get(&right).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterPairSubtract(left, right) => {
                    let lhs = *parameter_frame.get(&left).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    let rhs = *parameter_frame.get(&right).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::FieldAddParameter(id) => {
                    let rhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    value.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::FieldSubtractParameter(id) => {
                    let rhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    value.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterAddField(id) => {
                    let lhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_add(value)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterSubtractField(id) => {
                    let lhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_sub(value)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::CheckedScalar(ref plan) => {
                    // 只有真選中的 Return 葉才可在借入 self 後
                    // 啟動其已核准純 int 子呼叫工作。每個巢狀
                    // helper 使用原 SharedCallExit；任何內層錯誤
                    // 均先逆序退子框架，再由外層方法歸還唯一票據。
                    let mut calls = Vec::new();
                    plan.collect_calls(&mut calls);
                    let actual = if calls.is_empty() {
                        plan.evaluate(&parameter_frame)
                    } else {
                        // 原 Parser 左到右先求完整 int 算式，每個
                        // 真 Call 由 SharedCallExit 逐框架進退；外層
                        // 算術溢位不得重寫已完成的子函式 Returned。
                        private_self_execute_arithmetic_call(
                            plan, &parameter_frame, &self.method_helpers,
                            &mut exit_events, &mut ordinary_calls_entered,
                            &mut ordinary_calls_completed,
                        )
                    };
                    actual.map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?
                },
            };
            method_exited = true;
            exit_events.push(PrivateSharedCallExitEvent::Returned);
            Ok(calculated)
        })();
        if result.is_err() && !exit_events.is_empty()
            && !exit_events.iter().any(|event| matches!(
                event, PrivateSharedCallExitEvent::UnwindStarted
            )) {
            // helper 可能已在更內層真開始 Unwind；只能有一次事件，
            // 避免外層方法重複虛報退出起點。
            exit_events.push(PrivateSharedCallExitEvent::UnwindStarted);
        }
        // 真局部依逆序離域，無論正常返回、溢位或方法中途故障。
        // 只有實際完成初始化的局部才能留下 ScopeEnded 事件。
        for id in initialized_method_locals.iter().rev() {
            if parameter_frame.remove(id).is_none() {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
            method_local_events.push(PrivateSelfMethodLocalEvent::ScopeEnded(*id));
            exit_events.push(PrivateSharedCallExitEvent::LocalScopeEnded(*id));
        }
        initialized_method_locals.clear();
        // 不論成功或錯誤，實際參數框架不可洩漏至後續呼叫。
        parameter_frame.clear();
        // 不論成功與否，先按原 Checker 動態 self BorrowFact 原子歸還，
        // 再釋放真正結構資源；失敗不可偽裝成功 return。
        let released = tracker.finish_scope_verified(&tickets, &facts)
            .map_err(PrivateSelfMethodRuntimeError::Ticket);
        if released.is_ok() {
            for ticket in tickets.iter().rev() {
                exit_events.push(PrivateSharedCallExitEvent::LoanReturned(*ticket));
            }
        }
        if method_entered {
            exit_events.push(PrivateSharedCallExitEvent::FunctionExited {
                returned: method_exited,
            });
        }
        let result = match (result, released) {
            (Err(problem), _) => Err(problem),
            (Ok(_), Err(problem)) => Err(problem),
            (Ok(value), Ok(())) => tracker.check_direct_access(self.owner_id)
                .map_err(PrivateSelfMethodRuntimeError::Ticket).map(|()| value),
        };
        let ticket_returned = tracker.check_direct_access(self.owner_id).is_ok();
        if main_locals_complete && self.main_int_locals.iter().any(|(_, id, _, _)|
            !main_values.contains_key(id) && !pending_int.contains(id)) {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        let expected_pre_method_calls = self.main_init_plans.iter()
            .chain(self.main_int_assignments.iter().map(|(_, _, plan)| plan))
            .chain(self.main_owner_field_assignments.iter().map(|(_, plan)| plan))
            .chain(self.argument_plans.iter())
            .map(|expression| {
                let mut calls = Vec::new();
                expression.collect_calls(&mut calls);
                calls.len()
            })
            .sum();
        caller.events.extend_from_slice(&exit_events[copied_events..]);
        let outcome = PrivateSelfMethodOutcome {
            result, events: Vec::new(), owned_fields: Vec::new(), expected_owned_fields: Vec::new(), preceding_calls: Vec::new(), expected_preceding_spans: Vec::new(), method_entered, method_exited, ticket_returned,
            direct_bool_write_observed, branch_bool_write_observed,
            origin: PrivateSelfExecutionOrigin::from_plan(self),
            awaited_method: self.awaited_method, await_start_loan_index,
            ordinary_calls_completed, ordinary_calls_entered,
            branch_checked,
            nested_branch_checked,
            nested_write_applied, nested_else_write_applied,
            expected_nested_write: self.method_nested_true_write.is_some(),
            expected_nested_else_write: self.method_nested_else_write.is_some(),
            expected_nested_branch: self.method_nested_branch.map(|(condition, _)| condition),
            branch_write_applied,
            branch_else_write_applied,
            expected_branch_write: self.method_branch_write.is_some()
                || self.method_branch_bool_write.is_some(),
            expected_branch_else_write: self.method_branch_else_write.is_some(),
            expected_branch: self.method_branch.map(|(condition, _)| condition),
            caller_exited: false, caller_events: Vec::new(), entry_frames: Vec::new(), exited_callers: Vec::new(),
            main_local_slots_after_exit: main_values.len() + main_bool_values.len(),
            method_local_slots_after_exit: initialized_method_locals.len(),
            method_local_events,
            expected_method_local_ids: self.method_int_locals.iter().map(
                |(id, _)| *id,
            ).collect(),
            expected_method_assignment_ids: self.method_int_assignments.iter().map(
                |(id, _)| *id,
            ).collect(),
            expected_pre_method_calls,
            checked_loan: self.loan, exit_events,
        };
        if !outcome.has_balanced_method_frame() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        Ok(outcome)
    }
}


/// M2/B4：只解開原 Parser 的 Group 外殼，所有括號必須與
/// 原 Lexer '('/ ')' 真 Token 位置完全一致，且每一層內 span
/// 嚴格包含於外 span。不得掃描原字串重建運算式或補授權。
/// 已核准的型別、BorrowFact、方法簽章仍只由原 Checker 提供。
fn private_self_checked_ungrouped_return<'a>(
    verified: &VerifiedPrivateSource,
    term: &'a Term,
) -> Result<&'a Term, PrivateSelfMethodPrepareError> {
    let mut current = term;
    while let TermKind::Group(inner) = &current.kind {
        if current.start >= inner.start || inner.end >= current.end
            || !verified.tokens.iter().any(|token|
                token.start == current.start
                    && matches!(token.kind, TokenKind::Symbol('(')))
            || !verified.tokens.iter().any(|token|
                token.end == current.end
                    && matches!(token.kind, TokenKind::Symbol(')')))
        {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "方法 Group 返回缺少原 Lexer 括號來源或精確巢狀跨度",
            ));
        }
        current = inner;
    }
    Ok(current)
}

/// 只承認選中方法原參數名稱，且該 int 參數的型別／位置已在
/// prepare() 由原 Checker 封存的 VerifiedTypedMember(Parameter) 核對。
/// 此處重新核對原 Lexer Name token，不用文字比對猜測引用身分。
fn private_self_method_parameter_reference(
    verified: &VerifiedPrivateSource, method: &Function,
    first: Option<(DeclarationId, i64)>,
    additional: &[(DeclarationId, i64)], term: &Term,
) -> Option<DeclarationId> {
    let TermKind::Name(name) = &term.kind else { return None; };
    let parameter = method.params.iter().find(|param| &param.name == name)?;
    let id = DeclarationId(parameter.declaration_start);
    if !first.into_iter().chain(additional.iter().copied())
        .any(|(binding, _)| binding == id) {
        return None;
    }
    verified.tokens.iter().any(|token|
        token.start == term.start && token.end == term.end
            && matches!(&token.kind, TokenKind::Name(actual) if actual == name)
    ).then_some(id)
}
