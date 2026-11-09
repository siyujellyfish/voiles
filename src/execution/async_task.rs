// G18 V7：已核准 E1/E2/E3/E4/E5 的非公開直接等待執行基礎。
// 此階段的工作本體仍是 P17 原 Checker 核准的同步純值來源；
// 原私有 async fn 方法 Parser／Checker 與跨 await 真暫停尚未接通。
// 不把本模組的內部驗證冒稱 P18.6 A1/A2 或 G18 全項 Passed。
use std::collections::BTreeMap;
use std::rc::Rc;
use super::{ExecutionBudget, ExecutionOutcome, PreparationFailure, PreparedProgram, RuntimeFailure, Value};
use crate::borrow_safety::BorrowSafety;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectAwaitStatus {
    Prepared,
    /// 工作已保留於單執行緒協作式安全點，本體尚未執行。
    Suspended,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectAwaitError {
    UnknownTask,
    AlreadyAwaited,
    Cancelled,
    ActiveBorrowAcrossAwait,
}

/// 只有已取得正式 P01 Checker 全套 HIR／可達性授權的工作
/// 才可存入；尚未獲得 async fn 語法驗證，不能供公開語言使用。
struct CheckedTask {
    plan: Option<PreparedProgram>,
    budget: ExecutionBudget,
    status: DirectAwaitStatus,
    outcome: Option<ExecutionOutcome>,
}

/// 單執行緒、只在直接 await 時執行指定工作的內部子集。
/// 沒有 spawn／背景執行，不能偽稱本體在 prepare 時已執行。
#[derive(Default)]
pub(crate) struct DirectAwaitSession {
    /// 單執行緒會話的唯一身分；分段等待不得錯接另一個會話的相同工作編號。
    identity: Rc<()>,
    tasks: BTreeMap<u64, CheckedTask>,
    next_id: u64,
    execution_order: Vec<u64>,
    /// E5：此 V7 子集不容許任何仍有效的真借用票據跨 await。
    /// 未來接入私有 Checker 時必須使用同一執行上下文追蹤器。
    loans: BorrowSafety,
}

impl DirectAwaitSession {
    pub(crate) fn new() -> Self { Self::default() }

    /// 保留原 Parser／Checker 的拒絕，不憑準備成功偽造執行事件。
    pub(crate) fn prepare_verified_scalar(
        &mut self, source: Vec<u8>, budget: ExecutionBudget,
    ) -> Result<u64, PreparationFailure> {
        let plan = PreparedProgram::prepare(source)?;
        let id = self.next_id.checked_add(1)
            .expect("內部非同步工作身分已用盡，禁止回繞重用");
        self.next_id = id;
        self.tasks.insert(id, CheckedTask {
            plan: Some(plan), budget,
            status: DirectAwaitStatus::Prepared, outcome: None,
        });
        Ok(id)
    }

    pub(crate) fn status(&self, id: u64) -> Option<DirectAwaitStatus> {
        self.tasks.get(&id).map(|task| task.status)
    }

    pub(crate) fn execution_order(&self) -> &[u64] {
        &self.execution_order
    }

    /// E3：尚未啟動的工作屬合法安全點，取消不產生任何本體事件。
    /// 執行中的工作無法由此同步子集任意強制中斷。
    pub(crate) fn cancel_before_start(&mut self, id: u64)
        -> Result<(), DirectAwaitError> {
        let task = self.tasks.get_mut(&id).ok_or(DirectAwaitError::UnknownTask)?;
        if !matches!(task.status, DirectAwaitStatus::Prepared | DirectAwaitStatus::Suspended) {
            return Err(DirectAwaitError::AlreadyAwaited);
        }
        task.plan = None;
        task.status = DirectAwaitStatus::Cancelled;
        Ok(())
    }

    /// E1：第一次直接 await 才真正進入原來源 evaluator。
    /// E4：原 RuntimeFailure 原樣傳遞，不偽造正常 Returned。
    /// E5：真 BorrowSafety 非空時，禁止進入 await，保持工作未啟動。
    /// E2 完整的 suspend／wakeup 尚未接入；目前純值本體一次執行結束。
    pub(crate) fn await_direct(&mut self, id: u64)
        -> Result<&ExecutionOutcome, DirectAwaitError> {
        let status = self.tasks.get(&id)
            .ok_or(DirectAwaitError::UnknownTask)?.status;
        match status {
            DirectAwaitStatus::Prepared => {},
            DirectAwaitStatus::Cancelled => return Err(DirectAwaitError::Cancelled),
            _ => return Err(DirectAwaitError::AlreadyAwaited),
        }
        if self.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(DirectAwaitError::ActiveBorrowAcrossAwait);
        }
        let task = self.tasks.get_mut(&id).expect("已核對工作身分");
        let plan = task.plan.take().expect("準備完成工作必有唯一計畫");
        task.status = DirectAwaitStatus::Running;
        let outcome = plan.execute_with_trace_budget(task.budget);
        task.status = if outcome.result.is_ok() {
            DirectAwaitStatus::Succeeded
        } else {
            DirectAwaitStatus::Failed
        };
        task.outcome = Some(outcome);
        self.execution_order.push(id);
        Ok(self.tasks.get(&id).and_then(|task| task.outcome.as_ref())
            .expect("只記錄真正執行器產生的結果"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;
    use crate::ownership_ledger::DeclarationId;
    use crate::execution::{ControlEvent, Value};

    fn source(value: i64) -> Vec<u8> {
        format!("fn main()->int{{ return {value} }}").into_bytes()
    }

    #[test]
    fn v7_a1_準備不執行而首次直接等待才取得真來源結果() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(source(7), ExecutionBudget::default()).unwrap();
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Prepared));
        assert!(session.execution_order().is_empty());
        let outcome = session.await_direct(id).unwrap();
        assert_eq!(outcome.result, Ok(Value::Int(7)));
        assert!(outcome.events.iter().any(|event|
            matches!(event, ControlEvent::Returned { .. })));
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Succeeded));
        assert_eq!(session.execution_order(), &[id]);
    }

    #[test]
    fn v7_a1_不同工作只依實際直接等待順序執行() {
        let mut session = DirectAwaitSession::new();
        let first = session.prepare_verified_scalar(source(1), ExecutionBudget::default()).unwrap();
        let second = session.prepare_verified_scalar(source(2), ExecutionBudget::default()).unwrap();
        assert_eq!(session.await_direct(second).unwrap().result, Ok(Value::Int(2)));
        assert_eq!(session.status(first), Some(DirectAwaitStatus::Prepared));
        assert_eq!(session.await_direct(first).unwrap().result, Ok(Value::Int(1)));
        assert_eq!(session.execution_order(), &[second, first]);
    }

    #[test]
    fn v7_a1_正式checker拒絕不能建立非同步工作() {
        let mut session = DirectAwaitSession::new();
        assert!(session.prepare_verified_scalar(
            b"fn main()->int{ return true }".to_vec(), ExecutionBudget::default(),
        ).is_err());
        assert!(session.tasks.is_empty());
        assert!(session.execution_order().is_empty());
    }

    #[test]
    fn v7_a1_直接等待不可重複消耗同一工作() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(source(9), ExecutionBudget::default()).unwrap();
        assert!(session.await_direct(id).is_ok());
        assert_eq!(session.await_direct(id).err(), Some(DirectAwaitError::AlreadyAwaited));
        assert_eq!(session.execution_order(), &[id]);
    }

    #[test]
    fn v7_a2_取消未啟動工作不執行本體也不造返回() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(source(3), ExecutionBudget::default()).unwrap();
        session.cancel_before_start(id).unwrap();
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Cancelled));
        assert_eq!(session.await_direct(id).err(), Some(DirectAwaitError::Cancelled));
        assert!(session.execution_order().is_empty());
        assert!(session.tasks.get(&id).unwrap().outcome.is_none());
    }

    #[test]
    fn v7_a2_真執行預算錯誤不會偽造成功返回() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(source(4), ExecutionBudget {
            max_call_frames: 0, max_steps: 1,
        }).unwrap();
        let outcome = session.await_direct(id).unwrap();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Failed));
    }

    #[test]
    fn v7_e5_活躍真票據不得跨等待且歸還後才執行() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(source(5), ExecutionBudget::default()).unwrap();
        let ticket = session.loans.acquire(DeclarationId(25), BorrowKind::SharedRead).unwrap();
        assert_eq!(session.await_direct(id).err(),
            Some(DirectAwaitError::ActiveBorrowAcrossAwait));
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Prepared));
        assert!(session.execution_order().is_empty());
        session.loans.release(ticket).unwrap();
        assert_eq!(session.await_direct(id).unwrap().result, Ok(Value::Int(5)));
    }

    #[test]
    fn v7_e3_取消與未知工作不會重用工作身分() {
        let mut session = DirectAwaitSession::new();
        assert_eq!(session.cancel_before_start(100).err(), Some(DirectAwaitError::UnknownTask));
        let id = session.prepare_verified_scalar(source(1), ExecutionBudget::default()).unwrap();
        session.cancel_before_start(id).unwrap();
        assert_eq!(session.cancel_before_start(id).err(), Some(DirectAwaitError::AlreadyAwaited));
        let next = session.prepare_verified_scalar(source(2), ExecutionBudget::default()).unwrap();
        assert_ne!(id, next);
        assert_eq!(session.await_direct(next).unwrap().result, Ok(Value::Int(2)));
    }
}


