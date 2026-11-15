// G18／P18.6-M2／P18.4-B4：原 Checker 已驗證的 if/else
// 選中葉純 int 算術寫入 self，未選葉不得偷求值或假寫欄位。
// 唯一 BorrowTicket 仍須 ExclusiveWrite，失敗原 R05 真退票。
#[cfg(test)]
mod g18_m2_conditional_checked_int_self_write_tests {
    use super::*;

    const TRUE_BRANCH:&str=r#"struct Counter{
var n:int = 7
fn read(flag:bool,payload:int)->int{
if(flag){
self.n = payload+3
return self.n
}else{
return 22
}
return self.n+1
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true,19)
}
"#;

    const NESTED:&str=r#"struct Counter{
var n:int = 7
fn read(outer:bool,inner:bool,left:int,right:int)->int{
if(outer){
if(inner){
self.n = left+2
return self.n+5
}else{
self.n = right-2
return self.n-30
}
}
return self.n+1
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true,true,19,23)
}
"#;

    fn run(source:&str)->PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("只能使用原 Checker 真選葉、獨占 self BorrowFact 及 int ScalarHIR");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        assert!(!plan.method_branch_int_plans.is_empty());
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"Copyable int 不能假造 P16 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn m2_branch_arith_原if_true取真int形參加3後獨占寫入22() {
        let out=run(TRUE_BRANCH);
        assert_eq!(out.result,Ok(22));
        assert_eq!(out.branch_checked,Some(true));
        assert!(out.branch_write_applied);
    }

    #[test]
    fn m2_branch_arith_原if_false未選中高風險算式零動態寫入() {
        let src=TRUE_BRANCH.replace("payload+3","payload+9223372036854775807")
            .replace("owner.read(true,19)","owner.read(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Ok(22));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied && !out.branch_else_write_applied);
    }

    #[test]
    fn m2_branch_arith_選中true後算術溢位不提交寫入且真還票() {
        let src=TRUE_BRANCH.replace("payload+3","payload+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.branch_checked,Some(true));
        assert!(!out.branch_write_applied);
        assert!(out.method_entered && !out.method_exited);
        assert!(out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_branch_arith_選中else後才求右運算式溢位不影響true合法值() {
        let src=TRUE_BRANCH.replace(
            "self.n = payload+3\nreturn self.n\n}else{\nreturn 22",
            "return self.n\n}else{\nself.n = payload+9223372036854775807\nreturn self.n",
        ).replace("owner.read(true,19)","owner.read(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied && !out.branch_else_write_applied);
        assert!(out.ticket_returned);
    }

    #[test]
    fn m2_branch_arith_true與else各用原Checker不同int參數純加減() {
        let src=TRUE_BRANCH.replace("fn read(flag:bool,payload:int)->int",
            "fn read(flag:bool,left:int,right:int)->int")
            .replace("self.n = payload+3","self.n = left+3")
            .replace("return 22\n}\nreturn self.n+1",
                "self.n = right-4\nreturn self.n\n}\nreturn self.n+1")
            .replace("owner.read(true,19)","owner.read(true,19,23)");
        assert_eq!(run(&src).result,Ok(22));
        let out=run(&src.replace("owner.read(true,19,23)",
            "owner.read(false,19,23)"));
        assert_eq!(out.result,Ok(19));
        assert!(!out.branch_write_applied && out.branch_else_write_applied);
    }

    #[test]
    fn m2_branch_arith_巢狀原bool雙葉各執行獨立算術且唯一退票() {
        let yes=run(NESTED);
        assert_eq!(yes.result,Ok(26));
        assert_eq!(yes.nested_branch_checked,Some(true));
        assert!(yes.nested_write_applied && !yes.nested_else_write_applied);
        let no=run(&NESTED.replace("owner.read(true,true,19,23)",
            "owner.read(true,false,19,23)"));
        assert_eq!(no.result,Ok(-9));
        assert_eq!(no.nested_branch_checked,Some(false));
        assert!(no.nested_else_write_applied && !no.nested_write_applied);
    }

    #[test]
    fn m2_branch_arith_外層false不可預先求巢狀已核准溢位算式() {
        let src=NESTED.replace("left+2","left+9223372036854775807")
            .replace("right-2","right+9223372036854775807")
            .replace("owner.read(true,true,19,23)","owner.read(false,true,19,23)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!(out.nested_branch_checked,None);
        assert!(!out.nested_write_applied && !out.nested_else_write_applied);
    }

    #[test]
    fn m2_branch_arith_未授權型別const來源與變造原算式Span均拒絕() {
        for src in [
            TRUE_BRANCH.replace("self.n = payload+3","self.n = flag+3"),
            TRUE_BRANCH.replace("self.n = payload+3","self.n = missing+3"),
            TRUE_BRANCH.replace("var owner:Counter","const owner:Counter"),
            TRUE_BRANCH.replace("self.n = payload+3","self.n = self.n+3"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "未經原 Checker int/ExclusiveWrite 認證不得接通：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(TRUE_BRANCH).unwrap();
        let (kind,_) = plan.method_branch_write.unwrap();
        let PrivateSelfApprovedBranchWrite::CheckedIntAt(span)=kind else {
            panic!("原 Checker 應記錄選葉算術 RHS 真 NodeSpan");
        };
        assert_eq!(plan.method_branch_int_plans.len(),1);
        plan.method_branch_int_plans.insert(span,PrivateScalarReturnExpr::Constant(22));
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
