// G18／B4/M3：main 原 Checker 同源 Copyable bool Name 初始化、
// Group 值葉、前序 DeclarationId 真值槽；不放寬 E1 延遲佇列。
#[cfg(test)]
mod g18_b4_m3_main_bool_alias_chain_tests {
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

    fn source(locals: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{locals}"))
            .replace("return owner.read(true,true,5)", &format!("return {call}"))
    }

    fn run(source: &str, count: usize, expected: i64,
        branch: (Option<bool>, Option<bool>), calls: usize)
        -> PrivateSelfMethodOutcome {
        verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 應核准已初始化 Copyable bool 別名");
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("已核准原 Checker bool 別名不可被受限 Executor 錯拒");
        assert_eq!(plan.main_bool_locals.len(), count);
        assert_eq!(plan.main_bool_sources.len(), count);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(expected));
        assert_eq!((outcome.branch_checked,outcome.nested_branch_checked), branch);
        assert_eq!((outcome.ordinary_calls_entered,outcome.ordinary_calls_completed),
            (calls,calls));
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert_eq!((outcome.main_local_slots_after_exit,outcome.method_local_slots_after_exit),(0,0));
        assert!(outcome.events.is_empty(), "Copyable bool/int 不得偽造 OwnedStr ResourceId");
        outcome
    }

    #[test]
    fn b4_bool_alias_真Checker_const_true直接具名複製方法選中內層() {
        let src=source("const a:bool = true\nconst gate:bool = a\n",
            "owner.read(gate,true,5)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.main_bool_sources,vec![None,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1))]);
        run(&src,2,6,(Some(true),Some(true)),1);
    }

    #[test]
    fn b4_bool_alias_真Checker_var_false具名複製保留false與零helper() {
        let src=source("var initial:bool = false\nvar gate:bool = initial\n",
            "owner.read(gate,true,5)");
        run(&src,2,7,(Some(false),None),0);
    }

    #[test]
    fn b4_bool_alias_原Lexer多層Group包覆具名bool及其方法實參() {
        let src=source("const a:bool = ((true))\nconst gate:bool = (((a)))\n",
            "owner.read(((gate)),(false),5)");
        run(&src,2,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_alias_三層具名傳遞各有唯一原CheckerDeclId() {
        let src=source(
            "const a:bool = true\nconst b:bool = (a)\nconst c:bool = ((b))\n",
            "owner.read(c,false,5)",
        );
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        let ids=plan.main_bool_locals.iter().map(|(_,id,_)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(),3);
        assert_eq!(plan.main_bool_sources,vec![
            None,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)),Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[1].1)),
        ]);
        run(&src,3,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_alias_兩條獨立鏈不能錯混原Checker形參來源() {
        let src=source(
            "const a:bool = true\nconst b:bool = false\nconst yes:bool = (a)\nconst no:bool = ((b))\n",
            "owner.read(yes,no,5)",
        );
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.method_bool_sources,vec![
            Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[2].1)),
            Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[3].1)),
        ]);
        run(&src,4,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_alias_與前置main純helper交錯仍在借self前真返回() {
        let src=source(
            "const base:bool = true\nconst step:int = helper(7,2)\nconst gate:bool = (base)\n",
            "owner.read(gate,false,step)",
        );
        let out=run(&src,2,2,(Some(true),Some(false)),2);
        let first_exit=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited {returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(first_exit<loan, "main helper 必在方法 self 借票前真退出");
    }

    #[test]
    fn b4_bool_alias_已核准async直接await成功但E1延遲佇列保守拒前置別名() {
        let src=source(
            "const base:bool = true\nconst gate:bool = (base)\n",
            "await owner.read(gate,false,5)",
        ).replace("fn read(gate:bool,branch:bool,step:int)",
            "async fn read(gate:bool,branch:bool,step:int)");
        let out=run(&src,2,2,(Some(true),Some(false)),1);
        assert!(out.awaited_method);
        let mut queue=PrivateCheckedDeferredSelfAwaitQueue::new();
        assert!(matches!(queue.prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
        assert_eq!(queue.next_id,0);
        assert!(queue.jobs.is_empty());
        assert!(queue.execution_order().is_empty());
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn b4_bool_alias_準備後更改別名來源索引必零借票拒絕() {
        let src=source("const a:bool = true\nconst gate:bool = a\n",
            "owner.read(gate,true,5)");
        let mut plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        plan.main_bool_sources[1]=None;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        plan.main_bool_sources.pop();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_alias_同真值不同宣告身分不得交換別名來源() {
        let src=source(
            "const a:bool = true\nconst b:bool = true\nconst gate:bool = a\n",
            "owner.read(gate,false,5)",
        );
        let mut plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        let wrong=plan.main_bool_locals[1].1;
        plan.main_bool_sources[2]=Some(PrivateSelfBoolArgumentSource::MainLocal(wrong));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        run(&src,3,2,(Some(true),Some(false)),1);
    }

    #[test]
    fn b4_bool_alias_原Checker拒絕未知型別前置自指與未初始化別名() {
        for src in [
            source("const gate:bool = unknown\n","owner.read(gate,true,5)"),
            source("const gate:bool = gate\n","owner.read(gate,true,5)"),
            source("const number:int = 1\nconst gate:bool = number\n",
                "owner.read(gate,true,5)"),
            source("const gate:bool = true\nconst n:int = gate\n",
                "owner.read(gate,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission
            ).is_err(), "非法跨型別／前向別名應由原 Checker 靜態拒絕：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
