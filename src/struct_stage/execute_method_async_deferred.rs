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

/// E2：每次就緒、喚醒和取消均以原工作身分追加真轉移收據；
/// 除了原 ready 佇列及最後 status，還需要一份獨立轉移歷史，
/// 否則只要同時抹除兩本帳就可偽裝成沒有曾經執行的工作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateSelfReadyTransition {
    Prepared(u64),
    Suspended(u64),
    Woken(u64, PrivateDeferredAwaitStatus),
    Cancelled(u64),
}

/// E1-A 延遲、E3-A 未啟動安全點取消：不以名稱重編譯另一份
/// 方法、不移動已經進入的 self 動態框架，也不偷造 P16 事件。
#[derive(Default)]
struct PrivateCheckedDeferredSelfAwaitQueue {
    jobs: BTreeMap<u64, PrivateDeferredSelfAwaitJob>,
    next_id: u64,
    execution_order: Vec<u64>,
    /// 真單執行緒就緒工作；只能由明確 suspend 發行一次憑據。
    ready: std::collections::VecDeque<u64>,
    /// 來源獨立的追加式工作轉移收據，保留取消及已完成歷史。
    ready_history: Vec<PrivateSelfReadyTransition>,
    cancellation_order: Vec<u64>,
}

impl PrivateCheckedDeferredSelfAwaitQueue {
    fn new() -> Self { Self::default() }

    /// 每次會改動真任務狀態之前完整重播原發行、就緒、完成及
    /// 取消歷史，獨立比對 ready FIFO、工作狀態、兩份終態帳。
    /// 不依賴單一空 ready／空執行結果就宣稱從未有工作。
    fn verify_ready_history(&self) -> Result<(), PrivateDeferredAwaitError> {
        use PrivateDeferredAwaitStatus as S;
        use PrivateSelfReadyTransition as T;
        let mut statuses = BTreeMap::<u64, S>::new();
        let mut ready = std::collections::VecDeque::<u64>::new();
        let mut executed = Vec::<u64>::new();
        let mut cancelled = Vec::<u64>::new();
        let mut issued = 0u64;
        for event in &self.ready_history {
            match *event {
                T::Prepared(id) => {
                    if issued.checked_add(1) != Some(id)
                        || statuses.insert(id, S::Prepared).is_some() {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    }
                    issued = id;
                }
                T::Suspended(id) => {
                    if statuses.get(&id) != Some(&S::Prepared)
                        || ready.contains(&id) {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    }
                    statuses.insert(id, S::Suspended);
                    ready.push_back(id);
                }
                T::Woken(id, terminal) => {
                    if statuses.get(&id) != Some(&S::Suspended)
                        || !matches!(terminal, S::Succeeded | S::Failed) {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    }
                    let Some(index) = ready.iter().position(|candidate|
                        *candidate == id) else {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    };
                    ready.remove(index);
                    statuses.insert(id, terminal);
                    executed.push(id);
                }
                T::Cancelled(id) => {
                    let Some(status) = statuses.get(&id).copied() else {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    };
                    if !matches!(status, S::Prepared | S::Suspended) {
                        return Err(PrivateDeferredAwaitError::SourceChanged);
                    }
                    if status == S::Suspended {
                        let Some(index) = ready.iter().position(|candidate|
                            *candidate == id) else {
                            return Err(PrivateDeferredAwaitError::SourceChanged);
                        };
                        ready.remove(index);
                    }
                    statuses.insert(id, S::Cancelled);
                    cancelled.push(id);
                }
            }
        }
        if issued != self.next_id || ready != self.ready
            || executed != self.execution_order
            || cancelled != self.cancellation_order
            || statuses.len() != self.jobs.len() {
            return Err(PrivateDeferredAwaitError::SourceChanged);
        }
        for (id, status) in statuses {
            let Some(job) = self.jobs.get(&id) else {
                return Err(PrivateDeferredAwaitError::SourceChanged);
            };
            if job.status != status {
                return Err(PrivateDeferredAwaitError::SourceChanged);
            }
            let valid = match status {
                S::Prepared | S::Suspended =>
                    job.plan.is_some() && job.outcome.is_none(),
                S::Succeeded | S::Failed =>
                    job.plan.is_none() && job.outcome.as_ref().is_some_and(|outcome|
                        outcome.awaited_method && outcome.ticket_returned
                            && outcome.result.is_ok() == (status == S::Succeeded)
                            && outcome.has_balanced_self_borrow_trace()),
                S::Cancelled => job.plan.is_none() && job.outcome.is_none(),
                S::Running => false,
            };
            if !valid {
                return Err(PrivateDeferredAwaitError::SourceChanged);
            }
        }
        Ok(())
    }

