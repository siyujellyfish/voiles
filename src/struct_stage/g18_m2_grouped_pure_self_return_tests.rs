// G18／P18.6-M2／P18.4-B4：原 Checker 已核准的 Group
// 純值方法返回不能被真執行器錯拒；每一層括號只認原 Lexer
// 真 token，self BorrowTicket、R05 與型別權威不變。
#[cfg(test)]
mod g18_m2_grouped_pure_self_return_tests {
    use super::*;

    const READ: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    const PARAM: &str = r#"struct Counter{
var n:int = 7
fn read(p:int)->int{
return p
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(12)
}
"#;

    const PAIR: &str = r#"struct Counter{
var n:int = 7
fn read(p:int,q:int)->int{
return p+q
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(12,5)
}
"#;

    const WRITE: &str = r#"struct Counter{
var n:int = 7
fn set(p:int)->int{
self.n=p+2
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(12)
}
"#;

    fn checked(code: &str) -> PreparedPrivateSelfMethod {
        // 原 Checker 已經正式接受此 .voil 來源。
        verify_source_with_ability(
            code, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker須核准真Group方法返回");
        PreparedPrivateSelfMethod::prepare(code)
            .expect("原Checker核准返回式應有真BorrowTicket與動態工作計畫")
    }

    fn result(code: &str, expected: Result<i64, PrivateSelfMethodRuntimeError>) {
        let outcome=checked(code).execute().unwrap();
        assert_eq!(outcome.result,expected);
        assert!(outcome.method_entered && outcome.ticket_returned);
        assert!(outcome.has_balanced_self_borrow_trace());
        assert!(outcome.events.is_empty());
        assert_eq!((outcome.main_local_slots_after_exit,
                    outcome.method_local_slots_after_exit),(0,0));
    }

    #[test]
    fn m2_grouped_self_單層括號self欄位由原Checker共享票據真返回() {
        result(&READ.replace("return self.n","return (self.n)"),Ok(7));
    }

    #[test]
    fn m2_grouped_self_多層括號self欄位不能被誤判為未核准MethodCall() {
        result(&READ.replace("return self.n","return (((self.n)))"),Ok(7));
    }

    #[test]
    fn m2_grouped_self_括號Copyable方法參數用真DeclarationId取值() {
        result(&PARAM.replace("return p","return (p)"),Ok(12));
    }

    #[test]
    fn m2_grouped_self_多層方法參數回傳不讀取同名owner來源() {
        result(&PARAM.replace("return p","return (((p)))"),Ok(12));
    }

    #[test]
    fn m2_grouped_self_真Checker常值Return七與多層括號同一動態退出() {
        result(&READ.replace("return self.n","return 7"),Ok(7));
        result(&READ.replace("return self.n","return ((7))"),Ok(7));
    }

    #[test]
    fn m2_grouped_self_括號包覆整份self欄位加三保留原算術操作() {
        result(&READ.replace("return self.n","return (self.n+3)"),Ok(10));
        result(&READ.replace("return self.n","return ((self.n+3))"),Ok(10));
    }

    #[test]
    fn m2_grouped_self_括號包覆二元左self欄位仍須真HC14借票() {
        result(&READ.replace("return self.n","return (self.n)+3"),Ok(10));
    }

    #[test]
    fn m2_grouped_self_右端括號self欄位由原int參數加總() {
        result(&PARAM.replace("return p","return p+(self.n)"),Ok(19));
    }

    #[test]
    fn m2_grouped_self_兩個括號方法int參數保持各自真來源() {
        result(&PAIR.replace("return p+q","return (p)+(q)"),Ok(17));
    }

    #[test]
    fn m2_grouped_self_多層括號右整數常值由原Lexer驗證() {
        result(&PARAM.replace("return p","return ((p))+(3)"),Ok(15));
    }

    #[test]
    fn m2_grouped_self_獨占self寫入後括號讀取必用同張已核准BorrowTicket() {
        let src=WRITE.replace("return self.n","return ((self.n))");
        let plan=checked(&src);
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        result(&src,Ok(14));
    }

    #[test]
    fn m2_grouped_self_非同步self直接await方法也能真返回Group欄位() {
        let src=READ.replace("fn read()","async fn read()")
            .replace("return self.n","return ((self.n))")
            .replace("return owner.read()","return await owner.read()");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=q.prepare(&src).expect("原Checker async self直接await");
        q.suspend(id).unwrap();
        let (actual,out)=q.drive_next().unwrap().unwrap();
        assert_eq!(actual,id);
        assert_eq!(out.result,Ok(7));
        assert!(out.awaited_method && out.has_balanced_self_borrow_trace());
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn m2_grouped_self_括號純self整數運算溢位先Unwind再歸唯一票據() {
        let src=READ.replace("return self.n",
            "return ((self.n)+9223372036854775807)");
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.ticket_returned && !out.method_exited);
        assert!(out.has_balanced_self_borrow_trace());
        let unwind=out.exit_events.iter().position(|event|
            matches!(event,PrivateSharedCallExitEvent::UnwindStarted)).unwrap();
        let returned=out.exit_events.iter().position(|event|
            matches!(event,PrivateSharedCallExitEvent::LoanReturned(_))).unwrap();
        assert!(unwind<returned);
    }

    #[test]
    fn m2_grouped_self_錯誤返回型別與禁止消耗self仍須原Checker靜態拒絕() {
        for src in [
            READ.replace("return self.n","return (true)"),
            READ.replace("return self.n","return ((self))"),
            READ.replace("return self.n","return (self.n.move())"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker不得因Group放行不安全方法：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn m2_grouped_self_執行前括號原LexerToken被偷改不得沿用舊安全計畫() {
        let src=READ.replace("return self.n","return ((self.n))");
        let mut plan=checked(&src);
        let pos=plan.verified.tokens.iter().position(|token|
            matches!(token.kind,TokenKind::Symbol('('))
                && token.start>=plan.method_span.start
                && token.start<plan.method_span.end
        ).unwrap();
        plan.verified.tokens[pos].kind=TokenKind::Symbol('[');
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
