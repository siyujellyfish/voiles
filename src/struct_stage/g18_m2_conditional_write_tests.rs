// G18／P18.6-M2：原 Checker if 分支獨占 self.int 寫入／未選中分支不寫入。
// 僅單一原 Lexer int 常值賦值與單一提前返回；不放寬 Move／Borrow 權限。
#[cfg(test)]
mod g18_m2_conditional_write_tests {
    use super::*;
    use crate::borrow_safety::{BorrowFact, BorrowKind};

    const WRITE_TRUE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
self.n = 19
return self.n
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
        assert_eq!(plan.loan,
            BorrowFact::new(plan.owner_id, BorrowKind::ExclusiveWrite),
            "任一可達葉會寫入 self，整個方法入口就必須取得獨占票據");
        assert!(plan.method_branch_write.is_some());
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty());
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_write_true葉選中才取得獨占self並真寫入再讀取() {
        let out = run(WRITE_TRUE);
        assert_eq!(out.result, Ok(19));
        assert_eq!(out.branch_checked, Some(true));
        assert!(out.expected_branch_write);
        assert!(out.branch_write_applied);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_write_false即使方法取得獨占票據也不執行true葉寫入() {
        let out = run(&WRITE_TRUE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(22));
        assert_eq!(out.branch_checked, Some(false));
        assert!(out.expected_branch_write);
        assert!(!out.branch_write_applied);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_write_false無else時繼續讀取原欄位而非幽靈寫入() {
        let source = WRITE_TRUE.replace("}else{\nreturn 22\n}", "}");
        let out = run(&source.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.branch_checked, Some(false));
        assert!(!out.branch_write_applied);
    }

    #[test]
    fn m2_write_true提前返回不得求值未到達之溢位算式() {
        let source = WRITE_TRUE.replace("return self.n+1",
            "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(19));
        assert!(out.branch_write_applied);
    }

    #[test]
    fn m2_write_方法入口注入故障尚未選葉不許寫入但須真還票() {
        let plan = PreparedPrivateSelfMethod::prepare(WRITE_TRUE).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked, None);
        assert!(!out.branch_write_applied);
        assert!(out.expected_branch_write);
        assert!(out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_write_真欄位值與Lexer跨度被竄改須在執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(WRITE_TRUE).unwrap();
        let (value, span) = plan.method_branch_write.unwrap();
        plan.method_branch_write = Some((value + 1, span));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));

        let mut plan = PreparedPrivateSelfMethod::prepare(WRITE_TRUE).unwrap();
        plan.method_branch_write = Some((value,
            crate::common_ir::NodeSpan::new(span.start + 1, span.end)));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn m2_write_原Checker拒絕const欄位及不可寫接收者() {
        for source in [
            WRITE_TRUE.replace("var n:int", "const n:int"),
            WRITE_TRUE.replace("var owner:Counter", "const owner:Counter"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))
            ), "唯讀來源不得以分支繞過原 Checker／權限防護：{source}");
        }
    }

    #[test]
    fn m2_write_未核准的else欄位寫入仍保守拒絕() {
        let source = WRITE_TRUE.replace(
            "}else{\nreturn 22",
            "}else{\nself.n = 20\nreturn 22",
        );
        assert!(matches!(
            PreparedPrivateSelfMethod::prepare(&source),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))
                | Err(PrivateSelfMethodPrepareError::Check(_))
        ), "額外 else 寫入未在本批取得原 Checker 動態證據，不得放行");
    }
}