// V7 A2／E3：將真正 P16 ResourceId 與 HC14 BorrowTicket 接到同一個
// 內部暫停點退出工作框架。來源授權仍須由原 Checker 傳入：
// 此層不解析 async fn，不簽發新的語言權限，也不是完整協程執行器。
use crate::borrow_safety::{BorrowFact, BorrowSafetyError, BorrowTicket};
use crate::owned_str_runtime::{self, OwnedStrRuntime, ResourceEvent, StrRuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncSafePointState {
    Running,
    Suspended,
    Cancelled,
    Failed,
    Succeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncSafePointError {
    WrongState,
    NoFrame,
    NoActiveTicket,
    ActiveBorrowAcrossAwait,
    DirectAwait(DirectAwaitError),
    Borrow(BorrowSafetyError),
    Resource(StrRuntimeError),
}
impl From<BorrowSafetyError> for AsyncSafePointError {
    fn from(value: BorrowSafetyError) -> Self { Self::Borrow(value) }
}
impl From<StrRuntimeError> for AsyncSafePointError {
    fn from(value: StrRuntimeError) -> Self { Self::Resource(value) }
}

/// 只能從真 BorrowSafety.release 成功後記錄 LoanReturned；
/// 真 ResourceId 釋放先由 OwnedStrRuntime.leave_scope 完成，
/// 再從該 Runtime 已經產生的 ResourceEvent 逐筆轉錄來源。
#[derive(Debug, Clone, PartialEq, Eq)]
enum AsyncSafeExitEvent {
    LoanReturned(BorrowTicket),
    ResourceReleased(u64),
    FrameUnwound(usize),
}

#[derive(Debug)]
struct AsyncSafePointEvidence {
    resource_events: Vec<ResourceEvent>,
    exit_events: Vec<AsyncSafeExitEvent>,
    unwound_frames: usize,
}

/// E4-A：沿用真正 P17 執行器的原 RuntimeFailure；
/// 資源及票據的清理證據獨立保存，不以正常返回偽裝異常。
#[derive(Debug)]
struct AsyncSafePointFailure {
    error: RuntimeFailure,
    evidence: AsyncSafePointEvidence,
}

/// 由原 P17 CheckedTask 的真結果決定唯一終態；
/// 不以工作名稱、事先估計的回傳值或假事件決定成功／失敗。
#[derive(Debug)]
enum AsyncSafePointSettlement {
    Succeeded { value: Value, evidence: AsyncSafePointEvidence },
    Failed(AsyncSafePointFailure),
}

/// 暫停狀態保留原 P16 OwnedStrRuntime 和原 HC14 BorrowSafety；
/// 不以一組假的資源事件或 Rust Drop 推導 voiles 退出順序。
/// 已核准 BorrowFact 來源須由未來的 Checker→工作計畫提供。
struct AsyncSafePointFrames {
    state: AsyncSafePointState,
    /// 同一真 CheckedTask 停在安全點時，必須由原會話及工作身分喚醒或取消。
    pending_verified_await: Option<(Rc<()>, u64)>,
    resources: OwnedStrRuntime,
    loans: BorrowSafety,
    frames: Vec<Vec<(BorrowTicket, BorrowFact)>>,
    exit_events: Vec<AsyncSafeExitEvent>,
}

impl AsyncSafePointFrames {
    fn new() -> Self {
        Self {
            state: AsyncSafePointState::Running,
            pending_verified_await: None,
            resources: OwnedStrRuntime::new(),
            loans: BorrowSafety::new(),
            frames: Vec::new(),
            exit_events: Vec::new(),
        }
    }

    fn enter_frame(&mut self) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running {
            return Err(AsyncSafePointError::WrongState);
        }
        self.resources.enter_scope();
        self.frames.push(Vec::new());
        Ok(())
    }

    /// 僅供內部框架責任測試：真配置 P16 UTF-8 資源，
    /// 不冒充已從 async .voil 原始來源獲得宣告權限。
    fn declare_owned(&mut self, name: &str, text: &str)
        -> Result<u64, AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running {
            return Err(AsyncSafePointError::WrongState);
        }
        if self.frames.is_empty() { return Err(AsyncSafePointError::NoFrame); }
        self.resources.declare(name, true, text)?;
        Ok(self.resources.resource_id(name)?)
    }

    /// 呼叫者必須先持有原 Checker 的 BorrowFact；這裡只要求真
    /// BorrowSafety 追蹤器成功簽發，絕不把本方法當成靜態授權。
    fn acquire_for_frame(&mut self, fact: BorrowFact)
        -> Result<BorrowTicket, AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running {
            return Err(AsyncSafePointError::WrongState);
        }
        if self.frames.is_empty() { return Err(AsyncSafePointError::NoFrame); }
        let ticket = self.loans.acquire(fact.source, fact.kind)?;
        self.frames.last_mut().expect("先驗證存在框架").push((ticket, fact));
        Ok(ticket)
    }

    /// 經原 VA15 最後合法使用點授權者才能呼叫；真歸還前
    /// 先由 finish_selected_verified 核對票據／BorrowFact。
    fn return_latest_loan(&mut self) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running {
            return Err(AsyncSafePointError::WrongState);
        }
        let (ticket, fact) = self.frames.last()
            .ok_or(AsyncSafePointError::NoFrame)?
            .last().copied().ok_or(AsyncSafePointError::NoActiveTicket)?;
        self.loans.finish_selected_verified(&[ticket], &[fact])?;
        self.frames.last_mut().expect("已驗證存在框架").pop();
        self.exit_events.push(AsyncSafeExitEvent::LoanReturned(ticket));
        Ok(())
    }

    /// VB15-01-A：不可只依「曾經在 suspend 時沒有票據」來允許
    /// 後續恢復或取消。每次交付安全點都要重新驗證真正 HC14
    /// tracker 與各動態框架的有效票據，拒絕時必須零狀態變更。
    fn verify_no_live_borrows(&self) -> Result<(), AsyncSafePointError> {
        if self.frames.iter().any(|frame| !frame.is_empty())
            || self.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::ActiveBorrowAcrossAwait);
        }
        Ok(())
    }

    /// VB15-01-A：仍有效的任意真票據不得跨 await 暫停點。
    fn suspend(&mut self) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running || self.frames.is_empty() {
            return Err(AsyncSafePointError::WrongState);
        }
        self.verify_no_live_borrows()?;
        self.state = AsyncSafePointState::Suspended;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), AsyncSafePointError> {
        // 已綁定真等待工作的安全點不可由一般 resume 偷偷越過。
        if self.state != AsyncSafePointState::Suspended
            || self.pending_verified_await.is_some() {
            return Err(AsyncSafePointError::WrongState);
        }
        // 防止暫停之後再加入的有效 HC14 票據被繞過。
        self.verify_no_live_borrows()?;
        self.state = AsyncSafePointState::Running;
        Ok(())
    }

    /// E3／E4 同一個真 P16／HC14 退出核心：
    /// - 安全點取消時，跨 await 的有效票據早已被 VB15-01-A 禁止；
    /// - 執行中真 RuntimeFailure 則逐框架先還仍有效的 HC14 票據，
    ///   才釋放同一作用域的 OwnedUtf8，並向外層繼續。
    /// 所有 Returned／ResourceReleased 只能從實際追蹤器操作後記錄。
    fn unwind_frames(
        &mut self, target: AsyncSafePointState, release_live_loans: bool,
    ) -> Result<AsyncSafePointEvidence, AsyncSafePointError> {
        let count = self.frames.len();
        while !self.frames.is_empty() {
            let depth = self.frames.len();
            if release_live_loans {
                while let Some((ticket, fact)) =
                    self.frames.last().and_then(|frame| frame.last()).copied() {
                    self.loans.finish_selected_verified(&[ticket], &[fact])?;
                    self.frames.last_mut().expect("已確認存在動態框架").pop();
                    self.exit_events.push(AsyncSafeExitEvent::LoanReturned(ticket));
                }
            } else if self.frames.last().is_some_and(|frame| !frame.is_empty()) {
                return Err(AsyncSafePointError::ActiveBorrowAcrossAwait);
            }
            let before = self.resources.events().len();
            self.resources.leave_scope()?;
            // 真 P16 每個 Released(ID) 都必須是剛完成的動態離域，
            // 不接受由清理器自創或按數量補齊的假的資源釋放。
            let released = self.resources.events()[before..].iter().filter_map(|event| {
                if let ResourceEvent::Released(id) = event { Some(*id) } else { None }
            }).collect::<Vec<_>>();
            for id in released {
                self.exit_events.push(AsyncSafeExitEvent::ResourceReleased(id));
            }
            self.frames.pop();
            self.exit_events.push(AsyncSafeExitEvent::FrameUnwound(depth));
        }
        if self.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::ActiveBorrowAcrossAwait);
        }
        let resources = std::mem::replace(&mut self.resources, OwnedStrRuntime::new());
        let resource_events = resources.finish()?;
        if !owned_str_runtime::event_trace_balanced(&resource_events) {
            return Err(AsyncSafePointError::Resource(StrRuntimeError::OutstandingResource));
        }
        self.state = target;
        Ok(AsyncSafePointEvidence {
            resource_events,
            exit_events: self.exit_events.clone(),
            unwound_frames: count,
        })
    }

    /// E3-A：取消只能在合法 suspend 後，不能強制中止 Running。
    /// 有效票據或不符狀態時保持原有真資源不變。
    fn cancel_suspended(&mut self) -> Result<AsyncSafePointEvidence, AsyncSafePointError> {
        // 綁定工作未取消前，不得單獨釋放等待者框架而遺留 Prepared 工作。
        if self.state != AsyncSafePointState::Suspended
            || self.pending_verified_await.is_some() {
            return Err(AsyncSafePointError::WrongState);
        }
        self.verify_no_live_borrows()?;
        self.unwind_frames(AsyncSafePointState::Cancelled, false)
    }

    /// E4-A：工作在 Running 階段真正失敗，不等待可取消安全點；
    /// 原 RuntimeFailure 逐字保留給等待者，清理後狀態為 Failed
    /// 而非 Cancelled／Succeeded，並不虛構任何成功返回事件。
    fn fail_running(
        &mut self, error: RuntimeFailure,
    ) -> Result<AsyncSafePointFailure, AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running || self.frames.is_empty() {
            return Err(AsyncSafePointError::WrongState);
        }
        let evidence = self.unwind_frames(AsyncSafePointState::Failed, true)?;
        Ok(AsyncSafePointFailure { error, evidence })
    }

    /// V7／A1+A2：P17 真 Checker 已核准的直接等待工作，
    /// 從合法安全點交付執行器，完成後將真正 Value／RuntimeFailure
    /// 與 P16／HC14 框架的清理證據一併交付。
    ///
    /// 此私有橋接不解析使用者 await，也不為語言來源憑空提供
    /// OwnedStr／借用權限；資源框架的正式 BorrowFact 來源仍待接通。
    /// 真 P01 Checker 工作已準備，但尚未執行。此呼叫只進入安全暫停，
    /// 不觸發任何 Runtime／ResourceEvent；同一執行緒可先執行其他直接等待。
    /// Rc 身分避免不同會話的相同 task 編號被誤認成原工作。
    fn suspend_verified_await(
        &mut self, session: &mut DirectAwaitSession, task: u64,
    ) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running || self.frames.is_empty() {
            return Err(AsyncSafePointError::WrongState);
        }
        Self::verify_prepared_task(session, task)?;
        self.suspend()?;
        // 同一會話中的其他直接 await 不能搶先執行這份已保留計畫。
        session.tasks.get_mut(&task).expect("已驗證工作身分").status =
            DirectAwaitStatus::Suspended;
        self.pending_verified_await = Some((Rc::clone(&session.identity), task));
        Ok(())
    }

    fn verify_prepared_task(
        session: &DirectAwaitSession, task: u64,
    ) -> Result<(), AsyncSafePointError> {
        match session.status(task) {
            None => return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::UnknownTask)),
            Some(DirectAwaitStatus::Prepared) => {}
            Some(DirectAwaitStatus::Cancelled) =>
                return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::Cancelled)),
            Some(_) =>
                return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::AlreadyAwaited)),
        }
        if session.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::DirectAwait(
                DirectAwaitError::ActiveBorrowAcrossAwait,
            ));
        }
        Ok(())
    }

    fn verify_suspended_task(
        session: &DirectAwaitSession, task: u64,
    ) -> Result<(), AsyncSafePointError> {
        match session.status(task) {
            Some(DirectAwaitStatus::Suspended) => {}
            Some(DirectAwaitStatus::Cancelled) =>
                return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::Cancelled)),
            None => return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::UnknownTask)),
            Some(_) => return Err(AsyncSafePointError::DirectAwait(
                DirectAwaitError::AlreadyAwaited,
            )),
        }
        if session.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::DirectAwait(
                DirectAwaitError::ActiveBorrowAcrossAwait,
            ));
        }
        Ok(())
    }

    fn verify_pending_task(
        &self, session: &DirectAwaitSession, task: u64,
    ) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Suspended
            || !self.pending_verified_await.as_ref().is_some_and(|(owner, saved)|
                *saved == task && Rc::ptr_eq(owner, &session.identity)) {
            return Err(AsyncSafePointError::WrongState);
        }
        Ok(())
    }

    /// E2-A：只在已核准安全點喚醒指定工作。
    /// 暫停後如有新的真 BorrowTicket，必須先歸還；拒絕時保留原暫停及工作。
    fn wake_verified_await(
        &mut self, session: &mut DirectAwaitSession, task: u64,
    ) -> Result<AsyncSafePointSettlement, AsyncSafePointError> {
        self.verify_pending_task(session, task)?;
        Self::verify_suspended_task(session, task)?;
        // 上一次 suspend 時的快照不足以證明現在的等待者無有效票據。
        // 未通過前不能解除綁定，亦不能把 CheckedTask 重設為 Prepared。
        self.verify_no_live_borrows()?;
        self.pending_verified_await = None;
        self.resume()?;
        session.tasks.get_mut(&task).expect("仍保留原核准工作").status =
            DirectAwaitStatus::Prepared;
        let outcome = session.await_direct(task).map_err(AsyncSafePointError::DirectAwait)?;
        match &outcome.result {
            Ok(value) => {
                let evidence = self.unwind_frames(AsyncSafePointState::Succeeded, false)?;
                Ok(AsyncSafePointSettlement::Succeeded { value: *value, evidence })
            }
            Err(error) => {
                let failure = self.fail_running(error.clone())?;
                Ok(AsyncSafePointSettlement::Failed(failure))
            }
        }
    }

    /// E3-A：已暫停的原會話／工作一起取消；工作未曾執行，
    /// P16／HC14 的父子框架仍須依序真離域。拒絕跨會話或錯 task 身分。
    fn cancel_verified_await(
        &mut self, session: &mut DirectAwaitSession, task: u64,
    ) -> Result<AsyncSafePointEvidence, AsyncSafePointError> {
        self.verify_pending_task(session, task)?;
        // 必須先證明取消框架可安全退場，再消耗尚未啟動的真正工作。
        // 失敗時保持 session.Suspended／pending 身分／ResourceId 全部不變。
        self.verify_no_live_borrows()?;
        match session.status(task) {
            Some(DirectAwaitStatus::Suspended) =>
                session.cancel_before_start(task).map_err(AsyncSafePointError::DirectAwait)?,
            // 原工作若已在此單執行緒的暫停期間取消，仍可清理等待者。
            Some(DirectAwaitStatus::Cancelled) => {}
            None => return Err(AsyncSafePointError::DirectAwait(DirectAwaitError::UnknownTask)),
            Some(_) => return Err(AsyncSafePointError::DirectAwait(
                DirectAwaitError::AlreadyAwaited,
            )),
        }
        self.pending_verified_await = None;
        self.cancel_suspended()
    }

    /// 既有同步直接等待現在共用同一分段安全點；來源經 Checker 核准，
    /// 不需要等待外部喚醒時即在同一執行緒依序執行兩個步驟。
    fn await_verified_scalar(
        &mut self, session: &mut DirectAwaitSession, task: u64,
    ) -> Result<AsyncSafePointSettlement, AsyncSafePointError> {
        self.suspend_verified_await(session, task)?;
        self.wake_verified_await(session, task)
    }
}

