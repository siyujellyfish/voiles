// G18／P18.5-T4／V7-E3：真 async self Ready 工作退休後
// 交付唯一結果並移走 Checker 計畫及動態 Outcome，須以
// 独立原全域轉移位置收據保住 Prepared/Wake/Cancel 因果。
#[cfg(test)]
mod g18_v7_async_self_terminal_retirement_tests {
    use super::*;

    const READ: &str = r#"struct Counter{
var n:int = 7
async fn read()->int{
return self.n+3
}
}
fn main()->int{
var owner:Counter = Counter()
return await owner.read()
}
"#;
    const WRITE: &str = r#"struct Counter{
var n:int = 7
async fn set(n:int)->int{
self.n = n+2
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return await owner.set(12)
}
"#;

    fn issue(q: &mut PrivateCheckedDeferredSelfAwaitQueue, source: &str) -> u64 {
        q.prepare(source).expect("真原Checker async self已核准計畫")
    }
    fn legal(q: &PrivateCheckedDeferredSelfAwaitQueue) {
        q.verify_ready_history().expect("退休歷史全部收據一致");
    }

    #[test]
    fn v7_self_retire_正常完成後僅交付一次真方法Outcome不再持有工作幀() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,id);
        let finished=q.retire_terminal(id).unwrap();
        let PrivateSelfRetiredJob::Completed(out)=finished else {
            panic!("成功的async self必交付實際Outcome");
        };
        assert_eq!(out.result,Ok(10));
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(q.jobs.is_empty());
        assert!(q.outcome(id).is_none());
        assert_eq!(q.retired_status(id),Some(PrivateDeferredAwaitStatus::Succeeded));
        assert!(q.retired.get(&id).unwrap().checked_self_loan.is_some());
        legal(&q);
    }

    #[test]
    fn v7_self_retire_獨占self方法真動態結果退休後仍可由呼叫者查驗() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,WRITE);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        let PrivateSelfRetiredJob::Completed(out)=q.retire_terminal(id).unwrap()
            else { panic!("原Checker真self結果已交付"); };
        assert_eq!(out.result,Ok(14));
        assert_eq!(out.checked_loan.kind,BorrowKind::ExclusiveWrite);
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(q.execution_order(),[id]);
        assert!(q.jobs.is_empty());
        legal(&q);
    }

    #[test]
    fn v7_self_retire_受控溢位先Unwind與還票然後以Failed收據退休() {
        let source=READ.replace("return self.n+3",
            "return self.n+9223372036854775807");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,&source);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        let PrivateSelfRetiredJob::Completed(out)=q.retire_terminal(id).unwrap()
            else { panic!("Failed須交付真Runtime Outcome"); };
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(q.retired_status(id),Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(q.retired.get(&id).unwrap().result_succeeded,Some(false));
        legal(&q);
    }

    #[test]
    fn v7_self_retire_Prepared未啟動安全取消後只有Cancelled沒有假借票() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateSelfRetiredJob::Cancelled));
        assert_eq!(q.retired_status(id),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.retired.get(&id).unwrap().checked_self_loan,None);
        assert!(q.jobs.is_empty() && q.execution_order().is_empty());
        assert_eq!(q.cancellation_order(),[id]);
        legal(&q);
    }

    #[test]
    fn v7_self_retire_Suspended取消退出Ready後也能釋放整筆工作() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,WRITE);
        q.suspend(id).unwrap();
        assert_eq!(q.ready_ids(),vec![id]);
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateSelfRetiredJob::Cancelled));
        assert!(q.ready_ids().is_empty() && q.jobs.is_empty());
        assert!(q.outcome(id).is_none());
        legal(&q);
    }

    #[test]
    fn v7_self_retire_尚Prepared或Suspended均不能假裝已完成移除() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        assert!(matches!(q.retire_terminal(id),Err(PrivateDeferredAwaitError::WrongState)));
        q.suspend(id).unwrap();
        assert!(matches!(q.retire_terminal(id),Err(PrivateDeferredAwaitError::WrongState)));
        assert_eq!(q.ready_ids(),vec![id]);
        assert!(matches!(q.retire_terminal(99),Err(PrivateDeferredAwaitError::UnknownTask)));
        legal(&q);
    }

    #[test]
    fn v7_self_retire_同一真ID只能交付一次且不得再偽造喚醒() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        let _=q.retire_terminal(id).unwrap();
        assert!(matches!(q.retire_terminal(id),Err(PrivateDeferredAwaitError::AlreadyFinished)));
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::UnknownTask)));
        assert_eq!(q.execution_order(),[id]);
        legal(&q);
    }

    #[test]
    fn v7_self_retire_交錯取消與已完成退休後仍可註冊新的唯一工作ID() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        let b=issue(&mut q,WRITE);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
        let _=q.retire_terminal(a).unwrap();
        q.cancel(b).unwrap();
        assert!(matches!(q.retire_terminal(b).unwrap(),
            PrivateSelfRetiredJob::Cancelled));
        assert!(q.jobs.is_empty());
        let c=issue(&mut q,READ);
        assert_eq!(c,3);
        q.suspend(c).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,c);
        assert_eq!(q.execution_order(),[a,c]);
        legal(&q);
    }

    #[test]
    fn v7_self_retire_已退休的原Wake全域歷史不可事後整筆抹除() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        q.retire_terminal(id).unwrap();
        let old=q.ready_history.clone();
        q.ready_history.clear();
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.retired_status(id),Some(PrivateDeferredAwaitStatus::Succeeded));
        q.ready_history=old;
        legal(&q);
    }

    #[test]
    fn v7_self_retire_同時抹除原執行歷史與就緒帳也不能騙過已退休收據() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        q.retire_terminal(id).unwrap();
        q.execution_order.clear();
        q.ready_history.clear();
        q.next_id=0;
        assert!(q.jobs.is_empty() && q.ready.is_empty());
        assert!(matches!(q.verify_ready_history(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.retired_status(id),Some(PrivateDeferredAwaitStatus::Succeeded));
    }

    #[test]
    fn v7_self_retire_退休收據的原全域事件位置遭篡改即拒絕新增工作() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        q.suspend(a).unwrap();
        q.drive_next().unwrap();
        q.retire_terminal(a).unwrap();
        let original=q.retired.get(&a).unwrap().transitions.clone();
        q.retired.get_mut(&a).unwrap().transitions[0].0=9;
        assert!(matches!(q.prepare(READ),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,1);
        q.retired.get_mut(&a).unwrap().transitions=original;
        assert_eq!(issue(&mut q,READ),2);
        legal(&q);
    }

    #[test]
    fn v7_self_retire_退休成功結果不能竄改終態為Failed或取消() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        q.drive_next().unwrap();
        q.retire_terminal(id).unwrap();
        q.retired.get_mut(&id).unwrap().status=PrivateDeferredAwaitStatus::Failed;
        assert!(matches!(q.shutdown(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        q.retired.get_mut(&id).unwrap().status=PrivateDeferredAwaitStatus::Succeeded;
        q.retired.get_mut(&id).unwrap().checked_self_loan=None;
        assert!(matches!(q.shutdown(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
    }

    #[test]
    fn v7_self_retire_退休取消收據遭刪後Ready空也不代表未取消() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.cancel(id).unwrap();
        q.retire_terminal(id).unwrap();
        q.cancellation_order.clear();
        assert!(matches!(q.verify_ready_history(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.ready.is_empty() && q.jobs.is_empty());
    }

    #[test]
    fn v7_self_retire_某份退休收據變造不可導致另一份Pending部分取消() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let done=issue(&mut q,READ);
        let waiting=issue(&mut q,WRITE);
        q.suspend(done).unwrap();
        q.suspend(waiting).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,done);
        q.retire_terminal(done).unwrap();
        q.retired.get_mut(&done).unwrap().executed_at=Some(99);
        assert!(matches!(q.shutdown(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),vec![waiting]);
        assert_eq!(q.status(waiting),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.cancellation_order().is_empty());
    }

    #[test]
    fn v7_self_retire_整批取消後退休全部工作仍保留完整取消順序() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        let b=issue(&mut q,WRITE);
        let c=issue(&mut q,READ);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        assert_eq!(q.shutdown().unwrap(),vec![c,b,a]);
        for id in [a,b,c] {
            assert!(matches!(q.retire_terminal(id).unwrap(),
                PrivateSelfRetiredJob::Cancelled));
        }
        assert!(q.jobs.is_empty() && q.ready_ids().is_empty());
        assert_eq!(q.cancellation_order(),[c,b,a]);
        assert_eq!(q.retired.len(),3);
        assert!(q.shutdown().unwrap().is_empty());
        legal(&q);
    }
}
