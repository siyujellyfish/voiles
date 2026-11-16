// G18／P18.4-B4／P18.6-M3：原Checker已核准 main Copyable bool
// 具名來源在真值槽間逐宣告複製，不依值相同偷換宣告身分。
#[cfg(test)]
mod g18_b4_m3_main_bool_copy_chain_tests {
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

    fn source(decls: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{decls}"))
            .replace("owner.read(true,true,5)",call)
    }

    fn checked(src: &str, count: usize) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            src, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("Copyable bool 名稱來源須由原 Lexer/Checker 核准");
        let plan=PreparedPrivateSelfMethod::prepare(src)
            .expect("原Checker合法前向 bool 複製不得由執行器錯拒");
        assert_eq!(plan.main_bool_locals.len(),count);
        plan
    }

    fn run(src: &str, count: usize, expected: i64,
        branch: (Option<bool>,Option<bool>), calls: usize) -> PrivateSelfMethodOutcome {
        let plan=checked(src,count);
        let outcome=plan.execute().unwrap();
        assert_eq!(outcome.result,Ok(expected));
        assert_eq!((outcome.branch_checked,outcome.nested_branch_checked),branch);
        assert_eq!((outcome.ordinary_calls_entered,outcome.ordinary_calls_completed),
            (calls,calls));
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert!(outcome.events.is_empty());
        assert_eq!((outcome.main_local_slots_after_exit,outcome.method_local_slots_after_exit),
            (0,0));
        outcome
    }

    #[test]
    fn b4_bool_chain_原Checker真const_bool前向複製到方法旗標() {
        let src=source("const a:bool = true\nconst b:bool = a\n",
            "owner.read(b,true,5)");
        let plan=checked(&src,2);
        assert_eq!(plan.main_bool_locals[1].3,Some(plan.main_bool_locals[0].1));
        assert_ne!(plan.main_bool_locals[0].1,plan.main_bool_locals[1].1);
        run(&src,2,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_bool_chain_var_false經多層Group名稱複製不進入任何helper() {
        let src=source("var gate:bool = false\nconst alias:bool = ((gate))\n",
            "owner.read(((alias)),true,5)");
        run(&src,2,7,(Some(false),None),0);
    }

    #[test]
    fn b4_bool_chain_三層Copyable來源不同DeclId但同一真值() {
        let src=source("const a:bool = true\nconst b:bool = (a)\nconst c:bool = ((b))\n",
            "owner.read(c,b,5)");
        let plan=checked(&src,3);
        assert_eq!(plan.main_bool_locals[1].3,Some(plan.main_bool_locals[0].1));
        assert_eq!(plan.main_bool_locals[2].3,Some(plan.main_bool_locals[1].1));
        let unique=plan.main_bool_locals.iter().map(|item|item.1)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(),3);
        run(&src,3,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_bool_chain_兩個bool分支參數各從真前向局部讀值() {
        let src=source(
            "const initial:bool = false\nconst gate:bool = true\nconst branch:bool = ((initial))\n",
            "owner.read(gate,branch,5)");
        let plan=checked(&src,3);
        assert_eq!(plan.method_bool_sources,vec![
            Some(plan.main_bool_locals[1].1),Some(plan.main_bool_locals[2].1),
        ]);
        run(&src,3,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_chain_main純int_helper與bool別名在self借票前各守真動態順序() {
        let src=source(
            "const initial:bool = true\nconst alias:bool = initial\nconst step:int = helper(7,2)\n",
            "owner.read(alias,true,step)");
        let out=run(&src,2,6,(Some(true),Some(true)),2);
        let ordinary_exit=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(ordinary_exit<loan);
    }

    #[test]
    fn b4_bool_chain_async_self原Checker直接await不擴張延遲佇列() {
        let src=source(
            "const gate:bool = true\nconst alias:bool = (gate)\n",
            "await owner.read(alias,true,5)")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)");
        let plan=checked(&src,2);
        assert!(plan.awaited_method);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(6));
        assert!(out.awaited_method && out.ticket_returned
            && out.has_balanced_self_borrow_trace());
        // E1 延遲 FIFO 未開放 main 前置局部，保留原保守拒絕。
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert!(queue.execution_order().is_empty());
    }

    #[test]
    fn b4_bool_chain_準備後別名來源DeclId偽造不得提前進self框架() {
        let src=source("const a:bool = true\nconst b:bool = a\n",
            "owner.read(b,true,5)");
        let mut plan=checked(&src,2);
        plan.main_bool_locals[1].3=Some(DeclarationId(99887766));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_chain_準備後別名值變造不得以相同bool型別冒充原值() {
        let src=source("const a:bool = true\nconst b:bool = a\n",
            "owner.read(b,true,5)");
        let mut plan=checked(&src,2);
        plan.main_bool_locals[1].2=false;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_chain_非法前向未初始化自引或int來源原Checker仍拒絕() {
        for src in [
            source("const b:bool = a\nconst a:bool = true\n",
                "owner.read(b,true,5)"),
            source("const b:bool = b\n","owner.read(b,true,5)"),
            source("const n:int = 5\nconst b:bool = n\n","owner.read(b,true,5)"),
            source("const b:bool = helper(1,2)\n","owner.read(b,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"不得以執行器名義補定非法bool別名：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
