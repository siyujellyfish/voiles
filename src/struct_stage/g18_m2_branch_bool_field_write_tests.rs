// G18／P18.4-B4／P18.6-M2/M3：原Checker真if true選葉中的
// var self.bool 獨立常值寫入與普通函式共用MethodSafety／HC14 CFG。
#[cfg(test)]
mod g18_m2_branch_bool_field_write_tests {
    use super::*;

    const SOURCE:&str=r#"struct Counter{
var n:int = 7
var ready:bool = true
fn change(gate:bool)->int{
if(gate){
self.ready = false
return self.n
}
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.change(true)
}
"#;

    fn source(fields:&str, condition:&str, assignment:&str,
        main_steps:&str, call:&str) -> String {
        SOURCE.replace("var n:int = 7\nvar ready:bool = true\n",fields)
            .replace("if(gate){",&format!("if({condition}){{"))
            .replace("self.ready = false",assignment)
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{main_steps}"))
            .replace("return owner.change(true)",&format!("return {call}"))
    }

    fn checked(src:&str)->PreparedPrivateSelfMethod{
        verify_source_with_ability(
            src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker已核准選葉self.bool與唯一獨占self借票");
        let plan=PreparedPrivateSelfMethod::prepare(src)
            .expect("原Checker合法bool真選葉不能被私有Executor錯拒");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        plan
    }

    fn run(src:&str,expected:Option<(String,bool,bool)>,condition:bool)
        ->PrivateSelfMethodOutcome{
        let plan=checked(src);
        let out=plan.execute().expect("真正同一Checked self CFG需有P16結果");
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.branch_checked,Some(condition));
        assert_eq!(out.branch_bool_write_observed,expected);
        assert_eq!(out.branch_write_applied,condition);
        assert!(out.method_entered&&out.method_exited);
        assert!(out.ticket_returned&&out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable bool不得偽造OwnedStr ResourceId");
        assert_eq!((out.main_local_slots_after_exit,
            out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|ev|matches!(
            ev,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1,"分支不能額外簽發借票");
        out
    }

    #[test]
    fn m2_branch_bool_同名方法只可使用持有者型別() {
        let other = SOURCE.split("fn main").next().unwrap()
            .replace("Counter", "Another").replace("= 7", "= 91");
        let src = format!("{other}{SOURCE}");
        run(&src, Some(("ready".to_owned(), true, false)), true);
        let switched = src.replace("owner:Counter = Counter()", "owner:Another = Another()");
        let out = checked(&switched).execute().unwrap();
        assert_eq!(out.result, Ok(91));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(out.branch_bool_write_observed, Some(("ready".to_owned(), true, false)));
        let invalid = src.replace("owner:Counter = Counter()", "owner:Counter = Another()");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&invalid),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn m2_branch_bool_原Checker_true真選中才原子寫false() {
        run(SOURCE,Some(("ready".to_owned(),true,false)),true);
    }

    #[test]
    fn m2_branch_bool_原Checker_false分支不寫即使靜態必須獨占票() {
        let src=SOURCE.replace("owner.change(true)","owner.change(false)");
        run(&src,None,false);
    }

    #[test]
    fn m2_branch_bool_寫true至原false欄位必反映真P16前後值() {
        let src=source("var n:int = 7\nvar ready:bool = false\n",
            "gate","self.ready = true","","owner.change(true)");
        run(&src,Some(("ready".to_owned(),false,true)),true);
    }

    #[test]
    fn m2_branch_bool_原Lexer_Group包裹bool常值仍使用原Checker真來源() {
        let src=source("var n:int = 7\nvar ready:bool = true\n",
            "(((gate)))","self.ready = (((false)))","","owner.change(true)");
        run(&src,Some(("ready".to_owned(),true,false)),true);
    }

    #[test]
    fn m2_branch_bool_原Checker_self_bool真條件與分支寫入共用唯一獨占票() {
        let src=source("var n:int = 7\nvar ready:bool = true\n",
            "self.ready","self.ready = false","","owner.change(false)");
        let p=checked(&src);
        assert_eq!(p.method_branch_self_bool_field,Some("ready".to_owned()));
        run(&src,Some(("ready".to_owned(),true,false)),true);
    }

    #[test]
    fn m2_branch_bool_原Checker_self_bool假條件不許誤執行其寫入() {
        let src=source("var n:int = 7\nvar ready:bool = false\n",
            "self.ready","self.ready = true","","owner.change(true)");
        run(&src,None,false);
    }

    #[test]
    fn m2_branch_bool_main前序真覆寫後方法按新bool值寫入() {
        let src=source("var n:int = 7\nvar ready:bool = false\n",
            "gate","self.ready = false",
            "owner.ready = true\n","owner.change(true)");
        run(&src,Some(("ready".to_owned(),true,false)),true);
    }

    #[test]
    fn m2_branch_bool_const首個int欄位與var_bool真選葉寫入不互相封鎖() {
        let src=source("const n:int = 7\nvar ready:bool = true\n",
            "gate","self.ready = false","","owner.change(true)");
        run(&src,Some(("ready".to_owned(),true,false)),true);
    }

    #[test]
    fn m2_branch_bool_故障在方法入口之前不曾真選葉或寫欄位() {
        let p=checked(SOURCE);
        let failed=p.execute_with_fault(true).unwrap();
        assert_eq!(failed.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(failed.branch_checked,None);
        assert_eq!(failed.branch_bool_write_observed,None);
        assert!(!failed.branch_write_applied);
        assert!(failed.ticket_returned&&failed.has_balanced_self_borrow_trace());
        assert!(failed.events.is_empty());
        assert_eq!(p.execute().unwrap().branch_bool_write_observed,
            Some(("ready".to_owned(),true,false)));
    }

    #[test]
    fn m2_branch_bool_封存欄位身分或bool值竄改必在任何借票前拒絕() {
        let mut p=checked(SOURCE);
        p.method_branch_bool_write.as_mut().unwrap().0="forged".to_owned();
        assert!(matches!(p.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut p=checked(SOURCE);
        p.method_branch_bool_write.as_mut().unwrap().1=PrivateSelfBoolWriteSource::Literal(true);
        assert!(matches!(p.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn m2_branch_bool_原Checker拒絕const欄位與型別錯誤及未知欄位() {
        for src in [
            source("var n:int = 7\nconst ready:bool = true\n",
                "gate","self.ready = false","","owner.change(true)"),
            source("var n:int = 7\nvar ready:bool = true\n",
                "gate","self.ready = 1","","owner.change(true)"),
            source("var n:int = 7\nvar ready:bool = true\n",
                "gate","self.missing = false","","owner.change(true)"),
        ]{
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"非法寫入只能由原Checker靜態拒絕");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn m2_branch_bool_選葉讀取真形參與最新欄位() {
        let fields = "var n:int = 7\nvar ready:bool = true\nvar backup:bool = true\n";
        for rhs in ["(((gate)))", "self.backup"] {
            let src = source(fields, "self.ready", &format!("self.ready = {rhs}"),
                "owner.backup = false\n", "owner.change(false)");
            run(&src, Some(("ready".to_owned(), true, false)), true);
            let skipped = src.replace("if(self.ready)", "if(gate)");
            run(&skipped, None, false);
        }
    }

    #[test]
    fn m2_branch_bool_V7無前置作用直接await_真wake後始寫入() {
        let src=SOURCE.replace("fn change(gate:bool)", "async fn change(gate:bool)")
            .replace("return owner.change(true)",
                "return await owner.change(true)");
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=queue.prepare(&src).unwrap();
        queue.suspend(id).unwrap();
        assert!(queue.execution_order().is_empty());
        let (finished,out)=queue.drive_next().unwrap().unwrap();
        assert_eq!(finished,id);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.branch_bool_write_observed,
            Some(("ready".to_owned(),true,false)));
        assert!(out.awaited_method&&out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        queue.verify_ready_history().unwrap();
    }
}
