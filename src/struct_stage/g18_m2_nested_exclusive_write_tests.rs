// G18／P18.6-M2：巢狀 true／else 兩個可寫葉動態互斥，原 Checker
// ExclusiveWrite、Lexer int 位置、單張 BorrowTicket 與 R05 受控 Unwind。
#[cfg(test)]
mod g18_m2_nested_exclusive_write_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(true){
if(flag){
self.n=19
return self.n+5
}else{
self.n=23
return self.n-30
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
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert!(plan.method_nested_branch.is_some());
        let result = plan.execute().unwrap();
        assert!(result.has_balanced_self_borrow_trace());
        assert!(result.ticket_returned);
        assert!(result.events.is_empty(), "Copyable int 不允許虛構 UTF-8 資源");
        assert_eq!(result.main_local_slots_after_exit, 0);
        assert_eq!(result.method_local_slots_after_exit, 0);
        result
    }

    #[test]
    fn m2_nested_write_動態true只寫19後返回24且false葉不執行() {
        let result = run(SOURCE);
        assert_eq!(result.result, Ok(24));
        assert_eq!(result.branch_checked, Some(true));
        assert_eq!(result.nested_branch_checked, Some(true));
        assert!(result.nested_write_applied);
        assert!(!result.nested_else_write_applied);
    }

    #[test]
    fn m2_nested_write_動態false只寫23後減30返回負7() {
        let result = run(&SOURCE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(result.result, Ok(-7));
        assert_eq!(result.nested_branch_checked, Some(false));
        assert!(!result.nested_write_applied);
        assert!(result.nested_else_write_applied);
    }

    #[test]
    fn m2_nested_write_外層false不觀測內層也不寫任一葉() {
        let source = SOURCE.replace("if(true){\nif(flag)", "if(flag){\nif(flag)")
            .replace("owner.read(true)", "owner.read(false)");
        let result = run(&source);
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.branch_checked, Some(false));
        assert_eq!(result.nested_branch_checked, None);
        assert!(!result.nested_write_applied);
        assert!(!result.nested_else_write_applied);
    }

    #[test]
    fn m2_nested_write_只有true葉寫入時false葉不寫仍需入口獨占() {
        let source = SOURCE.replace("self.n=23\n", "")
            .replace("owner.read(true)", "owner.read(false)");
        let result = run(&source);
        assert_eq!(result.result, Ok(-23));
        assert!(!result.nested_write_applied);
        assert!(!result.nested_else_write_applied);
        assert!(result.expected_nested_write);
        assert!(!result.expected_nested_else_write);
    }

    #[test]
    fn m2_nested_write_只有else葉寫入時true葉不寫返回12() {
        let source = SOURCE.replace("self.n=19\n", "");
        let result = run(&source);
        assert_eq!(result.result, Ok(12));
        assert!(!result.nested_write_applied);
        assert!(!result.nested_else_write_applied);
        assert!(!result.expected_nested_write);
        assert!(result.expected_nested_else_write);
    }

    #[test]
    fn m2_nested_write_寫入完成後加法溢位真Unwind且仍退唯一票() {
        let source = SOURCE.replace("return self.n+5",
            "return self.n+9223372036854775807");
        let result = run(&source);
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(result.nested_write_applied);
        assert!(!result.nested_else_write_applied);
        assert!(result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!result.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_write_竄改兩葉原寫入計畫拒絕且方法故障零假更新() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let (_, span) = plan.method_nested_true_write.unwrap();
        plan.method_nested_true_write = Some((99, span));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let (value, span) = plan.method_nested_else_write.unwrap();
        plan.method_nested_else_write = Some((value,
            crate::common_ir::NodeSpan::new(span.start+1, span.end)));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let result = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(result.nested_branch_checked, None);
        assert!(!result.nested_write_applied && !result.nested_else_write_applied);
        assert!(result.ticket_returned);
        assert!(result.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_nested_write_非法常值欄位與不可寫receiver由原Checker拒絕() {
        for source in [
            SOURCE.replace("self.n=19", "self.n=true"),
            SOURCE.replace("self.n=23", "self.missing=23"),
            SOURCE.replace("var n:int", "const n:int"),
            SOURCE.replace("var owner:Counter", "const owner:Counter"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不得將非法巢狀欄位更新交由 Runtime 當成已授權：{source}");
        }
    }
}
