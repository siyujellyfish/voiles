// G18／P18.4-B4／P18.6-M2/M3：原 Checker 已核准 bool Group，
// 私有方法執行器不得只因 Parser 外層為 Group 而錯拒。
// 所有字面／參數身分仍來自唯一 Lexer／Checker，不另建轉型權限。
#[cfg(test)]
mod g18_m2_m3_grouped_bool_method_tests {
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

    fn checked(source: &str, booleans: usize) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 應允許合法 Group bool 引數與條件");
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("原 Checker 已允許的 Group bool 不得被受限執行器錯拒");
        assert_eq!(plan.method_bool_arguments.len(), booleans);
        let ids = plan.method_bool_arguments.iter().map(|(id, _)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), booleans, "每份 bool 都要使用原 Checker 唯一 DeclarationId");
        plan
    }

    fn run(source: &str, expected: i64, branch: (Option<bool>, Option<bool>),
        calls: (usize, usize)) -> PrivateSelfMethodOutcome {
        let plan = checked(source, 2);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(expected));
        assert_eq!((outcome.branch_checked, outcome.nested_branch_checked), branch);
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), calls);
        assert!(outcome.ticket_returned && outcome.has_balanced_self_borrow_trace());
        assert_eq!((outcome.main_local_slots_after_exit, outcome.method_local_slots_after_exit), (0, 0));
        assert!(outcome.events.is_empty(), "純 bool/int 不得假造 P16 資源事件");
        outcome
    }

    #[test]
    fn m2_group_bool_兩個原Checker括號實參真選中內層helper() {
        run(&SOURCE.replace("owner.read(true,true,5)", "owner.read((true),((true)),5)"),
            6, (Some(true), Some(true)), (1, 1));
    }

    #[test]
    fn m2_group_bool_內層false括號實參只跑真else函式() {
        run(&SOURCE.replace("owner.read(true,true,5)", "owner.read(((true)),(false),5)"),
            2, (Some(true), Some(false)), (1, 1));
    }

    #[test]
    fn m2_group_bool_外層false括號實參不進入任何子函式() {
        run(&SOURCE.replace("owner.read(true,true,5)", "owner.read(((false)),(true),5)"),
            7, (Some(false), None), (0, 0));
    }

    #[test]
    fn m2_group_bool_外層Group條件包裹真Checker布林形參() {
        let src = SOURCE.replace("if(gate){", "if(((gate))){");
        run(&src, 6, (Some(true), Some(true)), (1, 1));
        run(&src.replace("owner.read(true,true,5)", "owner.read(false,true,5)"),
            7, (Some(false), None), (0, 0));
    }

    #[test]
    fn m2_group_bool_內層Group條件仍使用原Checker獨立DeclId() {
        let src = SOURCE.replace("if(branch){", "if(((branch))){");
        run(&src, 6, (Some(true), Some(true)), (1, 1));
        run(&src.replace("owner.read(true,true,5)", "owner.read(true,false,5)"),
            2, (Some(true), Some(false)), (1, 1));
    }

    #[test]
    fn m2_group_bool_內外層Group常值條件與混合Group實參共同真執行() {
        let src = SOURCE.replace("if(gate){", "if((true)){")
            .replace("if(branch){", "if(((false))){")
            .replace("owner.read(true,true,5)", "owner.read(((true)),(true),5)");
        run(&src, 2, (Some(true), Some(false)), (1, 1));
    }

    #[test]
    fn m2_group_bool_重排bool_int形參及Group條件仍維持正確原Lexer位置() {
        let src = SOURCE.replace(
            "fn read(gate:bool,branch:bool,step:int)->int",
            "fn read(step:int,branch:bool,gate:bool)->int",
        ).replace("owner.read(true,true,5)", "owner.read((5+0),((false)),(true))")
            .replace("if(gate){", "if((gate)){")
            .replace("if(branch){", "if(((branch))){");
        run(&src, 2, (Some(true), Some(false)), (1, 1));
    }

    #[test]
    fn m2_group_bool_async_self直接await原FIFO保留唯一真借票() {
        let src = SOURCE.replace("fn read(gate:bool,branch:bool,step:int)",
                "async fn read(gate:bool,branch:bool,step:int)")
            .replace("if(gate){", "if((gate)){")
            .replace("if(branch){", "if(((branch))){")
            .replace("return owner.read(true,true,5)",
                "return await owner.read((true),((false)),5)");
        let mut queue = PrivateCheckedDeferredSelfAwaitQueue::new();
        let id = queue.prepare(&src).expect("原 Checker核准 async self Group bool 直接 await");
        queue.suspend(id).unwrap();
        let (_, outcome) = queue.drive_next().unwrap().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert!(outcome.awaited_method && outcome.ticket_returned);
        assert!(outcome.has_balanced_self_borrow_trace());
        assert_eq!((outcome.ordinary_calls_entered, outcome.ordinary_calls_completed), (1,1));
        queue.verify_ready_history().unwrap();
    }

    #[test]
    fn m2_group_bool_錯型別Group引數與未證名詞不得因解殼而放行() {
        for src in [
            SOURCE.replace("owner.read(true,true,5)", "owner.read((5),true,5)"),
            SOURCE.replace("owner.read(true,true,5)", "owner.read(true,(5),5)"),
            SOURCE.replace("if(branch){", "if(((step))){"),
            SOURCE.replace("if(gate){", "if(((unknown))){"),
        ] {
            assert!(verify_source_with_ability(
                &src, crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(), "原 Checker 必須拒絕錯型別或未知 bool 條件：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))),
                "不能由 Executor 解殼重新核准非法 bool 來源");
        }
    }

    #[test]
    fn m2_group_bool_準備後原Lexer括號遭改寫執行前不得進框架() {
        let source = SOURCE.replace(
            "owner.read(true,true,5)", "owner.read(((true)),(false),5)",
        );
        let mut plan = checked(&source, 2);
        let opening = source.find("(((true))").expect("原合法 bool 括號的唯一起點");
        let token = plan.verified.tokens.iter_mut().find(|tok|
            tok.start == opening && matches!(tok.kind, TokenKind::Symbol('('))
        ).expect("必有原 Lexer 真括號 token");
        token.kind = TokenKind::Symbol('[');
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
