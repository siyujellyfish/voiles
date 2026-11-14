// G18／P18.4-B4／M3：普通同步純 int helper 可作已核准
// 同步 self 方法的直接 int 實參。原 Caller 完成求參及 helper
// 真退出後才借 self；失敗不可憑空進方法或製造借用票據。
#[cfg(test)]
mod g18_b4_m3_helper_method_argument_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
return n
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(helper(9,2))
}
"#;

    fn run(src: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(src).unwrap();
        assert!(matches!(&plan.argument_plans[0], PrivateScalarReturnExpr::Call { .. }));
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(), "Copyable int 不能創造 P16 ResourceId");
        out
    }

    #[test]
    fn b4_method_arg_直接純helper先進退再借self返回7() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let inner=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited { returned:true }
        )).unwrap();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(inner<loan);
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_method_arg_兩個直接helper真求值完才取得唯一self票() {
        let src=SOURCE.replace("fn read(n:int)->int{\nreturn n",
            "fn read(a:int,b:int)->int{\nreturn a-b")
            .replace("owner.read(helper(9,2))",
                "owner.read(helper(9,2),helper(4,8))");
        let out=run(&src);
        assert_eq!(out.result,Ok(11));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(2,2));
        let exits=out.exit_events.iter().enumerate()
            .filter_map(|(i,event)|matches!(
                event,PrivateSharedCallExitEvent::FunctionExited { returned:true }
            ).then_some(i)).collect::<Vec<_>>();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(exits[0]<loan && exits[1]<loan);
    }

    #[test]
    fn b4_method_arg_具名main局部及原Copyable算術實參真交付helper() {
        let src=SOURCE.replace("var owner:Counter = Counter()",
            "var owner:Counter = Counter()\nconst seed:int = 9")
            .replace("helper(9,2)","helper(seed+2,3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn b4_method_arg_bool及int混合方法先普通helper再真選中條件葉() {
        let src=SOURCE.replace("fn read(n:int)->int{\nreturn n",
            "fn read(flag:bool,n:int)->int{\nif(flag){\nreturn self.n\n}\nreturn self.n")
            .replace("owner.read(helper(9,2))","owner.read(true,helper(9,2))");
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(out.exit_events[..loan].iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited { returned:true }
        )));
    }

    #[test]
    fn b4_method_arg_呼叫者第二純int實參溢位不進helper亦不借self() {
        let src=SOURCE.replace("helper(9,2)",
            "helper(9,9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(!out.method_entered && !out.method_exited);
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        // 尚未有任何普通函式或 self 框架真正 Enter；
        // R05 對此種純 Caller 求參失敗接受「空事件」，
        // 不能為了測試數量虛造 UnwindStarted。
        assert!(out.exit_events.is_empty());
    }

    #[test]
    fn b4_method_arg_前一helper真完成後下一實參失敗不假進第二函式() {
        let src=SOURCE.replace("fn read(n:int)->int{\nreturn n",
            "fn read(a:int,b:int)->int{\nreturn a-b")
            .replace("owner.read(helper(9,2))",
                "owner.read(helper(9,2),helper(9223372036854775807+1,2))");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.method_entered);
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_method_arg_純helper本體溢位先退失敗函式且尚未借self() {
        let src=SOURCE.replace("return a-b",
            "return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        assert!(!out.method_entered && !out.method_exited);
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited { returned:false }
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_method_arg_巢狀或非int普通函式拒絕及竄改實參計畫防偽() {
        for src in [
            SOURCE.replace("helper(9,2)","helper(helper(1,2),3)"),
            SOURCE.replace("helper(9,2)","1+helper(9,2)"),
            SOURCE.replace("helper(9,2)","helper(9)"),
            SOURCE.replace("helper(9,2)","helper(9,true)"),
            SOURCE.replace("fn helper(a:int,b:int)->int",
                "fn helper(a:int,b:bool)->int")
                .replace("return a-b","return a")
                .replace("helper(9,2)","helper(9,true)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不得接通原 Checker 未核准的函式實參：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.argument_plans[0]=PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
