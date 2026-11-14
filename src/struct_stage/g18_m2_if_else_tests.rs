// G18／P18.6-M2：原 Checker 的同步 self if/else 真控制路徑。
// bool 形參與 Lexer 常值雙路徑，else 只允許單一 int 常值 return。
#[cfg(test)]
mod g18_m2_if_else_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
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
        let prepared = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(prepared.method_branch_else.is_some(),
            "else 必須有原 Checker ReturnSpan 與 Lexer int 來源");
        let output = prepared.execute().unwrap();
        assert!(output.has_balanced_self_borrow_trace());
        assert!(output.ticket_returned);
        assert!(output.events.is_empty(), "純 bool/int 不應有偽造字串事件");
        assert_eq!(output.main_local_slots_after_exit, 0);
        assert_eq!(output.method_local_slots_after_exit, 0);
        output
    }

    #[test]
    fn m2_else_bool_true走真self早退未執行else() {
        let output = run(SOURCE);
        assert_eq!(output.result, Ok(7));
        assert_eq!(output.branch_checked, Some(true));
        assert_eq!(output.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_else_bool_false走原else整數返回不落至最後return() {
        let output = run(&SOURCE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(output.result, Ok(22));
        assert_eq!(output.branch_checked, Some(false));
        assert_eq!(output.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_else_false已返回不提前求值不可能到達之溢位() {
        let source = SOURCE.replace("owner.read(true)", "owner.read(false)")
            .replace("return self.n+1",
                "return self.n+9223372036854775807");
        let output = run(&source);
        assert_eq!(output.result, Ok(22));
        assert_eq!(output.branch_checked, Some(false));
    }

    #[test]
    fn m2_else_true分支不受未選中合法最大整數葉影響() {
        let source = SOURCE.replace("return 22",
            "return 9223372036854775807");
        let output = run(&source);
        assert_eq!(output.result, Ok(7));
        assert_eq!(output.branch_checked, Some(true));
    }

    #[test]
    fn m2_else_bool與常值if共用真分支判斷器但不混淆宣告身分() {
        let source = SOURCE.replace("fn read(flag:bool)", "fn read()")
            .replace("if(flag)", "if(false)")
            .replace("owner.read(true)", "owner.read()");
        let plan = PreparedPrivateSelfMethod::prepare(&source).unwrap();
        assert!(plan.method_bool_arguments.is_empty());
        assert!(plan.method_branch_parameter.is_none());
        let output = plan.execute().unwrap();
        assert_eq!(output.result, Ok(22));
        assert_eq!(output.branch_checked, Some(false));
        assert!(output.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_else_方法入口故障保留唯一真票據歸還且不可假造分支() {
        let plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let output = plan.execute_with_fault(true).unwrap();
        assert_eq!(output.result,
            Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(output.branch_checked, None);
        assert!(output.ticket_returned);
        assert!(output.has_balanced_self_borrow_trace());
        assert!(!output.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_else_修改已驗證else返回值或跨度須執行前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let (old_value, old_span) = plan.method_branch_else.unwrap();
        plan.method_branch_else = Some((old_value + 1, old_span));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_branch_else = Some((old_value,
            crate::common_ir::NodeSpan::new(old_span.start + 1, old_span.end)));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn m2_else_原Checker拒絕else回傳bool與錯誤bool實參() {
        for source in [
            SOURCE.replace("return 22", "return true"),
            SOURCE.replace("owner.read(true)", "owner.read(1)"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "應由原 Checker 先阻止不合法語意：{source}");
        }
    }
}
