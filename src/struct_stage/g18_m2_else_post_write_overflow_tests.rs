// G18／P18.6-M2：原 Checker else 獨占 self.int 寫入後真加減與受控溢位。
// 只對真正選中的分支計算，不重播另一支的假退出／資源事件。
#[cfg(test)]
mod g18_m2_else_post_write_overflow_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
self.n = 19
return self.n
}else{
self.n = 23
return self.n+5
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
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert!(plan.method_branch_else_calculation.is_some());
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "Copyable 欄位算術不得產生假的 OwnedStr 事件");
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_else_overflow_false選中else先寫入23後求值加5() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(28));
        assert_eq!(out.branch_checked, Some(false));
        assert!(out.branch_else_write_applied);
        assert!(!out.branch_write_applied);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_else_overflow_true選中原true葉不執行else算術() {
        let out = run(&SOURCE.replace("owner.read(false)", "owner.read(true)"));
        assert_eq!(out.result, Ok(19));
        assert_eq!(out.branch_checked, Some(true));
        assert!(!out.branch_else_write_applied);
        assert!(out.branch_write_applied);
    }

    #[test]
    fn m2_else_overflow_false真寫入後加法溢位必受控Unwind及真還票() {
        let source = SOURCE.replace(
            "return self.n+5", "return self.n+9223372036854775807",
        );
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.branch_else_write_applied, "溢位前必已實際完成 else 欄位寫入");
        assert!(!out.branch_write_applied);
        assert_eq!(out.branch_checked, Some(false));
        assert!(!out.method_exited);
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_else_overflow_true跳過else必溢位算式及假寫入() {
        let source = SOURCE.replace("owner.read(false)", "owner.read(true)")
            .replace("return self.n+5", "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(19));
        assert!(out.branch_write_applied);
        assert!(!out.branch_else_write_applied);
    }

    #[test]
    fn m2_else_overflow_false從真更新欄位計算減法可返回負整數() {
        let source = SOURCE.replace("return self.n+5", "return self.n-30");
        let out = run(&source);
        assert_eq!(out.result, Ok(-7));
        assert!(out.branch_else_write_applied);
        assert!(!out.branch_write_applied);
    }

    #[test]
    fn m2_else_overflow_即使只有else可寫也必取得唯一獨占self票據() {
        let source = SOURCE.replace("self.n = 19\n", "");
        let plan = PreparedPrivateSelfMethod::prepare(&source).unwrap();
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert!(plan.method_branch_write.is_none());
        assert!(plan.method_branch_else_write.is_some());
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(28));
        assert!(out.branch_else_write_applied);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_else_overflow_竄改原Checker已選else計畫必在真資源前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        assert_eq!(plan.method_branch_else_calculation,
            Some(PrivateSelfIntReturn::Add(5)));
        plan.method_branch_else_calculation = Some(PrivateSelfIntReturn::Add(6));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_else_overflow_不合法self欄位與bool算術仍由原Checker拒絕() {
        for source in [
            SOURCE.replace("return self.n+5", "return self.missing+5"),
            SOURCE.replace("return self.n+5", "return self.n+true"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不能將未核准 else Return 當成真安全工作：{source}");
        }
    }
}
