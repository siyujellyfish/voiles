// P18.5-T4／V7-E2：真正醒來／驅動 FIFO 之前，也必須
// 與已驗收 cancel／shutdown 共用原 Checker 工作的完整
// poll／checkpoint／取消與結果證據，不得錯用假 Prepared。
#[cfg(test)]
mod g18_t4_verified_wake_drive_history_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn value()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var hold:str = \"T4 真 wake drive 票據🪼\"\n",
            "return await value()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("須由原 Parser／Checker 簽發合法 async UTF-8 資源工作")
    }

    fn suspended<'a>(
        q: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall,
    ) -> u64 {
        let id = q.submit(plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        id
    }

    #[test]
    fn t4_drive_合法Prepared第一次drive只真執行至await並保留checkpoint() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        assert_eq!(q.drive_order(), &[(id, PrivateScalarGraphPoll::Suspended)]);
        assert!(q.pending_await(id).is_some());
        assert_eq!(q.ready_ids(), []);
        q.cancel(id).unwrap();
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn t4_drive_真Suspended喚醒後才重新排隊並完成同一工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
        assert!(q.ready_ids().is_empty());
        assert_eq!(q.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Completed),
        ]);
    }

    #[test]
    fn t4_drive_偽Completed歷史混入真Suspended後wake須拒絕且零入列() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.drive_order.push((id, PrivateScalarGraphPoll::Completed));
        assert_eq!(q.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert!(q.ready_ids().is_empty());
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(id).is_none());
        q.drive_order.pop();
        q.wake(id).unwrap();
        assert_eq!(q.ready_ids(), [id]);
    }

    #[test]
    fn t4_drive_有真checkpoint但假取消歷史不得以wake重取執行權() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.cancellation_order.push(id);
        assert_eq!(q.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), []);
        assert!(q.pending_await(id).is_some());
        assert_eq!(q.cancellation_order(), &[id]);
        q.cancellation_order.pop();
        q.wake(id).unwrap();
        assert_eq!(q.ready_ids(), [id]);
    }

    #[test]
    fn t4_drive_真暫停後被偽標Prepared不得重新消耗第一個poll() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        q.ready.push_back(id);
        let before = q.ready_ids();
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), before);
        assert_eq!(q.drive_order(), &[(id, PrivateScalarGraphPoll::Suspended)]);
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Suspended;
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_drive_同一工作重複入列不能讓drive_one消耗兩個token() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.ready.push_back(id);
        assert_eq!(q.ready_ids(), [id, id]);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id, id]);
        assert_eq!(q.drive_order().len(), 0);
        q.ready.pop_back();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_drive_未執行Prepared被塞入假Unwind不得污染FIFO後一份工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let first = q.submit(&plan).unwrap();
        let second = q.submit(&plan).unwrap();
        q.jobs.get_mut(&first).unwrap().continuation.events.push(
            PrivateScalarCallEvent::UnwindStarted,
        );
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [first, second]);
        assert_eq!(q.drive_order().len(), 0);
        assert_eq!(q.status(second), Some(PrivateDeferredAwaitStatus::Prepared));
        q.jobs.get_mut(&first).unwrap().continuation.events.clear();
        assert_eq!(q.drive_one().unwrap(),
            Some((first, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(),
            Some((second, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_drive_真完成工作偽標Prepared及重入列不得二次執行原結果() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        q.ready.push_back(id);
        let old_history = q.drive_order().to_vec();
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order(), old_history);
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
        q.ready.pop_front();
        q.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Succeeded;
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
    }
}
