// G18／P18.4-B4／M3：無方法 int 局部時最後直接
// return 已核准純 int helper，也必經原 Checker/ScalarHIR、
// 原 BorrowTicket 與普通函式共通退出責任。
#[cfg(test)]
mod g18_b4_m3_final_helper_without_locals_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return helper(9,2)
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(plan.method_int_locals.is_empty());
        assert!(matches!(&plan.return_calculation,
            PrivateSelfIntReturn::CheckedScalar(PrivateScalarReturnExpr::Call { .. })
        ));
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(), "Copyable int 不得假造 P16 UTF-8 資源");
        out
    }

    #[test]
    fn b4_final_無方法局部直接Return雙參數helper真返回7() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(out.method_entered && out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::Returned
        )).count(),2);
        let helper_exit=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:true }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(helper_exit<loan);
    }

    #[test]
    fn b4_final_零參數原Checker純helper也能當方法末尾直接Return() {
        let src=SOURCE.replace("return helper(9,2)","return helper()")
            .replace("fn helper(a:int,b:int)->int","fn helper()->int")
            .replace("return a-b","return (8+5)-2");
        let out=run(&src);
        assert_eq!(out.result,Ok(11));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn b4_final_四個獨立int形參可先求全實參再進普通函式() {
        let src=SOURCE.replace("return helper(9,2)","return helper(9,2,5,1)")
            .replace("fn helper(a:int,b:int)->int","fn helper(a:int,b:int,c:int,d:int)->int")
            .replace("return a-b","return a-b+c-d");
        let out=run(&src);
        assert_eq!(out.result,Ok(11));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn b4_final_方法已獨占更新self欄位再Return純helper只借還同一票() {
        let src=SOURCE.replace("return helper(9,2)","self.n=19\nreturn helper(9,2)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn b4_final_if_true提前回self欄位不得執行未達最後helper() {
        let src=SOURCE.replace("fn read()->int{",
            "fn read(flag:bool)->int{\nif(flag){\nreturn self.n\n}")
            .replace("owner.read()","owner.read(true)");
        let true_out=run(&src);
        assert_eq!(true_out.result,Ok(7));
        assert_eq!(true_out.branch_checked,Some(true));
        assert_eq!((true_out.ordinary_calls_entered,true_out.ordinary_calls_completed),(0,0));
        let false_out=run(&src.replace("owner.read(true)","owner.read(false)"));
        assert_eq!(false_out.result,Ok(7));
        assert_eq!(false_out.branch_checked,Some(false));
        assert_eq!((false_out.ordinary_calls_entered,false_out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn b4_final_最後引數溢位未進helper但仍真還self票() {
        let src=SOURCE.replace("helper(9,2)",
            "helper(9,9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
        assert!(!out.method_exited);
    }

    #[test]
    fn b4_final_helper本體溢位先退普通函式再歸還唯一借用票() {
        let src=SOURCE.replace("return a-b",
            "return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        let inner=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:false }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner<loan);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_final_原Checker錯型別巢狀函式與竄改方法Return計畫必拒絕() {
        for src in [
            SOURCE.replace("helper(9,2)","helper(9,true)"),
            SOURCE.replace("helper(9,2)","helper(9)"),
            SOURCE.replace("helper(9,2)","helper(9,helper(2,1))"),
            SOURCE.replace("return helper(9,2)","return 1+helper(9,2)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_)) |
                Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不允許未證純 int helper 混合語意：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.return_calculation=PrivateSelfIntReturn::CheckedScalar(
            PrivateScalarReturnExpr::Constant(7)
        );
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
