// G18／P18.4-B4：原 Checker 已核准單一 int 形參普通函式
// 進入 main／同步 self 方法真框架；呼叫者先求參數、失敗不得
// 偽造普通函式進入、成功時仍精確逐框架 Returned／LoanReturned。
#[cfg(test)]
mod g18_b4_single_int_helper_parameter_tests {
    use super::*;

    const MAIN: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n+1
}
}
fn helper(step:int)->int{
return step+3
}
fn main()->int{
var owner:Counter = Counter()
const seed:int = 4
const from_helper:int = helper(seed+2)
return owner.read(from_helper)
}
"#;

    const METHOD: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
const from_helper:int = helper(n+1)
return from_helper+2
}
}
fn helper(step:int)->int{
return step+3
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(4)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit, out.method_local_slots_after_exit), (0, 0));
        assert!(out.events.is_empty(), "純整數實參不能產生 OwnedUtf8 ResourceId");
        out
    }

    #[test]
    fn b4_arg_主程式已初始化具名int加減求參後真進helper返回10() {
        let out = run(MAIN);
        assert_eq!(out.result, Ok(10));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let first_exit = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(first_exit < loan, "main helper 必須先返回才借 self");
    }

    #[test]
    fn b4_arg_方法int形參加一先求值再呼叫helper且真歸票() {
        let out = run(METHOD);
        assert_eq!(out.result, Ok(10));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_enter = out.exit_events.iter().enumerate().filter_map(|(i, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        let loan_returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(loan < helper_enter && helper_enter < loan_returned);
    }

    #[test]
    fn b4_arg_方法var重賦使用目前真局部作helper實參並原子提交() {
        let source = METHOD.replace(
            "const from_helper:int = helper(n+1)\nreturn from_helper+2",
            "var from_helper:int = n\nfrom_helper=helper(from_helper+2)\nreturn from_helper",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(9));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, 4)
        )));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, 9)
        )));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
    }

    #[test]
    fn b4_arg_同一helper第一次初始化第二次重賦使用第一次返回槽() {
        let source = METHOD.replace(
            "const from_helper:int = helper(n+1)\nreturn from_helper+2",
            "var from_helper:int = helper(n)\nfrom_helper=helper(from_helper)\nreturn from_helper",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(10));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 7)));
        assert!(matches!(out.method_local_events[1],
            PrivateSelfMethodLocalEvent::Assigned(_, 10)));
    }

    #[test]
    fn b4_arg_main實參先溢位沒有任何helper進入或self借用() {
        let source = MAIN.replace("helper(seed+2)",
            "helper(9223372036854775807+1)");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert!(out.exit_events.is_empty(), "實參尚未完成，helper 不可假進入");
    }

    #[test]
    fn b4_arg_方法實參先溢位只得方法退出不應假造內層helper() {
        let source = METHOD.replace("helper(n+1)",
            "helper(n+9223372036854775807)");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered);
        assert!(!out.method_exited);
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        assert!(!out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, _)
        )));
    }

    #[test]
    fn b4_arg_單形參成功綁定後helper本體溢位先退出子框架再還票() {
        let source = METHOD.replace("return step+3",
            "return step+9223372036854775807")
            .replace("helper(n+1)", "helper(n)");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        let first_failed = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan_returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(first_failed < loan_returned);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_arg_原Checker拒絕多形參非法型別及篡改Helper身分() {
        for source in [
            METHOD.replace("fn helper(step:int)->int", "fn helper(step:int,more:int)->int")
                .replace("helper(n+1)", "helper(n+1,2)"),
            METHOD.replace("fn helper(step:int)->int", "fn helper(step:bool)->int"),
            METHOD.replace("return step+3", "return other+3"),
            METHOD.replace("helper(n+1)", "helper(1+helper(n))"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "不得繞過原 Checker 的 int 參數身分／副作用限制：{source}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper.as_mut().unwrap().parameter = Some(DeclarationId(999999));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper.as_mut().unwrap().returned =
            PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
