// P18.5-T4／V7-E3：原 Checker FIFO 取消／shutdown 對
// Prepared、Suspended 都必須具有真先前 poll／checkpoint
// 與唯一取消歷史；不能靠 status 標籤偽造可清理性。
#[cfg(test)]
mod g18_t4_pending_cancel_evidence_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn value()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var hold:str = \"T4 原真待取消資源🪼\"\n",
            "return await value()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("必須由原 Parser／Checker 建立真正 async P16 工作")
    }

    #[test]
    fn t4_pending_從未啟動Prepared可取消但絕不憑空產生函式或資源事件() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.ready_ids(), [id]);
        q.cancel(id).unwrap();
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.cancellation_order(), &[id]);
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        assert!(q.drive_order().is_empty());
        assert!(q.ready_ids().is_empty());
    }

    #[test]
    fn t4_pending_真正Suspended在安全點取消保留R05與P16完整反序退出() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        q.cancel(id).unwrap();
        let PrivateScalarRetiredJob::Cancelled(Some(cancelled)) =
            q.retire_terminal(id).unwrap() else {
                panic!("必須交付原執行中唯一 R05/P16 清理證據");
            };
        let audited = PrivateScalarCallOutcome {
            result: Err(PrivateScalarCallRuntimeError::StateMismatch),
            events: cancelled.events,
            resource_events: cancelled.resource_events,
            parameter_slots_after_exit: 0,
        };
        assert!(audited.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &audited.resource_events,
        ));
    }

    #[test]
    fn t4_pending_完成工作偽造Prepared不能消耗真結果也不可再取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let completed = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((completed, PrivateScalarGraphPoll::Suspended)));
        q.wake(completed).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((completed, PrivateScalarGraphPoll::Completed)));
        q.jobs.get_mut(&completed).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        let pending = q.submit(&plan).unwrap();
        let before_ready = q.ready_ids();
        assert_eq!(q.cancel(completed), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.ready_ids(), before_ready);
        assert!(q.outcome(completed).is_some());
        assert!(q.cancellation_order().is_empty());
    }

    #[test]
    fn t4_pending_已經取消的Prepared若偽造回待命不得重新取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.cancel(id).unwrap();
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        assert_eq!(q.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.cancellation_order(), &[id]);
        assert!(q.jobs.contains_key(&id));
    }

    #[test]
    fn t4_pending_從未啟動Prepared卻已有Unwind事件不能藉取消洗掉證據() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.jobs.get_mut(&id).unwrap().continuation.events.push(
            PrivateScalarCallEvent::UnwindStarted,
        );
        assert_eq!(q.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.jobs.get(&id).unwrap().continuation.events.len(), 1);
        assert_eq!(q.ready_ids(), [id]);
    }

    #[test]
    fn t4_pending_假Suspended但無原poll和checkpoint不得擬作安全點() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Suspended;
        assert_eq!(q.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert!(q.drive_order().is_empty());
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
    }

    #[test]
    fn t4_pending_真Completed偽裝Suspended不可以重用已完成的身份() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Suspended;
        assert_eq!(q.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert!(q.outcome(id).is_some());
        assert_eq!(q.drive_order().last(), Some(&(id, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_pending_真wake後Suspended仍可取消並按逆登錄順序shutdown() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let first = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((first, PrivateScalarGraphPoll::Suspended)));
        q.wake(first).unwrap();
        let second = q.submit(&plan).unwrap();
        assert_eq!(q.ready_ids(), [first, second]);
        // 已 wake 但尚未再次 poll，真 checkpoint／借票仍屬原
        // Suspended，shutdown 必允許在合法安全點先取消後登錄者。
        assert_eq!(q.shutdown().unwrap(), [second, first]);
        assert!(q.ready_ids().is_empty());
        assert_eq!(q.cancellation_order(), &[second, first]);
        assert_eq!(q.status(first), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.status(second), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(matches!(q.retire_terminal(first).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
        assert!(matches!(q.retire_terminal(second).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
    }
}
