// G18／P18.5-T3／V7：普通 async 函式執行入口／協作續行
// 必重驗同一原 Parser AST 和同一封存 Lexer Token。
// 來源字節與 HIR 相同不足以證明原 async 身分沒有被換掉。
#[cfg(test)]
mod g18_t3_scalar_async_frozen_token_recheck_tests {
    use super::*;

    const SOURCE: &str = concat!(
        "async fn ready(n:int)->int{\n",
        "return n+3\n}\n",
        "fn main()->int{\n",
        "return await ready(7)\n}\n",
    );

    fn checked() -> PreparedPrivateScalarCall {
        let plan = PreparedPrivateScalarCall::prepare(SOURCE)
            .expect("真原 Parser/Checker 須先核准 async fn 的直接 await");
        assert!(verified_private_source_tokens(
            SOURCE, &plan.verified.tokens,
            &plan.verified.structs, &plan.verified.functions,
        ));
        plan
    }

    fn await_index(plan: &PreparedPrivateScalarCall) -> usize {
        let main=plan.verified.functions.iter().find(|fun|fun.name=="main").unwrap();
        let StmtKind::Return(ret) = &main.body.statements[0].kind else {
            panic!("原 main 應為直接 return");
        };
        plan.verified.tokens.iter().position(|token|
            token.start == ret.expression.start
                && matches!(&token.kind,TokenKind::Name(name) if name=="await")
        ).expect("原 Lexer await Token 必須存在")
    }

    fn reject_before_runtime(plan: &PreparedPrivateScalarCall) {
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)),
            "已封存 AST/Token 遭變造必須在任何真框架前拒絕");
        assert!(matches!(PrivateScalarGraphContinuation::new(plan),
            Err(PrivateScalarCallPrepareError::SourceChanged)),
            "協作圖建立時即需拒絕，不可等進入 poll 才發現");
    }

    #[test]
    fn t3_scalar_token_原Checker普通async直接await實執行與FIFO喚醒完整通過() {
        let plan=checked();
        let outcome=plan.execute().unwrap();
        assert_eq!(outcome.result,Ok(10));
        assert!(outcome.has_balanced_frame_trace());
        let mut queue=PrivateScalarCooperativeQueue::new();
        let id=queue.submit(&plan).unwrap();
        assert_eq!(id,1);
        assert_eq!(queue.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result,Ok(10));
    }

    #[test]
    fn t3_scalar_token_封存Lexer的await前綴Name遭改即拒() {
        let mut plan=checked();
        let index=await_index(&plan);
        plan.verified.tokens[index].kind=TokenKind::Name("fake".to_owned());
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_封存Lexer的async函式被呼叫名稱遭改即拒() {
        let mut plan=checked();
        let index=await_index(&plan);
        plan.verified.tokens[index+1].kind=TokenKind::Name("other".to_owned());
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_封存Lexer呼叫左括號遭改不可啟動() {
        let mut plan=checked();
        let index=await_index(&plan);
        plan.verified.tokens[index+2].kind=TokenKind::Symbol('[');
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_封存Lexer呼叫右括號遭改不可啟動() {
        let mut plan=checked();
        let main=plan.verified.functions.iter().find(|fun|fun.name=="main").unwrap();
        let StmtKind::Return(ret)=&main.body.statements[0].kind else {panic!("原return")};
        let close=plan.verified.tokens.iter().position(|token|
            token.end==ret.expression.end
                && matches!(token.kind,TokenKind::Symbol(')'))
        ).unwrap();
        plan.verified.tokens[close].kind=TokenKind::Symbol(']');
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_封存async函式Keyword身分遭改不可建立真圖() {
        let mut plan=checked();
        let index=plan.verified.tokens.iter().position(|token|
            matches!(&token.kind,TokenKind::Keyword(word) if word=="async")
        ).expect("正式 Lexer 的 async 是 Keyword");
        plan.verified.tokens[index].kind=TokenKind::Keyword("sync".to_owned());
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_把原awaitCall_AST改成同步Call不得重播() {
        let mut plan=checked();
        let main=plan.verified.functions.iter_mut().find(|fun|fun.name=="main").unwrap();
        let StmtKind::Return(ret)=&mut main.body.statements[0].kind else {panic!("原return")};
        let TermKind::Call(call)=&mut ret.expression.kind else {panic!("原async Call")};
        assert!(call.awaited);
        call.awaited=false;
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_偷換原AST呼叫函式名稱但Token未變必拒() {
        let mut plan=checked();
        let main=plan.verified.functions.iter_mut().find(|fun|fun.name=="main").unwrap();
        let StmtKind::Return(ret)=&mut main.body.statements[0].kind else {panic!("原return")};
        let TermKind::Call(call)=&mut ret.expression.kind else {panic!("原async Call")};
        call.name="forged".to_owned();
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_偷換封存AST函式async宣告旗標不得偽造同步圖() {
        let mut plan=checked();
        plan.verified.functions.iter_mut()
            .find(|fun|fun.name=="ready").unwrap().is_async=false;
        reject_before_runtime(&plan);
    }

    #[test]
    fn t3_scalar_token_工作提交時非法原Token不得消耗ID或ready授權() {
        let mut tampered=checked();
        let index=await_index(&tampered);
        tampered.verified.tokens[index].kind=TokenKind::Name("fake".to_owned());
        let mut queue=PrivateScalarCooperativeQueue::new();
        assert_eq!(queue.submit(&tampered),Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.next_id,0);
        assert!(queue.ready_ids().is_empty());
        assert!(queue.drive_order().is_empty());
        let valid=checked();
        let id=queue.submit(&valid).unwrap();
        assert_eq!(id,1);
        assert_eq!(queue.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.status(id),Some(PrivateDeferredAwaitStatus::Suspended));
    }
}
