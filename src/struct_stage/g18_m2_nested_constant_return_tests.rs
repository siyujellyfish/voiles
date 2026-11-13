// G18／P18.6-M2：同一原 Checker 已核准巢狀 if 的整數常值雙葉返回。
// 原 Lexer 半開跨度、唯一 self 借用票據、未選分支零求值與假事件拒絕。
#[cfg(test)]
mod g18_m2_nested_constant_return_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(true){
if(flag){
return 41
}else{
return 23
}
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
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert!(plan.method_nested_branch.is_some());
        assert!(plan.method_branch_return.is_some());
        assert!(plan.method_nested_else_calculation.is_some());
        let result = plan.execute().unwrap();
        assert!(result.has_balanced_self_borrow_trace());
        assert!(result.ticket_returned);
        assert_eq!(result.main_local_slots_after_exit, 0);
        assert_eq!(result.method_local_slots_after_exit, 0);
        assert!(result.events.is_empty(), "Copyable int 常值不得憑空建立 UTF-8 資源");
        result
    }

    #[test]
    fn m2_nested_constant_動態true只返回原Lexer常值41() {
        let result = run(SOURCE);
        assert_eq!(result.result, Ok(41));
        assert_eq!(result.branch_checked, Some(true));
        assert_eq!(result.nested_branch_checked, Some(true));
        assert_eq!(result.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_constant_動態false只返回原Lexer常值23() {
        let result = run(&SOURCE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(result.result, Ok(23));
        assert_eq!(result.nested_branch_checked, Some(false));
        assert_eq!(result.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_constant_外層false未執行內層任何常值返回而落到尾端() {
        let source = SOURCE.replace("if(true){\nif(flag)", "if(flag){\nif(flag)")
            .replace("owner.read(true)", "owner.read(false)");
        let result = run(&source);
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.branch_checked, Some(false));
        assert_eq!(result.nested_branch_checked, None);
    }

    #[test]
    fn m2_nested_constant_true葉常值不執行else必溢位算式() {
        let source = SOURCE.replace("return 23", "return self.n+9223372036854775807");
        let result = run(&source);
        assert_eq!(result.result, Ok(41));
        assert_eq!(result.nested_branch_checked, Some(true));
    }

    #[test]
    fn m2_nested_constant_false葉常值不執行true必溢位算式() {
        let source = SOURCE.replace("return 41", "return self.n+9223372036854775807")
            .replace("owner.read(true)", "owner.read(false)");
        let result = run(&source);
        assert_eq!(result.result, Ok(23));
        assert_eq!(result.nested_branch_checked, Some(false));
    }

    #[test]
    fn m2_nested_constant_內層原Lexer布林常值也只選真正返回葉() {
        let source = SOURCE.replace("if(flag){\nreturn 41", "if(false){\nreturn 41");
        let plan = PreparedPrivateSelfMethod::prepare(&source).unwrap();
        assert!(plan.method_nested_branch_parameter.is_none());
        let result = run(&source);
        assert_eq!(result.result, Ok(23));
        assert_eq!(result.nested_branch_checked, Some(false));
    }

    #[test]
    fn m2_nested_constant_常值計畫篡改拒絕且注入故障零假返回() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_branch_return, Some(PrivateSelfIntReturn::Constant(41)));
        plan.method_branch_return = Some(PrivateSelfIntReturn::Constant(42));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_nested_else_calculation,
            Some(PrivateSelfIntReturn::Constant(23)));
        plan.method_nested_else_calculation = Some(PrivateSelfIntReturn::Constant(24));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let result = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(result.nested_branch_checked, None);
        assert!(result.ticket_returned);
        assert!(result.has_balanced_self_borrow_trace());
        assert!(!result.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_constant_錯型別與不存在欄位仍由原Checker拒絕() {
        for source in [
            SOURCE.replace("return 41", "return true"),
            SOURCE.replace("return 23", "return false"),
            SOURCE.replace("return 23", "return self.missing"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "原 Checker 未核准的巢狀返回不得進入 Runtime：{source}");
        }
    }
}
