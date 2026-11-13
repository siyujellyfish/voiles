// G18／P18.4-B4：原 Checker 普通函式在 main 真求值、返回後才進同步 self 方法。
// 只驗證零參數、無資源、單一 int 返回的工作圖；B4 仍需更多實作。
#[cfg(test)]
mod g18_b4_main_call_method_tests {
    use super::*;

    const MAIN_HELPER: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n+1
}
}
fn helper()->int{
return 4
}
fn main()->int{
var owner:Counter = Counter()
const from_helper:int = helper()
return owner.read(from_helper)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = PreparedPrivateSelfMethod::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn b4_main_原普通函式先返回後同步self取得票據且結果正確() {
        let out = run(MAIN_HELPER);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.ordinary_calls_completed, 1);
        assert!(out.events.is_empty());
        assert!(out.ticket_returned);
        let entries = out.exit_events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(index)
        ).collect::<Vec<_>>();
        let first_return = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0] < first_return && first_return < loan && loan < entries[1]);
    }

    #[test]
    fn b4_main_兄弟兩次零參數函式先各自返回再將兩值傳給self() {
        let source = MAIN_HELPER.replace(
            "const from_helper:int = helper()\nreturn owner.read(from_helper)",
            "const first:int = helper()\nconst second:int = helper()\nreturn owner.read(first+second)",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.ordinary_calls_completed, 2);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 3);
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 2);
    }

    #[test]
    fn b4_main_同一普通fn先在main後在方法局部各自真執行並歸還self() {
        let source = MAIN_HELPER.replace(
            "fn read(n:int)->int{\nreturn n+1",
            "fn read(n:int)->int{\nconst in_method:int = helper()\nreturn n+in_method",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.ordinary_calls_completed, 2);
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_enters_before_loan = out.exit_events[..loan].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count();
        let helper_enters_after_loan = out.exit_events[loan..].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count();
        assert_eq!((helper_enters_before_loan, helper_enters_after_loan), (1, 2));
    }

    #[test]
    fn b4_main_ordinary函式成功後方法中途故障必保留已完成返回並真還票() {
        let plan = PreparedPrivateSelfMethod::prepare(MAIN_HELPER).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.ordinary_calls_completed, 1);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn b4_main_ordinary返回後main後序算術溢位不得提前借用self或進方法() {
        let source = MAIN_HELPER.replace(
            "const from_helper:int = helper()\nreturn owner.read(from_helper)",
            "const from_helper:int = helper()\nconst overflow:int = 9223372036854775807+1\nreturn owner.read(from_helper)",
        );
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.ordinary_calls_completed, 1);
        assert!(!out.method_entered);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
                | PrivateSharedCallExitEvent::LoanReturned(_)
        )));
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_main_主函式已編譯Call來源被改成常數須在執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(MAIN_HELPER).unwrap();
        assert!(matches!(
            plan.main_init_plans[0], PrivateScalarReturnExpr::Call { .. }
        ));
        plan.main_init_plans[0] = PrivateScalarReturnExpr::Constant(4);
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn b4_main_普通函式未宣告或型別不相容必由原Checker拒絕() {
        for source in [
            MAIN_HELPER.replace("fn helper()->int{\nreturn 4\n}\n", ""),
            MAIN_HELPER.replace("const from_helper:int = helper()",
                "const from_helper:bool = helper()"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不能以動態純 int 計畫冒充 Checker 型別允許：{source}");
        }
    }
}
