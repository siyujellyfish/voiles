// G18／P18.5-T4／V7-E2：兩個以上合法 ready token 即使各自
// wake 次數與 poll 時間都正確，也不能互換全域 FIFO 次序。
#[cfg(test)]
mod g18_t4_fifo_issued_token_order_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn leaf()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var owned:str = \"真FIFO入列憑據🪼\"\n",
            "return await leaf()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("原 Lexer／Parser／Checker 真來源不得被測試偽造")
    }

    fn suspended<'a>(q: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall) -> u64 {
        let id = q.submit(plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        id
    }

    #[test]
    fn t4_issue_兩份合法Prepared交換ready仍須拒絕不執行() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = q.submit(&plan).unwrap();
        let b = q.submit(&plan).unwrap();
        assert_eq!(q.ready_ids(), [a,b]);
        q.ready.swap(0,1);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [b,a]);
        assert!(q.drive_order().is_empty());
        q.ready.swap(0,1);
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_issue_兩份合法Suspended依wake先後入列不可偷換順序() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = suspended(&mut q, &plan);
        q.wake(a).unwrap();
        q.wake(b).unwrap();
        assert_eq!(q.ready_ids(), [a,b]);
        assert_eq!(q.wake_order(), [a,b]);
        q.ready.swap(0,1);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order().len(), 2);
        assert_eq!(q.ready_ids(), [b,a]);
        q.ready.swap(0,1);
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(a).unwrap().result, Ok(11));
        assert_eq!(q.outcome(b).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_issue_先存在Prepared後真wake須保持原混合入列順序() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = q.submit(&plan).unwrap();
        let b = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Suspended)));
        q.wake(a).unwrap();
        assert_eq!(q.ready_ids(), [b,a]);
        q.ready.swap(0,1);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [a,b]);
        q.ready.swap(0,1);
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_issue_偽增第二份wake入列紀錄不得創造新token() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.ready_issues.push(PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Woken,
        });
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.wake_order(), [id]);
        q.ready_issues.pop();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_issue_偽造重複submit授權即使ready未變也拒絕() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.ready_issues.push(PrivateScalarReadyIssue {
            id, kind: PrivateScalarReadyIssueKind::Submitted,
        });
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order(), []);
        q.ready_issues.pop();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_issue_刪除真提交來源即使留單張ready仍不可動() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        let original = q.ready_issues.pop().unwrap();
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Prepared));
        q.ready_issues.push(original);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_issue_合法cancel刪除佇列中間token仍可保持剩餘FIFO() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = q.submit(&plan).unwrap();
        let c = q.submit(&plan).unwrap();
        q.wake(a).unwrap();
        assert_eq!(q.ready_ids(), [b,c,a]);
        q.cancel(c).unwrap();
        assert_eq!(q.ready_ids(), [b,a]);
        assert_eq!(q.cancellation_order(), [c]);
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.shutdown().unwrap(), [b]);
        assert_eq!(q.cancellation_order(), [c,b]);
    }

    #[test]
    fn t4_issue_錯序時shutdown與退休須全零副作用修復後正常() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = suspended(&mut q, &plan);
        let c = suspended(&mut q, &plan);
        q.wake(a).unwrap();
        q.wake(b).unwrap();
        q.wake(c).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.ready_ids(), [b,c]);
        q.ready.swap(0,1);
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.cancellation_order(), []);
        assert_eq!(q.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.status(c), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(matches!(q.retire_terminal(a),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(q.status(a), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.ready_ids(), [c,b]);
        q.ready.swap(0,1);
        assert!(matches!(q.retire_terminal(a).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
        assert_eq!(q.shutdown().unwrap(), [c,b]);
        assert_eq!(q.cancellation_order(), [c,b]);
    }
}
