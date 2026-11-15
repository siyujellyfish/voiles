// G18／P18.6-M2／P18.4-B4：原 Checker 已核准普通函式
// 直接作同步 self 欄位 RHS；借票前不得求本體，完成所有
// 純 int helper 真函式進退後才可提交唯一欄位寫入。
#[cfg(test)]
mod g18_m2_direct_helper_self_write_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var count:int = 7
fn set(n:int)->int{
self.count = add(n,3)
return self.count
}
}
fn add(a:int,b:int)->int{
return a+b
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(9)
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("只能依原 Parser／Checker int ScalarHIR 與自借票安全核准");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        assert!(plan.write_checked_int.is_some());
        assert!(plan.write_parameter.is_none() && plan.write_value.is_none());
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out=checked(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"Copyable int 不可假造資源事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn count(out: &PrivateSelfMethodOutcome) -> (usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn m2_direct_helper_借self唯一票後真add返回12再提交欄位() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(12));
        assert_eq!(count(&out),(1,1));
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let entered=out.exit_events.iter().enumerate().filter_map(|(i,event)|
            matches!(event,PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        let returned=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(loan<entered && entered<returned);
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn m2_direct_helper_兩個正式int形參值必逐一綁定各自DeclId() {
        let src=SOURCE.replace("fn set(n:int)->int{\nself.count = add(n,3)",
            "fn set(left:int,right:int)->int{\nself.count = add(left,right)")
            .replace("owner.set(9)","owner.set(9,7)");
        let out=run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(count(&out),(1,1));
    }

    #[test]
    fn m2_direct_helper_加減左右兩個真函式依Parser求值後寫18() {
        let src=SOURCE.replace("self.count = add(n,3)",
            "self.count = add(n,3)+sub(8,2)")
            .replace("fn main()->int{",
                "fn sub(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{");
        let out=run(&src);
        assert_eq!(out.result,Ok(18));
        assert_eq!(count(&out),(2,2));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn m2_direct_helper_巢狀原Checker兩層helper先內後外真寫10() {
        let src=SOURCE.replace("self.count = add(n,3)",
            "self.count = add(sub(n,2),3)")
            .replace("fn main()->int{",
                "fn sub(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{");
        let out=run(&src);
        assert_eq!(out.result,Ok(10));
        assert_eq!(count(&out),(2,2));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn m2_direct_helper_實參先算術溢位不准假進add且真還票() {
        let src=SOURCE.replace("add(n,3)",
            "add(n+9223372036854775807,3)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(0,0));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_direct_helper_真add本體溢位先子函式失敗再歸還借票() {
        let src=SOURCE.replace("return a+b",
            "return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,0));
        let failed=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).unwrap();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(failed<loan);
    }

    #[test]
    fn m2_direct_helper_外層純加法溢位保留已成功helper的真退出() {
        let src=SOURCE.replace("self.count = add(n,3)",
            "self.count = add(n,3)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,1));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn m2_direct_helper_經原Checker已證同步純值函式可重複執行不遺留票() {
        let plan=checked(SOURCE);
        for _ in 0..3 {
            let out=plan.execute().unwrap();
            assert_eq!(out.result,Ok(12));
            assert_eq!(count(&out),(1,1));
            assert!(out.has_balanced_self_borrow_trace());
            assert!(out.ticket_returned);
        }
    }

    #[test]
    fn m2_direct_helper_非法型別資源來源與竄改真函式圖都須拒絕() {
        for src in [
            SOURCE.replace("add(n,3)","add(true,3)"),
            SOURCE.replace("add(n,3)","missing(n,3)"),
            SOURCE.replace("fn add(a:int,b:int)->int{\nreturn a+b",
                "fn add(a:int,b:int)->int{\nvar s:str = \"資源不可進入\"\nreturn a+b"),
            SOURCE.replace("var owner:Counter","const owner:Counter"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "未經原Checker批准的直接self RHS不得執行：{src}");
        }
        let mut plan=checked(SOURCE);
        plan.write_checked_int=Some(PrivateScalarReturnExpr::Constant(12));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(SOURCE);
        plan.method_helpers.get_mut("add").unwrap().returned =
            PrivateScalarReturnExpr::Constant(12);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
