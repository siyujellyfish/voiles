// G18／P18.4-B4／M3：原方法 int 形參在無局部／if Return 的
// 純 helper 引數中須綁到 Checker 真 DeclarationId，不可使用準備時假值。
#[cfg(test)]
mod g18_b4_m3_dynamic_int_return_helper_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
return helper(n+3,2)
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(5)
}
"#;

    fn run(src: &str) -> PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(src).unwrap();
        assert!(plan.method_int_locals.is_empty());
        assert!(matches!(&plan.return_calculation,
            PrivateSelfIntReturn::CheckedScalar(PrivateScalarReturnExpr::Call { .. })
        ) || matches!(&plan.method_branch_return,
            Some(PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Call { .. }
            ))));
        let result=plan.execute().unwrap();
        assert!(result.has_balanced_self_borrow_trace() && result.ticket_returned);
        assert_eq!((result.main_local_slots_after_exit,result.method_local_slots_after_exit),(0,0));
        assert!(result.events.is_empty(),"純 int 不應建立假 OwnedUtf8 ResourceId");
        result
    }

    #[test]
    fn b4_param_單int形參在末尾Return當下真求helper結果6() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(6));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let inner=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:true }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner<loan);
    }

    #[test]
    fn b4_param_同來源兩次不同main實參改變方法真返回結果() {
        let first=run(SOURCE);
        let second=run(&SOURCE.replace("owner.read(5)","owner.read(20)"));
        assert_eq!(first.result,Ok(6));
        assert_eq!(second.result,Ok(21));
        assert_eq!((first.ordinary_calls_entered,second.ordinary_calls_entered),(1,1));
    }

    #[test]
    fn b4_param_兩個不同方法int形參綁定原DeclId及其宣告順序() {
        let src=SOURCE.replace("fn read(n:int)->int","fn read(a:int,b:int)->int")
            .replace("helper(n+3,2)","helper(a+3,b-1)")
            .replace("owner.read(5)","owner.read(5,2)");
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let reverse=src.replace("owner.read(5,2)","owner.read(2,5)");
        assert_eq!(run(&reverse).result,Ok(1));
    }

    #[test]
    fn b4_param_方法形參組括號與左右複合純int算術保留同一真HIR() {
        let src=SOURCE.replace("helper(n+3,2)","helper((n+3)-1,2)");
        let out=run(&src);
        assert_eq!(out.result,Ok(5));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn b4_param_if_true選中提前Return以目前int形參動態進helper() {
        let src=SOURCE.replace("return helper(n+3,2)",
            "if(true){\nreturn helper(n+3,2)\n}\nreturn self.n");
        let out=run(&src);
        assert_eq!(out.result,Ok(6));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let false_src=src.replace("if(true){","if(false){");
        let false_plan=PreparedPrivateSelfMethod::prepare(&false_src).unwrap();
        let false_out=false_plan.execute().unwrap();
        assert!(false_out.ticket_returned && false_out.has_balanced_self_borrow_trace());
        assert_eq!(false_out.result,Ok(7));
        assert_eq!((false_out.ordinary_calls_entered,false_out.ordinary_calls_completed),(0,0));
    }

    #[test]
    fn b4_param_已綁方法真形參後在helper返回算術溢位內後外退出() {
        let src=SOURCE.replace("return a-b",
            "return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        let inner=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:false }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner<loan);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_param_方法真形參本身加法溢位在進helper前回滾票() {
        let src=SOURCE.replace("owner.read(5)",
            "owner.read(9223372036854775807)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn b4_param_原Checker禁止錯型別未知方法形參巢狀Call與竄改原身份() {
        for src in [
            SOURCE.replace("fn read(n:int)->int","fn read(n:bool)->int")
                .replace("owner.read(5)","owner.read(true)"),
            SOURCE.replace("helper(n+3,2)","helper(m+3,2)"),
            SOURCE.replace("helper(n+3,2)","helper(n+3,helper(n,1))"),
            SOURCE.replace("return helper(n+3,2)","return 1+helper(n+3,2)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_)) |
                Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不得接受原 Checker 未證的來源：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helper.as_mut().unwrap().parameters.swap(0,1);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
