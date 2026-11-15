// G18／P18.6-A1/M2／V7：由原 Lexer/Parser/Checker 的
// AwaitMethodCall 唯一來源，直接 await 才在真 main 值框架中
// 進入 async self 方法並取得 R05/HC14 自動借票。此批不開放
// 有內部 await 的 async self 跨安全點續行，不冒稱 A1/A2 Closed。
#[cfg(test)]
mod g18_v7_async_self_direct_await_tests {
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

    fn prepare(source: &str) -> PreparedPrivateSelfMethod {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("原Checker須唯一核准真async方法直接await與唯一self借用");
        assert!(plan.awaited_method);
        assert!(matches!(
            &plan.verified.functions.iter().find(|f| f.name == "main").unwrap()
                .body.statements.last().unwrap().kind,
            StmtKind::Return(returned) if matches!(
                &returned.expression.kind, TermKind::AwaitMethodCall(_, _, _)
            ),
        ));
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = prepare(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.awaited_method);
        assert!(out.events.is_empty(), "純int不允許虛造P16資源事件");
        assert_eq!((out.main_local_slots_after_exit,
                    out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn v7_async_self_原LexerParserChecker識別async方法與await方法專用AST() {
        let plan = prepare(READ);
        assert_eq!(plan.loan.kind,BorrowKind::SharedRead);
        let main = plan.verified.functions.iter().find(|f|f.name=="main").unwrap();
        let StmtKind::Return(r) = &main.body.statements.last().unwrap().kind else {
            panic!("原Parser真return");
        };
        let TermKind::AwaitMethodCall(owner,method,args)=&r.expression.kind else {
            panic!("await 必須獨立於同步方法AST");
        };
        assert_eq!((owner.as_str(),method.as_str(),args.len()),
            ("owner","read",0));
        assert!(plan.verified.tokens.iter().any(|t|
            t.start==r.expression.start
                && matches!(&t.kind,TokenKind::Name(n) if n=="await")));
        assert_eq!(plan.loan.source,DeclarationId(main.body.statements[0].start));
    }

    #[test]
    fn v7_async_self_直接await才真進唯讀self框架並歸借票() {
        let out=run(READ);
        assert_eq!(out.result,Ok(10));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(out.method_entered && out.method_exited);
        let idx=out.await_start_loan_index.unwrap();
        assert!(matches!(out.exit_events[idx],E::LoanAcquired(_)));
        assert!(matches!(out.exit_events[idx+1],E::FunctionEntered));
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::LoanReturned(_))).count(),1);
    }

    #[test]
    fn v7_async_self_直接await獨占self方法真寫欄位並回傳() {
        let out=run(WRITE);
        assert_eq!(out.result,Ok(14));
        assert_eq!(out.checked_loan.kind,BorrowKind::ExclusiveWrite);
        assert_eq!(out.await_start_loan_index,Some(0));
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn v7_async_self_真main形參純函式必先退出再啟動async方法借票() {
        let source = WRITE.replace("owner.set(12)","owner.set(seed()+2)")
            .replace("fn main()->int{",
                "fn seed()->int{\nreturn 12\n}\nfn main()->int{");
        let out=run(&source);
        assert_eq!(out.result,Ok(16));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let borrow=out.await_start_loan_index.unwrap();
        assert_eq!(out.exit_events[..borrow].iter().filter(|e|
            matches!(e,E::FunctionExited{returned:true})).count(),1);
        assert_eq!(out.exit_events[..borrow].iter().filter(|e|
            matches!(e,E::LoanAcquired(_))).count(),0);
    }

    #[test]
    fn v7_async_self_真if選中葉內self獨占寫入而未選葉不寫() {
        let source = WRITE.replace(
            "async fn set(n:int)->int{\nself.n = n+2\nreturn self.n",
            "async fn set(flag:bool)->int{\nif(flag){\nself.n = 12\nreturn self.n\n}\nreturn self.n",
        ).replace("owner.set(12)","owner.set(false)");
        let no=run(&source);
        assert_eq!(no.result,Ok(7));
        assert!(!no.branch_write_applied);
        let yes=run(&source.replace("owner.set(false)","owner.set(true)"));
        assert_eq!(yes.result,Ok(12));
        assert!(yes.branch_write_applied);
    }

    #[test]
    fn v7_async_self_方法真溢位先Unwind再歸self票不得成功() {
        let source = READ.replace("return self.n+3",
            "return self.n+9223372036854775807");
        let out=run(&source);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.await_start_loan_index.is_some());
        let unwind=out.exit_events.iter().position(|e|matches!(
            e,E::UnwindStarted)).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,E::LoanReturned(_))).unwrap();
        assert!(unwind<loan);
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::FunctionExited{returned:false})).count(),1);
    }

    #[test]
    fn v7_async_self_借票前函式溢位不得假啟動async方法() {
        let source=WRITE.replace("owner.set(12)","owner.set(seed()+2)")
            .replace("fn main()->int{",
                "fn seed()->int{\nreturn 9223372036854775807+1\n}\nfn main()->int{");
        let out=run(&source);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!(out.await_start_loan_index,None);
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::LoanAcquired(_))).count(),0);
    }

    #[test]
    fn v7_async_self_未await的async方法和await同步方法均不得執行() {
        let no_await=READ.replace("return await owner.read()","return owner.read()");
        let sync=READ.replace("async fn read()","fn read()");
        for source in [no_await,sync] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))),
                "非同步宣告與直接等待必須同時被原Checker授權：{source}");
        }
    }

    #[test]
    fn v7_async_self_方法內再次直接await未證跨票據安全必靜態拒絕() {
        let source=READ.replace("return self.n+3","return await leaf()")
            .replace("fn main()->int{",
                "async fn leaf()->int{\nreturn 12\n}\nfn main()->int{");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn v7_async_self_錯參數非Copyable與未知方法均不能偷入await() {
        let wrong=WRITE.replace("owner.set(12)","owner.set(true)");
        let missing=READ.replace("owner.read()","owner.missing()");
        let resource=WRITE.replace("set(n:int)","set(n:str)")
            .replace("owner.set(12)","owner.set(\"unsafe\")");
        for source in [wrong,missing,resource] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不允許以錯型別或非Copyable繞過V7安全點：{source}");
        }
    }

    #[test]
    fn v7_async_self_常數接收者不得在await後執行獨占寫入() {
        let source=WRITE.replace("var owner:Counter","const owner:Counter");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn v7_async_self_原已簽await方法來源身份與真借票入場索引變造拒絕() {
        let mut plan=prepare(READ);
        plan.awaited_method=false;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut outcome=run(READ);
        outcome.await_start_loan_index=Some(1);
        assert!(!outcome.has_balanced_self_borrow_trace());
        let mut outcome=run(READ);
        outcome.awaited_method=false;
        assert!(!outcome.has_balanced_self_borrow_trace());
    }
}
