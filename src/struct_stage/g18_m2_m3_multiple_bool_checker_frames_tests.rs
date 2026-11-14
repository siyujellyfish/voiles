// G18／P18.6-M2、M3：兩個以上原 Checker bool 形參按
// Lexer／Parser 的位置與唯一 DeclarationId 分別控制巢狀真葉。
#[cfg(test)]
mod g18_m2_m3_multiple_bool_checker_frames_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(gate:bool,branch:bool,step:int)->int{
if(gate){
if(branch){
return helper(step+3,2)
}else{
return helper(step+2,5)
}
}
return self.n
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true,true,5)
}
"#;

    fn run(src: &str, bool_count: usize) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(src).unwrap();
        assert_eq!(plan.method_bool_arguments.len(), bool_count);
        let unique = plan.method_bool_arguments.iter().map(|(id,_)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), bool_count, "bool 必須各自擁有原 Checker DeclId");
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace() && out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(), "Copyable bool/int 不得假造 P16 ResourceId");
        out
    }

    #[test]
    fn m2_bool_兩個獨立真bool都true只選中內層true_helper返回6() {
        let out=run(SOURCE,2);
        assert_eq!(out.result,Ok(6));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(true),Some(true)));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),2);
    }

    #[test]
    fn m2_bool_外層true但第二個bool_false只選中else_helper返回2() {
        let src=SOURCE.replace("owner.read(true,true,5)","owner.read(true,false,5)");
        let out=run(&src,2);
        assert_eq!(out.result,Ok(2));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(true),Some(false)));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn m2_bool_外層false不論內層真假都不進入任何普通helper() {
        for call in ["owner.read(false,true,5)","owner.read(false,false,5)"] {
            let out=run(&SOURCE.replace("owner.read(true,true,5)",call),2);
            assert_eq!(out.result,Ok(7));
            assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(false),None));
            assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        }
    }

    #[test]
    fn m2_bool_多個bool與int倒置形參及第三個獨立bool均保持DeclId() {
        let reversed=SOURCE.replace(
            "fn read(gate:bool,branch:bool,step:int)->int",
            "fn read(step:int,branch:bool,gate:bool)->int",
        ).replace("owner.read(true,true,5)","owner.read(5,false,true)");
        let out=run(&reversed,2);
        assert_eq!(out.result,Ok(2));
        let third=SOURCE.replace(
            "fn read(gate:bool,branch:bool,step:int)->int",
            "fn read(gate:bool,branch:bool,extra:bool,step:int)->int",
        ).replace("owner.read(true,true,5)","owner.read(true,false,true,5)");
        let out=run(&third,3);
        assert_eq!(out.result,Ok(2));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn m2_bool_兩個不同bool選中巢狀獨占欄位更新仍只借還唯一票() {
        let src=SOURCE.replace(
            "return helper(step+3,2)",
            "self.n=19\nreturn helper(step+3,2)",
        ).replace(
            "return helper(step+2,5)",
            "self.n=23\nreturn helper(step+2,5)",
        ).replace("owner.read(true,true,5)","owner.read(true,false,5)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        let out=run(&src,2);
        assert_eq!(out.result,Ok(2));
        assert!(!out.nested_write_applied && out.nested_else_write_applied);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn m2_bool_外層false時內層引數及函式本體溢位均不得提前執行() {
        let src=SOURCE.replace(
            "return helper(step+3,2)",
            "return helper(step+3,9223372036854775807+1)",
        ).replace(
            "return a-b", "return (a+b)+9223372036854775807",
        ).replace("owner.read(true,true,5)","owner.read(false,true,5)");
        let out=run(&src,2);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_bool_真選中內層引數故障零Enter而本體故障內先外後() {
        let arg=SOURCE.replace(
            "return helper(step+3,2)",
            "return helper(step+3,9223372036854775807+1)",
        );
        let out=run(&arg,2);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),1);

        let body=SOURCE.replace("return a-b",
            "return (a+b)+9223372036854775807");
        let out=run(&body,2);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        let failed=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:false }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(failed<loan);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn m2_bool_原Checker錯型別缺bool及篡改多個bool形參身分一律拒絕() {
        for src in [
            SOURCE.replace("owner.read(true,true,5)","owner.read(true,5,true)"),
            SOURCE.replace("owner.read(true,true,5)","owner.read(true,5)"),
            SOURCE.replace("if(branch){","if(unbound){"),
            SOURCE.replace("fn read(gate:bool,branch:bool,step:int)",
                "fn read(gate:bool,branch:int,step:int)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                |Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "非法 bool/int 實參／未宣告條件不可繞過 Checker：{src}");
        }
        let mut swapped=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        swapped.method_bool_arguments.swap(0,1);
        assert!(matches!(swapped.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut erased=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        erased.method_bool_arguments.pop();
        assert!(matches!(erased.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
