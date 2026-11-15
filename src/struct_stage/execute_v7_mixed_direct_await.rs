// G18／P18.6-A1／V7-E2：將原 Checker 普通 async 與 async self
// 未啟動直接 await 工作放入同一單執行緒身份／喚醒次序入口。
//
// 兩份已驗收的真執行器各自持有完整 Checked AST／Token 與
// 動態資源／借票責任。此適配器只調度已核准的首次直接 await，
// 不重新解析、合成借票或聲稱已執行框架可跨內部 await 恢復。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateV7DirectAwaitKind {
    Ordinary,
    SelfMethod,
}

enum PrivateV7DirectAwaitResult<'a> {
    Ordinary(&'a PrivateScalarCallOutcome),
    SelfMethod(&'a PrivateSelfMethodOutcome),
}

/// 同一 Rust 會話有唯一遞增的全域工作 ID；不得把兩個原
/// Checker 內部分別從 1 起算的工作身分當作相同工作。
/// 不發背景工作。每次顯式 wake 都完整退出真工作後，
/// 才能喚醒另一份原計畫；順序只由實際 wake 決定。
#[derive(Default)]
struct PrivateV7MixedDirectAwaitSession {
    ordinary: PrivateCheckedDeferredAwaitQueue,
    methods: PrivateCheckedDeferredSelfAwaitQueue,
    jobs: BTreeMap<u64, (PrivateV7DirectAwaitKind, u64)>,
    next_id: u64,
    execution_order: Vec<u64>,
}

impl PrivateV7MixedDirectAwaitSession {
    fn new() -> Self { Self::default() }

    fn next_global_id(&self) -> Option<u64> { self.next_id.checked_add(1) }

    fn prepare_ordinary(
        &mut self, source: &str,
    ) -> Result<u64, PrivateScalarCallPrepareError> {
        let id = self.next_global_id().ok_or(
            PrivateScalarCallPrepareError::Unsupported(
                "V7 全域單執行緒直接 await 工作 ID 已用盡",
            ),
        )?;
        // 先以原 Checker 驗真實普通 async 直接 await，只有
        // 成功後才提交全域身份，靜態錯誤不消耗任何全域 ID。
        let local = self.ordinary.prepare(source)?;
        self.jobs.insert(id, (PrivateV7DirectAwaitKind::Ordinary, local));
        self.next_id = id;
        Ok(id)
    }

    fn prepare_method(
        &mut self, source: &str,
    ) -> Result<u64, PrivateSelfMethodPrepareError> {
        let id = self.next_global_id().ok_or(
            PrivateSelfMethodPrepareError::Unsupported(
                "V7 全域單執行緒直接 await 工作 ID 已用盡",
            ),
        )?;
        let local = self.methods.prepare(source)?;
        self.jobs.insert(id, (PrivateV7DirectAwaitKind::SelfMethod, local));
        self.next_id = id;
        Ok(id)
    }

    fn resolve(
        &self, id: u64,
    ) -> Result<(PrivateV7DirectAwaitKind, u64), PrivateDeferredAwaitError> {
        self.jobs.get(&id).copied().ok_or(PrivateDeferredAwaitError::UnknownTask)
    }

    fn status(&self, id: u64) -> Option<PrivateDeferredAwaitStatus> {
        let (kind, local) = self.resolve(id).ok()?;
        match kind {
            PrivateV7DirectAwaitKind::Ordinary => self.ordinary.status(local),
            PrivateV7DirectAwaitKind::SelfMethod => self.methods.status(local),
        }
    }

    fn execution_order(&self) -> &[u64] { &self.execution_order }

    fn suspend(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let (kind, local) = self.resolve(id)?;
        match kind {
            PrivateV7DirectAwaitKind::Ordinary => self.ordinary.suspend(local),
            PrivateV7DirectAwaitKind::SelfMethod => self.methods.suspend(local),
        }
    }

    fn cancel(&mut self, id: u64) -> Result<(), PrivateDeferredAwaitError> {
        let (kind, local) = self.resolve(id)?;
        match kind {
            PrivateV7DirectAwaitKind::Ordinary => self.ordinary.cancel(local),
            PrivateV7DirectAwaitKind::SelfMethod => self.methods.cancel(local),
        }
    }

    /// 同一執行緒的顯式 direct await：每次必完整執行一份
    /// 原 Checker 工作、保留它自己的真 outcome 與 exit trace，
    /// 才能在全域順序追加這筆已真正執行的工作 ID。
    fn wake(
        &mut self, id: u64,
    ) -> Result<PrivateV7DirectAwaitResult<'_>, PrivateDeferredAwaitError> {
        let (kind, local) = self.resolve(id)?;
        let outcome = match kind {
            PrivateV7DirectAwaitKind::Ordinary =>
                self.ordinary.wake(local).map(PrivateV7DirectAwaitResult::Ordinary)?,
            PrivateV7DirectAwaitKind::SelfMethod =>
                self.methods.wake(local).map(PrivateV7DirectAwaitResult::SelfMethod)?,
        };
        self.execution_order.push(id);
        Ok(outcome)
    }
}
