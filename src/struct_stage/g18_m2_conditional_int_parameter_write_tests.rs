// G18／P18.6-M2／P18.4-B4：已核准 Copyable int 方法形參
// 在**原 Checker 真選中 if／else 葉**持唯一 ExclusiveWrite
// self BorrowTicket 後寫 MixedStructRuntime；未選葉零寫入。
#[cfg(test)]
mod g18_m2_conditional_int_parameter_write_tests {
    use super::*;

    const TRUE_BRANCH: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool,payload:int)->int{
if(flag){
self.n = payload
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

    const NESTED_BRANCH: &str = r#"struct Counter{
var n:int = 7
fn read(outer:bool,inner:bool,left:int,right:int)->int{
if(outer){
if(inner){
self.n = left
return self.n+5
}else{
self.n = right
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
            .expect("必須原 Checker 核准選葉寫入、方法形參與 ExclusiveWrite");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"int 欄位不許假造 P16 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn m2_branch_param_原Checker_if_true動態int形參寫19後同欄位返回() {
        let plan=PreparedPrivateSelfMethod::prepare(TRUE_BRANCH).unwrap();
        let actual=plan.method_branch_write.unwrap().0;
        assert_eq!(actual, PrivateSelfApprovedBranchWrite::Parameter(
            plan.argument_binding.unwrap().0
        ));
        let out=run(TRUE_BRANCH);
        assert_eq!(out.result,Ok(19));
        assert_eq!(out.branch_checked,Some(true));
        assert!(out.branch_write_applied && !out.branch_else_write_applied);
    }

    #[test]
    fn m2_branch_param_原Checker_false未選中真寫入必返回原else常值22() {
        let src=TRUE_BRANCH.replace("owner.read(true,19)","owner.read(false,19)");
        let out=run(&src);
        assert_eq!(out.result,Ok(22));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied);
    }

    #[test]
    fn m2_branch_param_只在else真正選中才以方法形參寫23() {
        let src=TRUE_BRANCH.replace(
            "self.n = payload\nreturn self.n\n}else{\nreturn 22",
            "return self.n\n}else{\nself.n = payload\nreturn self.n"
        ).replace("owner.read(true,19)","owner.read(false,23)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert!(matches!(
            plan.method_branch_else_write,
            Some((PrivateSelfApprovedBranchWrite::Parameter(_),_))
        ));
        let out=run(&src);
        assert_eq!(out.result,Ok(23));
        assert!(!out.branch_write_applied && out.branch_else_write_applied);
        assert_eq!(out.branch_checked,Some(false));
    }

    #[test]
    fn m2_branch_param_true與else用不同DeclId分別寫值且另一葉零寫入() {
        let src=TRUE_BRANCH.replace(
            "fn read(flag:bool,payload:int)->int",
            "fn read(flag:bool,left:int,right:int)->int"
        ).replace("self.n = payload", "self.n = left")
            .replace("return 22\n}\nreturn self.n+1",
                "self.n = right\nreturn self.n\n}\nreturn self.n+1")
            .replace("owner.read(true,19)", "owner.read(true,19,23)");
        let yes=run(&src);
        assert_eq!(yes.result,Ok(19));
        assert!(yes.branch_write_applied && !yes.branch_else_write_applied);
        let no=run(&src.replace("owner.read(true,19,23)",
            "owner.read(false,19,23)"));
        assert_eq!(no.result,Ok(23));
        assert!(!no.branch_write_applied && no.branch_else_write_applied);
    }

    #[test]
    fn m2_branch_param_真巢狀if兩葉不同方法形參均依原bool選中() {
        let yes=run(NESTED_BRANCH);
        assert_eq!(yes.result,Ok(24));
        assert_eq!(yes.branch_checked,Some(true));
        assert_eq!(yes.nested_branch_checked,Some(true));
        assert!(yes.nested_write_applied && !yes.nested_else_write_applied);
        let no=run(&NESTED_BRANCH.replace(
            "owner.read(true,true,19,23)", "owner.read(true,false,19,23)"));
        assert_eq!(no.result,Ok(-7));
        assert_eq!(no.nested_branch_checked,Some(false));
        assert!(!no.nested_write_applied && no.nested_else_write_applied);
    }

    #[test]
    fn m2_branch_param_外層false完全不訪問巢狀形參寫入且main局部可傳值() {
        let src=NESTED_BRANCH.replace(
            "return owner.read(true,true,19,23)",
            "const amount:int = 19\nreturn owner.read(false,true,amount,23)");
        let out=run(&src);
        assert_eq!(out.result,Ok(8));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!(out.nested_branch_checked,None);
        assert!(!out.nested_write_applied && !out.nested_else_write_applied);
    }

    #[test]
    fn m2_branch_param_方法入口注入故障不得提前選葉或寫入且真退獨占票() {
        let plan=PreparedPrivateSelfMethod::prepare(NESTED_BRANCH).unwrap();
        let out=plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked,None);
        assert_eq!(out.nested_branch_checked,None);
        assert!(!out.nested_write_applied && !out.nested_else_write_applied);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(plan.execute().unwrap().result,Ok(24));
    }

    #[test]
    fn m2_branch_param_未授權bool欄位別名const與變造任一分支DeclId須拒絕() {
        for source in [
            TRUE_BRANCH.replace("self.n = payload","self.n = flag"),
            TRUE_BRANCH.replace("self.n = payload","self.n = missing"),
            TRUE_BRANCH.replace("var owner:Counter","const owner:Counter"),
            NESTED_BRANCH.replace("self.n = left","self.missing = left"),
            NESTED_BRANCH.replace("self.n = right","self.n = true"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ),"未經原 Checker 借用安全核准的選葉欄位操作不得接通：{source}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(TRUE_BRANCH).unwrap();
        let (_,span)=plan.method_branch_write.unwrap();
        plan.method_branch_write=Some((
            PrivateSelfApprovedBranchWrite::Parameter(DeclarationId(0)),span
        ));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(NESTED_BRANCH).unwrap();
        let (_,span)=plan.method_nested_else_write.unwrap();
        plan.method_nested_else_write=Some((
            PrivateSelfApprovedBranchWrite::Literal(999),span
        ));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
