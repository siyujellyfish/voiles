// G18／P18.4-B4、P18.6-M2/M3：main 已核准 Copyable bool
// 局部與方法參數的同一原 Parser/Checker 宣告身分、真 main 值槽。
// 不擴張一般 bool 運算、變數賦值或 Borrow/Move。
#[cfg(test)]
mod g18_b4_m3_main_bool_local_method_argument_tests {
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

    fn main_locals(declarations: &str, call: &str) -> String {
        SOURCE.replace(
            "var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{declarations}"),
        ).replace("return owner.read(true,true,5)", &format!("return {call}"))
    }

    fn checked(source: &str, count: usize) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 正式允許 Copyable bool main 局部與方法形參");
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("合法 bool main 局部不應遭執行器錯拒");
        assert_eq!(plan.main_bool_locals.len(), count);
        assert_eq!(plan.method_bool_sources.len(), plan.method_bool_arguments.len());
        plan
    }

    fn run(source: &str, count: usize, expected: i64,
        branch: (Option<bool>, Option<bool>), calls: usize) -> PrivateSelfMethodOutcome {
        let plan = checked(source, count);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(expected));
        assert_eq!((outcome.branch_checked, outcome.nested_branch_checked), branch);
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed),
            (calls,calls));
        assert!(outcome.has_balanced_self_borrow_trace() && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
        assert_eq!((outcome.main_local_slots_after_exit,outcome.method_local_slots_after_exit),
            (0,0));
        outcome
    }

    #[test]
    fn b4_main_bool_原Checker_const_true真值槽傳給方法選中內層helper() {
        let src=main_locals("const gate:bool = true\n", "owner.read(gate,true,5)");
        let plan=checked(&src,1);
        assert_eq!(plan.method_bool_sources, vec![Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)),None]);
        run(&src,1,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_main_bool_原Checker_var_false傳入方法外層false零helper() {
        let src=main_locals("var gate:bool = false\n", "owner.read(gate,true,5)");
        run(&src,1,7,(Some(false),None),0);
    }

    #[test]
    fn b4_main_bool_原Checker多層括號初始化和具名實參真bool來源() {
        let src=main_locals("const gate:bool = ((false))\n",
            "owner.read(((gate)),(true),5)");
        run(&src,1,7,(Some(false),None),0);
    }

    #[test]
    fn b4_main_bool_兩份bool局部維持獨立DeclarationId及真選枝() {
        let src=main_locals(
            "const gate:bool = true\nconst branch:bool = false\n",
            "owner.read(gate,((branch)),5)",
        );
        let plan=checked(&src,2);
        assert_ne!(plan.main_bool_locals[0].1,plan.main_bool_locals[1].1);
        assert_eq!(plan.method_bool_sources,vec![
            Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)),
            Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[1].1)),
        ]);
        run(&src,2,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_main_bool_原型別重排int_bool形參不混用main兩種值槽() {
        let source=SOURCE.replace(
            "fn read(gate:bool,branch:bool,step:int)->int",
            "fn read(step:int,branch:bool,gate:bool)->int",
        ).replace("var owner:Counter = Counter()\n",
            "var owner:Counter = Counter()\nconst n:int = 5\nconst a:bool = true\nconst b:bool = false\n")
            .replace("owner.read(true,true,5)", "owner.read(n,b,((a)))");
        run(&source,2,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_main_bool_main普通helper先返回再借self且保持選中葉helper() {
        let src=main_locals(
            "const gate:bool = true\nconst step:int = helper(7,2)\n",
            "owner.read(gate,true,step)",
        );
        let out=run(&src,1,6,(Some(true),Some(true)),2);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        let helper_before=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:true }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(helper_before<loan, "main helper 必先退出，才能借入唯一 self");
    }

    #[test]
    fn b4_main_bool_async_self直接await執行_延遲佇列前置局部保守拒絕() {
        let src=main_locals("const gate:bool = true\nconst branch:bool = false\n",
            "await owner.read(((gate)),branch,5)")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)");
        // 原 Checker 的直接 await 子集可以真執行。
        let plan=checked(&src,2);
        assert!(plan.awaited_method);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(2));
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        // V7 E1 延遲工作佇列當前明確禁止任何前置 main 局部。
        // 不得靠本次 bool 值槽工程放寬 E1／E3 的未核准範圍。
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.next_id,0);
        assert!(queue.execution_order().is_empty());
        assert!(queue.ready_history.is_empty());
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_main_bool_預備後主布林值槽竄改不可進入方法取得BorrowTicket() {
        let src=main_locals("const gate:bool = true\n", "owner.read(gate,true,5)");
        let mut plan=checked(&src,1);
        plan.main_bool_locals[0].2 = false;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_main_bool_布林實參來源跨DeclId偽裝不得先簽self票() {
        let src=main_locals(
            "const gate:bool = true\nconst branch:bool = false\n",
            "owner.read(gate,branch,5)",
        );
        let mut plan=checked(&src,2);
        plan.method_bool_sources.swap(0,1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src,2);
        plan.method_bool_sources.pop();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_main_bool_原Checker錯型別未知來源或含呼叫布林初始化均不可放行() {
        for src in [
            main_locals("const gate:bool = 5\n", "owner.read(gate,true,5)"),
            main_locals("const gate:bool = true\n", "owner.read(unknown,true,5)"),
            main_locals("const gate:bool = true\n", "owner.read(gate,5,5)"),
            main_locals("const gate:bool = helper(3,2)\n", "owner.read(gate,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src, crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(), "原Checker不能放行非法bool來源：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
