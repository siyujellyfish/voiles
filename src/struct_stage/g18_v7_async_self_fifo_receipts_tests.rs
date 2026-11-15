// G18／P18.6-A1／V7-E2：原 Checker 已准許的 async self
// 延遲工作，必須由真單執行緒 FIFO Ready 授權才可驅動。
// 無背景工作、無跨 async self 內層 await 重入。
#[cfg(test)]
mod g18_v7_async_self_fifo_receipts_tests {
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

    fn prepared(q: &mut PrivateCheckedDeferredSelfAwaitQueue, code: &str) -> u64 {
        q.prepare(code).expect("原 Checker S20/VB15 核准真 async self 來源")
    }
    fn good(q: &PrivateCheckedDeferredSelfAwaitQueue) {
        q.verify_ready_history().expect("真Ready/取消/終態獨立歷史對帳");
    }

    #[test]
    fn v7_self_fifo_尚未suspend的真Checked工作不能提前驅動或簽票() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        assert_eq!(q.ready_ids(),Vec::<u64>::new());
        assert!(q.drive_next().unwrap().is_none());
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.outcome(id).is_none());
        good(&q);
    }

    #[test]
    fn v7_self_fifo_就緒依真suspend順序不是ID大小() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        let c=prepared(&mut q,READ);
        q.suspend(c).unwrap();
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        assert_eq!(q.ready_ids(),vec![c,a,b]);
        let (first,out)=q.drive_next().unwrap().unwrap();
        assert_eq!(first,c);
        assert_eq!(out.result,Ok(10));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(q.ready_ids(),vec![a,b]);
        good(&q);
    }

    #[test]
    fn v7_self_fifo_連續驅動三個真self借票互不重疊且逐次歸還() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let mut ids=Vec::new();
        for code in [READ,WRITE,READ] {
            let id=prepared(&mut q,code);
            q.suspend(id).unwrap();
            ids.push(id);
        }
        for (id,expected) in ids.iter().zip([10,14,10]) {
            let (observed,out)=q.drive_next().unwrap().unwrap();
            assert_eq!(*id,observed);
            assert_eq!(out.result,Ok(expected));
            assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
            good(&q);
        }
        assert_eq!(q.execution_order(),ids);
        assert!(q.drive_next().unwrap().is_none());
    }

    #[test]
    fn v7_self_fifo_失敗工作先Unwind還票不妨礙後面就緒工作() {
        let failed=READ.replace("return self.n+3",
            "return self.n+9223372036854775807");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,&failed);
        let b=prepared(&mut q,WRITE);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let (_,out)=q.drive_next().unwrap().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(q.ready_ids(),vec![b]);
        assert_eq!(q.drive_next().unwrap().unwrap().0,b);
        assert_eq!(q.outcome(b).unwrap().result,Ok(14));
        good(&q);
    }

    #[test]
    fn v7_self_fifo_取消首筆Ready只能移除該筆其餘依次驅動() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        let c=prepared(&mut q,READ);
        for id in [a,b,c] { q.suspend(id).unwrap(); }
        q.cancel(a).unwrap();
        assert_eq!(q.ready_ids(),vec![b,c]);
        assert_eq!(q.cancellation_order(),[a]);
        assert_eq!(q.drive_next().unwrap().unwrap().0,b);
        assert_eq!(q.drive_next().unwrap().unwrap().0,c);
        assert_eq!(q.execution_order(),[b,c]);
        assert!(q.outcome(a).is_none());
        good(&q);
    }

    #[test]
    fn v7_self_fifo_明確非FIFO手動wake只能移除指定ID不竄改餘下順序() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        let c=prepared(&mut q,READ);
        for id in [a,b,c] { q.suspend(id).unwrap(); }
        assert_eq!(q.wake(b).unwrap().result,Ok(14));
        assert_eq!(q.ready_ids(),vec![a,c]);
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
        assert_eq!(q.drive_next().unwrap().unwrap().0,c);
        assert_eq!(q.execution_order(),[b,a,c]);
        good(&q);
    }

    #[test]
    fn v7_self_fifo_整批關閉先驗證後依逆登錄次序取消未啟動工作() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        let c=prepared(&mut q,READ);
        q.suspend(a).unwrap();
        q.suspend(c).unwrap();
        assert_eq!(q.shutdown().unwrap(),vec![c,b,a]);
        assert_eq!(q.cancellation_order(),[c,b,a]);
        assert!(q.ready_ids().is_empty());
        assert!(q.execution_order().is_empty());
        for id in [a,b,c] {
            assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Cancelled));
            assert!(q.outcome(id).is_none());
        }
        assert!(q.shutdown().unwrap().is_empty());
        good(&q);
    }

    #[test]
    fn v7_self_fifo_終結工作保留真退出結果而關閉只取消剩餘等待者() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let done=prepared(&mut q,READ);
        let pending=prepared(&mut q,WRITE);
        let empty=prepared(&mut q,READ);
        q.suspend(done).unwrap();
        q.suspend(pending).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,done);
        assert_eq!(q.shutdown().unwrap(),vec![empty,pending]);
        assert_eq!(q.execution_order(),[done]);
        assert_eq!(q.cancellation_order(),[empty,pending]);
        assert_eq!(q.outcome(done).unwrap().result,Ok(10));
        assert!(matches!(q.cancel(done),Err(PrivateDeferredAwaitError::AlreadyFinished)));
        good(&q);
    }

    #[test]
    fn v7_self_fifo_手動插入幽靈Ready在實際wake前即拒絕且零提交() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        q.suspend(a).unwrap();
        q.ready.push_back(99);
        let previous=q.ready_ids();
        assert!(matches!(q.drive_next(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(matches!(q.cancel(a),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),previous);
        assert!(q.execution_order().is_empty());
        assert!(q.outcome(a).is_none());
        q.ready.pop_back();
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
    }

    #[test]
    fn v7_self_fifo_交換兩張真Ready順序即使ID狀態都合法仍不可驅動() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        q.ready.swap(0,1);
        assert!(matches!(q.drive_next(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.execution_order().is_empty());
        q.ready.swap(0,1);
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
        good(&q);
    }

    #[test]
    fn v7_self_fifo_假造重複就緒收據無法通過全狀態重播() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        q.suspend(a).unwrap();
        q.ready_history.push(PrivateSelfReadyTransition::Suspended(a));
        assert!(matches!(q.drive_next(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.outcome(a).is_none());
        q.ready_history.pop();
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
    }

    #[test]
    fn v7_self_fifo_已成功的真方法不能改寫成Failed或失去原退出證據() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        q.suspend(a).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
        let len=q.ready_history.len();
        q.ready_history[len-1]=PrivateSelfReadyTransition::Woken(
            a,PrivateDeferredAwaitStatus::Failed);
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.outcome(a).unwrap().result,Ok(10));
        q.ready_history[len-1]=PrivateSelfReadyTransition::Woken(
            a,PrivateDeferredAwaitStatus::Succeeded);
        q.jobs.get_mut(&a).unwrap().outcome.as_mut().unwrap().ticket_returned=false;
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
    }

    #[test]
    fn v7_self_fifo_取消歷史若被刪除不能與空Ready佯稱從未取消() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        q.cancel(a).unwrap();
        q.cancellation_order.clear();
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.outcome(a).is_none());
        q.cancellation_order.push(a);
        good(&q);
    }

    #[test]
    fn v7_self_fifo_一筆終態已變造不可先取消另一合法工作才報錯() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let done=prepared(&mut q,READ);
        let pending=prepared(&mut q,WRITE);
        q.suspend(done).unwrap();
        q.suspend(pending).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,done);
        q.jobs.get_mut(&done).unwrap().status=PrivateDeferredAwaitStatus::Failed;
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(pending),Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.cancellation_order(),[]);
        assert_eq!(q.ready_ids(),vec![pending]);
    }

    #[test]
    fn v7_self_fifo_原Checked工作來源受損不得消耗Ready也不得假完成() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        q.suspend(a).unwrap();
        let plan=q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap();
        plan.awaited_method=false;
        let before=q.ready_history.clone();
        assert!(matches!(q.drive_next(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),vec![a]);
        assert_eq!(q.ready_history,before);
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(a).is_none());
        q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap().awaited_method=true;
        assert_eq!(q.drive_next().unwrap().unwrap().0,a);
        good(&q);
    }
}
