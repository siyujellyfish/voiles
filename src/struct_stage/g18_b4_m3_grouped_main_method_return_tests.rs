// G18／B4/M3／T3：main 原 Checker 已核准的 Group( MethodCall )
// 與 Group( AwaitMethodCall )；只有真葉節點可承擔自動借票及
// 原 ScalarHIR call_span，括號必逐層核對同一原 Lexer。
#[cfg(test)]
mod g18_b4_m3_grouped_main_method_return_tests {
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
        ).expect("原 Checker 應核准合法括號包覆的真同步方法／直接await");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("私有 Executor 應接受原 Checker Group 值葉與借票跨度")
    }

    fn run(src: &str, expected: i64, calls: usize,
        branch: (Option<bool>,Option<bool>))
        -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        let main=plan.verified.functions.iter().find(|fun|fun.name=="main").unwrap();
        let returned=match &main.body.statements.last().unwrap().kind {
            StmtKind::Return(r)=>r,
            _=>panic!("原Checker須真return"),
        };
        let leaf=private_self_checked_ungrouped_return(
            &plan.verified,&returned.expression,
        ).unwrap();
        assert_eq!(plan.call_span,leaf.node_span());
        assert_ne!(plan.call_span,returned.expression.node_span(),
            "此項必使用括號外殼，而呼叫借票仍用內葉來源");
        assert!(plan.verified.plan.scalar_hir.expression(plan.call_span).is_some());
        assert_eq!(plan.verified.plan.declarations.call_borrows_at(plan.call_span),
            Some(&[plan.loan][..]));
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),
            (calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(out.origin.call_span,plan.call_span);
        assert!(out.events.is_empty(),"Copyable純整數方法不可偽造OwnedStr");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_grouped_main_原Checker單層括號方法直接回傳真內葉() {
        let src=source("","(owner.read(true,true,5))");
        run(&src,6,1,(Some(true),Some(true)));
    }

    #[test]
    fn b4_grouped_main_原Checker二層括號方法Return仍只一個self票據() {
        let src=source("","((owner.read(true,false,5)))");
        let out=run(&src,2,1,(Some(true),Some(false)));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_grouped_main_多層括號false分支不憑字面值偷選helper() {
        let src=source("","(((owner.read(false,true,5))))");
        run(&src,7,0,(Some(false),None));
    }

    #[test]
    fn b4_grouped_main_原Checker具名bool與int真來源仍保留() {
        let src=source(
            "const gate:bool = true\nvar step:int = 3\nstep = helper(9,4)\n",
            "((owner.read((gate),false,(step))))",
        );
        let out=run(&src,2,2,(Some(true),Some(false)));
        let helper=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(helper<loan);
    }

    #[test]
    fn b4_grouped_main_原Checker空宣告後賦值再呼叫Group不錯讀佔位() {
        let src=source(
            "var gate:bool\nvar step:int\ngate = false\nstep = 5\n",
            "(owner.read(gate,true,step))",
        );
        run(&src,7,0,(Some(false),None));
    }

    #[test]
    fn b4_grouped_main_原Checker非公開async直接await括號仍有真安全點() {
        let src=source("","((await owner.read(true,false,5)))")
            .replace("fn read(gate:bool,branch:bool,step:int)",
                     "async fn read(gate:bool,branch:bool,step:int)");
        let out=run(&src,2,1,(Some(true),Some(false)));
        assert!(out.awaited_method);
        assert!(out.await_start_loan_index.is_some());
        assert_eq!(out.origin.call_span,
            checked(&src).call_span);
    }

    #[test]
    fn b4_grouped_main_原Checkerasync直接await有前置主函式局部仍禁止延遲FIFO() {
        let src=source(
            "var gate:bool\ngate = true\n",
            "((await owner.read(gate,false,5)))",
        ).replace("fn read(gate:bool,branch:bool,step:int)",
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
    fn b4_grouped_main_原Checker不合法括號與缺失方法靜態拒絕() {
        for src in [
            source("","(owner.read(true,true,5)"),
            source("","owner.read(true,true,5))"),
            source("","((owner.missing(true,true,5)))"),
            source("","((owner.read(3,true,5)))"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker不得認可不合法的括號或方法：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn b4_grouped_main_準備後callspan改成外括號不可冒充借票來源() {
        let src=source("","((owner.read(true,true,5)))");
        let mut plan=checked(&src);
        let main=plan.verified.functions.iter().find(|fun|fun.name=="main").unwrap();
        let outer=match &main.body.statements.last().unwrap().kind {
            StmtKind::Return(r)=>r.expression.node_span(),
            _=>panic!("錯誤來源"),
        };
        assert_ne!(outer,plan.call_span);
        plan.call_span=outer;
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_grouped_main_非方法Return不能靠Group隱瞞錯誤執行器路徑() {
        let src=source("","((helper(9,4)))");
        verify_source_with_ability(
            &src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker仍允許普通函式同步int返回");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }
}
