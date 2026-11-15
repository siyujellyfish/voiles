// G18／P18.6-A1／V7：同一原 Checker async self 直接 await
// 在跨 Rust API 呼叫期間保存尚未啟動的正式工作計畫。
// 此子集不引入背景派發，未聲稱已暫停的 async 方法內部框架
// 可跨其自身 await 恢復。真方法本體仍由唯一 Checked self
// Runtime 執行，BorrowTicket 只在真正喚醒後進入方法時簽發。

struct PrivateDeferredSelfAwaitJob {
    plan: Option<PreparedPrivateSelfMethod>,
    status: PrivateDeferredAwaitStatus,
    outcome: Option<PrivateSelfMethodOutcome>,
}

/// E1-A 延遲、E3-A 未啟動安全點取消：不以名稱重編譯另一份
/// 方法、不移動已經進入的 self 動態框架，也不偷造 P16 事件。
#[derive(Default)]
struct PrivateCheckedDeferredSelfAwaitQueue {
    jobs: BTreeMap<u64, PrivateDeferredSelfAwaitJob>,
    next_id: u64,
    execution_order: Vec<u64>,
}

impl PrivateCheckedDeferredSelfAwaitQueue {
    fn new() -> Self { Self::default() }

    fn prepare(
        &mut self, source: &str,
    ) -> Result<u64, PrivateSelfMethodPrepareError> {
        let plan = PreparedPrivateSelfMethod::prepare(source)?;
        // 此批僅允許 main 真建構一份合法 int struct owner，
        // 下一句即直接 await async self，沒有此前可執行
        // main 控制流程／其他副作用；其餘仍交給既有原
        // Checker 的 S20 MethodSafety、BorrowFact/HIR 判定。
        let main = plan.verified.functions.iter()
            .find(|function| function.name == "main")
            .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "V7 async self 工作缺少原 Checker main",
            ))?;
        if !plan.awaited_method
            || main.body.statements.len() != 2
            || !plan.main_int_locals.is_empty()
            || !plan.main_init_plans.is_empty()
            || !matches!(main.body.statements.last().map(|s| &s.kind),
                Some(StmtKind::Return(returned)) if
                    matches!(&returned.expression.kind,
                        TermKind::AwaitMethodCall(_, _, _)))
        {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "V7 async self 延遲計畫僅限無前置作用的直接 await",
            ));
        }
        // 不能用 u64 回繞製造重複真工作身分。
        let id = self.next_id.checked_add(1).ok_or(
            PrivateSelfMethodPrepareError::Unsupported(
                "V7 async self 延遲工作 ID 已用盡",
            ),
        )?;
        self.next_id = id;
        self.jobs.insert(id, PrivateDeferredSelfAwaitJob {
            plan: Some(plan), status: PrivateDeferredAwaitStatus::Prepared,
            outcome: None,
        });
        Ok(id)
    }

    fn status(&self, id: u64) -> Option<PrivateDeferredAwaitStatus> {
        self.jobs.get(&id).map(|job| job.status)
    }

    fn execution_order(&self) -> &[u64] { &self.execution_order }

    fn outcome(&self, id: u64) -> Option<&PrivateSelfMethodOutcome> {
        self.jobs.get(&id).and_then(|job| job.outcome.as_ref())
    }

    /// 直到 Rust 呼叫者明確執行 wake 前，都只保存真 Checked
    /// source／AST／Token，沒有 self 借票、資源、幀或 Returned。
    fn suspend(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if job.status != PrivateDeferredAwaitStatus::Prepared {
            return Err(PrivateDeferredAwaitError::WrongState);
        }
        job.status = PrivateDeferredAwaitStatus::Suspended;
        Ok(())
    }

    /// Prepared／Suspended 均尚未建立任何動態 self 框架，
    /// 因此只撤銷工作計畫身分，不預寫不存在的取消資源事件。
    fn cancel(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if !matches!(job.status,
            PrivateDeferredAwaitStatus::Prepared
                | PrivateDeferredAwaitStatus::Suspended) {
            return Err(PrivateDeferredAwaitError::AlreadyFinished);
        }
        job.plan = None;
        job.status = PrivateDeferredAwaitStatus::Cancelled;
        Ok(())
    }

    /// 真直接 await 啟動點：原 Checked main 實參先按原計畫求值，
    /// 接著原 HC14 self BorrowTicket 只由既有 execute() 真取得。
    /// 受控 RuntimeFailure 以原 Outcome 保留 Unwind／還票。
    fn wake(
        &mut self, id: u64,
    ) -> Result<&PrivateSelfMethodOutcome, PrivateDeferredAwaitError> {
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
            .expect("Suspended async self 工作必持原 Checker 唯一計畫")
            .execute() {
            Ok(outcome) => outcome,
            Err(_) => {
                // 來源不一致被拒絕時，保留仍可安全取消的
                // 未啟動工作；不可消耗 ID 或假寫方法失敗。
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
        Ok(job.outcome.as_ref().expect("只保留真自借票執行結果"))
    }
}
