// G18／P18.5-T3／V7：原 Checker 封存 async self 工作
// 與真執行 Outcome 必須有完全相同的來源位元組、DeclarationId、
// 方法與呼叫位置、BorrowFact；個別 R05 票據平衡不足以對帳 ID。
#[cfg(test)]
mod g18_t3_async_self_outcome_origin_binding_tests {
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

    fn plan(source: &str) -> PreparedPrivateSelfMethod {
        PreparedPrivateSelfMethod::prepare(source).expect("真原Checker核准")
    }
    fn issue(q: &mut PrivateCheckedDeferredSelfAwaitQueue, source: &str) -> u64 {
        q.prepare(source).expect("獨立Checker來源封存工作")
    }
    fn done(q: &mut PrivateCheckedDeferredSelfAwaitQueue, source: &str) -> u64 {
        let id=issue(q,source);
        q.suspend(id).unwrap();
        q.wake(id).unwrap();
        id
    }
    fn swap_outcomes(q: &mut PrivateCheckedDeferredSelfAwaitQueue, a: u64, b: u64) {
        let mut first=q.jobs.remove(&a).expect("第一筆真工作");
        let mut second=q.jobs.remove(&b).expect("第二筆真工作");
        std::mem::swap(&mut first.outcome,&mut second.outcome);
        q.jobs.insert(a,first);
        q.jobs.insert(b,second);
    }

    #[test]
    fn t3_async_origin_原Checker真方法在動態結果封存相同完整來源() {
        for src in [READ,WRITE] {
            let p=plan(src);
            let expected=PrivateSelfExecutionOrigin::from_plan(&p);
            let actual=p.execute().unwrap();
            assert_eq!(actual.origin,expected);
            assert_eq!(actual.origin.source.as_ref(),src.as_bytes());
            assert_eq!(actual.origin.owner,p.owner_id);
            assert_eq!(actual.origin.call_span,p.call_span);
            assert_eq!(actual.origin.method_span,p.method_span);
            assert_eq!(actual.origin.self_loan,p.loan);
            assert!(actual.has_balanced_self_borrow_trace());
        }
    }

