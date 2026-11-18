// G18／P18.4-B4／P18.6-M2：原 Checker 已核准的直接
// self.bool FieldAssign，唯一 ExclusiveWrite 自借與 P16 真原子寫入。
// 不製造第二套 Parser／BorrowTicket／OwnedStr 資源事件。
#[cfg(test)]
mod g18_m2_self_bool_direct_write_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
var ready:bool = true
var backup:bool = false
fn change(gate:bool)->int{
self.ready = gate
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.change(false)
}
"#;

    fn source(fields: &str, body: &str, main_steps: &str, call: &str) -> String {
        SOURCE.replace(
            "var n:int = 7\nvar ready:bool = true\nvar backup:bool = false\n",
            fields,
        ).replace("self.ready = gate\nreturn self.n",body)
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{main_steps}"))
            .replace("return owner.change(false)",&format!("return {call}"))
    }

    fn variant(fields: &str, rhs: &str, main_steps: &str, call: &str) -> String {
        source(fields,&format!("self.ready = {rhs}\nreturn self.n"),main_steps,call)
    }

    fn fields(ready: bool, backup: bool) -> String {
        format!("var n:int = 7\nvar ready:bool = {ready}\nvar backup:bool = {backup}\n")
    }

    fn checked(text: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            text,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 必須核准已驗證獨占 self.bool 方法欄位寫入");
        let plan=PreparedPrivateSelfMethod::prepare(text)
            .expect("原 Checker 合法 self.bool 方法直接賦值不應錯拒");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        assert_eq!(plan.verified.plan.declarations.call_borrows_at(plan.call_span),
            Some([plan.loan].as_slice()));
        plan
    }

    fn run(text: &str, before: bool, after: bool,
        expected_source: PrivateSelfBoolWriteSource) -> PrivateSelfMethodOutcome {
        let plan=checked(text);
        assert_eq!(plan.direct_method_bool_write,
            Some(("ready".to_owned(),expected_source,after)));
        let result=plan.execute().expect("同一 P16 owner 實際執行 bool寫入");
        assert_eq!(result.result,Ok(7));
        assert_eq!(result.direct_bool_write_observed,
            Some(("ready".to_owned(),before,after)));
        assert!(result.method_entered && result.method_exited);
        assert!(result.ticket_returned && result.has_balanced_self_borrow_trace());
        assert!(result.events.is_empty(),"Copyable bool不可產生 OwnedStr 事件");
        assert_eq!((result.main_local_slots_after_exit,
            result.method_local_slots_after_exit),(0,0));
        assert_eq!(result.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1,"不可以為 bool 寫入另借第二張 self 票");
        result
    }

    #[test]
    fn m2_bool_write_原Lexer_bool_false直接真原子覆寫() {
        let src=variant(&fields(true,false),"false","","owner.change(false)");
        run(&src,true,false,PrivateSelfBoolWriteSource::Literal(false));
    }

    #[test]
    fn m2_bool_write_原Lexer_bool_true覆寫原false預設() {
        let src=variant(&fields(false,false),"true","","owner.change(false)");
        run(&src,false,true,PrivateSelfBoolWriteSource::Literal(true));
    }

    #[test]
    fn m2_bool_write_方法bool形參原DeclId真值入同一self欄位() {
        let src=SOURCE.to_owned();
        let plan=checked(&src);
        run(&src,true,false,PrivateSelfBoolWriteSource::Parameter(
            plan.method_bool_arguments[0].0,
        ));
    }

    #[test]
    fn m2_bool_write_main具名bool真值於借self前傳入方法() {
        let src=variant(&fields(false,false),"gate",
            "const desired:bool = true\n","owner.change(desired)");
        let plan=checked(&src);
        run(&src,false,true,PrivateSelfBoolWriteSource::Parameter(
            plan.method_bool_arguments[0].0,
        ));
    }

    #[test]
    fn m2_bool_write_同一self備用bool欄位真讀再覆寫主bool() {
        let src=variant(&fields(true,false),"self.backup",
            "","owner.change(false)");
        run(&src,true,false,PrivateSelfBoolWriteSource::SelfField("backup".to_owned()));
    }

    #[test]
    fn m2_bool_write_main先寫backup後獨占方法再從真P16讀來源() {
        let src=variant(&fields(false,false),"self.backup",
            "owner.backup = true\n","owner.change(false)");
        run(&src,false,true,PrivateSelfBoolWriteSource::SelfField("backup".to_owned()));
    }

    #[test]
    fn m2_bool_write_原Lexer多重Group_self欄位仍保留唯一欄位身分() {
        let src=variant(&fields(true,false),"(((self.backup)))",
            "","owner.change(false)");
        run(&src,true,false,PrivateSelfBoolWriteSource::SelfField("backup".to_owned()));
    }

    #[test]
    fn m2_bool_write_原Checker同欄位自讀覆寫不產生多張借票() {
        let src=variant(&fields(true,false),"self.ready",
            "","owner.change(false)");
        run(&src,true,true,PrivateSelfBoolWriteSource::SelfField("ready".to_owned()));
    }

    #[test]
    fn m2_bool_write_第一個int欄位為const仍可獨占寫另一個var_bool() {
        let src=variant(&fields(true,false).replace("var n:int","const n:int"),
            "false","","owner.change(false)");
        run(&src,true,false,PrivateSelfBoolWriteSource::Literal(false));
    }

    #[test]
    fn m2_bool_write_方法入口注入故障不提交bool且歸還唯一借票() {
        let plan=checked(SOURCE);
        let failed=plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(failed.direct_bool_write_observed,None);
        assert!(failed.method_entered && !failed.method_exited);
        assert!(failed.ticket_returned && failed.has_balanced_self_borrow_trace());
        assert!(failed.events.is_empty());
        assert_eq!(plan.execute().unwrap().direct_bool_write_observed,
            Some(("ready".to_owned(),true,false)));
    }

    #[test]
    fn m2_bool_write_真欄位已提交但返回溢位仍須Unwind及還票() {
        let src=source(&fields(true,false),
            "self.ready = false\nreturn self.n+9223372036854775807",
            "","owner.change(false)");
        let plan=checked(&src);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.direct_bool_write_observed,
            Some(("ready".to_owned(),true,false)));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty());
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn m2_bool_write_準備期篡改目標來源與預期值均須事前SourceChanged() {
        let mut plan=checked(SOURCE);
        plan.direct_method_bool_write.as_mut().unwrap().0="backup".to_owned();
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(SOURCE);
        plan.direct_method_bool_write.as_mut().unwrap().2=true;
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(SOURCE);
        plan.direct_method_bool_write.as_mut().unwrap().1 =
            PrivateSelfBoolWriteSource::Literal(true);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn m2_bool_write_原Checker靜態拒絕const或int冒充bool與未知self欄位() {
        for src in [
            variant(&fields(true,false).replace("var ready:bool","const ready:bool"),
                "false","","owner.change(false)"),
            variant(&fields(true,false),
                "7","","owner.change(false)"),
            source(&fields(true,false),
                "self.missing = false\nreturn self.n",
                "","owner.change(false)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"靜態不合法不可由私有執行器放行");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn m2_bool_write_async_self直接await延遲喚醒後才真寫bool並還票() {
        let src=SOURCE.replace("fn change(gate:bool)", "async fn change(gate:bool)")
            .replace("return owner.change(false)", "return await owner.change(false)");
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=queue.prepare(&src).expect("原 Checker E1 無前置作用直接await");
        queue.suspend(id).unwrap();
        assert!(queue.execution_order().is_empty(),"wake之前不能偷寫bool");
        let (finished,out)=queue.drive_next().unwrap().unwrap();
        assert_eq!(finished,id);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.direct_bool_write_observed,
            Some(("ready".to_owned(),true,false)));
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        queue.verify_ready_history().unwrap();
    }
}
