// G18／P18.5-T3／R05：真 Lexer／Parser／Checker 方法與普通函式
// 動態事件之間，外層 self BorrowTicket 不得因孫層函式
// Returned／Unwind 而提前歸還。與原 HC14 身分和 SharedExit 共帳。
#[cfg(test)]
mod g18_r05_t3_outer_ticket_grandchild_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;

    const READ: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return seed()+2
}
}
fn seed()->int{
return 14
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    const WRITE: &str = r#"struct Counter{
var n:int = 7
fn set(n:int)->int{
self.n = add(n,3)
return self.n
}
}
fn add(a:int,b:int)->int{
return a+b
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(9)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("只用原 Checker 同步 self 與純 int helper 正式核准來源");
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"Copyable int 不得偽造資源");
        out
    }

    fn loan_moved_before_helper_exit(
        events: &[E], helper_index: usize,
    ) -> Vec<E> {
        let mut trace=events.to_vec();
        let loan_at=trace.iter().position(|e|matches!(
            e,E::LoanReturned(_)
        )).expect("原 self 方法應有唯一歸還");
        let loan=trace.remove(loan_at);
        let helper_exit_at=trace.iter().enumerate().filter_map(|(i,e)|
            matches!(e,E::FunctionExited{..}).then_some(i)
        ).nth(helper_index).expect("原真 helper 必須已入／退出");
        assert!(trace[..helper_exit_at].iter().any(|e|matches!(
            e,E::FunctionEntered
        )));
        trace.insert(helper_exit_at,loan);
        trace
    }

    #[test]
    fn t3_grandchild_原Checker唯讀self方法真helper完成後才歸唯一票() {
        let out=run(READ);
        assert_eq!(out.result,Ok(16));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(private_shared_call_exit_balanced(&out.exit_events,true));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,E::LoanReturned(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,E::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn t3_grandchild_正常helper已Returned但尚未Exited不得先還外self票() {
        let out=run(READ);
        let wrong=loan_moved_before_helper_exit(&out.exit_events,0);
        assert_eq!(wrong.len(),out.exit_events.len());
        assert_eq!(wrong.iter().filter(|e|matches!(e,E::LoanReturned(_))).count(),1);
        assert!(!private_shared_call_exit_balanced(&wrong,true));
        assert!(private_shared_call_exit_balanced(&out.exit_events,true));
    }

    #[test]
    fn t3_grandchild_真內層溢位已Unwind但尚未Exited不能預退self票() {
        let src=READ.replace("return 14","return 9223372036854775807+1");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited);
        let wrong=loan_moved_before_helper_exit(&out.exit_events,0);
        assert_eq!(wrong.iter().filter(|e|matches!(e,E::UnwindStarted)).count(),1);
        assert!(!private_shared_call_exit_balanced(&wrong,false));
        assert!(private_shared_call_exit_balanced(&out.exit_events,false));
    }

    #[test]
    fn t3_grandchild_雙個真純helper第二個Returned後仍不得提前還self票() {
        let src=READ.replace("return seed()+2","return seed()+other()")
            .replace("fn main()->int{",
                "fn other()->int{\nreturn 2\n}\nfn main()->int{");
        let out=run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(2,2));
        let wrong=loan_moved_before_helper_exit(&out.exit_events,1);
        assert!(!private_shared_call_exit_balanced(&wrong,true));
        assert!(private_shared_call_exit_balanced(&out.exit_events,true));
    }

    #[test]
    fn t3_grandchild_獨占self欄位寫入純helper結果也不能提前退票() {
        let out=run(WRITE);
        assert_eq!(out.result,Ok(12));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let wrong=loan_moved_before_helper_exit(&out.exit_events,0);
        assert!(!private_shared_call_exit_balanced(&wrong,true));
        assert!(private_shared_call_exit_balanced(&out.exit_events,true));
    }

    #[test]
    fn t3_grandchild_沒有子helper的真self方法在自身Returned後歸票合法() {
        let src=READ.replace("return seed()+2","return self.n+2")
            .replace("fn seed()->int{\nreturn 14\n}\n","");
        let out=run(&src);
        assert_eq!(out.result,Ok(9));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(private_shared_call_exit_balanced(&out.exit_events,true));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,E::LoanReturned(_)
        )).count(),1);
    }
}
