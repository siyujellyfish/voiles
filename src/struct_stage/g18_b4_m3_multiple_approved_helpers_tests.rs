// G18／P18.4-B4／P18.6-M3：單份原 Checker 函式圖中多個
// 不同純 Copyable int helper 的真名稱、形參、退出與 self 票據時序。
#[cfg(test)]
mod g18_b4_m3_multiple_approved_helpers_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(a:int,b:int)->int{
return a-b
}
}
fn plus(a:int,b:int)->int{
return a+b
}
fn minus(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(plus(9,2),minus(9,2))
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("多個普通純 int 函式必須通過原 Checker");
        assert_eq!(plan.method_helpers.len(), 2);
        assert!(plan.method_helpers.contains_key("plus"));
        assert!(plan.method_helpers.contains_key("minus"));
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned || !out.method_entered);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        assert!(out.events.is_empty(), "Copyable int 不可假造 ResourceId");
        out
    }

    fn count(out: &PrivateSelfMethodOutcome) -> (usize,usize) {
        (out.ordinary_calls_entered, out.ordinary_calls_completed)
    }

    #[test]
    fn b4_multi_兩個不同原Checker純函式依名稱分別返回並借入self() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(4));
        assert_eq!(count(&out), (2,2));
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited {returned:true}
        )).count(), 2);
    }

    #[test]
    fn b4_multi_主函式各具名局部先真執行不同helper再借self() {
        let src = SOURCE.replace(
            "return owner.read(plus(9,2),minus(9,2))",
            "const left:int = plus(9,2)\nconst right:int = minus(9,2)\nreturn owner.read(left,right)",
        );
        let out = run(&src);
        assert_eq!(out.result, Ok(4));
        assert_eq!(count(&out), (2,2));
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn b4_multi_方法真局部與末尾Return各呼叫不同helper共用歸票() {
        let src = SOURCE.replace(
            "return a-b\n}\n}\nfn plus",
            "const saved:int = plus(a,b)\nreturn minus(saved,2)\n}\n}\nfn plus",
        ).replace("owner.read(plus(9,2),minus(9,2))","owner.read(5,2)");
        let out = run(&src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(count(&out), (2,2));
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 0);
        assert_eq!(out.exit_events[loan..].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 3);
    }

    #[test]
    fn b4_multi_原Checker布林選枝各使用不同普通helper且未選零事件() {
        let src = SOURCE.replace(
            "fn read(a:int,b:int)->int{\nreturn a-b",
            "fn read(flag:bool,n:int)->int{\nif(flag){\nreturn plus(n,2)\n}\nreturn minus(n,2)",
        ).replace("owner.read(plus(9,2),minus(9,2))","owner.read(true,5)");
        let true_out = run(&src);
        assert_eq!(true_out.result, Ok(7));
        assert_eq!(count(&true_out), (1,1));
        let false_out = run(&src.replace("owner.read(true,5)","owner.read(false,5)"));
        assert_eq!(false_out.result, Ok(3));
        assert_eq!(count(&false_out), (1,1));
    }

    #[test]
    fn b4_multi_前一不同函式完成後第二求參溢位不得提前借self() {
        let src = SOURCE.replace("minus(9,2))",
            "minus(9,9223372036854775807+1))");
        let out = run(&src);
        assert_eq!(out.result,
            Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out), (1,1));
        assert!(!out.method_entered && !out.method_exited);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_multi_第二函式本體溢位不得把第一函式成功回滾掉() {
        let src = SOURCE.replace(
            "fn minus(a:int,b:int)->int{\nreturn a-b",
            "fn minus(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807",
        );
        let out = run(&src);
        assert_eq!(out.result,
            Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out), (2,1));
        assert!(!out.method_entered);
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(), 1);
    }

    #[test]
    fn b4_multi_不存在的函式與巢狀helper及非純值helper仍拒絕() {
        for src in [
            SOURCE.replace("minus(9,2)","missing(9,2)"),
            SOURCE.replace("plus(9,2)","plus(minus(9,2),2)"),
            SOURCE.replace("fn minus(a:int,b:int)->int{\nreturn a-b",
                "fn minus(a:int,b:int)->int{\nvar s:str = \"不能混入純值\"\nreturn a-b"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "沒有原 Checker 純 int 同源證據不得接受：{src}");
        }
    }

    #[test]
    fn b4_multi_竄改任一助手原DeclId或返回計畫必在真執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helpers.get_mut("minus").unwrap().parameters.swap(0,1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helpers.get_mut("plus").unwrap().returned =
            PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
