// G18／B4/M2：原Checker可複製 owner.int 欄位至 main int 局部。
// 實際讀取來自同一 MixedStructRuntime 的目前值，不用準備期快照
// 偽造 TypedHIR FieldRead 或方法自借票前的欄位狀態。
#[cfg(test)]
mod g18_b4_m2_main_field_copy_local_tests {
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
return owner.read(false,true,5)
}
"#;

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(false,true,5)", &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker應核准已初始化owner Copyable int欄位直接讀取");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("原Checker已驗證的 owner.int Copyable 局部來源不可錯拒")
    }

    fn run(src: &str, expected: i64, reads: usize, calls: usize,
        branch: (Option<bool>,Option<bool>)) -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        assert_eq!(plan.main_owner_field_read_sources.len(),reads);
        assert_eq!(plan.main_scalar_steps.iter().filter(|step|matches!(
            step,PrivateSelfMainScalarStep::IntDeclareOwnerField(_)
        )).count(),reads);
        for span in plan.main_owner_field_read_sources.values() {
            assert!(plan.verified.plan.scalar_hir.expression(*span).is_some());
        }
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable int讀取不得假造str ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_field_copy_原Checker預設owner_int直接建立const局部再傳方法() {
        let src=source("const saved:int = owner.n\n",
            "owner.read(true,false,saved)");
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps[0],PrivateSelfMainScalarStep::IntDeclareOwnerField(0));
        run(&src,4,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_先寫十一再真正讀取owner欄位可傳方法() {
        let src=source("owner.n = 11\nconst saved:int = owner.n\n",
            "owner.read(true,false,saved)");
        run(&src,8,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_先Copy七再寫十一局部保留原值身份() {
        let src=source("const saved:int = owner.n\nowner.n = 11\n",
            "owner.read(true,false,saved)");
        run(&src,4,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_兩個具名讀取各綁不同原CheckerDeclId與真欄位時序() {
        let src=source(
            "const before:int = owner.n\nowner.n = 11\nconst after:int = owner.n\n",
            "owner.read(true,false,after)",
        );
        let plan=checked(&src);
        let first=plan.main_int_locals[0].1;
        let second=plan.main_int_locals[1].1;
        assert_ne!(first,second);
        assert_ne!(plan.main_owner_field_read_sources[&first],
            plan.main_owner_field_read_sources[&second]);
        run(&src,8,2,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_合法多層Group包覆owner欄位讀取仍是內葉ScalarHIR() {
        let src=source(
            "const saved:int = ((owner.n))\n",
            "((owner.read(true,false,saved)))",
        );
        run(&src,4,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_欄位先複製到var再純整數重賦保留真正次序() {
        let src=source(
            "var saved:int = owner.n\nsaved = saved+2\n",
            "owner.read(true,false,saved)",
        );
        run(&src,6,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_field_copy_普通helper先寫owner欄位再讀取進main值槽() {
        let src=source(
            "owner.n = helper(9,4)\nconst saved:int = owner.n\n",
            "owner.read(false,true,saved)",
        );
        let out=run(&src,5,1,1,(Some(false),None));
        let helper_exit=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let self_loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(helper_exit<self_loan);
    }

    #[test]
    fn b4_field_copy_含bool局部與int欄位更新的呼叫前置真圖() {
        let src=source(
            "var gate:bool = true\nowner.n = 11\nconst saved:int = owner.n\ngate = false\n",
            "owner.read(gate,true,saved)",
        );
        run(&src,11,1,0,(Some(false),None));
    }

    #[test]
    fn b4_field_copy_原Checker_const_owner可複製唯讀欄位且self共享借票() {
        let src=source(
            "const saved:int = owner.n\n",
            "owner.read(false,true,saved)",
        ).replace("var owner:Counter = Counter()",
            "const owner:Counter = Counter()");
        run(&src,7,1,0,(Some(false),None));
    }

    #[test]
    fn b4_field_copy_先寫入helper溢位不建立後續偽局部或self借票() {
        let src=source(
            "owner.n = helper(9223372036854775807,0)+1\nconst saved:int = owner.n\n",
            "owner.read(false,true,saved)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert_eq!(out.main_local_slots_after_exit,0);
    }

    #[test]
    fn b4_field_copy_原Checker合法async直接await但前置欄位讀取不開背景工作() {
        let src=source(
            "const saved:int = owner.n\n",
            "await owner.read(true,false,saved)",
        ).replace("fn read(gate:bool,branch:bool,step:int)",
            "async fn read(gate:bool,branch:bool,step:int)");
        let out=run(&src,4,1,1,(Some(true),Some(false)));
        assert!(out.awaited_method);
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(queue.next_id,0);
        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn b4_field_copy_變造FieldRead原Span或工作步驟必SourceChanged() {
        let src=source("const saved:int = owner.n\n",
            "owner.read(true,false,saved)");
        let mut plan=checked(&src);
        let id=plan.main_int_locals[0].1;
        plan.main_owner_field_read_sources.remove(&id);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_scalar_steps[0]=PrivateSelfMainScalarStep::IntDeclare(0);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_field_copy_原Checker拒絕不存在欄位與不合法型別複製() {
        for src in [
            source("const saved:int = owner.missing\n",
                "owner.read(false,true,saved)"),
            source("const saved:bool = owner.n\n",
                "owner.read(saved,true,5)"),
            source("const saved:int = missing.n\n",
                "owner.read(false,true,saved)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker應拒絕非法owner欄位來源：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn b4_field_copy_複合欄位算術尚無同一型別工作圖仍保守拒絕() {
        let src=source(
            "const saved:int = owner.n+1\n",
            "owner.read(true,false,saved)",
        );
        verify_source_with_ability(
            &src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker已核准Copyable欄位於int算術中讀取");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }
}
