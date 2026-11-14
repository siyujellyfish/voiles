// G18／P18.4-B4：兩個原 Checker int 形參以 Parser 宣告順序
// 綁定唯一 DeclarationId，真呼叫者依原實參順序先求值；
// 任何未完成的實參不得假進函式，helper 本體故障內後外退出。
#[cfg(test)]
mod g18_b4_two_int_helper_argument_order_tests {
    use super::*;

    const MAIN: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n
}
}
fn combine(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
const seed:int = 9
const result:int = combine(seed+2,3)
return owner.read(result)
}
"#;

    const METHOD: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
const from_helper:int = combine(n+7,n-2)
return from_helper
}
}
fn combine(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(5)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert_eq!(plan.method_helper.as_ref().unwrap().parameters.len(), 2);
        assert_ne!(
            plan.method_helper.as_ref().unwrap().parameters[0],
            plan.method_helper.as_ref().unwrap().parameters[1],
        );
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit, out.method_local_slots_after_exit), (0, 0));
        assert!(out.events.is_empty(), "純整數兩參數不得偽造 UTF-8 資源");
        out
    }

    #[test]
    fn b4_two_args_主程式依實參順序求已初始化seed及整數結果8() {
        let out = run(MAIN);
        assert_eq!(out.result, Ok(8));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let completed = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(completed < loan);
    }

    #[test]
    fn b4_two_args_方法動態形參計算兩個不同實參後在self借用期間求差9() {
        let out = run(METHOD);
        assert_eq!(out.result, Ok(9));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, 9)
        )));
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_enter = out.exit_events.iter().enumerate().filter_map(|(i, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(i)
        ).nth(1).unwrap();
        assert!(loan < helper_enter);
    }

    #[test]
    fn b4_two_args_兩個不同DeclarationId作實參不可交換順序或把值當身份() {
        let source = MAIN.replace(
            "const seed:int = 9\nconst result:int = combine(seed+2,3)",
            "const first:int = 3\nconst second:int = 20\nconst result:int = combine(second,first)",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(17));
        let reverse = source.replace("combine(second,first)", "combine(first,second)");
        assert_eq!(run(&reverse).result, Ok(-17));
    }

    #[test]
    fn b4_two_args_方法var重賦以同一真局部作兩個Copyable引數原子提交() {
        let source = METHOD.replace(
            "const from_helper:int = combine(n+7,n-2)\nreturn from_helper",
            "var from_helper:int = n\nfrom_helper=combine(from_helper+4,from_helper-1)\nreturn from_helper",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(5));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, 5)
        )));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, 5)
        )));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
    }

    #[test]
    fn b4_two_args_先初始化再使用第一次返回值作下一次雙參數Helper() {
        let source = METHOD.replace(
            "const from_helper:int = combine(n+7,n-2)\nreturn from_helper",
            "var from_helper:int = combine(n+1,2)\nfrom_helper=combine(from_helper,n-1)\nreturn from_helper",
        );
        let out = run(&source);
        assert_eq!(out.result, Ok(0));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
        assert!(matches!(out.method_local_events[0],
            PrivateSelfMethodLocalEvent::Initialized(_, 4)));
        assert!(matches!(out.method_local_events[1],
            PrivateSelfMethodLocalEvent::Assigned(_, 0)));
    }

    #[test]
    fn b4_two_args_第一或第二實參溢位均不得假造已進helper框架() {
        for call in [
            "combine(9223372036854775807+1,n-2)",
            "combine(n+7,9223372036854775807+1)",
        ] {
            let source = METHOD.replace("combine(n+7,n-2)", call);
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
    }

    #[test]
    fn b4_two_args_兩參數都已綁定後返回溢位先退內層後還self票() {
        let source = METHOD.replace(
            "return a-b", "return (a+b)+9223372036854775807",
        );
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        let inner_exit = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner_exit < loan);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_two_args_三參數非法型別未知名稱與篡改雙DeclarationId必拒絕() {
        for source in [
            METHOD.replace("fn combine(a:int,b:int)->int",
                "fn combine(a:int,b:int,c:int)->int")
                .replace("combine(n+7,n-2)", "combine(n+7,n-2,1)"),
            METHOD.replace("fn combine(a:int,b:int)->int",
                "fn combine(a:int,b:bool)->int")
                .replace("return a-b", "return a"),
            METHOD.replace("return a-b", "return a-other"),
            METHOD.replace("combine(n+7,n-2)", "combine(n+7,combine(n,1))"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "不得接受未證多參／非法型別／巢狀呼叫：{source}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper.as_mut().unwrap().parameters.swap(0, 1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        let same = plan.method_helper.as_ref().unwrap().parameters[0];
        plan.method_helper.as_mut().unwrap().parameters[1] = same;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
