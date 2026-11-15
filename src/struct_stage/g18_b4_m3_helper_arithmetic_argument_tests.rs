// G18／P18.4-B4／P18.6-M3：原 Checker 已可核准
// 方法 Return 中 helper(leaf()+int) 的同源純 int AST；
// 真執行必對各層 TypedHIR 左至右求值、真 Enter/Exit
// 及 HC14 self 票據清理，不在 prepare 假完成任何函式。
#[cfg(test)]
mod g18_b4_m3_helper_arithmetic_argument_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;

    const DIRECT: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return outer(inner()+2)
}
}
fn inner()->int{
return 5
}
fn outer(n:int)->int{
return n+1
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("方法純int巢狀算術實參必使用原Checker/ScalarHIR");
        assert_eq!(plan.loan.kind,BorrowKind::SharedRead);
        plan
    }

    fn execute(source: &str) -> PrivateSelfMethodOutcome {
        let result=checked(source).execute().expect("原Checker真方法框架");
        assert!(result.has_balanced_self_borrow_trace());
        assert!(result.ticket_returned);
        assert!(result.events.is_empty(),"純int不得偽造P16資源");
        assert_eq!((result.main_local_slots_after_exit,
                    result.method_local_slots_after_exit),(0,0));
        result
    }

    fn calls(out: &PrivateSelfMethodOutcome)->(usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn m3_helper_arg_arith_真Checker頂層Call保存子Call加法TypedHIR() {
        let plan=checked(DIRECT);
        let PrivateSelfIntReturn::CheckedScalar(
            PrivateScalarReturnExpr::Call{arguments,awaited:false,..}
        )=&plan.return_calculation else { panic!("原Checker頂層Call"); };
        assert_eq!(arguments.len(),1);
        assert!(matches!(&arguments[0],PrivateScalarCallArgument::Int(
            PrivateScalarReturnExpr::Addition(_,_)
        )));
        let out=execute(DIRECT);
        assert_eq!(out.result,Ok(8));
        assert_eq!(calls(&out),(2,2));
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn m3_helper_arg_arith_先子helper返回再父helper入框架與歸self票() {
        let out=execute(DIRECT);
        let a=&out.exit_events;
        assert!(matches!(a[0],E::LoanAcquired(_)));
        assert!(matches!(a[1],E::FunctionEntered));
        assert!(matches!(a[2],E::FunctionEntered));
        assert!(matches!(a[3],E::Returned));
        assert!(matches!(a[4],E::FunctionExited{returned:true}));
        assert!(matches!(a[5],E::FunctionEntered));
        assert!(matches!(a[6],E::Returned));
        assert!(matches!(a[7],E::FunctionExited{returned:true}));
        assert!(matches!(a[8],E::Returned));
        assert!(matches!(a[9],E::LoanReturned(_)));
        assert!(matches!(a[10],E::FunctionExited{returned:true}));
        assert_eq!(a.len(),11);
    }

    #[test]
    fn m3_helper_arg_arith_真雙層Group括號不重做inner呼叫() {
        let source=DIRECT.replace("outer(inner()+2)","outer(((inner()+2)))");
        let out=execute(&source);
        assert_eq!(out.result,Ok(8));
        assert_eq!(calls(&out),(2,2));
    }

    #[test]
    fn m3_helper_arg_arith_減法先常值後子Call返回9() {
        let source=DIRECT.replace("outer(inner()+2)","outer(13-inner())");
        let out=execute(&source);
        assert_eq!(out.result,Ok(9));
        assert_eq!(calls(&out),(2,2));
    }

    #[test]
    fn m3_helper_arg_arith_巢狀加法由原AST左至右各計算一次() {
        let source=DIRECT.replace("outer(inner()+2)","outer(inner()+2+3)");
        let out=execute(&source);
        assert_eq!(out.result,Ok(11));
        assert_eq!(calls(&out),(2,2));
    }

    #[test]
    fn m3_helper_arg_arith_多形參父helper的兩個算術實參各有真子退出() {
        let source=DIRECT.replace("return outer(inner()+2)",
                "return outer(inner()+2,inner()+3)")
            .replace("fn outer(n:int)->int{\nreturn n+1",
                "fn outer(a:int,b:int)->int{\nreturn a+b");
        let out=execute(&source);
        assert_eq!(out.result,Ok(15));
        assert_eq!(calls(&out),(3,3));
    }

    #[test]
    fn m3_helper_arg_arith_兩層父子helper加算術再套上層helper() {
        let source=DIRECT.replace("return outer(inner()+2)",
                "return outer(merge(inner()+2,inner()+3))")
            .replace("fn main()->int{",
                "fn merge(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{");
        let out=execute(&source);
        assert_eq!(out.result,Ok(16));
        assert_eq!(calls(&out),(4,4));
    }

    #[test]
    fn m3_helper_arg_arith_內層算術溢位只完成子helper不可假進父helper() {
        let source=DIRECT.replace("inner()+2",
            "inner()+9223372036854775807");
        let out=execute(&source);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(calls(&out),(1,1));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::FunctionExited{returned:true})).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::FunctionExited{returned:false})).count(),1);
        let unwind=out.exit_events.iter().position(|e|
            matches!(e,E::UnwindStarted)).unwrap();
        let loan=out.exit_events.iter().position(|e|
            matches!(e,E::LoanReturned(_))).unwrap();
        assert!(unwind<loan);
    }

    #[test]
    fn m3_helper_arg_arith_子helper本體故障父helper不能入框架() {
        let source=DIRECT.replace("return 5",
            "return 9223372036854775807+1");
        let out=execute(&source);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(calls(&out),(1,0));
        assert!(out.ticket_returned);
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::LoanReturned(_))).count(),1);
    }

    #[test]
    fn m3_helper_arg_arith_父helper故障時必先正常退出子helper() {
        let source=DIRECT.replace("return n+1",
            "return n+9223372036854775807");
        let out=execute(&source);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(calls(&out),(2,1));
        assert_eq!(out.exit_events.iter().filter(|e|
            matches!(e,E::FunctionExited{returned:false})).count(),2);
        assert!(out.ticket_returned);
    }

    #[test]
    fn m3_helper_arg_arith_if選葉真執行與false旁路零副作用() {
        let source=DIRECT.replace("fn read()->int{\nreturn outer(inner()+2)",
            "fn read(flag:bool)->int{\nif(flag){\nreturn outer(inner()+2)\n}\nreturn self.n")
            .replace("owner.read()","owner.read(true)");
        let yes=execute(&source);
        assert_eq!(yes.result,Ok(8));
        assert_eq!(calls(&yes),(2,2));
        let no=execute(&source.replace("owner.read(true)","owner.read(false)"));
        assert_eq!(no.result,Ok(7));
        assert_eq!(calls(&no),(0,0));
    }

    #[test]
    fn m3_helper_arg_arith_async_self直接await相同純值子圖與延遲借票() {
        let source=DIRECT.replace("fn read()->int","async fn read()->int")
            .replace("return owner.read()","return await owner.read()");
        let out=execute(&source);
        assert_eq!(out.result,Ok(8));
        assert_eq!(calls(&out),(2,2));
        assert!(out.awaited_method);
        assert_eq!(out.await_start_loan_index,Some(0));
        assert!(out.ticket_returned);
    }

    #[test]
    fn m3_helper_arg_arith_錯型別async子函式與self欄位不得冒充純helper() {
        for source in [
            DIRECT.replace("inner()+2","inner()+true"),
            DIRECT.replace("inner()+2","inner()+self.n"),
            DIRECT.replace("fn inner()->int","async fn inner()->int"),
            DIRECT.replace("return outer(inner()+2)","return outer(missing()+2)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "禁止未簽發純int巢狀工作計畫：{source}");
        }
    }

    #[test]
    fn m3_helper_arg_arith_篡改真inner返回計畫或父Call算術來源必拒絕() {
        let mut plan=checked(DIRECT);
        plan.method_helpers.get_mut("inner").unwrap().returned =
            PrivateScalarReturnExpr::Constant(6);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(DIRECT);
        plan.return_calculation=PrivateSelfIntReturn::Constant(8);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
