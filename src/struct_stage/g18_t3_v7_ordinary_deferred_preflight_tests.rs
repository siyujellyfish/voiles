// G18／P18.5-T3／V7：普通async函式未啟動等待工作
// 在取消、喚醒與下一工作建立前，與原Checker執行器共用
// 完整Lexer/TypedHIR/FunctionExit來源；污染須零部分提交。
#[cfg(test)]
mod g18_t3_v7_ordinary_deferred_preflight_tests {
    use super::*;

    const BASIC: &str = "async fn ready()->int{ return 7 }\nfn main()->int{ return await ready() }\n";
    const OTHER: &str = "async fn ready()->int{ return 11 }\nfn main()->int{ return await ready() }\n";
    const RESOURCE: &str = "async fn ready()->int{\nvar hold:str = \"🪼原UTF-8資源\"\nreturn 8\n}\nfn main()->int{ return await ready() }\n";

    fn make(q: &mut PrivateCheckedDeferredAwaitQueue, text: &str) -> u64 {
        q.prepare(text).expect("原Parser/Checker應核准普通async直接await")
    }

    fn repair(q: &mut PrivateCheckedDeferredAwaitQueue, id: u64, text: &str) {
        q.jobs.get_mut(&id).unwrap().plan =
            Some(PreparedPrivateScalarCall::prepare(text).unwrap());
        q.verify_pending_sources().expect("原Checker完整來源恢復");
    }

    #[test]
    fn t3_v7_ordinary_preflight_真完整Checker來源與直接await正常執行共用() {
        let plan=PreparedPrivateScalarCall::prepare(BASIC).unwrap();
        assert!(plan.checked_source_still_intact());
        assert_eq!(plan.execute().unwrap().result,Ok(7));
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.suspend(a).unwrap();
        assert_eq!(q.wake(a).unwrap().result,Ok(7));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Succeeded));
        q.verify_pending_sources().unwrap();
    }

    #[test]
    fn t3_v7_ordinary_preflight_Prepared回傳HIR遭變造取消前必拒絕且零移交() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap()
            .main_dynamic_call=Some(PrivateScalarReturnExpr::Constant(9));
        assert_eq!(q.cancel(a),Err(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.execution_order().is_empty());
        assert!(q.jobs[&a].plan.is_some());
        repair(&mut q,a,BASIC);
        q.cancel(a).unwrap();
        assert!(q.jobs[&a].plan.is_none());
        q.verify_pending_sources().unwrap();
    }

    #[test]
    fn t3_v7_ordinary_preflight_Suspended原awaitToken遭變造不得偽造喚醒() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.suspend(a).unwrap();
        let plan=q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap();
        let token=plan.verified.tokens.iter_mut().find(|t|
            matches!(&t.kind,TokenKind::Name(word) if word=="await"))
            .expect("原Lexer應有await");
        token.kind=TokenKind::Name("forged".into());
        assert_eq!(q.wake(a).err(),Some(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.execution_order().is_empty());
        assert!(q.jobs[&a].outcome.is_none());
        repair(&mut q,a,BASIC);
        assert_eq!(q.wake(a).unwrap().result,Ok(7));
    }

    #[test]
    fn t3_v7_ordinary_preflight_旁觀污染計畫不得讓另一合法任務先喚醒() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let good=make(&mut q,BASIC);
        let bad=make(&mut q,OTHER);
        for id in [good,bad] { q.suspend(id).unwrap(); }
        q.jobs.get_mut(&bad).unwrap().plan.as_mut().unwrap()
            .callee_name="missing".to_owned();
        assert_eq!(q.wake(good).err(),Some(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(q.status(good),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.execution_order().is_empty());
        repair(&mut q,bad,OTHER);
        assert_eq!(q.wake(good).unwrap().result,Ok(7));
        assert_eq!(q.wake(bad).unwrap().result,Ok(11));
        assert_eq!(q.execution_order(),[good,bad]);
    }

    #[test]
    fn t3_v7_ordinary_preflight_污染旁觀工作不得先取消另一份合法來源() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let good=make(&mut q,BASIC);
        let bad=make(&mut q,OTHER);
        q.jobs.get_mut(&bad).unwrap().plan.as_mut().unwrap()
            .original[0]=b'X';
        assert_eq!(q.cancel(good),Err(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(q.status(good),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.jobs[&good].plan.is_some());
        repair(&mut q,bad,OTHER);
        q.cancel(good).unwrap();
        assert_eq!(q.status(good),Some(PrivateDeferredAwaitStatus::Cancelled));
        q.verify_pending_sources().unwrap();
    }

    #[test]
    fn t3_v7_ordinary_preflight_受損計畫不得發行新工作ID且來源可恢復() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap()
            .main_dynamic_call=Some(PrivateScalarReturnExpr::Constant(7));
        let before=q.next_id;
        assert!(matches!(q.prepare(OTHER),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,before);
        assert_eq!(q.jobs.len(),1);
        repair(&mut q,a,BASIC);
        assert_eq!(make(&mut q,OTHER),a+1);
    }

    #[test]
    fn t3_v7_ordinary_preflight_污染原普通函式簽章不得提前進Suspended() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.jobs.get_mut(&a).unwrap().plan.as_mut().unwrap().callee_name="other".into();
        assert_eq!(q.suspend(a),Err(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.jobs[&a].outcome.is_none());
        repair(&mut q,a,BASIC);
        q.suspend(a).unwrap();
        assert_eq!(q.wake(a).unwrap().result,Ok(7));
    }

    #[test]
    fn t3_v7_ordinary_preflight_正常資源僅真wake後建立釋放且取消零事件() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let cancelled=make(&mut q,RESOURCE);
        let live=make(&mut q,RESOURCE);
        q.suspend(cancelled).unwrap();
        q.suspend(live).unwrap();
        assert!(q.jobs.values().all(|job|job.outcome.is_none()));
        q.cancel(cancelled).unwrap();
        assert!(q.jobs[&cancelled].plan.is_none());
        assert!(q.jobs[&cancelled].outcome.is_none());
        let out=q.wake(live).unwrap();
        assert_eq!(out.result,Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(out.resource_events.as_slice(),[
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b)
        ] if a==b));
    }

    #[test]
    fn t3_v7_ordinary_preflight_受控溢位保留原失敗結果不錯拒後續合法工作() {
        let overflow="async fn ready()->int{ return 9223372036854775807+1 }\nfn main()->int{ return await ready() }\n";
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let bad=make(&mut q,overflow);
        let good=make(&mut q,BASIC);
        q.suspend(bad).unwrap();
        q.suspend(good).unwrap();
        let result=q.wake(bad).unwrap();
        assert_eq!(result.result,Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(result.has_balanced_frame_trace());
        q.verify_pending_sources().unwrap();
        assert_eq!(q.wake(good).unwrap().result,Ok(7));
        assert_eq!(q.execution_order(),[bad,good]);
    }

    #[test]
    fn t3_v7_ordinary_preflight_工作ID耗盡應保守拒絕不得panic或回繞() {
        let mut q=PrivateCheckedDeferredAwaitQueue::new();
        let a=make(&mut q,BASIC);
        q.next_id=u64::MAX;
        assert!(matches!(q.prepare(OTHER),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,u64::MAX);
        assert_eq!(q.jobs.len(),1);
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Prepared));
        q.next_id=a;
        assert_eq!(make(&mut q,OTHER),a+1);
        q.verify_pending_sources().unwrap();
    }
}
