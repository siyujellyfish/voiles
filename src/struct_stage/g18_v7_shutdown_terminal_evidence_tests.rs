// G18／P18.5-T4／V7-A2：整批 shutdown 不能只看
// terminal checkpoint 已清空，必須先核對每份工作真正的
// FIFO、R05／HC14、P16 證據，再取消仍待執行的任何一份。
#[cfg(test)]
mod g18_v7_shutdown_terminal_evidence_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"批次關閉原 Checker UTF-8🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )).expect("僅用原 Parser／Checker 核准來源")
    }

    fn complete(
        queue: &mut PrivateScalarCooperativeQueue<'_>, id: u64,
    ) {
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(11));
    }

    fn suspended(
        queue: &mut PrivateScalarCooperativeQueue<'_>, id: u64,
    ) {
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_shutdown_已完成工作保留真結果且待取消工作逆登錄順序安全關閉() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let complete_id = queue.submit(&plan).unwrap();
        complete(&mut queue, complete_id);
        let suspended_id = queue.submit(&plan).unwrap();
        suspended(&mut queue, suspended_id);
        let prepared_id = queue.submit(&plan).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [prepared_id, suspended_id]);
        assert_eq!(queue.cancellation_order(), &[prepared_id, suspended_id]);
        assert_eq!(queue.status(complete_id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert!(queue.outcome(complete_id).unwrap().has_balanced_frame_trace());
        assert_eq!(queue.ready_ids(), []);
    }

    #[test]
    fn t4_shutdown_正常已取消Prepared與已執行Suspended仍可重複安全預檢() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let live = queue.submit(&plan).unwrap();
        suspended(&mut queue, live);
        let unstarted = queue.submit(&plan).unwrap();
        queue.cancel(live).unwrap();
        queue.cancel(unstarted).unwrap();
        assert_eq!(queue.shutdown().unwrap(), []);
        assert_eq!(queue.cancellation_order(), &[live, unstarted]);
        assert!(queue.cancellation(live).is_some());
        assert!(queue.cancellation(unstarted).is_none());
        assert_eq!(queue.status(live), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn t4_shutdown_某已取消真框架刪Unwind時拒絕並保留其他Prepared() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let cancelled = queue.submit(&plan).unwrap();
        suspended(&mut queue, cancelled);
        queue.cancel(cancelled).unwrap();
        let pending = queue.submit(&plan).unwrap();
        let actual = queue.jobs.get_mut(&cancelled).unwrap().cancellation.as_mut().unwrap();
        let original = actual.events.clone();
        actual.events.retain(|e| !matches!(e, PrivateScalarCallEvent::UnwindStarted));
        let ready = queue.ready_ids();
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), ready);
        assert_eq!(queue.cancellation_order(), &[cancelled]);
        assert_eq!(queue.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        queue.jobs.get_mut(&cancelled).unwrap().cancellation.as_mut().unwrap().events = original;
        assert_eq!(queue.shutdown().unwrap(), [pending]);
    }

    #[test]
    fn t4_shutdown_某已完成工作事件帳被竄改必先拒絕而不取消其他工作() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let finished = queue.submit(&plan).unwrap();
        complete(&mut queue, finished);
        let pending = queue.submit(&plan).unwrap();
        let outcome = queue.jobs.get_mut(&finished).unwrap()
            .continuation.outcome.as_mut().unwrap();
        let original = outcome.events.clone();
        outcome.events.retain(|e| !matches!(e,
            PrivateScalarCallEvent::FunctionExited { returned: true, .. }
        ));
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [pending]);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        queue.jobs.get_mut(&finished).unwrap()
            .continuation.outcome.as_mut().unwrap().events = original;
        assert_eq!(queue.shutdown().unwrap(), [pending]);
    }

    #[test]
    fn t4_shutdown_從未驅動卻假標Succeeded不能阻止後續來源修復() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let faked = queue.submit(&plan).unwrap();
        let pending = queue.submit(&plan).unwrap();
        queue.jobs.get_mut(&faked).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Succeeded;
        queue.ready.retain(|item| *item != faked);
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [pending]);
        assert!(queue.cancellation_order().is_empty());
        queue.jobs.get_mut(&faked).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        queue.ready.push_front(faked);
        assert_eq!(queue.shutdown().unwrap(), [pending, faked]);
    }

    #[test]
    fn t4_shutdown_原Checker真預算耗盡Failed有完整R05帳可安全關閉其餘工作() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let failed = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((failed, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(failed).unwrap().result,
            Err(PrivateScalarCallRuntimeError::CallBudget));
        let pending = queue.submit(&plan).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [pending]);
        assert_eq!(queue.status(failed), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(queue.status(pending), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.outcome(failed).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn t4_shutdown_終結Succeeded被移除原結果即使無checkpoint也不得報安全關閉() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let finished = queue.submit(&plan).unwrap();
        complete(&mut queue, finished);
        let pending = queue.submit(&plan).unwrap();
        let original = queue.jobs.get_mut(&finished).unwrap()
            .continuation.outcome.take().unwrap();
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [pending]);
        assert_eq!(queue.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        queue.jobs.get_mut(&finished).unwrap().continuation.outcome = Some(original);
        assert_eq!(queue.shutdown().unwrap(), [pending]);
    }

    #[test]
    fn t4_shutdown_已真執行取消工作被篡成無清理證據仍須整批零更動() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let cancelled = queue.submit(&plan).unwrap();
        suspended(&mut queue, cancelled);
        queue.cancel(cancelled).unwrap();
        let pending = queue.submit(&plan).unwrap();
        let original = queue.jobs.get_mut(&cancelled).unwrap()
            .cancellation.take().unwrap();
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [pending]);
        assert_eq!(queue.cancellation_order(), &[cancelled]);
        assert_eq!(queue.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        queue.jobs.get_mut(&cancelled).unwrap().cancellation = Some(original);
        assert_eq!(queue.shutdown().unwrap(), [pending]);
        assert!(matches!(queue.retire_terminal(cancelled).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }
}
