// G18／P18.6-M2／P18.4-B4：原 Checker 真 if／else 選中葉的
// 同步純 int helper 進退後才能獨占寫 self 欄位。
// 只有真選中 RHS 產生普通函式框架，溢位須先 Unwind 再歸還 self 票。
#[cfg(test)]
mod g18_m2_selected_helper_self_write_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn write(flag:bool,payload:int)->int{
if(flag){
self.n = add(payload,3)
return self.n
}else{
return 22
}
return self.n+1
}
}
fn add(a:int,b:int)->int{
return a+b
}
fn main()->int{
var owner:Counter = Counter()
return owner.write(true,19)
}
"#;

    const NESTED: &str = r#"struct Counter{
var n:int = 7
fn write(outer:bool,inner:bool,left:int,right:int)->int{
if(outer){
if(inner){
self.n = add(left,2)
return self.n+5
}else{
self.n = sub(right,2)
return self.n-30
}
}
return self.n+1
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
return owner.write(true,true,19,23)
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("必須是原 Lexer／Parser／Checker 已核准的同步 self／純 int helper");
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert!(!plan.method_branch_int_plans.is_empty());
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = checked(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"純 Copyable int 不得偽造 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn count(out: &PrivateSelfMethodOutcome) -> (usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn m2_helper_write_原if_true在self獨占借票後執行add並真寫22() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(22));
        assert_eq!(out.branch_checked,Some(true));
        assert!(out.branch_write_applied);
        assert_eq!(count(&out),(1,1));
        let acquire=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_enter=out.exit_events.iter().enumerate()
            .filter_map(|(i,event)|matches!(
                event,PrivateSharedCallExitEvent::FunctionEntered
            ).then_some(i)).nth(1).unwrap();
        let loan_return=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(acquire<helper_enter && helper_enter<loan_return);
    }

    #[test]
    fn m2_helper_write_原if_false即便helper本體會溢位也不得進入() {
        let src=SOURCE.replace("return a+b","return (a+b)+9223372036854775807")
            .replace("owner.write(true,19)","owner.write(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Ok(22));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied);
        assert_eq!(count(&out),(0,0));
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_helper_write_真helper本體溢位先退子框架再歸self票() {
        let src=SOURCE.replace("return a+b","return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,0));
        assert!(out.method_entered && !out.method_exited);
        assert!(!out.branch_write_applied);
        let helper_exit=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).unwrap();
        let loan_return=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(helper_exit<loan_return);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn m2_helper_write_真else選葉才啟動add並寫入24() {
        let src=SOURCE.replace(
            "self.n = add(payload,3)\nreturn self.n\n}else{\nreturn 22",
            "return self.n\n}else{\nself.n = add(payload,5)\nreturn self.n",
        ).replace("owner.write(true,19)","owner.write(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Ok(24));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied && out.branch_else_write_applied);
        assert_eq!(count(&out),(1,1));
    }

    #[test]
    fn m2_helper_write_未選true仍執行else時僅進一份helper() {
        let src=SOURCE.replace(
            "return 22\n}\nreturn self.n+1",
            "self.n = add(payload,7)\nreturn self.n\n}\nreturn self.n+1",
        ).replace("owner.write(true,19)","owner.write(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Ok(26));
        assert_eq!(count(&out),(1,1));
        assert!(out.branch_else_write_applied);
    }

    #[test]
    fn m2_helper_write_巢狀true及else分別唯一選葉且用不同helper() {
        let yes=run(NESTED);
        assert_eq!(yes.result,Ok(26));
        assert_eq!(count(&yes),(1,1));
        assert!(yes.nested_write_applied && !yes.nested_else_write_applied);
        let no=run(&NESTED.replace(
            "owner.write(true,true,19,23)","owner.write(true,false,19,23)",
        ));
        assert_eq!(no.result,Ok(-9));
        assert_eq!(count(&no),(1,1));
        assert!(!no.nested_write_applied && no.nested_else_write_applied);
    }

    #[test]
    fn m2_helper_write_外層false時兩個巢狀helper都不得提前執行() {
        let src=NESTED.replace("return a+b","return (a+b)+9223372036854775807")
            .replace("return a-b","return (a-b)+9223372036854775807")
            .replace("owner.write(true,true,19,23)",
                "owner.write(false,true,19,23)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!(out.nested_branch_checked,None);
        assert_eq!(count(&out),(0,0));
        assert!(!out.nested_write_applied && !out.nested_else_write_applied);
    }

    #[test]
    fn m2_helper_write_原BinaryInt左端helper先真返回再加常值寫24() {
        let src=SOURCE.replace("add(payload,3)","add(payload,3)+2");
        let out=run(&src);
        assert_eq!(out.result,Ok(24));
        assert_eq!(count(&out),(1,1));
        assert!(out.branch_write_applied);
    }

    #[test]
    fn m2_helper_write_兩層已核准helper依實參次序進退後寫入20() {
        let src=SOURCE.replace("self.n = add(payload,3)",
            "self.n = add(sub(payload,2),3)")
            .replace("fn main()->int{",
                "fn sub(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{");
        let out=run(&src);
        assert_eq!(out.result,Ok(20));
        assert_eq!(count(&out),(2,2));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn m2_helper_write_非法型別未知函式與篡改封存ScalarHIR必先拒絕() {
        for src in [
            SOURCE.replace("add(payload,3)","add(flag,3)"),
            SOURCE.replace("add(payload,3)","missing(payload,3)"),
            SOURCE.replace("fn add(a:int,b:int)->int{\nreturn a+b",
                "fn add(a:int,b:int)->int{\nvar s:str = \"不准資源\"\nreturn a+b"),
            SOURCE.replace("var owner:Counter","const owner:Counter"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "原 Checker 未授權的欄位 RHS 不可執行：{src}");
        }
        let mut plan=checked(SOURCE);
        let (kind,_) = plan.method_branch_write.unwrap();
        let PrivateSelfApprovedBranchWrite::CheckedIntAt(span)=kind else {
            panic!("原選葉應封存真 Checker RHS Span");
        };
        plan.method_branch_int_plans.insert(span,
            PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(SOURCE);
        plan.method_helpers.get_mut("add").unwrap().returned =
            PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
