// G18／P18.4-B4／M3：方法最後的原 Checker int Return
// 直接進入普通同步 helper，Caller 實參先求值，helper 真退場後
// 才由 R05 逆序清理方法局部並歸還唯一 self BorrowTicket。
#[cfg(test)]
mod g18_b4_method_return_helper_exit_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
const base:int = n
return helper(base+2,4)
}
}
fn helper(a:int,b:int)->int{
return (a+b)-2
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(3)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(matches!(plan.return_calculation,
            PrivateSelfIntReturn::CheckedScalar(PrivateScalarReturnExpr::Call { .. })));
        let outcome = plan.execute().unwrap();
        assert!(outcome.has_balanced_self_borrow_trace());
        assert!(outcome.ticket_returned);
        assert_eq!(outcome.main_local_slots_after_exit, 0);
        assert_eq!(outcome.method_local_slots_after_exit, 0);
        assert!(outcome.events.is_empty(), "純 Copyable int 不得虛構 UTF-8 資源");
        outcome
    }

    #[test]
    fn b4_exit_方法直接return已核准雙int_helper結果7且真還self票() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(7));
        assert!(out.method_entered && out.method_exited);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let exit = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let scope = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(exit < scope && scope < loan, "helper 返回後才退出方法局部與票據");
    }

    #[test]
    fn b4_exit_方法已核准局部加減作多形參return_helper仍保留原位置() {
        let src = SOURCE.replace(
            "return helper(base+2,4)", "return helper(base+2,base+4)",
        );
        let out = run(&src);
        assert_eq!(out.result, Ok(10));
        assert_eq!(out.method_local_events.iter().filter(|e| matches!(
            e, PrivateSelfMethodLocalEvent::Initialized(_, 3)
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 2);
    }

    #[test]
    fn b4_exit_方法var先重賦合法值然後真正return_helper用目前局部() {
        let src = SOURCE.replace(
            "const base:int = n\nreturn helper(base+2,4)",
            "var base:int = n\nbase=base+2\nreturn helper(base,4)",
        );
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert!(out.method_local_events.iter().any(|e| matches!(
            e, PrivateSelfMethodLocalEvent::Assigned(_, 5)
        )));
        let assigned = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalAssigned(_)
        )).unwrap();
        let entered = out.exit_events.iter().enumerate().filter_map(|(i,e)|
            matches!(e, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        assert!(assigned < entered);
    }

    #[test]
    fn b4_exit_先helper初始化局部再return同helper應為兩次不同真框架() {
        let src = SOURCE.replace(
            "const base:int = n", "const base:int = helper(n,2)",
        );
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 3)));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 3);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 3);
    }

    #[test]
    fn b4_exit_return實參溢位不進普通helper但清理已初始化局部與self票() {
        let src = SOURCE.replace(
            "helper(base+2,4)", "helper(base,9223372036854775807+1)",
        );
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert_eq!(out.method_local_events.len(), 2);
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 3)));
        assert!(matches!(out.method_local_events[1],
            PrivateSelfMethodLocalEvent::ScopeEnded(_)));
    }

    #[test]
    fn b4_exit_helper本體溢位要先退出子框架再逆序離域及還票() {
        let src = SOURCE.replace("return (a+b)-2", "return (a+b)+9223372036854775807");
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        let inner = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let scope = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner < scope && scope < loan);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_exit_直接return零參數純helper亦需真呼叫及退出() {
        let src = SOURCE.replace("helper(base+2,4)", "helper()")
            .replace("fn helper(a:int,b:int)->int{\nreturn (a+b)-2",
                "fn helper()->int{\nreturn (8+5)-2");
        let out = run(&src);
        assert_eq!(out.result, Ok(11));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 2);
    }

    #[test]
    fn b4_exit_原Checker禁止return中隱藏函式呼叫型別錯配與計畫篡改() {
        for src in [
            SOURCE.replace("return helper(base+2,4)", "return 1+helper(base+2,4)"),
            SOURCE.replace("return helper(base+2,4)", "return helper(base+2,true)"),
            SOURCE.replace("return helper(base+2,4)", "return helper(base+2)"),
            SOURCE.replace("return (a+b)-2", "return helper(a,b)"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "原 Checker 未核准 return Call 不得進入 Runtime：{src}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.return_calculation = PrivateSelfIntReturn::CheckedScalar(
            PrivateScalarReturnExpr::Constant(7),
        );
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut out = run(SOURCE);
        out.ordinary_calls_entered += 1;
        assert!(!out.has_balanced_self_borrow_trace());
    }
}
