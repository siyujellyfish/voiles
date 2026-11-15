// G18／P18.5-T3／V7-A1：執行前以真來源 Lexer Token
// 重新對帳已封存 async 方法直接 await AST；不可只核對
// 文字和 HIR 快照，忽略動態計畫內的符號／詞彙身份。
#[cfg(test)]
mod g18_t3_async_self_token_provenance_recheck_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
async fn read()->int{
return self.n+3
}
}
fn main()->int{
var owner:Counter = Counter()
return await owner.read()
}
"#;

    fn checked() -> PreparedPrivateSelfMethod {
        let plan = PreparedPrivateSelfMethod::prepare(SOURCE)
            .expect("原 Checker 必先簽發合法 async self 直接 await");
        assert!(plan.awaited_method);
        assert!(verified_private_source_tokens(
            SOURCE, &plan.verified.tokens,
            &plan.verified.structs, &plan.verified.functions,
        ));
        plan
    }

    fn rejected(plan: PreparedPrivateSelfMethod) {
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)),
            "任何已封存 Lexer/Parser/Checker 身分不一致必須在BorrowTicket前拒絕");
    }

    fn token_index(plan: &PreparedPrivateSelfMethod) -> usize {
        plan.verified.tokens.iter().position(|token|
            token.start == plan.call_span.start
                && matches!(&token.kind, TokenKind::Name(name) if name == "await")
        ).expect("原Lexer真await Token")
    }

    #[test]
    fn t3_async_token_原Checker六相鄰詞彙及同步舊來源均可真執行() {
        let plan = checked();
        let out = plan.execute().unwrap();
        assert_eq!(out.result,Ok(10));
        assert!(out.await_start_loan_index.is_some());
        assert!(out.has_balanced_self_borrow_trace());
        let old = SOURCE.replace("async fn read()","fn read()")
            .replace("return await owner.read()","return owner.read()");
        let sync = PreparedPrivateSelfMethod::prepare(&old).unwrap().execute().unwrap();
        assert!(!sync.awaited_method);
        assert_eq!(sync.result,Ok(10));
        assert!(sync.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_async_token_偷改await前綴Name不得當成合法等待來源() {
        let mut plan = checked();
        let start=token_index(&plan);
        plan.verified.tokens[start].kind = TokenKind::Name("fake".to_owned());
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改接收者Token雖原AST未改也須拒絕() {
        let mut plan = checked();
        let start=token_index(&plan);
        plan.verified.tokens[start+1].kind = TokenKind::Name("other".to_owned());
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改點號Token不得越過方法安全Checker() {
        let mut plan = checked();
        let start=token_index(&plan);
        plan.verified.tokens[start+2].kind = TokenKind::Symbol('!');
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改方法名Token但TypedHIR不變仍須拒絕() {
        let mut plan = checked();
        let start=token_index(&plan);
        plan.verified.tokens[start+3].kind = TokenKind::Name("set".to_owned());
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改左括號Token不得認領真位置參數() {
        let mut plan = checked();
        let start=token_index(&plan);
        plan.verified.tokens[start+4].kind = TokenKind::Symbol('[');
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改尾括號Token但返回跨度相同仍須拒絕() {
        let mut plan = checked();
        let last=plan.verified.tokens.iter().position(|token|
            token.end == plan.call_span.end
                && matches!(token.kind,TokenKind::Symbol(')'))
        ).expect("原await方法最後右括號");
        plan.verified.tokens[last].kind = TokenKind::Symbol(']');
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷改方法async宣告Token也不能忽略原來源() {
        let mut plan = checked();
        let index=plan.verified.tokens.iter().position(|token|
            matches!(&token.kind,TokenKind::Name(word) if word=="async")
        ).expect("原async fn頭的Lexer身分");
        plan.verified.tokens[index].kind = TokenKind::Name("sync".to_owned());
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷換AST接收者但原Lexer文字不變必拒() {
        let mut plan = checked();
        let main=plan.verified.functions.iter_mut().find(|f|f.name=="main").unwrap();
        let StmtKind::Return(returned)=&mut main.body.statements.last_mut().unwrap().kind else {
            panic!("原main直接Return");
        };
        let TermKind::AwaitMethodCall(receiver,_,_)=&mut returned.expression.kind else {
            panic!("原Parser必有AwaitMethodCall");
        };
        *receiver="other".to_string();
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷換AST方法名但位置與票據不變必拒() {
        let mut plan = checked();
        let main=plan.verified.functions.iter_mut().find(|f|f.name=="main").unwrap();
        let StmtKind::Return(returned)=&mut main.body.statements.last_mut().unwrap().kind else {
            panic!("原main直接Return");
        };
        let TermKind::AwaitMethodCall(_,method,_)=&mut returned.expression.kind else {
            panic!("原Parser必有AwaitMethodCall");
        };
        *method="forged".to_string();
        rejected(plan);
    }

    #[test]
    fn t3_async_token_偷抹await_AST身分改同步MethodCall必拒絕() {
        let mut plan = checked();
        let main=plan.verified.functions.iter_mut().find(|f|f.name=="main").unwrap();
        let StmtKind::Return(returned)=&mut main.body.statements.last_mut().unwrap().kind else {
            panic!("原main直接Return");
        };
        let TermKind::AwaitMethodCall(receiver,method,args)=&returned.expression.kind else {
            panic!("原Parser必有AwaitMethodCall");
        };
        returned.expression.kind =
            TermKind::MethodCall(receiver.clone(),method.clone(),args.clone());
        rejected(plan);
    }
}
