// G18／P18.4-B4／M3：原 Checker 真 if(bool) 選中提前
// Return 純 int helper，未選葉不可提前求值、Enter 或偽造借用退出。
#[cfg(test)]
mod g18_b4_m3_if_selected_helper_return_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var value:int = 7
fn read(flag:bool)->int{
if flag{
return helper(3,2)
}
return self.value
}
}
fn helper(a:int,b:int)->int{
return a+b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(matches!(&plan.method_branch_return,
            Some(PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Call { .. }
            ))));
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit, out.method_local_slots_after_exit), (0, 0));
        assert!(out.events.is_empty(), "純 int Return 不能虛構 UTF-8 ResourceId");
        out
    }

    #[test]
    fn b4_selected_動態bool_true真進helper並提前Return5再還票() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.branch_checked, Some(true));
        assert!(out.method_entered && out.method_exited);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let enter = out.exit_events.iter().enumerate().filter_map(|(i,e)|
            matches!(e, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(enter < loan);
    }

    #[test]
    fn b4_selected_動態bool_false跳過未選helper且返回self原欄位7() {
        let src = SOURCE.replace("owner.read(true)", "owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
    }

    #[test]
    fn b4_selected_未選true葉即便helper返回會溢位仍不得提前求值() {
        let src = SOURCE.replace("return a+b",
            "return (a+b)+9223372036854775807")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_selected_真選中helper返回溢位時內層先失敗再歸還self票() {
        let src = SOURCE.replace("return a+b",
            "return (a+b)+9223372036854775807");
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        let failed = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(failed < loan);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_selected_真選中實參先溢位不得假造helper進入() {
        let src = SOURCE.replace("helper(3,2)",
            "helper(9223372036854775807+1,2)");
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert!(out.method_entered && !out.method_exited);
    }

    #[test]
    fn b4_selected_未選實參即使會溢位仍返回欄位且不進helper() {
        let src = SOURCE.replace("helper(3,2)",
            "helper(9223372036854775807+1,2)")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_selected_先獨占寫入self後選中helper返回與借用仍只一張票() {
        let src = SOURCE.replace(
            "if flag{\nreturn helper(3,2)", "if flag{\nself.value = 10\nreturn helper(3,2)",
        );
        let plan = PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        let out = run(&src);
        assert_eq!(out.result, Ok(5));
        assert!(out.branch_write_applied);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn b4_selected_錯型別巢狀普通呼叫與竄改真選葉必拒絕() {
        for src in [
            SOURCE.replace("return helper(3,2)", "return helper(3,true)"),
            SOURCE.replace("return helper(3,2)", "return helper(3)"),
            SOURCE.replace("return helper(3,2)", "return helper(3,helper(1,2))"),
            SOURCE.replace("fn helper(a:int,b:int)->int", "fn helper(a:int,b:bool)->int")
                .replace("return a+b", "return a"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "原 Checker 未核准的 if-Return Call 不得執行：{src}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_branch_return = Some(PrivateSelfIntReturn::Constant(5));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut out = run(SOURCE);
        out.ordinary_calls_completed = 0;
        assert!(!out.has_balanced_self_borrow_trace());
    }
}
