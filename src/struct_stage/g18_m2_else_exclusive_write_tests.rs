// G18／P18.6-M2：原 Checker 雙分支獨占 self.int 寫入與唯一真 BorrowTicket。
// 仍只開放各葉單一原 Lexer int 常值賦值，接同欄位 Copyable 提前返回。
#[cfg(test)]
mod g18_m2_else_exclusive_write_tests {
    use super::*;
    use crate::borrow_safety::{BorrowFact, BorrowKind};

    const BOTH_WRITE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
self.n = 19
return self.n
}else{
self.n = 23
return self.n
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
        assert_eq!(plan.loan,
            BorrowFact::new(plan.owner_id, BorrowKind::ExclusiveWrite),
            "任一路徑寫入均必須由原 Checker 簽發整體獨占 self 權限");
        assert!(plan.method_branch_else_write.is_some());
        assert!(plan.method_branch_else_field_return.is_some());
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "Copyable int 不得虛構字串資源");
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_else_write_true寫入19但不執行else寫入23() {
        let out = run(BOTH_WRITE);
        assert_eq!(out.result, Ok(19));
        assert_eq!(out.branch_checked, Some(true));
        assert!(out.branch_write_applied);
        assert!(!out.branch_else_write_applied);
        assert!(out.expected_branch_write && out.expected_branch_else_write);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_else_write_false只寫入23並從最新self欄位讀取返回() {
        let out = run(&BOTH_WRITE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(23));
        assert_eq!(out.branch_checked, Some(false));
        assert!(!out.branch_write_applied);
        assert!(out.branch_else_write_applied);
        assert!(out.expected_branch_write && out.expected_branch_else_write);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_else_write_僅else寫入仍由Checker要求方法入口獨占() {
        let source = BOTH_WRITE.replace("self.n = 19\n", "")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(23));
        assert!(!out.expected_branch_write);
        assert!(!out.branch_write_applied);
        assert!(out.branch_else_write_applied);
    }

    #[test]
    fn m2_else_write_true時不執行未選分支也不求值末尾溢位() {
        let source = BOTH_WRITE.replace("return self.n+1",
            "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(19));
        assert!(out.branch_write_applied);
        assert!(!out.branch_else_write_applied);
    }

    #[test]
    fn m2_else_write_false時不執行未選真葉且不求值末尾溢位() {
        let source = BOTH_WRITE.replace("owner.read(true)", "owner.read(false)")
            .replace("return self.n+1",
                "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(23));
        assert!(!out.branch_write_applied);
        assert!(out.branch_else_write_applied);
    }

    #[test]
    fn m2_else_write_方法入口故障不得假造兩葉寫入仍真歸還獨占票據() {
        let plan = PreparedPrivateSelfMethod::prepare(BOTH_WRITE).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked, None);
        assert!(!out.branch_write_applied);
        assert!(!out.branch_else_write_applied);
        assert!(out.expected_branch_else_write);
        assert!(out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_else_write_已核准else值及Return位置遭篡改執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(BOTH_WRITE).unwrap();
        let (value, span) = plan.method_branch_else_write.unwrap();
        plan.method_branch_else_write = Some((value + 1, span));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(BOTH_WRITE).unwrap();
        let original = plan.method_branch_else_field_return.unwrap();
        plan.method_branch_else_field_return = Some(
            crate::common_ir::NodeSpan::new(original.start + 1, original.end),
        );
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_else_write_原Checker拒絕const與不合法else型別且未證控制保守拒絕() {
        for source in [
            BOTH_WRITE.replace("var n:int", "const n:int"),
            BOTH_WRITE.replace("var owner:Counter", "const owner:Counter"),
            BOTH_WRITE.replace("self.n = 23", "self.n = true"),
            BOTH_WRITE.replace("return self.n\n}\nreturn self.n+1",
                "return self.missing\n}\nreturn self.n+1"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "不可繞過原 Checker／同步執行器安全界線：{source}");
        }
    }
}
