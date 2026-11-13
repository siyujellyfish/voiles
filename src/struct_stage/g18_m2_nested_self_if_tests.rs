// G18／P18.6-M2：原 Checker 巢狀 if 雙葉返回之真動態選擇與借用清理。
// 僅允許原 Lexer bool 內層常值，兩葉皆 Copyable int，沒有資源 Move。
#[cfg(test)]
mod g18_m2_nested_self_if_tests {
    use super::*;

    const NESTED: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
if(true){
return self.n+5
}else{
return self.n
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
        assert!(plan.method_nested_branch.is_some());
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "Copyable int 巢狀控制不應造出假資源");
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        out
    }

    #[test]
    fn m2_nested_外層true內層true返回原Checker加法值且真歸還self() {
        let out = run(NESTED);
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.nested_branch_checked, Some(true));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_外層true內層false返回原self欄位而不執行true加法() {
        let source = NESTED.replace("if(true){\nreturn self.n+5", "if(false){\nreturn self.n+5");
        let out = run(&source);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.nested_branch_checked, Some(false));
    }

    #[test]
    fn m2_nested_外層false不進內層if且只求值最後return() {
        let source = NESTED.replace("owner.read(true)", "owner.read(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!(out.nested_branch_checked, None);
    }

    #[test]
    fn m2_nested_外層true內層true真溢位受控Unwind並真還票() {
        let source = NESTED.replace("return self.n+5",
            "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.nested_branch_checked, Some(true));
        assert!(!out.method_exited);
        assert!(out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_內層false跳過不可達溢位且外層false也不進內層() {
        let source = NESTED.replace("return self.n+5",
            "return self.n+9223372036854775807")
            .replace("if(true){\nreturn self.n+", "if(false){\nreturn self.n+");
        let out = run(&source);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.nested_branch_checked, Some(false));

        let source = NESTED.replace("return self.n+5",
            "return self.n+9223372036854775807")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.nested_branch_checked, None);
    }

    #[test]
    fn m2_nested_方法入口故障不得假造巢狀條件或正常Returned() {
        let plan = PreparedPrivateSelfMethod::prepare(NESTED).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked, None);
        assert_eq!(out.nested_branch_checked, None);
        assert!(out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(!out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_竄改內層bool條件或返回來源跨度必在真資源前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(NESTED).unwrap();
        let (value, span) = plan.method_nested_branch.unwrap();
        plan.method_nested_branch = Some((!value, span));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(NESTED).unwrap();
        plan.method_nested_branch = Some((value,
            crate::common_ir::NodeSpan::new(span.start+1, span.end)));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_nested_內層條件錯型別與欄位不存在仍由原Checker拒絕() {
        for source in [
            NESTED.replace("if(true){\nreturn self.n+5", "if(1){\nreturn self.n+5"),
            NESTED.replace("}else{\nreturn self.n", "}else{\nreturn self.missing"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不得藉巢狀 if 繞過 Checker 既有安全限制：{source}");
        }
    }
}
