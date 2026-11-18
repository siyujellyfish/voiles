// G18／P18.4-B4／P18.6-M2：原 Checker method_bool_condition
// 早已核准 self.bool；本批真 MethodSafety→BorrowTicket→P16 讀取。
#[cfg(test)]
mod g18_b4_m2_self_bool_condition_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
var ready:bool = true
var backup:bool = false
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

    fn source(fields: &str, main_steps: &str,
        outer: &str, inner: &str) -> String {
        SOURCE.replace("var ready:bool = true\nvar backup:bool = false\n",fields)
            .replace("if(gate){", &format!("if({outer}){{"))
            .replace("if(branch){", &format!("if({inner}){{"))
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{main_steps}"))
    }

    fn checked(text: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            text,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 的 self.bool 方法條件已合法");
        PreparedPrivateSelfMethod::prepare(text)
            .expect("合法 self.bool 條件必進原借票與真 P16")
    }

    fn run(text: &str, expected: i64,
        branches: (Option<bool>, Option<bool>), helpers: usize)
        -> PrivateSelfMethodOutcome {
        let plan=checked(text);
        let outcome=plan.execute().expect("真 SharedCallExit 方法執行");
        assert_eq!(outcome.result,Ok(expected));
        assert_eq!((outcome.branch_checked,outcome.nested_branch_checked),branches);
        assert_eq!((outcome.ordinary_calls_entered,outcome.ordinary_calls_completed),
            (helpers,helpers));
        assert!(outcome.method_entered && outcome.method_exited);
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert!(outcome.events.is_empty(),"Copyable bool 不得創造 OwnedStr 事件");
        assert_eq!((outcome.main_local_slots_after_exit,
            outcome.method_local_slots_after_exit),(0,0));
        assert_eq!(outcome.exit_events.iter().filter(|ev|matches!(
            ev,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        outcome
    }

    #[test]
    fn b4_self_bool_原Checker外層self_ready_true以唯一共享票選中() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","branch");
        let p=checked(&src);
        assert_eq!(p.method_branch_self_bool_field,Some("ready".to_owned()));
        assert_eq!(p.loan.kind,BorrowKind::SharedRead);
        run(&src,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_self_bool_原Checker外層self_ready_false不進巢狀helper() {
        let src=source("var ready:bool = false\nvar backup:bool = true\n",
            "","self.ready","branch");
        run(&src,7,(Some(false),None),0);
    }

    #[test]
    fn b4_self_bool_方法內層self_backup_false由相同共享票真讀() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","gate","self.backup");
        let p=checked(&src);
        assert_eq!(p.method_nested_self_bool_field,Some("backup".to_owned()));
        assert_eq!(p.loan.kind,BorrowKind::SharedRead);
        run(&src,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_self_bool_原Lexer多層Group_內外self欄位各自證實() {
        let src=source("var ready:bool = true\nvar backup:bool = true\n",
            "","(((self.ready)))","((self.backup))");
        run(&src,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_self_bool_相同bool真值不同欄位不可擅自視為同一身分() {
        let src=source("var ready:bool = true\nvar backup:bool = true\n",
            "","self.ready","self.backup");
        let p=checked(&src);
        assert_ne!(p.method_branch_self_bool_field,p.method_nested_self_bool_field);
        run(&src,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_self_bool_原Checker_const_bool欄位也可共享自借讀條件() {
        let src=source("const ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","self.backup");
        run(&src,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_self_bool_前序main寫入bool後方法真讀最新值而非預設() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "owner.ready = false\n","self.ready","branch");
        let p=checked(&src);
        assert_eq!(p.method_branch,Some((false,p.method_branch.unwrap().1)));
        run(&src,7,(Some(false),None),0);
    }

    #[test]
    fn b4_self_bool_前序main寫true讓原false欄位在方法內真選中() {
        let src=source("var ready:bool = false\nvar backup:bool = false\n",
            "owner.ready = true\n","self.ready","self.backup");
        run(&src,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_self_bool_獨占self寫int的分支可在同一票讀bool條件() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","branch")
            .replace(
                "if(self.ready){\nif(branch){\nreturn helper(step+3,2)\n}else{\nreturn helper(step+2,5)\n}\n}",
                "if(self.ready){\nself.n = step\nreturn self.n\n}"
            );
        let p=checked(&src);
        assert_eq!(p.loan.kind,BorrowKind::ExclusiveWrite);
        let outcome=p.execute().unwrap();
        assert_eq!(outcome.result,Ok(5));
        assert_eq!(outcome.branch_checked,Some(true));
        assert!(outcome.branch_write_applied);
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert!(outcome.events.is_empty());
        assert_eq!(outcome.exit_events.iter().filter(|ev|matches!(
            ev,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_self_bool_方法真共享讀取後故障反向還票且可再次執行() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","self.backup");
        let plan=checked(&src);
        let failed=plan.execute_with_fault(true).expect("方法錯誤必受控退出");
        assert_eq!(failed.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failed.method_entered && !failed.method_exited);
        assert!(failed.ticket_returned && failed.has_balanced_self_borrow_trace());
        assert!(failed.events.is_empty());
        assert_eq!(plan.execute().unwrap().result,Ok(2));
    }

    #[test]
    fn b4_self_bool_變造外層或內層欄位封存均先SourceChanged() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","self.backup");
        let mut outer=checked(&src);
        outer.method_branch_self_bool_field=Some("backup".to_owned());
        assert!(matches!(outer.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut inner=checked(&src);
        inner.method_nested_self_bool_field=Some("ready".to_owned());
        assert!(matches!(inner.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_self_bool_原Checker_async_self直接await真wake後讀bool條件() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "owner.ready = false\n","self.ready","branch")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)")
            .replace("return owner.read(true,true,5)",
                "return await owner.read(true,true,5)");
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=queue.prepare(&src).expect("原 Checker合法 async self 直接await");
        assert_eq!(queue.status(id),Some(PrivateDeferredAwaitStatus::Prepared));
        queue.suspend(id).unwrap();
        assert_eq!(queue.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.execution_order().len(),0,"wake 前不可假入方法或發自借票");
        let (finished,out)=queue.drive_next().unwrap().expect("真FIFO wake與借票");
        assert_eq!(finished,id);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.branch_checked,Some(false));
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_self_bool_async_安全點取消未執行方法不產生借票或helper() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "","self.ready","self.backup")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)")
            .replace("return owner.read(true,true,5)",
                "return await owner.read(true,true,5)");
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=queue.prepare(&src).unwrap();
        queue.suspend(id).unwrap();
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id),Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.execution_order().is_empty());
        assert!(queue.drive_next().unwrap().is_none());
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_self_bool_原Checker拒絕非bool或不存在的self條件() {
        for src in [
            source("var ready:bool = true\nvar backup:bool = false\n",
                "","self.n","branch"),
            source("var ready:bool = true\nvar backup:bool = false\n",
                "","self.missing","branch"),
            source("var ready:bool = true\nvar backup:bool = false\n",
                "","gate","self.n"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原 Checker 必須拒絕非法 self bool 條件");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
