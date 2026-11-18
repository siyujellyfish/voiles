// G18／P18.4-B4／P18.6-M3：main 原 Checker var bool 直線
// 賦值（已初始化 Copyable、最後一組無副作用），真 RHS DeclId 與
// 方法形參值槽一致；不擴張一般 CFG 或 V7 E1 前置工作。
#[cfg(test)]
mod g18_b4_m3_main_bool_assignment_tests {
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

    fn run(src: &str, count: usize, assignments: usize, expected: i64,
        branch: (Option<bool>, Option<bool>), calls: usize)
        -> PrivateSelfMethodOutcome {
        verify_source_with_ability(src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker應核准合法var bool賦值及方法呼叫");
        let plan=PreparedPrivateSelfMethod::prepare(src)
            .expect("原Checker已核准的bool賦值不得由受限Executor錯拒");
        assert_eq!(plan.main_bool_locals.len(),count);
        assert_eq!(plan.main_bool_assignments.len(),assignments);
        let outcome=plan.execute().unwrap();
        assert_eq!(outcome.result,Ok(expected));
        assert_eq!((outcome.branch_checked,outcome.nested_branch_checked),branch);
        assert_eq!((outcome.ordinary_calls_entered,outcome.ordinary_calls_completed),
            (calls,calls));
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert!(outcome.events.is_empty(), "Copyable bool不得假造OwnedStr事件");
        assert_eq!((outcome.main_local_slots_after_exit,outcome.method_local_slots_after_exit),(0,0));
        outcome
    }

    #[test]
    fn b4_bool_assign_原Checker_var_true更新false後方法零helper() {
        let src=source("var gate:bool = true\ngate = false\n",
            "owner.read(gate,true,5)");
        run(&src,1,1,7,(Some(false),None),0);
    }

    #[test]
    fn b4_bool_assign_原Checker_var_false更新true後方法只跑真選葉() {
        let src=source("var gate:bool = false\ngate = true\n",
            "owner.read(gate,true,5)");
        run(&src,1,1,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_bool_assign_原Lexer合法Group常值與具名bool_RHS可真求值() {
        let src=source(
            "var gate:bool = true\nconst branch:bool = ((false))\ngate = ((branch))\n",
            "owner.read(((gate)),true,5)",
        );
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.main_bool_assignments[0].2,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[1].1)));
        run(&src,2,1,7,(Some(false),None),0);
    }

    #[test]
    fn b4_bool_assign_同一變數多次直線更新以最後真值傳方法() {
        let src=source(
            "var gate:bool = true\ngate = false\ngate = true\n",
            "owner.read(gate,false,5)",
        );
        run(&src,1,2,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_assign_同一真DeclId自我複製只讀舊值再提交() {
        let src=source("var gate:bool = true\ngate = ((gate))\n",
            "owner.read(gate,true,5)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.main_bool_assignments[0].2,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)));
        run(&src,1,1,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_bool_assign_交錯兩個可變bool正確保持逐筆原來源時序() {
        let src=source(
            "var gate:bool = true\nvar branch:bool = false\ngate = branch\nbranch = true\n",
            "owner.read(gate,branch,5)",
        );
        let out=run(&src,2,2,7,(Some(false),None),0);
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn b4_bool_assign_原Checker合法async直接await執行成功但E1佇列拒絕前置() {
        let src=source("var gate:bool = false\ngate = true\n",
            "await owner.read(gate,false,5)")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)");
        let out=run(&src,1,1,2,(Some(true),Some(false)),1);
        assert!(out.awaited_method);
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.next_id,0);
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_bool_assign_原Checker_const賦值與未知名稱靜態拒絕() {
        for src in [
            source("const gate:bool = true\ngate = false\n",
                "owner.read(gate,true,5)"),
            source("var gate:bool = true\nunknown = false\n",
                "owner.read(gate,true,5)"),
            source("var gate:bool = true\ngate = 3\n",
                "owner.read(gate,true,5)"),
            source("var gate:bool = true\ngate = missing\n",
                "owner.read(gate,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(), "原Checker不能接受非法var bool賦值：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn b4_bool_assign_準備後更改目標或RHS來源必在借self前拒絕() {
        let src=source(
            "var a:bool = true\nvar gate:bool = false\ngate = a\n",
            "owner.read(gate,true,5)",
        );
        let mut plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        plan.main_bool_assignments[0].0=plan.main_bool_locals[0].1;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        plan.main_bool_assignments[0].2=None;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_assign_不同int局部真函式交錯由原Parser順序成功執行() {
        let src=source(
            "var gate:bool = true\nconst step:int = helper(7,2)\ngate = false\n",
            "owner.read(gate,true,step)",
        );
        // #831 以原 Parser 一體步驟表真正打通此前 Unsupported：
        // main helper 必真完成 Returned／Exited，後續 bool 更新後
        // 才能以最後值進入方法並歸還唯一 self BorrowTicket。
        let outcome=run(&src,1,1,7,(Some(false),None),1);
        let first_exit=outcome.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=outcome.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(first_exit<loan, "普通 helper 應先於方法自借票真退出");
    }
}
