// G18 V7：已核准 E1/E2/E3/E4/E5 的非公開直接等待執行基礎。
// 此階段的工作本體仍是 P17 原 Checker 核准的同步純值來源；
// 原私有 async fn 方法 Parser／Checker 與跨 await 真暫停尚未接通。
// 不把本模組的內部驗證冒稱 P18.6 A1/A2 或 G18 全項 Passed。
use std::collections::BTreeMap;
use super::{ExecutionBudget, ExecutionOutcome, PreparationFailure, PreparedProgram};
use crate::borrow_safety::BorrowSafety;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectAwaitStatus {
    Prepared,
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
        if task.status != DirectAwaitStatus::Prepared {
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
use crate::borrow_safety::{BorrowFact, BorrowKind, BorrowSafetyError, BorrowTicket};
use crate::owned_str_runtime::{self, OwnedStrRuntime, ResourceEvent, StrRuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncSafePointState {
    Running,
    Suspended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncSafePointError {
    WrongState,
    NoFrame,
    NoActiveTicket,
    ActiveBorrowAcrossAwait,
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

/// 暫停狀態保留原 P16 OwnedStrRuntime 和原 HC14 BorrowSafety；
/// 不以一組假的資源事件或 Rust Drop 推導 voiles 退出順序。
/// 已核准 BorrowFact 來源須由未來的 Checker→工作計畫提供。
struct AsyncSafePointFrames {
    state: AsyncSafePointState,
    resources: OwnedStrRuntime,
    loans: BorrowSafety,
    frames: Vec<Vec<(BorrowTicket, BorrowFact)>>,
    exit_events: Vec<AsyncSafeExitEvent>,
}

impl AsyncSafePointFrames {
    fn new() -> Self {
        Self {
            state: AsyncSafePointState::Running,
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

    /// VB15-01-A：仍有效的任意借用票據不得跨 await 暫停點。
    /// 必須先真的歸還票據，不能清空 Vec 偽裝安全。
    fn suspend(&mut self) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Running || self.frames.is_empty() {
            return Err(AsyncSafePointError::WrongState);
        }
        if self.frames.iter().any(|frame| !frame.is_empty())
            || self.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::ActiveBorrowAcrossAwait);
        }
        self.state = AsyncSafePointState::Suspended;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), AsyncSafePointError> {
        if self.state != AsyncSafePointState::Suspended {
            return Err(AsyncSafePointError::WrongState);
        }
        self.state = AsyncSafePointState::Running;
        Ok(())
    }

    /// E3-A：只能在真安全暫停點取消；先內層後外層離域。
    /// 每次 ResourceReleased 均來自 OwnedStrRuntime 剛剛真產生的事件。
    /// 若狀態不符，完整持有者仍留在 self，可依法稍後退出。
    fn cancel_suspended(&mut self) -> Result<AsyncSafePointEvidence, AsyncSafePointError> {
        if self.state != AsyncSafePointState::Suspended {
            return Err(AsyncSafePointError::WrongState);
        }
        if self.frames.iter().any(|frame| !frame.is_empty())
            || self.loans.verify_static_snapshot(std::iter::empty()).is_err() {
            return Err(AsyncSafePointError::ActiveBorrowAcrossAwait);
        }
        let count = self.frames.len();
        while !self.frames.is_empty() {
            let depth = self.frames.len();
            let before = self.resources.events().len();
            self.resources.leave_scope()?;
            let released = self.resources.events()[before..].iter().filter_map(|event| {
                if let ResourceEvent::Released(id) = event { Some(*id) } else { None }
            }).collect::<Vec<_>>();
            for id in released {
                self.exit_events.push(AsyncSafeExitEvent::ResourceReleased(id));
            }
            self.frames.pop();
            self.exit_events.push(AsyncSafeExitEvent::FrameUnwound(depth));
        }
        // 所有真正建立的資源已依相同 OwnedStrRuntime 退出，
        // finish 的完整性檢查不可用合成的事件向量替代。
        let resources = std::mem::replace(&mut self.resources, OwnedStrRuntime::new());
        let resource_events = resources.finish()?;
        if !owned_str_runtime::event_trace_balanced(&resource_events) {
            return Err(AsyncSafePointError::Resource(StrRuntimeError::OutstandingResource));
        }
        self.state = AsyncSafePointState::Cancelled;
        Ok(AsyncSafePointEvidence {
            resource_events,
            exit_events: self.exit_events.clone(),
            unwound_frames: count,
        })
    }
}

#[cfg(test)]
mod v7_a2_safe_point_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;
    use crate::ownership_ledger::DeclarationId;

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
}
