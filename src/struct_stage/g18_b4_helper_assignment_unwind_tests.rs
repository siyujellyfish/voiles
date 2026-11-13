// G18／P18.4-B4：原 Checker var int 方法重賦值直接 helper RHS。
// 只在真 RHS 到達時進入普通函式；失敗不提交 LocalAssigned，
// R05 先退失敗 helper 再逆序清理方法局部與 self 唯一 BorrowTicket。
#[cfg(test)]
mod g18_b4_helper_assignment_unwind_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var value:int = 7
fn read()->int{
var temp:int = 2
temp=helper()
return temp+1
}
}
fn helper()->int{
return (8+5)-2
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        let result = plan.execute().unwrap();
        assert!(result.has_balanced_self_borrow_trace());
        assert!(result.ticket_returned);
        assert_eq!(result.main_local_slots_after_exit, 0);
        assert_eq!(result.method_local_slots_after_exit, 0);
        assert!(result.events.is_empty(), "Copyable int 賦值不能創造假 UTF-8 資源");
        result
    }

    #[test]
    fn b4_helper_assign_原Checker真helper算術作var更新返回12並還票() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(12));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert_eq!(out.method_local_events.len(), 3);
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 2)));
        assert!(matches!(out.method_local_events[1],
            PrivateSelfMethodLocalEvent::Assigned(_, 11)));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 2);
    }

    #[test]
    fn b4_helper_assign_兩次var重賦各走真helper框架最後結果11() {
        let source = SOURCE.replace("temp=helper()", "temp=helper()\ntemp=helper()")
            .replace("return temp+1", "return temp");
        let out = run(&source);
        assert_eq!(out.result, Ok(11));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
        assert_eq!(out.method_local_events.iter().filter(|e| matches!(
            e, PrivateSelfMethodLocalEvent::Assigned(_, 11)
        )).count(), 2);
    }

    #[test]
    fn b4_helper_assign_原Checker初始化與重賦兩次真呼叫不混用框架() {
        let source = SOURCE.replace("var temp:int = 2", "var temp:int = helper()");
        let out = run(&source);
        assert_eq!(out.result, Ok(12));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 11)));
        assert!(matches!(out.method_local_events[1],
            PrivateSelfMethodLocalEvent::Assigned(_, 11)));
    }

    #[test]
    fn b4_helper_assign_前序純整數更新先執行後才進入真helper() {
        let source = SOURCE.replace("temp=helper()", "temp=temp+1\ntemp=helper()");
        let out = run(&source);
        assert_eq!(out.result, Ok(12));
        let updates = out.method_local_events.iter().filter_map(|event| match event {
            PrivateSelfMethodLocalEvent::Assigned(_, value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(updates, vec![3, 11]);
        let first_update = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalAssigned(_)
        )).unwrap();
        let helper_enter = out.exit_events.iter().enumerate().filter_map(|(i, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        assert!(first_update < helper_enter);
    }

    #[test]
    fn b4_helper_assign_真helper溢位不能提交var而須內後外退出還票() {
        let source = SOURCE.replace("return (8+5)-2", "return 9223372036854775807+1");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        assert!(out.method_entered);
        assert!(!out.method_exited);
        assert!(!out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, _)
        )));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        let inner_exit = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let local_end = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let loan_end = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner_exit < local_end && local_end < loan_end);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_helper_assign_已有前序更新後右值溢位不得偽造第二次LocalAssigned() {
        let source = SOURCE.replace("temp=helper()", "temp=temp+1\ntemp=helper()")
            .replace("return (8+5)-2", "return 9223372036854775807+1");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.method_local_events.iter().filter(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, _)
        )).count(), 1);
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, 3)
        )));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_helper_assign_篡改已核准Call計畫或偽造呼叫次數必拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert!(matches!(plan.method_int_assignments[0].1,
            PrivateScalarReturnExpr::Call { .. }));
        plan.method_int_assignments[0].1 = PrivateScalarReturnExpr::Constant(11);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut outcome = run(SOURCE);
        outcome.ordinary_calls_completed += 1;
        assert!(!outcome.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_helper_assign_原Checker拒絕const目標與將helper嵌入算式及錯型別() {
        for source in [
            SOURCE.replace("var temp:int = 2", "const temp:int = 2"),
            SOURCE.replace("temp=helper()", "temp=1+helper()"),
            SOURCE.replace("fn helper()->int{\nreturn (8+5)-2",
                "fn helper()->bool{\nreturn true"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "原 Checker 非法的賦值／函式語意不得進入 Runtime：{source}");
        }
    }
}
