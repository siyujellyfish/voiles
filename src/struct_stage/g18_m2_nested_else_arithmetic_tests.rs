// G18／P18.6-M2：原 Checker 真巢狀 else 分支 Copyable self.int 加減與 Unwind。
// 僅實際到達的內層 return 可求值，原 BorrowSafety 唯一票據對稱歸還。
#[cfg(test)]
mod g18_m2_nested_else_arithmetic_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(true){
if(flag){
return self.n+5
}else{
return self.n+3
}
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
        assert!(plan.method_nested_branch.is_some());
        assert!(plan.method_nested_else_calculation.is_some());
        let outcome = plan.execute().unwrap();
        assert!(outcome.has_balanced_self_borrow_trace());
        assert!(outcome.ticket_returned);
        assert_eq!(outcome.main_local_slots_after_exit, 0);
        assert_eq!(outcome.method_local_slots_after_exit, 0);
        assert!(outcome.events.is_empty(), "Copyable 算術不應虛構 UTF-8 資源");
        outcome
    }

    #[test]
    fn m2_nested_else_math_動態false才計算巢狀else加法並返回10() {
        let result = run(SOURCE);
        assert_eq!(result.result, Ok(10));
        assert_eq!(result.branch_checked, Some(true));
        assert_eq!(result.nested_branch_checked, Some(false));
        assert_eq!(result.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_else_math_動態true只走內層true返回12不執行else() {
        let result = run(&SOURCE.replace("owner.read(false)", "owner.read(true)"));
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.nested_branch_checked, Some(true));
    }

    #[test]
    fn m2_nested_else_math_動態false以原Checker減法回傳負值() {
        let result = run(&SOURCE.replace("return self.n+3", "return self.n-20"));
        assert_eq!(result.result, Ok(-13));
        assert_eq!(result.nested_branch_checked, Some(false));
    }

    #[test]
    fn m2_nested_else_math_動態false已到溢位必真Unwind不假Returned並歸票() {
        let result = run(&SOURCE.replace(
            "return self.n+3", "return self.n+9223372036854775807",
        ));
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(result.nested_branch_checked, Some(false));
        assert!(result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_else_math_內層true完全不執行未選else的溢位算式() {
        let source = SOURCE.replace("owner.read(false)", "owner.read(true)")
            .replace("return self.n+3", "return self.n+9223372036854775807");
        let result = run(&source);
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.nested_branch_checked, Some(true));
    }

    #[test]
    fn m2_nested_else_math_外層false根本不到內層else即使其必溢位() {
        let source = SOURCE.replace("if(true){\nif(flag)", "if(flag){\nif(flag)")
            .replace("return self.n+3", "return self.n+9223372036854775807");
        let result = run(&source);
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.branch_checked, Some(false));
        assert_eq!(result.nested_branch_checked, None);
    }

    #[test]
    fn m2_nested_else_math_竄改else計算被SourceChanged拒絕且故障零假選葉() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_nested_else_calculation,
            Some(PrivateSelfIntReturn::Add(3)));
        plan.method_nested_else_calculation = Some(PrivateSelfIntReturn::Add(4));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let outcome = plan.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(outcome.nested_branch_checked, None);
        assert!(outcome.ticket_returned);
        assert!(outcome.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_nested_else_math_非法bool加法及不存在欄位仍需原Checker拒絕() {
        for source in [
            SOURCE.replace("return self.n+3", "return self.n+true"),
            SOURCE.replace("return self.n+3", "return self.missing+3"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "未經原 Checker 驗證的巢狀 else 計算不能進入 Runtime：{source}");
        }
    }
}
