// G18／P18.4-B4／M3：原 Checker 核准、有限 AST 深度的
// 純 int helper 巢狀作為同步 self 方法直接實參；真框架
// 每層先完成求參才 Enter，任何錯誤不得提早取得 self 票據。
#[cfg(test)]
mod g18_b4_m3_nested_pure_helper_method_argument_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
return n
}
}
fn add(a:int,b:int)->int{
return a+b
}
fn sub(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(add(sub(9,2),3))
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("只能由原 Checker 核准真同步方法與純 Copyable int 函式");
        assert_eq!(plan.method_helpers.len(), 2);
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "純 int 不得假造 P16 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn counts(out: &PrivateSelfMethodOutcome) -> (usize,usize) {
        (out.ordinary_calls_entered, out.ordinary_calls_completed)
    }

    #[test]
    fn b4_nested_兩個不同普通函式由內而外真完成後才借self() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(10));
        assert_eq!(counts(&out), (2,2));
        let loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),2);
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_nested_反向函式身分不可誤用另一helper的計算圖() {
        let src=SOURCE.replace("add(sub(9,2),3)","sub(add(9,2),3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!(counts(&out),(2,2));
    }

    #[test]
    fn b4_nested_三層有限原Parser工作圖逐一真進退且不重借self() {
        let src=SOURCE.replace("add(sub(9,2),3)","add(sub(add(5,2),1),3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(9));
        assert_eq!(counts(&out),(3,3));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_nested_同一父函式左右兩個巢狀實參先各自完成() {
        let src=SOURCE.replace("add(sub(9,2),3)",
            "sub(add(9,2),sub(9,5))");
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(counts(&out),(3,3));
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),3);
    }

    #[test]
    fn b4_nested_第一子函式完成但第二實參溢位父函式不得假進入() {
        let src=SOURCE.replace("add(sub(9,2),3)",
            "add(sub(9,2),9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,1));
        assert!(!out.method_entered);
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_nested_子函式本體溢位只退出真子框架不會進父函式() {
        let src=SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
            "fn sub(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,0));
        assert!(!out.method_entered);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(),1);
    }

    #[test]
    fn b4_nested_父函式本體溢位須保留子函式成功退出帳() {
        let src=SOURCE.replace("fn add(a:int,b:int)->int{\nreturn a+b",
            "fn add(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(2,1));
        assert!(!out.method_entered);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(),1);
    }

    #[test]
    fn b4_nested_含巢狀函式算術包裝資源副作用及偽造原計畫均拒絕() {
        for src in [
            SOURCE.replace("add(sub(9,2),3)","add(sub(9,2)+1,3)"),
            SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
                "fn sub(a:int,b:int)->int{\nvar s:str = \"不允許的資源\"\nreturn a-b"),
            SOURCE.replace("add(sub(9,2),3)","add(unknown(9,2),3)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "未具原 Checker／純 int 退出證據不得進入：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helpers.get_mut("sub").unwrap().returned =
            PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
