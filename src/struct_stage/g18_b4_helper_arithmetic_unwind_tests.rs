// G18／P18.4-B4：同一原 Checker 純 int helper 真加減、main／self
// 巢狀框架與單次 Unwind，沿用原 BorrowTicket 及 R05 退出帳。
#[cfg(test)]
mod g18_b4_helper_arithmetic_unwind_tests {
    use super::*;

    const METHOD: &str = r#"struct Counter{
var value:int = 7
fn read()->int{
const stable:int = 3
const from_helper:int = helper()
return stable+from_helper
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

    const MAIN: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n+1
}
}
fn helper()->int{
return (8+5)-2
}
fn main()->int{
var owner:Counter = Counter()
const from_helper:int = helper()
return owner.read(from_helper)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        let outcome = plan.execute().unwrap();
        assert!(outcome.has_balanced_self_borrow_trace());
        assert!(outcome.ticket_returned);
        assert!(outcome.events.is_empty(), "Copyable int 算術不得捏造 UTF-8 ResourceId");
        assert_eq!(outcome.main_local_slots_after_exit, 0);
        assert_eq!(outcome.method_local_slots_after_exit, 0);
        outcome
    }

    #[test]
    fn b4_helper_math_普通函式在main先真加減回傳11然後方法回傳12() {
        let outcome = run(MAIN);
        assert_eq!(outcome.result, Ok(12));
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (1, 1));
        let helper_exit = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let loan = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(helper_exit < loan, "main helper 必須先退出，才可借用 self");
    }

    #[test]
    fn b4_helper_math_方法持有self時真執行純整數巢狀算式並返回14() {
        let outcome = run(METHOD);
        assert_eq!(outcome.result, Ok(14));
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (1, 1));
        let loan = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_enter = outcome.exit_events.iter().enumerate().filter_map(|(i, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        let returned_loan = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(loan < helper_enter && helper_enter < returned_loan);
    }

    #[test]
    fn b4_helper_math_同一真helper在main與方法各執行一次且同源進退對帳() {
        let source = MAIN.replace(
            "fn read(n:int)->int{\nreturn n+1",
            "fn read(n:int)->int{\nconst local:int = helper()\nreturn n+local",
        );
        let outcome = run(&source);
        assert_eq!(outcome.result, Ok(22));
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (2, 2));
        assert_eq!(outcome.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 3);
        assert_eq!(outcome.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 3);
    }

    #[test]
    fn b4_helper_math_main普通函式加法溢位不能借用self且必真退出失敗框架() {
        let source = MAIN.replace(
            "return (8+5)-2", "return 9223372036854775807+1",
        );
        let outcome = run(&source);
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!outcome.method_entered);
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (1, 0));
        assert_eq!(outcome.exit_events, vec![
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::UnwindStarted,
            PrivateSharedCallExitEvent::FunctionExited { returned: false },
        ]);
    }

    #[test]
    fn b4_helper_math_方法內普通函式溢位先退內層後退self且零Returned() {
        let source = METHOD.replace(
            "return (8+5)-2", "return 9223372036854775807+1",
        );
        let outcome = run(&source);
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(outcome.method_entered);
        assert!(!outcome.method_exited);
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (1, 0));
        assert_eq!(outcome.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert!(!outcome.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
        let inner_exit = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan_exit = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner_exit < loan_exit);
        assert_eq!(outcome.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).count(), 2);
    }

    #[test]
    fn b4_helper_math_先初始化局部後普通函式溢位仍逆序清理已達前綴() {
        let source = METHOD.replace(
            "return (8+5)-2", "return 9223372036854775807+1",
        );
        let outcome = run(&source);
        assert_eq!(outcome.method_local_events.len(), 2);
        let id = match outcome.method_local_events[0] {
            PrivateSelfMethodLocalEvent::Initialized(id, 3) => id,
            ref other => panic!("首個已初始化局部事件錯誤：{other:?}"),
        };
        assert_eq!(
            outcome.method_local_events[1],
            PrivateSelfMethodLocalEvent::ScopeEnded(id),
        );
        let unwind = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).unwrap();
        let scope_end = outcome.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        assert!(unwind < scope_end);
        assert!(!outcome.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalInitialized(ident)
                if *ident != id
        )));
    }

    #[test]
    fn b4_helper_math_改寫原Checker加減計畫及偽造已進入計數均拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper = Some(PrivateSelfApprovedHelper {
            name: "helper".to_owned(),
            parameter: None,
            returned: PrivateScalarReturnExpr::Constant(11),
        });
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut outcome = run(METHOD);
        outcome.ordinary_calls_entered += 1;
        assert!(!outcome.has_balanced_self_borrow_trace());
        let mut outcome = run(&METHOD.replace(
            "return (8+5)-2", "return 9223372036854775807+1",
        ));
        outcome.ordinary_calls_completed = 1;
        assert!(!outcome.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_helper_math_原Checker非法型別與未核准helper呼叫不得進Runtime() {
        for source in [
            MAIN.replace("return (8+5)-2", "return true"),
            MAIN.replace("return (8+5)-2", "return missing"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "原 Checker 未核准的返回不得進入 helper 真執行：{source}");
        }
        let source = MAIN.replace("return (8+5)-2", "return helper()");
        assert!(matches!(
            PreparedPrivateSelfMethod::prepare(&source),
            Err(PrivateSelfMethodPrepareError::Check(_))
                | Err(PrivateSelfMethodPrepareError::Unsupported(_))
        ), "未經證明的遞迴 helper 不得假充無副作用常值");
    }
}