    #[test]
    fn t3_async_origin_合法Read與Write兩份Outcome雖都平衡也不得交換工作ID() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=done(&mut q,READ);
        let b=done(&mut q,WRITE);
        assert!(q.outcome(a).unwrap().has_balanced_self_borrow_trace());
        assert!(q.outcome(b).unwrap().has_balanced_self_borrow_trace());
        swap_outcomes(&mut q,a,b);
        assert!(q.outcome(a).unwrap().has_balanced_self_borrow_trace());
        assert!(q.outcome(b).unwrap().has_balanced_self_borrow_trace());
        assert!(matches!(q.verify_ready_history(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(matches!(q.retire_terminal(a),Err(PrivateDeferredAwaitError::SourceChanged)));
        swap_outcomes(&mut q,a,b);
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_相同Loan與方法跨度的不同原始碼結果交換也必拒絕() {
        let changed=READ.replace("self.n+3","self.n+9");
        let a_plan=plan(READ);
        let b_plan=plan(&changed);
        assert_eq!(a_plan.loan,b_plan.loan);
        assert_eq!(a_plan.call_span,b_plan.call_span);
        assert_eq!(a_plan.method_span,b_plan.method_span);
        assert_eq!(READ.len(),changed.len());
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=done(&mut q,READ);
        let b=done(&mut q,&changed);
        assert_eq!(q.outcome(a).unwrap().result,Ok(10));
        assert_eq!(q.outcome(b).unwrap().result,Ok(16));
        swap_outcomes(&mut q,a,b);
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.cancellation_order().is_empty());
        swap_outcomes(&mut q,a,b);
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_未啟動工作封存ownerDeclarationId被改不得發Ready() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        let old=q.jobs.get(&id).unwrap().origin.owner;
        q.jobs.get_mut(&id).unwrap().origin.owner=DeclarationId(999999);
        assert!(matches!(q.suspend(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),vec![]);
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        q.jobs.get_mut(&id).unwrap().origin.owner=old;
        q.suspend(id).unwrap();
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_未啟動工作呼叫Span變造不得簽發等待憑據() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        let old=q.jobs.get(&id).unwrap().origin.call_span;
        q.jobs.get_mut(&id).unwrap().origin.call_span =
            crate::common_ir::NodeSpan::new(1,2);
        assert!(matches!(q.suspend(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.ready_ids().is_empty());
        q.jobs.get_mut(&id).unwrap().origin.call_span=old;
        q.suspend(id).unwrap();
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_未啟動完整UTF8封存被改不能只靠同長及票據通過() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.jobs.get_mut(&id).unwrap().origin.source[0]=b'X';
        assert!(matches!(q.suspend(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_history.len(),1);
        assert!(q.ready_ids().is_empty());
        q.jobs.get_mut(&id).unwrap().origin.source[0]=b's';
        q.suspend(id).unwrap();
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_準備後偷換真Checker原計畫票據即使帳單獨合法也拒() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        let old=q.jobs.get(&id).unwrap().plan.as_ref().unwrap().loan;
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().loan=
            BorrowFact::new(DeclarationId(99999),BorrowKind::SharedRead);
        assert!(matches!(q.suspend(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.ready_ids().is_empty());
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().loan=old;
        q.suspend(id).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().0,id);
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_真Outcome的來源位元組遭變造即使退出平衡也禁止退休() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=done(&mut q,READ);
        q.jobs.get_mut(&id).unwrap().outcome.as_mut().unwrap()
            .origin.source[0]=b'X';
        assert!(q.outcome(id).unwrap().has_balanced_self_borrow_trace());
        assert!(matches!(q.retire_terminal(id),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.jobs.contains_key(&id) && q.retired.is_empty());
        q.jobs.get_mut(&id).unwrap().outcome.as_mut().unwrap()
            .origin.source[0]=b's';
        q.retire_terminal(id).unwrap();
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_真Outcome方法Span與計畫不同不得冒領原Checked框架() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=done(&mut q,READ);
        let old=q.jobs.get(&id).unwrap().outcome.as_ref().unwrap().origin.method_span;
        q.jobs.get_mut(&id).unwrap().outcome.as_mut().unwrap()
            .origin.method_span=crate::common_ir::NodeSpan::new(1,2);
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        q.jobs.get_mut(&id).unwrap().outcome.as_mut().unwrap().origin.method_span=old;
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_受控溢位Failed也須保存原來源而非借別的Outcome() {
        let broken=READ.replace("self.n+3","self.n+9223372036854775807");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=done(&mut q,&broken);
        let out=q.outcome(id).unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(out.origin.source.as_ref(),broken.as_bytes());
        assert_eq!(q.retired_status(id),None);
        let PrivateSelfRetiredJob::Completed(moved)=q.retire_terminal(id).unwrap()
            else { panic!("Failed必交付原RuntimeOutcome"); };
        assert_eq!(moved.origin.source.as_ref(),broken.as_bytes());
        assert_eq!(moved.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_取消尚未執行工作也應先稽核原Checker封存來源() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.jobs.get_mut(&id).unwrap().origin.source[0]=b'X';
        assert!(matches!(q.cancel(id),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.cancellation_order().is_empty());
        q.jobs.get_mut(&id).unwrap().origin.source[0]=b's';
        q.cancel(id).unwrap();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateSelfRetiredJob::Cancelled));
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn t3_async_origin_另一份終態來源錯置不得先取消合法等待者() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let done=issue(&mut q,READ);
        let wait=issue(&mut q,WRITE);
        q.suspend(done).unwrap();
        q.suspend(wait).unwrap();
        q.wake(done).unwrap();
        q.jobs.get_mut(&done).unwrap().outcome.as_mut().unwrap()
            .origin.call_span=crate::common_ir::NodeSpan::new(1,2);
        assert!(matches!(q.shutdown(),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(wait),Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.ready_ids(),vec![wait]);
        assert!(q.cancellation_order().is_empty());
    }

    #[test]
    fn t3_async_origin_終態退休後交付的真結果仍攜帶可核驗來源() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=done(&mut q,READ);
        let expected=q.jobs.get(&id).unwrap().origin.clone();
        let PrivateSelfRetiredJob::Completed(out)=q.retire_terminal(id).unwrap()
            else { panic!("有效唯一結果"); };
        assert_eq!(out.origin,expected);
        assert_eq!(out.checked_loan,out.origin.self_loan);
        assert_eq!(q.retired.get(&id).unwrap().checked_self_loan,Some(expected.self_loan));
        assert!(q.jobs.is_empty());
        q.verify_ready_history().unwrap();
    }
}
