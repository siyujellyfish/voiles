// G18／P18.4-B4／P18.6-M3：原 Checker 直接同步 self
// int 實參的純算術可包含已驗證普通函式返回；只有真
// 到達 Call 才進退出框架，全部完成後方可取得 self 票據。
#[cfg(test)]
mod g18_b4_m3_method_argument_int_call_arithmetic_tests {
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
return owner.read(1+sub(9,2))
}
"#;

    fn run(source:&str)->PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("必須由原 Lexer／Parser／Checker 核准純 int 算術及原函式");
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "純 int 不可虛構 P16 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn count(out:&PrivateSelfMethodOutcome)->(usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn b4_math_call_左常值加原Checker普通函式結果8且函式先退self後借() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(8));
        assert_eq!(count(&out),(1,1));
        let finished=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(finished<loan);
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_math_call_左右算術順序互換仍各自從同一真frame求值() {
        let src=SOURCE.replace("1+sub(9,2)","sub(9,2)+1");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!(count(&out),(1,1));
    }

    #[test]
    fn b4_math_call_兩個同源不同函式可作左右算術運算元() {
        let src=SOURCE.replace("1+sub(9,2)","add(8,2)-sub(9,4)");
        let out=run(&src);
        assert_eq!(out.result,Ok(5));
        assert_eq!(count(&out),(2,2));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),2);
    }

    #[test]
    fn b4_math_call_多層巢狀helper結果再算術亦保留真進退順序() {
        let src=SOURCE.replace("1+sub(9,2)","1+add(sub(9,2),3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(11));
        assert_eq!(count(&out),(2,2));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_math_call_先完成左函式後右算術溢位不得提前進方法() {
        let src=SOURCE.replace("1+sub(9,2)",
            "sub(9,2)+(9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,1));
        assert!(!out.method_entered && !out.method_exited);
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_math_call_左算術提前溢位禁止右函式虛構Enter() {
        let src=SOURCE.replace("1+sub(9,2)",
            "(9223372036854775807+1)+sub(9,2)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(0,0));
        assert!(!out.method_entered);
    }

    #[test]
    fn b4_math_call_普通函式本體溢位先退出該函式不得借self() {
        let src=SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
            "fn sub(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,0));
        assert!(!out.method_entered);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(),1);
    }

    #[test]
    fn b4_math_call_非法呼叫型別資源或竄改int算式來源必拒絕() {
        for src in [
            SOURCE.replace("sub(9,2)","sub(9,true)"),
            SOURCE.replace("1+sub(9,2)","1+missing(9,2)"),
            SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
                "fn sub(a:int,b:int)->int{\nvar owned:str = \"不允許資源\"\nreturn a-b"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "未經原 Checker 直接函式與 Copyable 算式核准：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.argument_plans[0]=PrivateScalarReturnExpr::Constant(8);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
