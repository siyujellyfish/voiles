// G18／P18.6-M2：真原 Checker 同步 self 方法 if 常值選葉／提前返回。
// 此批不推定 bool 參數、一般方法迴圈或非 Copyable 責任已驗收。
#[cfg(test)]
mod g18_m2_self_if_return_tests {
    use super::*;

    const CONDITIONAL_TRUE: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
if(true){
return self.n
}
return self.n+1
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    fn outcome(source: &str) -> PrivateSelfMethodOutcome {
        let out = PreparedPrivateSelfMethod::prepare(source).unwrap()
            .execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "Copyable self.int 不得偽造 ResourceId");
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_self_if_真true葉返回self最新欄位且不執行最後return() {
        let out = outcome(CONDITIONAL_TRUE);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.expected_branch, Some(true));
        assert!(out.method_entered && out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_self_if_真false跳過提前return而執行已核准最後return() {
        let out = outcome(&CONDITIONAL_TRUE.replace("if(true)", "if(false)"));
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_self_if_未命中算式溢位葉不應在true提前return後執行() {
        let src = CONDITIONAL_TRUE.replace(
            "return self.n+1",
            "return self.n+9223372036854775807",
        );
        let out = outcome(&src);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
    }

    #[test]
    fn m2_self_if_false葉後續溢位必受控故障且先歸還同張self票據() {
        let src = CONDITIONAL_TRUE.replace("if(true)", "if(false)")
            .replace("return self.n+1", "return self.n+9223372036854775807");
        let out = outcome(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
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
    fn m2_self_if_方法入口注入故障不得假造未到達if條件或回傳() {
        let plan = PreparedPrivateSelfMethod::prepare(CONDITIONAL_TRUE).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked, None);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(!out.method_exited);
    }

    #[test]
    fn m2_self_if_來源bool值或提前return位置遭篡改執行前必拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(CONDITIONAL_TRUE).unwrap();
        assert!(plan.method_branch.is_some());
        let (value, span) = plan.method_branch.unwrap();
        plan.method_branch = Some((!value, span));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(CONDITIONAL_TRUE).unwrap();
        let (_, original_span) = plan.method_branch.unwrap();
        plan.method_branch = Some((true,
            crate::common_ir::NodeSpan::new(
                original_span.start + 1, original_span.end,
            ),
        ));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_self_if_條件錯型別與非法self欄位仍由原Checker先拒絕() {
        for source in [
            CONDITIONAL_TRUE.replace("if(true)", "if(1)"),
            CONDITIONAL_TRUE.replace("return self.n\n}", "return self.missing\n}"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "方法 if 不能繞過原 Checker：{source}");
        }
    }
}
