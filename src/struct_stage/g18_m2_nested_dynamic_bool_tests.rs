// G18／P18.6-M2：已核准 Checker bool 形參在巢狀 self if 真動態框架讀取。
// 原 Parser/Lexer 名稱身分、兩葉互斥、未選中零求值、唯一借用歸票。
#[cfg(test)]
mod g18_m2_nested_dynamic_bool_tests {
    use super::*;
    use crate::borrow_safety::BorrowKind;

    const DYNAMIC: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(true){
if(flag){
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
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert!(plan.method_nested_branch.is_some());
        assert_eq!(
            plan.method_nested_branch_parameter,
            plan.method_bool_arguments.first().map(|(id, _)| *id),
            "內層 bool 名稱只能與原 Checker 方法形參同一 DeclarationId",
        );
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        assert!(out.events.is_empty(), "Copyable int 不得憑空產生 str 資源");
        out
    }

    #[test]
    fn m2_nested_dynamic_外層常true內層真bool參數true回傳加法12() {
        let out = run(DYNAMIC);
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.nested_branch_checked, Some(true));
        assert_eq!(out.expected_nested_branch, Some(true));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_dynamic_外層常true內層真bool參數false回傳self欄位7() {
        let out = run(&DYNAMIC.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.nested_branch_checked, Some(false));
        assert_eq!(out.expected_nested_branch, Some(false));
    }

    #[test]
    fn m2_nested_dynamic_外層也用相同bool形參false時內層根本未進入() {
        let source = DYNAMIC.replace("if(true){\nif(flag)", "if(flag){\nif(flag)")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!(out.nested_branch_checked, None);
        assert_eq!(out.expected_nested_branch, Some(false));
    }

    #[test]
    fn m2_nested_dynamic_外層與內層同參數true時完成唯一正常返回() {
        let source = DYNAMIC.replace("if(true){\nif(flag)", "if(flag){\nif(flag)");
        let out = run(&source);
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.nested_branch_checked, Some(true));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_nested_dynamic_內層參數false不執行可能溢位的true返回葉() {
        let source = DYNAMIC.replace("return self.n+5",
            "return self.n+9223372036854775807")
            .replace("owner.read(true)", "owner.read(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.nested_branch_checked, Some(false));
    }

    #[test]
    fn m2_nested_dynamic_內層參數true已到達溢位必真Unwind並還票() {
        let source = DYNAMIC.replace("return self.n+5",
            "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.nested_branch_checked, Some(true));
        assert!(out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )));
    }

    #[test]
    fn m2_nested_dynamic_竄改內層真參數DeclarationId或靜態預期值均拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(DYNAMIC).unwrap();
        assert!(plan.method_nested_branch_parameter.is_some());
        plan.method_nested_branch_parameter = None;
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(DYNAMIC).unwrap();
        let (value, span) = plan.method_nested_branch.unwrap();
        plan.method_nested_branch = Some((!value, span));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_nested_dynamic_原Checker拒絕內層int條件及未宣告bool名稱() {
        for source in [
            DYNAMIC.replace("if(flag){\nreturn self.n+5", "if(7){\nreturn self.n+5"),
            DYNAMIC.replace("if(flag){\nreturn self.n+5", "if(other){\nreturn self.n+5"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "內層必須保持原 Checker 已驗證 bool 型別／身分：{source}");
        }
    }
}
