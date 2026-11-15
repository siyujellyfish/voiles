// G18／P18.6-A1／V7-E2：原 Parser/Checker 普通 async
// 與 async self 直接 await 在同一 Rust 會話使用唯一全域
// 身分，顯式串行 wake；未喚醒工作不得建立方法／函式事件。
#[cfg(test)]
mod g18_v7_mixed_direct_await_session_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;

    const ORDINARY: &str = concat!(
        "async fn leaf()->int{\nreturn 11\n}\n",
        "fn main()->int{\nreturn await leaf()\n}\n",
    );
    const METHOD: &str = r#"struct Counter{
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

    fn ordinary(q:&mut PrivateV7MixedDirectAwaitSession, code:&str)->u64 {
        q.prepare_ordinary(code).expect("原Checker真普通async直接await")
    }
    fn method(q:&mut PrivateV7MixedDirectAwaitSession, code:&str)->u64 {
        q.prepare_method(code).expect("原Checker真async self直接await")
    }

    #[test]
    fn v7_mixed_兩種原Checker工作內部各從1起算全域身份仍必唯一() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        assert_eq!([a,b],[1,2]);
        assert_eq!(q.jobs[&a],(PrivateV7DirectAwaitKind::Ordinary,1));
        assert_eq!(q.jobs[&b],(PrivateV7DirectAwaitKind::SelfMethod,1));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(q.execution_order().is_empty());
        assert!(matches!(q.wake(a),Err(PrivateDeferredAwaitError::WrongState)));
        assert!(matches!(q.wake(b),Err(PrivateDeferredAwaitError::WrongState)));
    }

    #[test]
    fn v7_mixed_普通async先wake真返回再async_self後wake真歸票() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let PrivateV7DirectAwaitResult::Ordinary(one)=q.wake(a).unwrap() else {
            panic!("必由原普通 Checker 圖回傳")
        };
        assert_eq!(one.result,Ok(11));
        assert!(one.has_balanced_frame_trace());
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Suspended));
        let PrivateV7DirectAwaitResult::SelfMethod(two)=q.wake(b).unwrap() else {
            panic!("必由原async self Checker借票")
        };
        assert_eq!(two.result,Ok(10));
        assert!(two.has_balanced_self_borrow_trace());
        assert!(two.ticket_returned);
        assert_eq!(q.execution_order(),[a,b]);
    }

    #[test]
    fn v7_mixed_async_self先wake再普通async都各獨立真退出() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let PrivateV7DirectAwaitResult::SelfMethod(out)=q.wake(b).unwrap() else {
            panic!("原SelfMethod")
        };
        assert_eq!(out.result,Ok(10));
        assert_eq!(out.exit_events.iter().filter(|event|
            matches!(event,E::LoanReturned(_))).count(),1);
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        let PrivateV7DirectAwaitResult::Ordinary(out)=q.wake(a).unwrap() else {
            panic!("原普通async圖")
        };
        assert_eq!(out.result,Ok(11));
        assert_eq!(q.execution_order(),[b,a]);
    }

    #[test]
    fn v7_mixed_兩種未啟動工作皆可安全取消且不得留下執行憑據() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.cancel(a).unwrap();
        q.suspend(b).unwrap();
        q.cancel(b).unwrap();
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(q.ordinary.jobs.get(&1).unwrap().outcome.is_none());
        assert!(q.methods.jobs.get(&1).unwrap().outcome.is_none());
        assert!(q.execution_order().is_empty());
        assert!(matches!(q.wake(a),Err(PrivateDeferredAwaitError::Cancelled)));
        assert!(matches!(q.wake(b),Err(PrivateDeferredAwaitError::Cancelled)));
    }

    #[test]
    fn v7_mixed_普通async失敗不吞掉後續async_self工作() {
        let fail=ORDINARY.replace("return 11","return 9223372036854775807+1");
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,&fail);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let PrivateV7DirectAwaitResult::Ordinary(out)=q.wake(a).unwrap() else {
            panic!("普通函式")
        };
        assert_eq!(out.result,Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Failed));
        let PrivateV7DirectAwaitResult::SelfMethod(out)=q.wake(b).unwrap() else {
            panic!("同步self真借票")
        };
        assert_eq!(out.result,Ok(10));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(q.execution_order(),[a,b]);
    }

    #[test]
    fn v7_mixed_async_self溢位不破壞另一份普通async真工作() {
        let fail=METHOD.replace("return self.n+3",
            "return self.n+9223372036854775807");
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,&fail);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let PrivateV7DirectAwaitResult::SelfMethod(out)=q.wake(b).unwrap() else {
            panic!("真async self")
        };
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Failed));
        let PrivateV7DirectAwaitResult::Ordinary(out)=q.wake(a).unwrap() else {
            panic!("真普通async")
        };
        assert_eq!(out.result,Ok(11));
        assert_eq!(q.execution_order(),[b,a]);
    }

    #[test]
    fn v7_mixed_普通async原Checker拒絕不消耗全域或內部工作ID() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let bad=ORDINARY.replace("await leaf()","leaf()");
        assert!(matches!(q.prepare_ordinary(&bad),
            Err(PrivateScalarCallPrepareError::Check(_))
                | Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(q.next_id,0);
        assert_eq!(q.ordinary.next_id,0);
        assert!(q.jobs.is_empty());
        assert_eq!(ordinary(&mut q,ORDINARY),1);
    }

    #[test]
    fn v7_mixed_async_self原Checker錯型別或漏await不消耗全域ID() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        for src in [
            METHOD.replace("await owner.read()","owner.read()"),
            METHOD.replace("async fn read()","fn read()"),
        ] {
            assert!(matches!(q.prepare_method(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))));
            assert_eq!(q.next_id,0);
            assert_eq!(q.methods.next_id,0);
            assert!(q.jobs.is_empty());
        }
        assert_eq!(method(&mut q,METHOD),1);
    }

    #[test]
    fn v7_mixed_普通await封存Lexer被改只拒此工作但不污染self() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let scalar=q.ordinary.jobs.get_mut(&1).unwrap().plan.as_mut().unwrap();
        let index=scalar.verified.tokens.iter().position(|token|
            matches!(&token.kind,TokenKind::Name(word) if word=="await")
        ).unwrap();
        scalar.verified.tokens[index].kind=TokenKind::Name("forged".into());
        assert!(matches!(q.wake(a),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(q.execution_order().is_empty());
        let PrivateV7DirectAwaitResult::SelfMethod(out)=q.wake(b).unwrap() else {
            panic!("原async self不可受另工作污染")
        };
        assert_eq!(out.result,Ok(10));
        assert_eq!(q.execution_order(),[b]);
    }

    #[test]
    fn v7_mixed_async_self原Lexer被改不阻擋另一普通async安全執行() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        let plan=q.methods.jobs.get_mut(&1).unwrap().plan.as_mut().unwrap();
        let index=plan.verified.tokens.iter().position(|token|
            token.start==plan.call_span.start
                && matches!(&token.kind,TokenKind::Name(word) if word=="await")
        ).unwrap();
        plan.verified.tokens[index].kind=TokenKind::Name("forged".into());
        assert!(matches!(q.wake(b),Err(PrivateDeferredAwaitError::SourceChanged)));
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Suspended));
        let PrivateV7DirectAwaitResult::Ordinary(out)=q.wake(a).unwrap() else {
            panic!("普通async不受另一方法篡改污染")
        };
        assert_eq!(out.result,Ok(11));
        assert_eq!(q.execution_order(),[a]);
    }

    #[test]
    fn v7_mixed_完成後不得再次wake或取消且全域順序不重播() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        let a=ordinary(&mut q,ORDINARY);
        let b=method(&mut q,METHOD);
        q.suspend(a).unwrap();
        q.suspend(b).unwrap();
        assert!(matches!(q.wake(a).unwrap(),PrivateV7DirectAwaitResult::Ordinary(_)));
        assert!(matches!(q.wake(b).unwrap(),PrivateV7DirectAwaitResult::SelfMethod(_)));
        assert!(matches!(q.wake(a),Err(PrivateDeferredAwaitError::WrongState)));
        assert!(matches!(q.wake(b),Err(PrivateDeferredAwaitError::WrongState)));
        assert!(matches!(q.cancel(a),Err(PrivateDeferredAwaitError::AlreadyFinished)));
        assert!(matches!(q.cancel(b),Err(PrivateDeferredAwaitError::AlreadyFinished)));
        assert_eq!(q.execution_order(),[a,b]);
        assert!(matches!(q.wake(99),Err(PrivateDeferredAwaitError::UnknownTask)));
    }

    #[test]
    fn v7_mixed_全域ID用盡必先拒絕且兩種原工作不得被分配() {
        let mut q=PrivateV7MixedDirectAwaitSession::new();
        q.next_id=u64::MAX;
        assert!(matches!(q.prepare_ordinary(ORDINARY),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(matches!(q.prepare_method(METHOD),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert!(q.jobs.is_empty());
        assert_eq!(q.ordinary.next_id,0);
        assert_eq!(q.methods.next_id,0);
        q.next_id=0;
        assert_eq!(ordinary(&mut q,ORDINARY),1);
        assert_eq!(method(&mut q,METHOD),2);
        assert_eq!(q.execution_order(),[]);
    }
}
