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

    /// T3／V7：取消／就緒之前，不僅核對被指定的工作，
    /// 也必須核對所有仍 Prepared/Suspended 的同源計畫。
    /// 原執行器自身的 Lexer／Checker 完整來源守門是唯一權威。
    /// 此處只讀；不能因某份污染工作被取消後移除 AST，
    /// 就讓整個佇列假稱其從未持有被變造的準備工作。
    fn verify_pending_sources(&self) -> Result<(), PrivateDeferredAwaitError> {
        for job in self.jobs.values() {
            let legitimate = match job.status {
                PrivateDeferredAwaitStatus::Prepared
                | PrivateDeferredAwaitStatus::Suspended =>
                    job.outcome.is_none() && job.plan.as_ref()
                        .is_some_and(PreparedPrivateScalarCall::checked_source_still_intact),
                PrivateDeferredAwaitStatus::Succeeded
                | PrivateDeferredAwaitStatus::Failed =>
                    job.plan.is_none() && job.outcome.as_ref().is_some_and(|out|
                        out.has_balanced_frame_trace()
                            && out.result.is_ok()
                                == (job.status == PrivateDeferredAwaitStatus::Succeeded)),
                PrivateDeferredAwaitStatus::Cancelled =>
                    job.plan.is_none() && job.outcome.is_none(),
                PrivateDeferredAwaitStatus::Running => false,
            };
            if !legitimate {
                return Err(PrivateDeferredAwaitError::SourceChanged);
            }
        }
        Ok(())
    }

    fn prepare(&mut self, source: &str)
        -> Result<u64, PrivateScalarCallPrepareError> {
        self.verify_pending_sources().map_err(|_|
            PrivateScalarCallPrepareError::Unsupported(
                "V7普通async延遲佇列仍有來源不一致工作，禁止新增ID",
            ))?;
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
        let id = self.next_id.checked_add(1).ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "V7普通async延遲工作ID已用盡，禁止回繞",
            ),
        )?;
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
        self.verify_pending_sources()?;
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
        self.verify_pending_sources()?;
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
        self.verify_pending_sources()?;
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
