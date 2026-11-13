// G18／P18.6-M2：原 Checker 同步 self 方法頂層 else 單 Return 真欄位與加減。
// 一張真 BorrowTicket；未選葉不求值；Checked ReturnSpan 與原 Lexer 運算子防偽。
#[cfg(test)]
mod g18_m2_direct_else_self_return_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
return self.n+5
}else{
return self.n+3
}
return self.n+1
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(false)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert!(plan.method_branch_else_write.is_none());
        assert!(plan.method_branch_else_field_return.is_some());
        assert!(plan.method_branch_else_calculation.is_some());
        let result = plan.execute().unwrap();
        assert!(result.has_balanced_self_borrow_trace());
        assert!(result.ticket_returned);
        assert_eq!(result.main_local_slots_after_exit, 0);
        assert_eq!(result.method_local_slots_after_exit, 0);
        assert!(result.events.is_empty(), "單純 int 返回不應建立 UTF-8 資源");
        result
    }

    #[test]
    fn m2_direct_else_self_動態false原Checker欄位加法返回10() {
        let result = run(SOURCE);
        assert_eq!(result.result, Ok(10));
        assert_eq!(result.branch_checked, Some(false));
        assert_eq!(result.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_direct_else_self_動態true只計算原Checker真葉加法12() {
        let result = run(&SOURCE.replace("owner.read(false)", "owner.read(true)"));
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.branch_checked, Some(true));
    }

    #[test]
    fn m2_direct_else_self_動態false直接返回同張票據讀取的欄位7() {
        let result = run(&SOURCE.replace("return self.n+3", "return self.n"));
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.branch_checked, Some(false));
    }

    #[test]
    fn m2_direct_else_self_動態false原Checker減法返回負13() {
        let result = run(&SOURCE.replace("return self.n+3", "return self.n-20"));
        assert_eq!(result.result, Ok(-13));
        assert_eq!(result.branch_checked, Some(false));
    }

    #[test]
    fn m2_direct_else_self_真正抵達else溢位必Unwind不假Returned仍退票() {
        let result = run(&SOURCE.replace(
            "return self.n+3", "return self.n+9223372036854775807",
        ));
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_direct_else_self_動態true完全跳過else必溢位算式() {
        let source = SOURCE.replace("owner.read(false)", "owner.read(true)")
            .replace("return self.n+3", "return self.n+9223372036854775807");
        let result = run(&source);
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.branch_checked, Some(true));
    }

    #[test]
    fn m2_direct_else_self_已編else計算篡改拒絕且入口故障無假返回() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_branch_else_calculation,
            Some(PrivateSelfIntReturn::Add(3)));
        plan.method_branch_else_calculation = Some(PrivateSelfIntReturn::Add(4));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_branch_else_field_return = None;
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let result = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(result.branch_checked, None);
        assert!(result.ticket_returned);
        assert!(result.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_direct_else_self_錯型別與錯欄位由原Checker拒絕() {
        for source in [
            SOURCE.replace("return self.n+3", "return self.n+true"),
            SOURCE.replace("return self.n+3", "return self.missing+3"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "非法 else 返回不得跳過原 Checker：{source}");
        }
    }
}