    fn ready_ids(&self) -> Vec<u64> {
        self.ready.iter().copied().collect()
    }

    fn cancellation_order(&self) -> &[u64] {
        &self.cancellation_order
    }

    /// E2：呼叫者明確驅動就緒 FIFO 第一份工作，不存在背景
    /// 自動排程；一份方法直到 Returned／Unwind+LoanReturned 後
    /// 才可能開始另一份，不容交錯持有兩個 self 借票框架。
    fn drive_next(
        &mut self,
    ) -> Result<Option<(u64, &PrivateSelfMethodOutcome)>, PrivateDeferredAwaitError> {
        self.verify_ready_history()?;
        let Some(id) = self.ready.front().copied() else {
            return Ok(None);
        };
        let result = self.wake(id)?;
        Ok(Some((id, result)))
    }

    /// E3：只在所有工作收據、就緒及終態均可對帳時，才按
    /// 逆登錄順序取消其餘尚未啟動工作，避免取消一半才發現
    /// 別份真工作的 Ready／BorrowTicket 已被篡改。
    fn shutdown(&mut self) -> Result<Vec<u64>, PrivateDeferredAwaitError> {
        self.verify_ready_history()?;
        let pending = self.jobs.iter().rev().filter_map(|(&id, job)|
            matches!(job.status,
                PrivateDeferredAwaitStatus::Prepared
                    | PrivateDeferredAwaitStatus::Suspended).then_some(id)
        ).collect::<Vec<_>>();
        for id in &pending {
            self.cancel(*id)?;
        }
        Ok(pending)
    }

    fn prepare(
        &mut self, source: &str,
    ) -> Result<u64, PrivateSelfMethodPrepareError> {
        self.verify_ready_history().map_err(|_|
            PrivateSelfMethodPrepareError::Unsupported(
                "V7 async self FIFO 歷史不一致，拒絕配發新工作 ID",
            ))?;
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
        self.ready_history.push(PrivateSelfReadyTransition::Prepared(id));
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
        self.verify_ready_history()?;
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if job.status != PrivateDeferredAwaitStatus::Prepared {
            return Err(PrivateDeferredAwaitError::WrongState);
        }
        job.status = PrivateDeferredAwaitStatus::Suspended;
        self.ready.push_back(id);
        self.ready_history.push(PrivateSelfReadyTransition::Suspended(id));
        Ok(())
    }

    /// Prepared／Suspended 均尚未建立任何動態 self 框架，
    /// 因此只撤銷工作計畫身分，不預寫不存在的取消資源事件。
    fn cancel(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        self.verify_ready_history()?;
        let job = self.jobs.get_mut(&id)
            .ok_or(PrivateDeferredAwaitError::UnknownTask)?;
        if !matches!(job.status,
            PrivateDeferredAwaitStatus::Prepared
                | PrivateDeferredAwaitStatus::Suspended) {
            return Err(PrivateDeferredAwaitError::AlreadyFinished);
        }
        if job.status == PrivateDeferredAwaitStatus::Suspended {
            let Some(index) = self.ready.iter().position(|candidate|
                *candidate == id) else {
                return Err(PrivateDeferredAwaitError::SourceChanged);
            };
            self.ready.remove(index);
        }
        job.plan = None;
        job.status = PrivateDeferredAwaitStatus::Cancelled;
        self.ready_history.push(PrivateSelfReadyTransition::Cancelled(id));
        self.cancellation_order.push(id);
        Ok(())
    }

    /// 真直接 await 啟動點：原 Checked main 實參先按原計畫求值，
    /// 接著原 HC14 self BorrowTicket 只由既有 execute() 真取得。
    /// 受控 RuntimeFailure 以原 Outcome 保留 Unwind／還票。
    fn wake(
        &mut self, id: u64,
    ) -> Result<&PrivateSelfMethodOutcome, PrivateDeferredAwaitError> {
        self.verify_ready_history()?;
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
        let Some(index) = self.ready.iter().position(|candidate|
            *candidate == id) else {
            return Err(PrivateDeferredAwaitError::SourceChanged);
        };
        self.ready.remove(index);
        self.ready_history.push(PrivateSelfReadyTransition::Woken(
            id, job.status,
        ));
        self.execution_order.push(id);
        Ok(job.outcome.as_ref().expect("只保留真自借票執行結果"))
    }
}
