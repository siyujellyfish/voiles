// G18／P18.4-B4／M3：原 Checker if/else 的真正 else
// Return 純 helper、未選葉零事件、else 獨占寫入後的單票據歸還。
#[cfg(test)]
mod g18_b4_m3_if_else_helper_return_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
return self.n
}else{
return helper(3,2)
}
return self.n
}
}
fn helper(a:int,b:int)->int{
return a+b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(false)
}
"#;

    fn run(src: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(src).unwrap();
        assert!(matches!(&plan.method_branch_else_calculation,
            Some(PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Call { .. }
            ))));
        assert!(plan.method_branch_else_field_return.is_some());
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit, out.method_local_slots_after_exit), (0, 0));
        assert!(out.events.is_empty(), "Copyable bool/int helper 不得造假 UTF-8 ResourceId");
        out
    }

    #[test]
    fn b4_else_動態bool_false選中else真helper返回5並還唯一票() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let inner = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner < loan);
    }

    #[test]
    fn b4_else_動態bool_true只return_self欄位不提前呼叫else_helper() {
        let src = SOURCE.replace("owner.read(false)", "owner.read(true)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
    }

    #[test]
    fn b4_else_未選else即便helper本體溢位也必須返回原self7() {
        let src = SOURCE.replace("owner.read(false)", "owner.read(true)")
            .replace("return a+b", "return (a+b)+9223372036854775807");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_else_選中else_helper本體溢位先退內層再歸還self票() {
        let src = SOURCE.replace("return a+b", "return (a+b)+9223372036854775807");
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        let inner = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner < loan);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn b4_else_選中else第二實參先溢位不得假進普通helper() {
        let src = SOURCE.replace(
            "helper(3,2)", "helper(3,9223372036854775807+1)",
        );
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn b4_else_未選else第二實參即使溢位亦不可偽造錯誤() {
        let src = SOURCE.replace("owner.read(false)", "owner.read(true)")
            .replace("helper(3,2)", "helper(3,9223372036854775807+1)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_else_獨占else先寫self後return_helper仍只進退一張票() {
        let src = SOURCE.replace(
            "else{\nreturn helper(3,2)",
            "else{\nself.n = 10\nreturn helper(3,2)",
        );
        let plan = PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        let out = run(&src);
        assert_eq!(out.result, Ok(5));
        assert!(out.branch_else_write_applied);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn b4_else_非法混型遞迴與竄改else真Return計畫皆拒絕() {
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
            ), "原 Checker 未授權的 else Call 必須拒絕：{src}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_branch_else_calculation = Some(PrivateSelfIntReturn::Constant(5));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut outcome = run(SOURCE);
        outcome.ordinary_calls_completed = 0;
        assert!(!outcome.has_balanced_self_borrow_trace());
    }
}
