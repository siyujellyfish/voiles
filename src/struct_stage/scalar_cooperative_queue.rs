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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        // 原 CheckedGraph 的提交／建立位置也必須以原 Lexer
        // 身分證據起始；非法封存 Token 不得消耗工作 ID、
        // 持有新框架或延遲至首次 poll 才被發現。
        let source = std::str::from_utf8(&plan.verified.source.bytes)
            .map_err(|_| PrivateScalarCallPrepareError::SourceChanged)?;
        if plan.original.as_ref() != plan.verified.source.bytes.as_slice()
            || !verified_private_source_tokens(
                source, &plan.verified.tokens,
                &plan.verified.structs, &plan.verified.functions,
            )
            || !plan.verified.plan.is_consistent(
                &plan.verified.structs, &plan.verified.functions, source,
            ) || !plan.matches_source(source) {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
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
            // 跨 Rust 呼叫真正暫停／恢復時，原 Runner 不得
            // 用最初 prepare 時已通過的詞彙身分替換本輪證據。
            || !verified_private_source_tokens(
                source, &self.plan.verified.tokens,
                &self.plan.verified.structs, &self.plan.verified.functions,
            )
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

#[derive(Debug, PartialEq, Eq)]
struct PrivateScalarSuspensionReceipt {
    target: String,
    steps: usize,
    values: Vec<i64>,
    frames: Vec<String>,
    events: Vec<PrivateScalarCallEvent>,
    resources: Vec<crate::owned_str_runtime::ResourceEvent>,
}

impl PrivateScalarSuspensionReceipt {
    fn capture(c: &PrivateScalarGraphContinuation<'_>) -> Option<Self> {
        let saved = c.checkpoint.as_ref()?;
        Some(Self { target: saved.awaiting_name.clone(), steps: saved.steps,
            values: saved.values.clone(), frames: saved.frames.iter().map(|f| f.name.clone()).collect(),
            events: c.events.clone(), resources: saved.resources.events().to_vec() })
    }
    fn matches(&self, c: &PrivateScalarGraphContinuation<'_>) -> bool {
        c.checkpoint.as_ref().is_some_and(|s| self.target == s.awaiting_name
            && self.steps == s.steps && self.values == s.values
            && self.frames.iter().eq(s.frames.iter().map(|f| &f.name))
            && self.events == c.events && self.resources == s.resources.events())
    }
}

struct PrivateScalarCooperativeJob<'a> {
    suspension: Option<PrivateScalarSuspensionReceipt>,
    origin: &'a PreparedPrivateScalarCall,
    completed: Option<PrivateScalarCallOutcome>,
    cancelled: Option<PrivateScalarGraphCancellation>,
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

/// T4／E2：已合法退休的真工作，不再持有 Checker 動態框架，
/// 但每份歷史仍應有獨立、不可由空 jobs 推導為空的終態收據。
/// 收據記錄原全域歷史位置及內容，後續提交／驅動／關閉
/// 不可藉由同步刪除多本原帳而抹除既已完成的真 poll／wake。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarRetiredEvidence {
    status: PrivateDeferredAwaitStatus,
    polls: Vec<(usize, PrivateScalarGraphPoll)>,
    wakes: Vec<(usize, PrivateScalarWakeReceipt)>,
    issued: Vec<(usize, PrivateScalarReadyIssueKind)>,
    cancellations: Vec<usize>,
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
    /// 退休工作交付完整 Outcome／取消證據之後，只保留
    /// 無資源持有權的唯讀歷史收據；不復活已退出的真框架。
    retired: BTreeMap<u64, PrivateScalarRetiredEvidence>,
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
            suspension: None, origin: plan, completed: None, cancelled: None, continuation, cancellation: None,
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
        if !job.suspension.as_ref().is_some_and(|receipt| receipt.matches(&job.continuation)) {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        let same_target = match saved.tasks.last() {
            Some(PrivateScalarTask::InvokeSelf(site)) => site.awaited && site.name == saved.awaiting_name
                && PrivateScalarSelfRead::checked(site.call_span, &site.owner, &site.name,
                    site.loan.source, &job.origin.verified).as_ref() == Some(*site),
            Some(PrivateScalarTask::EnterCall(name, _, _, true)) => *name == saved.awaiting_name
                && job.origin.verified.functions.iter().any(|f| f.name == *name && f.is_async),
            _ => false,
        };
        if !same_target { return Err(PrivateScalarCooperativeError::Engine); }
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
            // 空佇列不等於真的沒有待執行工作；任何已簽發、
            // 尚未消耗的 Prepared／Woken token 遺失都不得假回 None。
            self.verify_ready_fifo()?;
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
        job.suspension = PrivateScalarSuspensionReceipt::capture(&job.continuation);
        if poll == PrivateScalarGraphPoll::Completed {
            job.completed = job.continuation.outcome.clone();
        }
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
        // 即使所有工作均已正常完成／取消且自 jobs 退休，
        // next_id 仍是唯一真實簽發 ID 上限；ready_issues 的
        // Submitted 歷史必須無遺漏地包含 1..=next_id。
        // 否則刪掉最後一筆已退休工作授權後，空 ready／jobs
        // 會被錯誤認定為合法，並破壞下一個 ID 的完整性。
        if last_submitted != self.next_id {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        // 每個真 Submitted ID 恰好對應一份仍活躍工作，
        // 或已有完整終態憑據的已退休工作；不得直接
        // 刪去 jobs 或退休後的終態收據而假稱空佇列。
        if self.jobs.len().checked_add(self.retired.len()) != Some(submitted.len())
            || self.retired.keys().any(|id|
                !submitted.contains(id) || self.jobs.contains_key(id))
        {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        for (&id, evidence) in &self.retired {
            let actual = self.capture_retired_evidence(id, evidence.status);
            if actual != *evidence {
                return Err(PrivateScalarCooperativeError::Engine);
            }
            let last_poll = evidence.polls.last().map(|(_, poll)| *poll);
            let terminal_consistent = match evidence.status {
                PrivateDeferredAwaitStatus::Succeeded
                | PrivateDeferredAwaitStatus::Failed =>
                    last_poll == Some(PrivateScalarGraphPoll::Completed)
                        && evidence.cancellations.is_empty(),
                PrivateDeferredAwaitStatus::Cancelled =>
                    last_poll != Some(PrivateScalarGraphPoll::Completed)
                        && evidence.cancellations.len() == 1,
                _ => false,
            };
            if !terminal_consistent {
                return Err(PrivateScalarCooperativeError::Engine);
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
        // T4／E2：即使原工作已正式退休，仍必須由不可重用的
        // submit ID 核對其真 poll／wake 歷史。過去僅對仍在 jobs
        // 的狀態檢查完整性，刪除一筆退休工作的 Woken token
        // 連同 wake 收據後，空 ready/jobs 會錯誤通過。
        // 此處不重建或重新執行任何已退休的真動態框架。
        if self.drive_order.iter().any(|(id, _)| !submitted.contains(id))
            || self.cancellation_order.iter().any(|id| !submitted.contains(id))
        {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        for id in &submitted {
            let drives = self.drive_order.iter()
                .filter_map(|(logged, poll)|
                    (*logged == *id).then_some(*poll))
                .collect::<Vec<_>>();
            let wakes = self.wake_order.iter()
                .filter(|receipt| receipt.id == *id).count();
            // 首次 poll 必須由 Submitted，後續每次都要先有
            // 真 wake。最後停在 Suspended 時允許一次尚未消耗
            // 的 wake，成功／失敗完成只允許尾端唯一 Completed。
            if wakes > drives.len()
                || drives.len().checked_sub(wakes).is_none_or(|delta|
                    delta > 1)
                || drives.iter().take(drives.len().saturating_sub(1))
                    .any(|poll| *poll != PrivateScalarGraphPoll::Suspended)
                || drives.iter().filter(|poll|
                    **poll == PrivateScalarGraphPoll::Completed
                ).count() > 1
            {
                return Err(PrivateScalarCooperativeError::Engine);
            }
            // 退休後的收據時間也不可倒退、移至原 poll 之前
            // 或移過下一次原 poll；共用已驗證的真時序檢查器。
            self.verify_wake_chronology(*id)?;
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

        // E2/T4：由真 poll 與真 wake 的數量重建每份**仍存活**
        // 工作是否尚有未消耗的入列授權。僅驗「ready 中存在
        // 的身分順序」會漏掉被直接刪去的 token，甚至令空
        // 佇列回 Ok(None) 假裝工作均已驅動；必須雙向對帳。
        // 已退休工作的舊簽發歷史仍供 wake/submit 連續性稽核，
        // 但不能重新生出已移交的任何動態工作。
        let mut pending = Vec::<(usize, u64)>::new();
        for (&id, job) in &self.jobs {
            let polls = self.drive_order.iter().filter(|(logged, _)|
                *logged == id
            ).count();
            let wakes = self.wake_order.iter().filter(|receipt|
                receipt.id == id
            ).count();
            let status = job.continuation.status;
            let outstanding = match status {
                PrivateDeferredAwaitStatus::Prepared => {
                    if polls != 0 || wakes != 0 {
                        return Err(PrivateScalarCooperativeError::Engine);
                    }
                    true
                }
                PrivateDeferredAwaitStatus::Suspended => {
                    if polls == 0 {
                        return Err(PrivateScalarCooperativeError::Engine);
                    }
                    if wakes == polls {
                        true
                    } else if wakes.checked_add(1) == Some(polls) {
                        false
                    } else {
                        return Err(PrivateScalarCooperativeError::Engine);
                    }
                }
                PrivateDeferredAwaitStatus::Running =>
                    return Err(PrivateScalarCooperativeError::Engine),
                PrivateDeferredAwaitStatus::Succeeded
                | PrivateDeferredAwaitStatus::Failed
                | PrivateDeferredAwaitStatus::Cancelled => false,
            };
            if outstanding {
                let (position, issue) = self.ready_issues.iter().enumerate().rev()
                    .find(|(_, issue)| issue.id == id)
                    .ok_or(PrivateScalarCooperativeError::Engine)?;
                let expected_kind = match status {
                    PrivateDeferredAwaitStatus::Prepared =>
                        PrivateScalarReadyIssueKind::Submitted,
                    PrivateDeferredAwaitStatus::Suspended =>
                        PrivateScalarReadyIssueKind::Woken,
                    _ => unreachable!("只有 Prepared/Suspended 才有未消耗 token"),
                };
                if issue.kind != expected_kind {
                    return Err(PrivateScalarCooperativeError::Engine);
                }
                pending.push((position, id));
            }
        }
        pending.sort_unstable_by_key(|(position, _)| *position);
        if pending.iter().map(|(_, id)| *id).ne(self.ready.iter().copied()) {
            return Err(PrivateScalarCooperativeError::Engine);
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
        if !std::ptr::eq(job.origin, continuation.plan) || !job.origin.checked_source_still_intact()
            || job.completed.is_some() || job.cancelled.is_some() {
            return Err(PrivateScalarCooperativeError::Engine);
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
                if job.suspension.is_some() || !polls.is_empty() || continuation.checkpoint.is_some()
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
        job.suspension = None;
        job.cancelled = job.cancellation.clone();
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
        if !std::ptr::eq(job.origin, job.continuation.plan) || !job.origin.checked_source_still_intact()
            || job.completed != job.continuation.outcome || job.cancelled != job.cancellation {
            return Err(PrivateScalarCooperativeError::Engine);
        }
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

    /// 只從已發生的真全域事件擷取歷史位置，不建立任何
    /// 新 SharedExit／ResourceEvent／BorrowTicket。既有
    /// 事件向量在工作退休後仍保留且僅能追加。
    fn capture_retired_evidence(
        &self, id: u64, status: PrivateDeferredAwaitStatus,
    ) -> PrivateScalarRetiredEvidence {
        PrivateScalarRetiredEvidence {
            status,
            polls: self.drive_order.iter().copied().enumerate()
                .filter_map(|(index, (logged, poll))|
                    (logged == id).then_some((index, poll))).collect(),
            wakes: self.wake_order.iter().copied().enumerate()
                .filter_map(|(index, wake)|
                    (wake.id == id).then_some((index, wake))).collect(),
            issued: self.ready_issues.iter().copied().enumerate()
                .filter_map(|(index, issue)|
                    (issue.id == id).then_some((index, issue.kind))).collect(),
            cancellations: self.cancellation_order.iter().enumerate()
                .filter_map(|(index, logged)|
                    (*logged == id).then_some(index)).collect(),
        }
    }

    fn retire_terminal(
        &mut self, id: u64,
    ) -> Result<PrivateScalarRetiredJob, PrivateScalarCooperativeError> {
        let status = self.verify_terminal_evidence(id)?;
        self.verify_ready_fifo()?;
        if self.retired.contains_key(&id) {
            return Err(PrivateScalarCooperativeError::Engine);
        }
        let receipt = self.capture_retired_evidence(id, status);
        let mut job = self.jobs.remove(&id)
            .ok_or(PrivateScalarCooperativeError::UnknownTask)?;
        self.retired.insert(id, receipt);
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
        // 原 E3 契約：任何 Running（真正在執行中的狀態）
        // 優先回 WrongState；入列憑據只稽核其餘可取消／
        // 可退休工作，不能改寫既定錯誤分類或部分取消。
        if self.jobs.values().any(|job|
            job.continuation.status == PrivateDeferredAwaitStatus::Running
        ) {
            return Err(PrivateScalarCooperativeError::WrongState);
        }
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
