// G18／P18.6-M2／P18.4-B4：原 Lexer／Parser／Checker
// Copyable int 方法形參在**唯一獨占 self BorrowTicket**存活期
// 才可按原 DeclarationId 寫真 MixedStructRuntime int 欄位。
// 不把 main 的同名局部誤當方法參數，也不於 prepare 預寫值。
#[cfg(test)]
mod g18_m2_self_int_parameter_field_write_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var count:int = 7
fn set(n:int)->int{
self.count = n
return self.count
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(9)
}
"#;

    fn run(source:&str)->PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("方法參數／欄位寫入僅得由原 Checker 的真 BorrowFact 核准");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.method_entered && out.method_exited);
        assert!(out.events.is_empty(),"純 int 欄位不可假建 P16 ResourceId");
        out
    }

    #[test]
    fn m2_param_write_真Checker_int形參動態值9實際寫入self欄位() {
        let plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert!(plan.write_value.is_none());
        assert_eq!(plan.write_parameter,plan.argument_binding.map(|(id,_)|id));
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(9));
    }

    #[test]
    fn m2_param_write_main原具名int局部先求值才作方法實參寫欄位() {
        let src=SOURCE.replace("return owner.set(9)",
            "const payload:int = 13\nreturn owner.set(payload)");
        let out=run(&src);
        assert_eq!(out.result,Ok(13));
        assert_eq!(out.main_local_slots_after_exit,0);
    }

    #[test]
    fn m2_param_write_兩個不同DeclId形參僅允許真正right寫欄位() {
        let src=SOURCE.replace("fn set(n:int)->int{\nself.count = n",
            "fn set(left:int,right:int)->int{\nself.count = right")
            .replace("owner.set(9)","owner.set(4,19)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.write_parameter,plan.additional_argument_bindings[0..1]
            .first().map(|(id,_)|*id));
        let out=run(&src);
        assert_eq!(out.result,Ok(19));
    }

    #[test]
    fn m2_param_write_原bool與int混合宣告只認int對應的形參欄位來源() {
        let src=SOURCE.replace("fn set(n:int)->int{\nself.count = n",
            "fn set(flag:bool,n:int)->int{\nself.count = n")
            .replace("owner.set(9)","owner.set(true,15)");
        let out=run(&src);
        assert_eq!(out.result,Ok(15));
        assert_eq!(out.branch_checked,None);
    }

    #[test]
    fn m2_param_write_原Checker多次執行每次自動取得新真self票並寫同源值() {
        let plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        for _ in 0..3 {
            let out=plan.execute().unwrap();
            assert_eq!(out.result,Ok(9));
            assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
            assert_eq!(out.exit_events.iter().filter(|e|matches!(
                e,PrivateSharedCallExitEvent::LoanAcquired(_)
            )).count(),1);
            assert_eq!(out.exit_events.iter().filter(|e|matches!(
                e,PrivateSharedCallExitEvent::LoanReturned(_)
            )).count(),1);
        }
    }

    #[test]
    fn m2_param_write_原本Lexer_int常值直接寫入路徑未退步() {
        let src=SOURCE.replace("self.count = n","self.count = 42");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        assert_eq!(plan.write_value,Some(42));
        assert_eq!(plan.write_parameter,None);
        assert_eq!(run(&src).result,Ok(42));
    }

    #[test]
    fn m2_param_write_真方法入口注入故障不得假寫入且須歸還獨占票() {
        let plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let failure=plan.execute_with_fault(true).unwrap();
        assert_eq!(failure.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failure.method_entered && !failure.method_exited);
        assert!(failure.ticket_returned && failure.has_balanced_self_borrow_trace());
        assert_eq!(plan.execute().unwrap().result,Ok(9));
    }

    #[test]
    fn m2_param_write_非法型別const接收者錯名與竄改原DeclId一律拒絕() {
        for src in [
            SOURCE.replace("self.count = n","self.count = true"),
            SOURCE.replace("self.count = n","self.count = unknown"),
            SOURCE.replace("var owner:Counter","const owner:Counter"),
            SOURCE.replace("fn set(n:int)->int","fn set(n:bool)->int"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ),"原 Checker 沒有授權的欄位寫入應先拒絕：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.write_parameter=Some(DeclarationId(0));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.write_value=Some(999);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
