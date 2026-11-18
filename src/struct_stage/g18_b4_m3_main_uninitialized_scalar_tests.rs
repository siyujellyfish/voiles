// G18／B4／M3：原 Checker var int／bool 尚未初始化的真宣告
// 只保留 DeclarationId；首次合法 Assign 之後才有真 Copyable 值槽。
#[cfg(test)]
mod g18_b4_m3_main_uninitialized_scalar_tests {
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

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(true,true,5)", &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 必須認證已核准的首次 Copyable 賦值");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("原 Checker 合法先宣告後初始化不可由受限 Executor 錯拒")
    }

    fn run(src: &str, expected: i64, calls: usize,
        branch: (Option<bool>,Option<bool>)) -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"純 Copyable int/bool 不得偽造 OwnedStr 事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_uninit_main_原Checker_var_int先宣告後常值首次提交() {
        let src=source("var step:int\nstep = 5\n",
            "owner.read(true,false,step)");
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps[0],PrivateSelfMainScalarStep::IntDeclareEmpty(0));
        assert!(matches!(plan.main_init_plans[0],PrivateScalarReturnExpr::Unreachable));
        run(&src,2,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_uninit_main_原Checker_var_bool先宣告後true首次提交() {
        let src=source("var gate:bool\ngate = true\n",
            "owner.read(gate,false,5)");
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps[0],PrivateSelfMainScalarStep::BoolDeclareEmpty(0));
        run(&src,2,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_uninit_main_原Checker_bool先宣告後false不假預設() {
        let src=source("var gate:bool\ngate = false\n",
            "owner.read(gate,true,5)");
        run(&src,7,0,(Some(false),None));
    }

    #[test]
    fn b4_uninit_main_原Checker初次具名int與多層Group再算術() {
        let src=source(
            "const base:int = 3\nvar step:int\nstep = ((base+4))-2\n",
            "owner.read(true,false,step)",
        );
        run(&src,2,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_uninit_main_原Checker_bool初次由已初始化名稱Copyable傳遞() {
        let src=source(
            "const source:bool = true\nvar gate:bool\ngate = ((source))\n",
            "owner.read(gate,true,5)",
        );
        let plan=checked(&src);
        assert_eq!(plan.main_bool_assignments[0].2,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)));
        run(&src,6,1,(Some(true),Some(true)));
    }

    #[test]
    fn b4_uninit_main_兩個未初始化var交錯賦值後才可複製() {
        let src=source(
            "var step:int\nvar gate:bool\nstep = 5\ngate = true\n",
            "owner.read(gate,true,step)",
        );
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps.len(),4);
        run(&src,6,1,(Some(true),Some(true)));
    }

    #[test]
    fn b4_uninit_main_未初始化int先經真helper完成後才簽self借票() {
        let src=source(
            "var step:int\nstep = helper(9,4)\n",
            "owner.read(true,false,step)",
        );
        let out=run(&src,2,2,(Some(true),Some(false)));
        let finish=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(finish<loan);
    }

    #[test]
    fn b4_uninit_main_首次helper溢位不可填值亦不得進方法或後續bool() {
        let src=source(
            "var step:int\nvar gate:bool\nstep = helper(9223372036854775807,0)+1\ngate = true\n",
            "owner.read(gate,true,step)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(out.main_local_slots_after_exit,0);
    }

    #[test]
    fn b4_uninit_main_未被讀取的空var不得偽造零值或退出錯誤() {
        let src=source(
            "var spare:int\nvar unused:bool\n",
            "owner.read(true,false,5)",
        );
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps,vec![
            PrivateSelfMainScalarStep::IntDeclareEmpty(0),
            PrivateSelfMainScalarStep::BoolDeclareEmpty(0),
        ]);
        run(&src,2,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_uninit_main_原Checker已核准async直接await仍不放寬E1延遲佇列() {
        let src=source("var gate:bool\ngate = true\n",
            "await owner.read(gate,false,5)")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                     "async fn read(gate:bool,branch:bool,step:int)");
        let out=run(&src,2,1,(Some(true),Some(false)));
        assert!(out.awaited_method);
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.next_id,0);
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_uninit_main_來源步驟Empty變成有值或重播必先SourceChanged() {
        let src=source("var step:int\nstep = 5\n",
            "owner.read(true,false,step)");
        let mut plan=checked(&src);
        plan.main_scalar_steps[0]=PrivateSelfMainScalarStep::IntDeclare(0);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_scalar_steps.push(PrivateSelfMainScalarStep::IntDeclareEmpty(0));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_int_assignments[0].0=plan.owner_id;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_uninit_main_原Checker拒絕未初始化讀取自指常值型別混用() {
        for src in [
            source("var step:int\n","owner.read(true,false,step)"),
            source("var gate:bool\n","owner.read(gate,true,5)"),
            source("var step:int\nstep = step\n","owner.read(true,false,step)"),
            source("var gate:bool\ngate = gate\n","owner.read(gate,true,5)"),
            source("var step:int\nstep = true\n","owner.read(true,false,step)"),
            source("var gate:bool\ngate = 3\n","owner.read(gate,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission
            ).is_err(),"原 Checker 不得放行空宣告後未初始化讀取：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
