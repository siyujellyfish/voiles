// G18／P18.6-M2／P18.4-B4：原 Checker Copyable int
// 純加減作同步 self.int 寫入 RHS。動態求值必在唯一
// ExclusiveWrite BorrowTicket 有效時，溢位不得假寫欄位。
#[cfg(test)]
mod g18_m2_self_checked_int_write_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var count:int = 7
fn set(n:int)->int{
self.count = n+3
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
            .expect("只准原 Parser／Checker int 算術與真自動 self BorrowFact");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        assert!(plan.write_checked_int.is_some());
        assert!(plan.write_value.is_none() && plan.write_parameter.is_none());
        let out=plan.execute().expect("真欄位執行／退出須使用原 HC14／P16");
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"Copyable int 不可虛構 OwnedStr ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn m2_checked_write_真int參數加常值後只在持獨占票時寫入12() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(12));
        assert!(out.method_entered && out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn m2_checked_write_兩int真DeclId彼此加法後寫入16() {
        let src=SOURCE.replace("fn set(n:int)->int{\nself.count = n+3",
            "fn set(left:int,right:int)->int{\nself.count = left+right")
            .replace("owner.set(9)","owner.set(9,7)");
        assert_eq!(run(&src).result,Ok(16));
    }

    #[test]
    fn m2_checked_write_嵌套左右減法仍依原Checker有號i64求值() {
        let src=SOURCE.replace("self.count = n+3","self.count = (n+3)-(2+1)");
        assert_eq!(run(&src).result,Ok(9));
    }

    #[test]
    fn m2_checked_write_main先真普通helper求參再寫入self欄位() {
        let src=SOURCE.replace("fn main()->int{",
            "fn helper(n:int)->int{\nreturn n+1\n}\nfn main()->int{")
            .replace("owner.set(9)","owner.set(helper(8))");
        let out=run(&src);
        assert_eq!(out.result,Ok(12));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn m2_checked_write_真int加法溢位不得假寫self並先退出真借票() {
        let src=SOURCE.replace("self.count = n+3",
            "self.count = n+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned);
        assert!(out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_checked_write_真int減法下溢受控退票可重複執行合法原計畫() {
        let src=SOURCE.replace("self.count = n+3",
            "self.count = (n-9223372036854775807)-2")
            .replace("owner.set(9)","owner.set(0)");
        let plan=PreparedPrivateSelfMethod::prepare(&src).unwrap();
        for _ in 0..2 {
            let out=plan.execute().unwrap();
            assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
            assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        }
    }

    #[test]
    fn m2_checked_write_未核准來源不得繞過Checker與借用安全() {
        for src in [
            SOURCE.replace("self.count = n+3","self.count = n+true"),
            SOURCE.replace("self.count = n+3","self.count = other+3"),
            SOURCE.replace("var owner:Counter","const owner:Counter"),
            SOURCE.replace("self.count = n+3","self.count = self.count+3"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "缺少原 Checker 合法純 int RHS／獨占票安全不得放行：{src}");
        }
    }

    #[test]
    fn m2_checked_write_同值造假算式或夾帶動態寫入來源必於執行前拒絕() {
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.write_checked_int=Some(PrivateScalarReturnExpr::Constant(12));
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.write_parameter=plan.argument_binding.map(|(id,_)|id);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
