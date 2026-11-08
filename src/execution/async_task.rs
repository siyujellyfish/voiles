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
