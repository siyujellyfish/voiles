// G18／P18.5-T4／V7-E2：對帳原 submit／wake 簽發與尚未
// 消耗的所有 ready token；不能刪掉合法 token 讓 FIFO 假裝已空。
#[cfg(test)]
mod g18_t4_fifo_missing_ready_conservation_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn leaf()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var owned:str = \"佇列遺失不可假完成🪼\"\n",
            "return await leaf()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("只能由原 Lexer／Parser／Checker 核准 async 真來源")
    }

    fn suspend<'a>(q: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall) -> u64 {
        let id = q.submit(plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        id
    }

    #[test]
    fn t4_missing_單一Prepared被偷刪ready不可假報空佇列() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.ready.pop_front(), Some(id));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), []);
        assert!(q.drive_order().is_empty());
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Prepared));
        q.ready.push_back(id);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_missing_後一Prepared被刪仍不可先執行佇列頭() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let first = q.submit(&plan).unwrap();
        let last = q.submit(&plan).unwrap();
        assert_eq!(q.ready.pop_back(), Some(last));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [first]);
        assert_eq!(q.drive_order(), []);
        q.ready.push_back(last);
        assert_eq!(q.drive_one().unwrap(),
            Some((first, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(),
            Some((last, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_missing_真wake後刪除唯一ready不得使Suspended假空() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.ready.pop_front(), Some(id));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order().len(), 1);
        assert_eq!(q.wake_order(), [id]);
        assert!(q.pending_await(id).is_some());
        q.ready.push_back(id);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_missing_兩個真wake刪掉尾端也不得執行合法頭() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspend(&mut q, &plan);
        let b = suspend(&mut q, &plan);
        q.wake(a).unwrap();
        q.wake(b).unwrap();
        assert_eq!(q.ready.pop_back(), Some(b));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [a]);
        assert_eq!(q.drive_order().len(), 2);
        q.ready.push_back(b);
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_missing_整批關閉遇Prepared中間憑據遺失全員不可先取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = q.submit(&plan).unwrap();
        let b = q.submit(&plan).unwrap();
        let c = q.submit(&plan).unwrap();
        assert_eq!(q.ready.remove(1), Some(b));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [a,c]);
        assert_eq!(q.cancellation_order(), []);
        assert_eq!(q.status(a), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.status(b), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.status(c), Some(PrivateDeferredAwaitStatus::Prepared));
        q.ready.insert(1,b);
        assert_eq!(q.shutdown().unwrap(), [c,b,a]);
        assert_eq!(q.cancellation_order(), [c,b,a]);
        assert!(q.ready_ids().is_empty());
    }

    #[test]
    fn t4_missing_合法Suspended尚未wake應真回報無待驅動工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.wake_order(), []);
        assert_eq!(q.drive_one().unwrap(), None);
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
        assert_eq!(q.drive_one().unwrap(), None);
    }

    #[test]
    fn t4_missing_合法wake後安全取消會消耗憑據並允許真空佇列() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        q.cancel(id).unwrap();
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.wake_order(), [id]);
        assert_eq!(q.cancellation_order(), [id]);
        assert_eq!(q.drive_one().unwrap(), None);
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
        assert_eq!(q.drive_one().unwrap(), None);
    }

    #[test]
    fn t4_missing_合法完成退休後歷史仍存但不能假生新ready() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.drive_one().unwrap(), None);
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.drive_one().unwrap(), None);
        let next = q.submit(&plan).unwrap();
        assert_eq!(next, id+1);
        assert_eq!(q.ready_ids(), [next]);
        assert_eq!(q.shutdown().unwrap(), [next]);
    }

    #[test]
    fn t4_missing_退休後最後Submitted歷史被抹除不得假回空() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        assert_eq!(q.ready_ids(), []);
        let issued = q.ready_issues.pop().unwrap();
        assert_eq!(issued, PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Submitted,
        });
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert!(q.drive_order().is_empty());
        assert_eq!(q.next_id, id);
        q.ready_issues.push(issued);
        assert_eq!(q.drive_one().unwrap(), None);
        assert_eq!(q.submit(&plan).unwrap(), id + 1);
    }

    #[test]
    fn t4_missing_雙工作退休後裁掉尾筆提交仍拒絕新工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let first = q.submit(&plan).unwrap();
        let second = q.submit(&plan).unwrap();
        assert_eq!(q.shutdown().unwrap(), [second, first]);
        for id in [second, first] {
            assert!(matches!(q.retire_terminal(id).unwrap(),
                PrivateScalarRetiredJob::Cancelled(None)));
        }
        let issued = q.ready_issues.pop().unwrap();
        assert_eq!(issued, PrivateScalarReadyIssue {
            id: second, kind: PrivateScalarReadyIssueKind::Submitted,
        });
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.cancellation_order(), [second, first]);
        q.ready_issues.push(issued);
        assert_eq!(q.drive_one().unwrap(), None);
        assert_eq!(q.submit(&plan).unwrap(), second + 1);
    }

    #[test]
    fn t4_missing_退休後next_id大於真提交歷史不得續發() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        q.next_id += 1;
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert!(q.ready_ids().is_empty());
        q.next_id = id;
        assert_eq!(q.submit(&plan).unwrap(), id + 1);
    }

    #[test]
    fn t4_missing_全空佇列偽造發號上限不可冒充零工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        assert_eq!(q.drive_one().unwrap(), None);
        q.next_id = 1;
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), []);
        assert!(q.drive_order().is_empty());
        q.next_id = 0;
        assert_eq!(q.drive_one().unwrap(), None);
        assert_eq!(q.submit(&plan).unwrap(), 1);
    }

    #[test]
    fn t4_missing_退休完成後同步刪除Woken與wake收據也不可假報空() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        let issue = q.ready_issues.pop().unwrap();
        let receipt = q.wake_order.pop().unwrap();
        assert_eq!(issue, PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Woken,
        });
        assert_eq!(receipt.id, id);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.next_id, id);
        assert_eq!(q.ready_ids(), []);
        q.ready_issues.push(issue);
        q.wake_order.push(receipt);
        assert_eq!(q.drive_one().unwrap(), None);
        assert_eq!(q.submit(&plan).unwrap(), id + 1);
    }

    #[test]
    fn t4_missing_退休完成後篡改wake時間不得跳過原poll時序() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        let original = q.wake_order[0];
        assert_eq!(original.after_drive_count, 1);
        q.wake_order[0].after_drive_count = 0;
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        q.wake_order[0] = original;
        assert_eq!(q.drive_one().unwrap(), None);
    }

    #[test]
    fn t4_missing_退休後插入不存在工作身分的幽靈poll必拒絕() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        q.drive_order.push((id + 1, PrivateScalarGraphPoll::Suspended));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order.pop(),
            Some((id + 1, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(), None);
        assert_eq!(q.submit(&plan).unwrap(), id + 1);
    }

    #[test]
    fn t4_missing_退休完成後偽造Completed再Suspended不可重啟已交付工作() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspend(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        q.drive_order.push((id, PrivateScalarGraphPoll::Suspended));
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order.pop(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(), None);
    }

    #[test]
    fn t4_missing_另一Prepared仍存活時退休wake竄改須整批零取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let retired = suspend(&mut q, &plan);
        q.wake(retired).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((retired, PrivateScalarGraphPoll::Completed)));
        assert!(matches!(q.retire_terminal(retired).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        let live = q.submit(&plan).unwrap();
        let issue = q.ready_issues.remove(1);
        let receipt = q.wake_order.pop().unwrap();
        assert_eq!(issue.unwrap().kind, PrivateScalarReadyIssueKind::Woken);
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [live]);
        assert_eq!(q.status(live), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.cancellation_order(), []);
        q.ready_issues.insert(1, PrivateScalarReadyIssue {
            id: retired, kind: PrivateScalarReadyIssueKind::Woken,
        });
        q.wake_order.push(receipt);
        assert_eq!(q.shutdown().unwrap(), [live]);
        assert_eq!(q.cancellation_order(), [live]);
    }

    #[test]
    fn t4_missing_退休後偽造未知工作取消歷史不得延用空佇列() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        q.cancellation_order.push(id + 1);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.cancellation_order.pop(), Some(id + 1));
        assert_eq!(q.drive_one().unwrap(), None);
    }
}
