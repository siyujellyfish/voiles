// G18／P18.4-B4／M3：巢狀真 if(flag) 雙葉都是同一原 Checker
// 純 int 普通 helper，只有選中葉真正 Enter、返回或受控 Unwind。
#[cfg(test)]
mod g18_b4_m3_nested_if_helper_return_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(true){
if(flag){
return helper(9,2)
}else{
return helper(4,8)
}
}
return self.n+1
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true)
}
"#;

    fn run(src: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(src).unwrap();
        assert!(plan.method_nested_branch.is_some());
        assert!(matches!(&plan.method_branch_return,
            Some(PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Call { .. }
            ))));
        assert!(matches!(&plan.method_nested_else_calculation,
            Some(PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Call { .. }
            ))));
        let out = plan.execute().unwrap();
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(), "Copyable 純 int 不得虛構 P16 資源");
        out
    }

    #[test]
    fn b4_nested_外層真內層bool_true唯一helper返回7再歸還票() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(true),Some(true)));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let inner_exit = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner_exit < loan);
    }

    #[test]
    fn b4_nested_內層bool_false僅選else_helper返回負4() {
        let out=run(&SOURCE.replace("owner.read(true)","owner.read(false)"));
        assert_eq!(out.result,Ok(-4));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(true),Some(false)));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),2);
    }

    #[test]
    fn b4_nested_外層bool_false完全跳過內層兩個helper() {
        let src=SOURCE.replace("if(true){\nif(flag)","if(flag){\nif(flag)")
            .replace("owner.read(true)","owner.read(false)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(false),None));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);
    }

    #[test]
    fn b4_nested_選中true時未選else的實參即使溢位亦零事件() {
        let src=SOURCE.replace("helper(4,8)",
            "helper(9223372036854775807+1,8)");
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_nested_選中else時未選true的實參溢位也不可提前求值() {
        let src=SOURCE.replace("helper(9,2)",
            "helper(9223372036854775807+1,2)")
            .replace("owner.read(true)","owner.read(false)");
        let out=run(&src);
        assert_eq!(out.result,Ok(-4));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_nested_選中else第二實參溢位不能進helper且唯一Unwind() {
        let src=SOURCE.replace("helper(4,8)",
            "helper(4,9223372036854775807+1)")
            .replace("owner.read(true)","owner.read(false)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_nested_雙葉可獨占寫入但僅被選一葉且真helper先退出再還票() {
        let src=SOURCE.replace("return helper(9,2)","self.n=19\nreturn helper(9,2)")
            .replace("return helper(4,8)","self.n=23\nreturn helper(4,8)")
            .replace("owner.read(true)","owner.read(false)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        let out=run(&src);
        assert_eq!(out.result,Ok(-4));
        assert!(!out.nested_write_applied && out.nested_else_write_applied);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn b4_nested_選中helper溢位內後外退出且篡改else來源必拒絕() {
        let src=SOURCE.replace("return a-b","return (a+b)+9223372036854775807")
            .replace("owner.read(true)","owner.read(false)");
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
        let mut tampered=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        tampered.method_nested_else_calculation=Some(PrivateSelfIntReturn::Constant(-4));
        assert!(matches!(tampered.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let illegal=SOURCE.replace("return helper(4,8)","return helper(4,helper(2,1))");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&illegal),
            Err(PrivateSelfMethodPrepareError::Check(_)) |
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }
}