#[cfg(test)]
mod v7_a2_safe_point_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;
    use crate::ownership_ledger::DeclarationId;

    #[test]
    fn v7_a1_a2_直接等待真成功值與父子ownedutf8由共用退出清理() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 19 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let parent = frames.declare_owned("父", "原真 UTF-8🪼").unwrap();
        frames.enter_frame().unwrap();
        let child = frames.declare_owned("子", "第二 UTF-8").unwrap();
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Prepared));
        let result = frames.await_verified_scalar(&mut session, id).unwrap();
        let AsyncSafePointSettlement::Succeeded { value, evidence } = result else {
            panic!("原 P17 正常求值不可偽裝成 Failed");
        };
        assert_eq!(value, Value::Int(19));
        assert_eq!(evidence.unwound_frames, 2);
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(parent), ResourceEvent::Created(child),
            ResourceEvent::Released(child), ResourceEvent::Released(parent),
        ]);
        assert_eq!(evidence.exit_events, [
            AsyncSafeExitEvent::ResourceReleased(child),
            AsyncSafeExitEvent::FrameUnwound(2),
            AsyncSafeExitEvent::ResourceReleased(parent),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Succeeded);
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Succeeded));
        assert_eq!(session.execution_order(), &[id]);
        assert_eq!(frames.resume(), Err(AsyncSafePointError::WrongState));
    }

    #[test]
    fn v7_a1_a2_直接等待原執行預算錯誤完整保留並執行資源回滾() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 3 }".to_vec(),
            ExecutionBudget { max_call_frames: 1, max_steps: 0 },
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("來源", "仍需真釋放").unwrap();
        let result = frames.await_verified_scalar(&mut session, id).unwrap();
        let AsyncSafePointSettlement::Failed(failure) = result else {
            panic!("原 P17 預算耗盡不可偽造成成功");
        };
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Failed));
        let original = session.tasks.get(&id).unwrap()
            .outcome.as_ref().unwrap().result.as_ref().unwrap_err();
        assert_eq!(&failure.error, original);
        assert_eq!(failure.evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert_eq!(failure.evidence.exit_events, [
            AsyncSafeExitEvent::ResourceReleased(resource),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Failed);
    }

    #[test]
    fn v7_a1_a2_活躍借用阻止工作啟動歸還後直接等待才正常退出() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 7 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("持有", "不可以提前釋放").unwrap();
        let ticket = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(24), BorrowKind::SharedRead,
        )).unwrap();
        assert!(matches!(frames.await_verified_scalar(&mut session, id),
            Err(AsyncSafePointError::ActiveBorrowAcrossAwait)));
        assert_eq!(frames.state, AsyncSafePointState::Running);
        assert_eq!(frames.resources.resource_id("持有"), Ok(resource));
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Prepared));
        assert!(session.execution_order().is_empty());
        frames.return_latest_loan().unwrap();
        let result = frames.await_verified_scalar(&mut session, id).unwrap();
        let AsyncSafePointSettlement::Succeeded { value, evidence } = result else {
            panic!("合法歸票之後應取得真結果");
        };
        assert_eq!(value, Value::Int(7));
        assert_eq!(evidence.exit_events, [
            AsyncSafeExitEvent::LoanReturned(ticket),
            AsyncSafeExitEvent::ResourceReleased(resource),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(session.execution_order(), &[id]);
    }

    #[test]
    fn v7_a1_a2_待等待工作被取消不能冒充成功也不釋放現存框架() {
        let mut session = DirectAwaitSession::new();
        let cancelled = session.prepare_verified_scalar(
            b"fn main()->int{ return 1 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        session.cancel_before_start(cancelled).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("來源", "仍然有效").unwrap();
        assert!(matches!(frames.await_verified_scalar(&mut session, cancelled),
            Err(AsyncSafePointError::DirectAwait(DirectAwaitError::Cancelled))));
        assert_eq!(frames.state, AsyncSafePointState::Running);
        assert_eq!(frames.resources.resource_id("來源"), Ok(resource));
        assert!(session.execution_order().is_empty());
        let valid = session.prepare_verified_scalar(
            b"fn main()->int{ return 2 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let result = frames.await_verified_scalar(&mut session, valid).unwrap();
        assert!(matches!(result, AsyncSafePointSettlement::Succeeded {
            value: Value::Int(2), ..
        }));
        assert_eq!(session.execution_order(), &[valid]);
    }

    #[test]
    fn v7_a1_a2_等待器自身活躍借用亦不得偷啟動並損失框架資源() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 6 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let ticket = session.loans.acquire(DeclarationId(301), BorrowKind::ExclusiveWrite).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("唯一", "未啟動").unwrap();
        assert_eq!(frames.await_verified_scalar(&mut session, id).err(),
            Some(AsyncSafePointError::DirectAwait(
                DirectAwaitError::ActiveBorrowAcrossAwait)));
        assert_eq!(frames.state, AsyncSafePointState::Running);
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Prepared));
        assert_eq!(frames.resources.resource_id("唯一"), Ok(resource));
        session.loans.release(ticket).unwrap();
        let result = frames.await_verified_scalar(&mut session, id).unwrap();
        assert!(matches!(result, AsyncSafePointSettlement::Succeeded {
            value: Value::Int(6), ..
        }));
    }

    #[test]
    fn v7_a2_e4_真執行中錯誤跨兩框架先還票再逆序釋放資源() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let parent = frames.declare_owned("外", "父層🪼").unwrap();
        let outer_ticket = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(74), BorrowKind::SharedRead,
        )).unwrap();
        frames.enter_frame().unwrap();
        let child_first = frames.declare_owned("內一", "子一").unwrap();
        let child_second = frames.declare_owned("內二", "子二").unwrap();
        let inner_ticket = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(75), BorrowKind::ExclusiveWrite,
        )).unwrap();
        let original = RuntimeFailure {
            span: None, reason: "既有 P17 執行期失敗",
        };
        let failed = frames.fail_running(original.clone()).unwrap();
        assert_eq!(failed.error, original);
        assert_eq!(failed.evidence.unwound_frames, 2);
        assert_eq!(failed.evidence.resource_events, [
            ResourceEvent::Created(parent),
            ResourceEvent::Created(child_first),
            ResourceEvent::Created(child_second),
            ResourceEvent::Released(child_second),
            ResourceEvent::Released(child_first),
            ResourceEvent::Released(parent),
        ]);
        assert_eq!(failed.evidence.exit_events, [
            AsyncSafeExitEvent::LoanReturned(inner_ticket),
            AsyncSafeExitEvent::ResourceReleased(child_second),
            AsyncSafeExitEvent::ResourceReleased(child_first),
            AsyncSafeExitEvent::FrameUnwound(2),
            AsyncSafeExitEvent::LoanReturned(outer_ticket),
            AsyncSafeExitEvent::ResourceReleased(parent),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Failed);
        assert!(frames.loans.verify_static_snapshot(std::iter::empty()).is_ok());
        assert_eq!(frames.resume(), Err(AsyncSafePointError::WrongState));
        assert!(matches!(frames.cancel_suspended(), Err(AsyncSafePointError::WrongState)));
    }

    #[test]
    fn v7_a2_e4_原P17真步數錯誤傳到等待者並清理仍持有的UTF8() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 11 }".to_vec(),
            ExecutionBudget { max_call_frames: 8, max_steps: 0 },
        ).unwrap();
        let outcome = session.await_direct(id).unwrap();
        let error = outcome.result.as_ref().unwrap_err().clone();
        // await_direct 回傳工作內部 outcome 的借用；先完成事件斷言
        // 再讀取 session 狀態，不能交錯可變與不可變借用。
        assert!(outcome.events.is_empty());
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Failed));
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("來源", "異常仍須清理🪼").unwrap();
        let failure = frames.fail_running(error.clone()).unwrap();
        assert_eq!(failure.error, error);
        assert_eq!(failure.evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert_eq!(failure.evidence.exit_events, [
            AsyncSafeExitEvent::ResourceReleased(resource),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Failed);
    }

    #[test]
    fn v7_a2_e4_尚未真正進框架與安全暫停時不可冒充執行錯誤() {
        let mut frames = AsyncSafePointFrames::new();
        let error = RuntimeFailure { span: None, reason: "測試錯誤" };
        assert!(matches!(frames.fail_running(error.clone()),
            Err(AsyncSafePointError::WrongState)));
        frames.enter_frame().unwrap();
        let original = frames.declare_owned("原本", "仍存活").unwrap();
        frames.suspend().unwrap();
        assert!(matches!(frames.fail_running(error.clone()),
            Err(AsyncSafePointError::WrongState)));
        assert_eq!(frames.resources.resource_id("原本"), Ok(original));
        frames.resume().unwrap();
        let failed = frames.fail_running(error).unwrap();
        assert_eq!(failed.evidence.resource_events, [
            ResourceEvent::Created(original), ResourceEvent::Released(original),
        ]);
        assert!(matches!(frames.fail_running(RuntimeFailure {
            span: None, reason: "不得重播",
        }), Err(AsyncSafePointError::WrongState)));
    }

    #[test]
    fn v7_a2_e4_同框架雙真借用在錯誤邊逆序歸還後才釋放() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("同層", "UTF-8").unwrap();
        let first = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(101), BorrowKind::SharedRead,
        )).unwrap();
        let second = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(102), BorrowKind::SharedRead,
        )).unwrap();
        let failure = frames.fail_running(RuntimeFailure {
            span: None, reason: "已核准執行期故障",
        }).unwrap();
        assert_eq!(failure.evidence.exit_events, [
            AsyncSafeExitEvent::LoanReturned(second),
            AsyncSafeExitEvent::LoanReturned(first),
            AsyncSafeExitEvent::ResourceReleased(resource),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
        assert_eq!(failure.evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
    }

    #[test]
    fn v7_a2_父子真resourceid於暫停取消時按內外逆序唯一釋放() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let parent = frames.declare_owned("父", "繁體🪼").unwrap();
        frames.enter_frame().unwrap();
        let child = frames.declare_owned("子", "UTF-8 子").unwrap();
        frames.suspend().unwrap();
        let outcome = frames.cancel_suspended().unwrap();
        assert_eq!(outcome.unwound_frames, 2);
        assert_eq!(outcome.resource_events, [
            ResourceEvent::Created(parent),
            ResourceEvent::Created(child),
            ResourceEvent::Released(child),
            ResourceEvent::Released(parent),
        ]);
        assert!(outcome.exit_events.windows(2).any(|two|
            matches!(two, [AsyncSafeExitEvent::ResourceReleased(id),
                AsyncSafeExitEvent::FrameUnwound(2)] if *id == child)));
        assert_eq!(frames.state, AsyncSafePointState::Cancelled);
    }

    #[test]
    fn v7_a2_真票據存活時不可暫停而已歸還後可安全取消() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("文", "甲").unwrap();
        let ticket = frames.acquire_for_frame(BorrowFact::new(
            DeclarationId(45), BorrowKind::SharedRead,
        )).unwrap();
        assert_eq!(frames.suspend(), Err(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(frames.state, AsyncSafePointState::Running);
        frames.return_latest_loan().unwrap();
        assert!(frames.loans.verify_static_snapshot(std::iter::empty()).is_ok());
        frames.suspend().unwrap();
        let outcome = frames.cancel_suspended().unwrap();
        assert_eq!(outcome.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert_eq!(outcome.exit_events, [
            AsyncSafeExitEvent::LoanReturned(ticket),
            AsyncSafeExitEvent::ResourceReleased(resource),
            AsyncSafeExitEvent::FrameUnwound(1),
        ]);
    }

    #[test]
    fn v7_a2_真票據獨占衝突不可繞過暫停前安全檢查() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let id = DeclarationId(99);
        let shared = frames.acquire_for_frame(
            BorrowFact::new(id, BorrowKind::SharedRead)).unwrap();
        assert_eq!(frames.acquire_for_frame(
            BorrowFact::new(id, BorrowKind::ExclusiveWrite)
        ), Err(AsyncSafePointError::Borrow(BorrowSafetyError::BorrowConflict)));
        assert_eq!(frames.suspend(), Err(AsyncSafePointError::ActiveBorrowAcrossAwait));
        frames.return_latest_loan().unwrap();
        assert_eq!(shared.fact(), BorrowFact::new(id, BorrowKind::SharedRead));
        frames.suspend().unwrap();
        assert_eq!(frames.cancel_suspended().unwrap().unwound_frames, 1);
    }

    #[test]
    fn v7_a2_取消不是任意執行中強制中斷且失敗不遺失真持有者() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let id = frames.declare_owned("本", "安全").unwrap();
        assert!(matches!(frames.cancel_suspended(), Err(AsyncSafePointError::WrongState)));
        assert_eq!(frames.resources.resource_id("本"), Ok(id));
        frames.suspend().unwrap();
        frames.resume().unwrap();
        assert_eq!(frames.resources.resource_id("本"), Ok(id));
        frames.suspend().unwrap();
        let outcome = frames.cancel_suspended().unwrap();
        assert_eq!(outcome.resource_events, [
            ResourceEvent::Created(id), ResourceEvent::Released(id),
        ]);
        assert_eq!(frames.resume(), Err(AsyncSafePointError::WrongState));
    }

    #[test]
    fn v7_a2_真正求值故障暫存資源先清理再安全取消現存持有者() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let id = frames.declare_owned("本", "初值").unwrap();
        assert_eq!(frames.resources.assign_checked("本", "失敗值", true),
            Err(StrRuntimeError::EvaluationFailed));
        frames.suspend().unwrap();
        let outcome = frames.cancel_suspended().unwrap();
        assert!(matches!(outcome.resource_events.as_slice(), [
            ResourceEvent::Created(original),
            ResourceEvent::Created(temporary),
            ResourceEvent::Released(released_temporary),
            ResourceEvent::Released(released_original),
        ] if *original == id && temporary == released_temporary
            && original == released_original && temporary != original));
    }
    #[test]
    fn v7_a2_分段安全點允許其他已核准工作先執行再喚醒原工作() {
        let mut session = DirectAwaitSession::new();
        let first = session.prepare_verified_scalar(
            b"fn main()->int{ return 21 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let second = session.prepare_verified_scalar(
            b"fn main()->int{ return 34 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let parent = frames.declare_owned("父", "父🪼").unwrap();
        frames.enter_frame().unwrap();
        let child = frames.declare_owned("子", "子🪼").unwrap();
        frames.suspend_verified_await(&mut session, first).unwrap();
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert_eq!(session.status(first), Some(DirectAwaitStatus::Suspended));
        assert!(session.execution_order().is_empty());
        assert_eq!(session.await_direct(first).err(), Some(DirectAwaitError::AlreadyAwaited));
        assert_eq!(frames.resume(), Err(AsyncSafePointError::WrongState));
        assert_eq!(session.await_direct(second).unwrap().result, Ok(Value::Int(34)));
        let result = frames.wake_verified_await(&mut session, first).unwrap();
        let AsyncSafePointSettlement::Succeeded { value, evidence } = result else {
            panic!("安全喚醒後應取得原 Checker 真值");
        };
        assert_eq!(value, Value::Int(21));
        assert_eq!(session.execution_order(), &[second, first]);
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(parent), ResourceEvent::Created(child),
            ResourceEvent::Released(child), ResourceEvent::Released(parent),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Succeeded);
    }

    #[test]
    fn v7_a2_已暫停工作取消先阻止啟動再真實反序清理() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 10 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let parent = frames.declare_owned("外", "外層").unwrap();
        frames.enter_frame().unwrap();
        let child = frames.declare_owned("內", "內層").unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        assert!(matches!(frames.cancel_suspended(), Err(AsyncSafePointError::WrongState)));
        let evidence = frames.cancel_verified_await(&mut session, id).unwrap();
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Cancelled));
        assert!(session.execution_order().is_empty());
        assert!(session.tasks.get(&id).unwrap().outcome.is_none());
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(parent), ResourceEvent::Created(child),
            ResourceEvent::Released(child), ResourceEvent::Released(parent),
        ]);
        assert_eq!(frames.state, AsyncSafePointState::Cancelled);
        assert!(matches!(frames.wake_verified_await(&mut session, id),
            Err(AsyncSafePointError::WrongState)));
    }

    #[test]
    fn v7_a2_不同會話相同工作編號不能冒用喚醒或取消() {
        let mut original = DirectAwaitSession::new();
        let id = original.prepare_verified_scalar(
            b"fn main()->int{ return 17 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut impostor = DirectAwaitSession::new();
        let fake = impostor.prepare_verified_scalar(
            b"fn main()->int{ return 99 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        assert_eq!(id, fake);
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        frames.suspend_verified_await(&mut original, id).unwrap();
        assert!(matches!(frames.wake_verified_await(&mut impostor, fake),
            Err(AsyncSafePointError::WrongState)));
        assert!(matches!(frames.cancel_verified_await(&mut impostor, fake),
            Err(AsyncSafePointError::WrongState)));
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert_eq!(impostor.status(fake), Some(DirectAwaitStatus::Prepared));
        let result = frames.wake_verified_await(&mut original, id).unwrap();
        assert!(matches!(result, AsyncSafePointSettlement::Succeeded {
            value: Value::Int(17), ..
        }));
        assert!(impostor.execution_order().is_empty());
    }

    #[test]
    fn v7_a2_暫停後會話新增真借用需歸還才能喚醒() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 8 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        let ticket = session.loans.acquire(
            DeclarationId(203), BorrowKind::SharedRead,
        ).unwrap();
        assert!(matches!(frames.wake_verified_await(&mut session, id),
            Err(AsyncSafePointError::DirectAwait(
                DirectAwaitError::ActiveBorrowAcrossAwait))));
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Suspended));
        session.loans.release(ticket).unwrap();
        let result = frames.wake_verified_await(&mut session, id).unwrap();
        assert!(matches!(result, AsyncSafePointSettlement::Succeeded {
            value: Value::Int(8), ..
        }));
    }

    #[test]
    fn v7_a2_工作在暫停期間被提前取消仍可清理等待框架() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 4 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("保留", "UTF-8🪼").unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        session.cancel_before_start(id).unwrap();
        assert!(matches!(frames.wake_verified_await(&mut session, id),
            Err(AsyncSafePointError::DirectAwait(DirectAwaitError::Cancelled))));
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert_eq!(frames.resources.resource_id("保留"), Ok(resource));
        let evidence = frames.cancel_verified_await(&mut session, id).unwrap();
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert!(session.execution_order().is_empty());
    }


    #[test]
    fn v7_a2_喚醒前等待者新增真票據不得繞過VB15且狀態原子保留() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 31 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("持有", "待恢復🪼").unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        // 動態追蹤器有真正票據，即使未加入框架清單也不能被誤判為空。
        let ticket = frames.loans.acquire(
            DeclarationId(901), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(frames.wake_verified_await(&mut session, id).err(),
            Some(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert!(frames.pending_verified_await.is_some());
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Suspended));
        assert!(session.execution_order().is_empty());
        assert_eq!(frames.resources.resource_id("持有"), Ok(resource));
        frames.loans.release(ticket).unwrap();
        let AsyncSafePointSettlement::Succeeded { value, evidence } =
            frames.wake_verified_await(&mut session, id).unwrap() else {
                panic!("真票據歸還後才應恢復成功");
            };
        assert_eq!(value, Value::Int(31));
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert_eq!(session.execution_order(), &[id]);
    }

    #[test]
    fn v7_a2_取消前等待者真票據未還不得提前消耗工作及持有者() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 32 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("本體", "不能提前釋放").unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        let ticket = frames.loans.acquire(
            DeclarationId(902), BorrowKind::ExclusiveWrite,
        ).unwrap();
        assert_eq!(frames.cancel_verified_await(&mut session, id).err(),
            Some(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Suspended));
        assert!(session.tasks.get(&id).unwrap().plan.is_some());
        assert!(session.execution_order().is_empty());
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert!(frames.pending_verified_await.is_some());
        assert_eq!(frames.resources.resource_id("本體"), Ok(resource));
        frames.loans.release(ticket).unwrap();
        let evidence = frames.cancel_verified_await(&mut session, id).unwrap();
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Cancelled));
        assert!(session.tasks.get(&id).unwrap().plan.is_none());
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert!(session.execution_order().is_empty());
    }

    #[test]
    fn v7_a2_普通resume亦須重新核對暫停後加入的真正票據() {
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("原", "資源🪼").unwrap();
        frames.suspend().unwrap();
        let ticket = frames.loans.acquire(
            DeclarationId(903), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(frames.resume(), Err(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(frames.state, AsyncSafePointState::Suspended);
        assert_eq!(frames.resources.resource_id("原"), Ok(resource));
        frames.loans.release(ticket).unwrap();
        frames.resume().unwrap();
        assert_eq!(frames.state, AsyncSafePointState::Running);
        frames.suspend().unwrap();
        let evidence = frames.cancel_suspended().unwrap();
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
    }

    #[test]
    fn v7_a2_暫停期間框架清單仍有真BorrowFact應禁止取消和喚醒() {
        let mut session = DirectAwaitSession::new();
        let id = session.prepare_verified_scalar(
            b"fn main()->int{ return 33 }".to_vec(), ExecutionBudget::default(),
        ).unwrap();
        let mut frames = AsyncSafePointFrames::new();
        frames.enter_frame().unwrap();
        let resource = frames.declare_owned("清單", "同一持有者").unwrap();
        frames.suspend_verified_await(&mut session, id).unwrap();
        let fact = BorrowFact::new(DeclarationId(904), BorrowKind::SharedRead);
        let ticket = frames.loans.acquire(fact.source, fact.kind).unwrap();
        frames.frames.last_mut().unwrap().push((ticket, fact));
        assert_eq!(frames.cancel_verified_await(&mut session, id).err(),
            Some(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(frames.wake_verified_await(&mut session, id).err(),
            Some(AsyncSafePointError::ActiveBorrowAcrossAwait));
        assert_eq!(session.status(id), Some(DirectAwaitStatus::Suspended));
        assert_eq!(frames.resources.resource_id("清單"), Ok(resource));
        frames.loans.finish_selected_verified(&[ticket], &[fact]).unwrap();
        frames.frames.last_mut().unwrap().pop();
        let evidence = frames.cancel_verified_await(&mut session, id).unwrap();
        assert_eq!(evidence.resource_events, [
            ResourceEvent::Created(resource), ResourceEvent::Released(resource),
        ]);
        assert!(session.execution_order().is_empty());
    }
}
