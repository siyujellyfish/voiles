// G18／P18.6-A1／V7-E1/E3：真正原 Checker async self
// 直接 await 工作跨 Rust 呼叫保存，只有 wake 才建立 HC14
// self BorrowTicket，未啟動安全點取消不得有動態框架事件。
#[cfg(test)]
mod g18_v7_async_self_deferred_session_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;

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

    fn prepared(queue: &mut PrivateCheckedDeferredSelfAwaitQueue, code: &str) -> u64 {
        queue.prepare(code).expect("唯一原Parser/Checker async self直接await須通過")
    }

    #[test]
    fn v7_async_deferred_真Checker尚未啟動時只有Prepared無動態結果() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        assert_eq!(id,1);
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.outcome(id).is_none());
        assert!(q.execution_order().is_empty());
        assert!(q.jobs.get(&id).unwrap().plan.as_ref().unwrap().awaited_method);
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::WrongState)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.execution_order().is_empty());
    }

    #[test]
    fn v7_async_deferred_真正suspend跨呼叫後wake才借入唯讀self並返回() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        q.suspend(id).unwrap();
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(id).is_none());
        let out=q.wake(id).unwrap();
        assert_eq!(out.result,Ok(10));
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.method_entered && out.method_exited);
        let start=out.await_start_loan_index.unwrap();
        assert!(matches!(out.exit_events[start],E::LoanAcquired(_)));
        assert!(matches!(out.exit_events[start+1],E::FunctionEntered));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.execution_order(),[id]);
        assert!(q.jobs.get(&id).unwrap().plan.is_none());
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::WrongState)));
    }

    #[test]
    fn v7_async_deferred_獨占self寫欄位必在真正wake後執行() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,WRITE);
        assert!(q.execution_order().is_empty());
        q.suspend(id).unwrap();
        let out=q.wake(id).unwrap();
        assert_eq!(out.result,Ok(14));
        assert_eq!(out.checked_loan.kind,BorrowKind::ExclusiveWrite);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::LoanReturned(_))).count(),1);
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Succeeded));
    }

    #[test]
    fn v7_async_deferred_兩個真Checker工作依明確wake次序才各執行一次() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=prepared(&mut q,READ);
        let b=prepared(&mut q,WRITE);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        assert!(q.execution_order().is_empty());
        assert_eq!(q.wake(b).unwrap().result,Ok(14));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(a).is_none());
        assert_eq!(q.wake(a).unwrap().result,Ok(10));
        assert_eq!(q.execution_order(),[b,a]);
        assert!(q.outcome(a).unwrap().has_balanced_self_borrow_trace());
        assert!(q.outcome(b).unwrap().has_balanced_self_borrow_trace());
    }

    #[test]
    fn v7_async_deferred_Prepared未啟動安全取消不得造假BorrowTicket() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        q.cancel(id).unwrap();
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(q.outcome(id).is_none());
        assert!(q.jobs.get(&id).unwrap().plan.is_none());
        assert!(q.execution_order().is_empty());
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::Cancelled)));
        assert!(matches!(q.cancel(id),Err(PrivateDeferredAwaitError::AlreadyFinished)));
    }

    #[test]
    fn v7_async_deferred_Suspended未啟動取消不得虛構清理事件() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,WRITE);
        q.suspend(id).unwrap();
        q.cancel(id).unwrap();
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(q.outcome(id).is_none());
        assert!(q.execution_order().is_empty());
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::Cancelled)));
        assert!(matches!(q.suspend(id),Err(PrivateDeferredAwaitError::WrongState)));
    }

    #[test]
    fn v7_async_deferred_真方法內溢位仍須Unwind並唯一還票標記Failed() {
        let source=READ.replace("return self.n+3",
            "return self.n+9223372036854775807");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,&source);
        q.suspend(id).unwrap();
        let out=q.wake(id).unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        let unwind=out.exit_events.iter().position(|e|matches!(
            e,E::UnwindStarted)).unwrap();
        let returned=out.exit_events.iter().position(|e|matches!(
            e,E::LoanReturned(_))).unwrap();
        assert!(unwind<returned);
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(q.execution_order(),[id]);
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::WrongState)));
    }

    #[test]
    fn v7_async_deferred_原Checker靜態拒絕不得配發async工作ID() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        for code in [
            READ.replace("return await owner.read()","return owner.read()"),
            READ.replace("async fn read()","fn read()"),
            WRITE.replace("owner.set(12)","owner.set(true)"),
        ] {
            assert!(matches!(q.prepare(&code),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "非法非同步方法不能消耗任何工作身分：{code}");
            assert_eq!(q.next_id,0);
            assert!(q.jobs.is_empty());
        }
        assert_eq!(prepared(&mut q,READ),1);
    }

    #[test]
    fn v7_async_deferred_有前置main局部的工作不能假裝無副作用等待() {
        let source=READ.replace(
            "var owner:Counter = Counter()\nreturn",
            "var owner:Counter = Counter()\nvar n:int = 1\nreturn",
        );
        assert!(PreparedPrivateSelfMethod::prepare(&source).is_ok());
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(q.prepare(&source),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,0);
        assert!(q.jobs.is_empty());
    }

    #[test]
    fn v7_async_deferred_已封存async語意被更改wake先拒且保留Suspended() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        q.suspend(id).unwrap();
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().awaited_method=false;
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(id).is_none());
        assert!(q.execution_order().is_empty());
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().awaited_method=true;
        assert_eq!(q.wake(id).unwrap().result,Ok(10));
    }

    #[test]
    fn v7_async_deferred_封存Lexer的await遭改不得預先啟動self票() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=prepared(&mut q,READ);
        q.suspend(id).unwrap();
        let plan=q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap();
        let index=plan.verified.tokens.iter().position(|token|
            token.start==plan.call_span.start
                && matches!(&token.kind,TokenKind::Name(name) if name=="await")
        ).unwrap();
        plan.verified.tokens[index].kind=TokenKind::Name("fake".to_owned());
        assert!(matches!(q.wake(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.outcome(id).is_none());
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap()
            .verified.tokens[index].kind=TokenKind::Name("await".to_owned());
        assert_eq!(q.wake(id).unwrap().result,Ok(10));
    }

    #[test]
    fn v7_async_deferred_工作ID耗盡不回繞且不能消耗新計畫() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        q.next_id=u64::MAX;
        assert!(matches!(q.prepare(READ),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,u64::MAX);
        assert!(q.jobs.is_empty());
        q.next_id=0;
        assert_eq!(prepared(&mut q,READ),1);
        assert!(matches!(q.cancel(9),Err(PrivateDeferredAwaitError::UnknownTask)));
    }
}
