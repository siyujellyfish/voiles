// G18／P18.4-B4：不另設 int helper 引數數量上限。
// 原 Checker 的多參數簽章、TypedMember／DeclarationId、Caller
// 求值順序是唯一權威；任何未完成實參都不假進普通函式框架。
#[cfg(test)]
mod g18_b4_verified_int_helper_arity_tests {
    use super::*;

    const MAIN: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n
}
}
fn aggregate(a:int,b:int,c:int,d:int)->int{
return a-b+c-d
}
fn main()->int{
var owner:Counter = Counter()
const seed:int = 10
const result:int = aggregate(seed,2,6,1)
return owner.read(result)
}
"#;

    const METHOD: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
const from_helper:int = aggregate(n+3,2,n-1,1)
return from_helper
}
}
fn aggregate(a:int,b:int,c:int,d:int)->int{
return a-b+c-d
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(5)
}
"#;

    fn run(source: &str, count: usize) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        let helper = plan.method_helper.as_ref().unwrap();
        assert_eq!(helper.parameters.len(), count);
        let unique = helper.parameters.iter().copied().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), count, "各原 Checker 形參必須保有獨立身分");
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit, out.method_local_slots_after_exit), (0, 0));
        assert!(out.events.is_empty(), "純 int 多參數不得虛構 P16 UTF-8 資源");
        out
    }

    #[test]
    fn b4_arity_四個int原Checker形參在main獨立綁定並回傳13() {
        let out = run(MAIN, 4);
        assert_eq!(out.result, Ok(13));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        let exit = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        let borrow = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(exit < borrow);
    }

    #[test]
    fn b4_arity_四個int動態方法形參在self存活期內真呼叫結果9() {
        let out = run(METHOD, 4);
        assert_eq!(out.result, Ok(9));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 1));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, 9)
        )));
    }

    #[test]
    fn b4_arity_四個int方法var_rhs與先後原子更新使用相同真框架() {
        let source = METHOD.replace(
            "const from_helper:int = aggregate(n+3,2,n-1,1)\nreturn from_helper",
            "var from_helper:int = n\nfrom_helper=aggregate(from_helper+3,2,from_helper-1,1)\nreturn from_helper",
        );
        let out = run(&source, 4);
        assert_eq!(out.result, Ok(9));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Initialized(_, 5)
        )));
        assert!(out.method_local_events.iter().any(|event| matches!(
            event, PrivateSelfMethodLocalEvent::Assigned(_, 9)
        )));
    }

    #[test]
    fn b4_arity_五個形參與兩次四參helper重入均使用唯一宣告身分() {
        let five = MAIN.replace("fn aggregate(a:int,b:int,c:int,d:int)->int",
            "fn aggregate(a:int,b:int,c:int,d:int,e:int)->int")
            .replace("return a-b+c-d", "return a-b+c-d+e")
            .replace("aggregate(seed,2,6,1)", "aggregate(seed,2,6,1,3)");
        let out = run(&five, 5);
        assert_eq!(out.result, Ok(16));
        let twice = METHOD.replace(
            "const from_helper:int = aggregate(n+3,2,n-1,1)\nreturn from_helper",
            "var from_helper:int = aggregate(n,1,2,1)\nfrom_helper=aggregate(from_helper,1,2,1)\nreturn from_helper",
        );
        let out = run(&twice, 4);
        assert_eq!(out.result, Ok(5));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (2, 2));
    }

    #[test]
    fn b4_arity_六個形參與混合正負實參保持原Parser位置序列() {
        let source = MAIN.replace("fn aggregate(a:int,b:int,c:int,d:int)->int",
            "fn aggregate(a:int,b:int,c:int,d:int,e:int,f:int)->int")
            .replace("return a-b+c-d", "return a-b+c-d+e-f")
            .replace("aggregate(seed,2,6,1)", "aggregate(seed,2,6,1,3,4)");
        let out = run(&source, 6);
        assert_eq!(out.result, Ok(12));
        let reverse = source.replace(
            "aggregate(seed,2,6,1,3,4)", "aggregate(4,3,1,6,2,seed)",
        );
        assert_eq!(run(&reverse, 6).result, Ok(-12));
    }

    #[test]
    fn b4_arity_首筆或最後一筆實參溢位都不可記錄未達helper進入() {
        for call in [
            "aggregate(9223372036854775807+1,2,n-1,1)",
            "aggregate(n+3,2,n-1,9223372036854775807+1)",
        ] {
            let source = METHOD.replace("aggregate(n+3,2,n-1,1)", call);
            let out = run(&source, 4);
            assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
            assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (0, 0));
            assert!(out.method_entered);
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
    fn b4_arity_四形參全綁定後本體溢位先退普通框架再還唯一self票() {
        let source = METHOD.replace(
            "return a-b+c-d", "return (a+b+c+d)+9223372036854775807",
        );
        let out = run(&source, 4);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered, out.ordinary_calls_completed), (1, 0));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
        let inner = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner < returned);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn b4_arity_錯誤實參數型別未知參數與篡改身分不得進Runtime() {
        for source in [
            METHOD.replace("aggregate(n+3,2,n-1,1)", "aggregate(n+3,2,n-1)"),
            METHOD.replace("fn aggregate(a:int,b:int,c:int,d:int)->int",
                "fn aggregate(a:int,b:int,c:int,d:bool)->int")
                .replace("aggregate(n+3,2,n-1,1)", "aggregate(n+3,2,n-1,true)")
                .replace("return a-b+c-d", "return a-b+c"),
            METHOD.replace("return a-b+c-d", "return a-b+c-other"),
            METHOD.replace("aggregate(n+3,2,n-1,1)",
                "aggregate(n+3,2,n-1,aggregate(1,2,3,4))"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "原 Checker 未授權型別／參數／巢狀語意不可接通：{source}");
        }
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper.as_mut().unwrap().parameters.swap(0, 3);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = PreparedPrivateSelfMethod::prepare(METHOD).unwrap();
        plan.method_helper.as_mut().unwrap().parameters.push(DeclarationId(999999));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
