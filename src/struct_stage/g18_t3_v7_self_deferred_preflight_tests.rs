// G18／P18.5-T3/T4／V7-E1/E3：就緒、取消與關閉前的
// 真原Lexer/Checker方法來源預檢。不允許污染的 Prepared
// 或 Suspended 計畫先被抹除、退休或掩護其它正常工作。
// 所有案例只在 Rust 私有測試中蓄意竄改已封存計畫。
#[cfg(test)]
mod g18_t3_v7_self_deferred_preflight_tests {
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

    const HELPER: &str = r#"struct Counter{
var n:int = 7
async fn read()->int{
return seed()+2
}
}
fn seed()->int{
return 5
}
fn main()->int{
var owner:Counter = Counter()
return await owner.read()
}
"#;

    const LOCALS: &str = r#"struct Counter{
var n:int = 7
async fn read()->int{
var value:int = 5
return value+2
}
}
fn main()->int{
var owner:Counter = Counter()
return await owner.read()
}
"#;

    fn issue(q: &mut PrivateCheckedDeferredSelfAwaitQueue, src: &str) -> u64 {
        q.prepare(src).expect("唯一原Checker直接await async self來源")
    }

    fn intact(q: &PrivateCheckedDeferredSelfAwaitQueue) {
        q.verify_ready_history().expect("所有Live/Retired任務仍有完整獨立收據");
    }

    fn restore(q: &mut PrivateCheckedDeferredSelfAwaitQueue, id: u64, src: &str) {
        q.jobs.get_mut(&id).unwrap().plan =
            Some(PreparedPrivateSelfMethod::prepare(src).unwrap());
    }

    #[test]
    fn t3_v7_self_preflight_與真execute共用同一完整來源權威() {
        let plan=PreparedPrivateSelfMethod::prepare(READ).unwrap();
        assert!(plan.checked_source_still_intact());
        assert_eq!(plan.execute().unwrap().result,Ok(10));
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        q.suspend(id).unwrap();
        let out=q.drive_next().unwrap().unwrap().1;
        assert_eq!(out.result,Ok(10));
        assert!(out.has_balanced_self_borrow_trace());
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_Prepared回傳計畫被改不得先取消消證() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,READ);
        let plan=q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap();
        plan.return_calculation=PrivateSelfIntReturn::Constant(999);
        assert!(matches!(q.verify_ready_history(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(matches!(q.cancel(id),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.cancellation_order().is_empty());
        assert_eq!(q.ready_history.len(),1);
        restore(&mut q,id,READ);
        q.cancel(id).unwrap();
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_Suspended純helper子計畫被改不得假喚醒() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,HELPER);
        q.suspend(id).unwrap();
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap()
            .method_helpers.get_mut("seed").unwrap().returned =
                PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(q.drive_next(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.ready_ids(),vec![id]);
        assert!(q.execution_order().is_empty());
        restore(&mut q,id,HELPER);
        assert_eq!(q.drive_next().unwrap().unwrap().1.result,Ok(7));
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_原Lexer的await詞彙遭替換整批關閉須先拒絕() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        let b=issue(&mut q,HELPER);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let plan=q.jobs.get_mut(&b).unwrap().plan.as_mut().unwrap();
        let token=plan.verified.tokens.iter_mut()
            .find(|token|matches!(&token.kind,TokenKind::Name(word) if word=="await"))
            .expect("真 await Lexer token");
        token.kind=TokenKind::Name("other".to_owned());
        assert!(matches!(q.shutdown(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),vec![a,b]);
        assert!(q.cancellation_order().is_empty());
        restore(&mut q,b,HELPER);
        assert_eq!(q.shutdown().unwrap(),vec![b,a]);
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_受損準備計畫不得替下一ID註冊合法外觀() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap().field_value=100;
        let next=q.next_id;
        let history=q.ready_history.len();
        assert!(matches!(q.prepare(READ),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,next);
        assert_eq!(q.jobs.len(),1);
        assert_eq!(q.ready_history.len(),history);
        restore(&mut q,a,READ);
        let b=issue(&mut q,READ);
        assert_eq!(b,a+1);
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_獨立真方法局部HIR受損不得進入Ready() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=issue(&mut q,LOCALS);
        q.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap()
            .method_int_locals[0].1=PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(q.suspend(id),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.ready_ids().is_empty());
        assert_eq!(q.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        restore(&mut q,id,LOCALS);
        q.suspend(id).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().1.result,Ok(7));
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_其它合法Ready不得掩蓋污染工作先被執行() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let good=issue(&mut q,READ);
        let bad=issue(&mut q,HELPER);
        q.suspend(good).unwrap();
        q.suspend(bad).unwrap();
        q.jobs.get_mut(&bad).unwrap().plan.as_mut().unwrap()
            .return_calculation=PrivateSelfIntReturn::Constant(17);
        assert!(matches!(q.drive_next(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_ids(),vec![good,bad]);
        assert!(q.execution_order().is_empty());
        assert!(q.outcome(good).is_none());
        restore(&mut q,bad,HELPER);
        assert_eq!(q.drive_next().unwrap().unwrap().0,good);
        assert_eq!(q.drive_next().unwrap().unwrap().0,bad);
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_真已完工可正常退休但另一污染計畫必拒絕移交() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let done=issue(&mut q,READ);
        q.suspend(done).unwrap();
        q.drive_next().unwrap();
        let bad=issue(&mut q,HELPER);
        q.suspend(bad).unwrap();
        q.jobs.get_mut(&bad).unwrap().plan.as_mut().unwrap()
            .method_helpers.get_mut("seed").unwrap().returned =
                PrivateScalarReturnExpr::Constant(19);
        assert!(matches!(q.retire_terminal(done),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert!(q.retired.is_empty());
        assert!(q.outcome(done).is_some());
        restore(&mut q,bad,HELPER);
        let PrivateSelfRetiredJob::Completed(out)=q.retire_terminal(done).unwrap()
            else { panic!("真已完成Outcome應唯一移交"); };
        assert_eq!(out.result,Ok(10));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(q.drive_next().unwrap().unwrap().0,bad);
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_多工作全取消必先證明所有來源一致而不部分提交() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        let b=issue(&mut q,LOCALS);
        let c=issue(&mut q,HELPER);
        q.suspend(a).unwrap();
        q.suspend(c).unwrap();
        q.jobs.get_mut(&b).unwrap().plan.as_mut().unwrap().field_value=123;
        let before=q.ready_history.clone();
        assert!(matches!(q.shutdown(),
            Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.ready_history,before);
        assert_eq!(q.ready_ids(),vec![a,c]);
        assert!(q.cancellation_order().is_empty());
        assert!(q.jobs.values().all(|job|
            matches!(job.status,PrivateDeferredAwaitStatus::Prepared
                | PrivateDeferredAwaitStatus::Suspended)));
        restore(&mut q,b,LOCALS);
        assert_eq!(q.shutdown().unwrap(),vec![c,b,a]);
        intact(&q);
    }

    #[test]
    fn t3_v7_self_preflight_成功退休收據不持AST仍可與新合法工作並存() {
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let a=issue(&mut q,READ);
        q.suspend(a).unwrap();
        q.drive_next().unwrap();
        let _=q.retire_terminal(a).unwrap();
        assert!(q.jobs.is_empty());
        assert_eq!(q.retired_status(a),Some(PrivateDeferredAwaitStatus::Succeeded));
        let b=issue(&mut q,HELPER);
        q.suspend(b).unwrap();
        assert_eq!(q.drive_next().unwrap().unwrap().1.result,Ok(7));
        assert_eq!(q.retired_status(a),Some(PrivateDeferredAwaitStatus::Succeeded));
        intact(&q);
    }
}
