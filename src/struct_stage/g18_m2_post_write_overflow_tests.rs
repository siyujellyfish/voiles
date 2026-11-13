// G18／P18.6-M2：真獨占 self 寫入之後才執行選中 return 算式；
// 可能溢位的分支必按原 R05 清理，不能偽造已正常返回。
#[cfg(test)]
mod g18_m2_post_write_overflow_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
self.n = 19
return self.n+5
}else{
return 22
}
return self.n+1
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(plan.method_branch_return.is_some(),
            "true 葉加減必由原 Checker/Parser/Lexer 建立真計畫");
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty());
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_after_write_真正更新self欄位後才求值true葉加法() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(24));
        assert_eq!(out.branch_checked, Some(true));
        assert!(out.branch_write_applied);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_after_write_false不執行true葉寫入也不求值其回傳算式() {
        let out = run(&SOURCE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(22));
        assert_eq!(out.branch_checked, Some(false));
        assert!(!out.branch_write_applied);
    }

    #[test]
    fn m2_after_write_已寫入後加法真溢位必Unwind且歸還唯一獨占票據() {
        let source = SOURCE.replace(
            "return self.n+5", "return self.n+9223372036854775807",
        );
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.branch_write_applied, "溢位必須發生在真正寫入之後");
        assert_eq!(out.branch_checked, Some(true));
        assert!(!out.method_exited);
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_after_write_false不受不可達true葉溢位影響() {
        let source = SOURCE.replace("owner.read(true)", "owner.read(false)")
            .replace("return self.n+5",
                "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(22));
        assert!(!out.branch_write_applied);
        assert_eq!(out.branch_checked, Some(false));
    }

    #[test]
    fn m2_after_write_減法從真已寫欄位讀值且正確返回負整數() {
        let source = SOURCE.replace("return self.n+5", "return self.n-20");
        let out = run(&source);
        assert_eq!(out.result, Ok(-1));
        assert!(out.branch_write_applied);
        assert_eq!(out.branch_checked, Some(true));
    }

    #[test]
    fn m2_after_write_無寫入的共享self方法早返加法仍正確() {
        let source = SOURCE.replace("self.n = 19\n", "")
            .replace("return self.n+5", "return self.n+5");
        let plan = PreparedPrivateSelfMethod::prepare(&source).unwrap();
        assert!(plan.method_branch_write.is_none());
        assert_eq!(plan.loan.kind, crate::borrow_safety::BorrowKind::SharedRead);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(!out.branch_write_applied);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_after_write_已核准true葉加法計畫遭竄改必執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_branch_return,
            Some(PrivateSelfIntReturn::Add(5)));
        plan.method_branch_return = Some(PrivateSelfIntReturn::Add(100));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_after_write_原Checker拒絕不存在欄位或非int的return加法() {
        for source in [
            SOURCE.replace("return self.n+5", "return self.missing+5"),
            SOURCE.replace("return self.n+5", "return self.n+true"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "本批不准繞過原 Checker 的型別及欄位安全限制：{source}");
        }
    }
}
