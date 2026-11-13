// G18／P18.6-M2：原 Checker 具名 bool Copyable 形參在同步 self 真控制邊決定早返。
// 不把 bool 當 i64 0/1、不開放未核准的跨借用或 bool 非同步。
#[cfg(test)]
mod g18_m2_bool_parameter_tests {
    use super::*;

    const BOOL_TRUE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
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
        let prepared = PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(prepared.method_bool_argument.is_some(),
            "bool 形參必須由原 Checker 建立專屬框架身分");
        let out = prepared.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        assert!(out.events.is_empty(), "bool/int 不得虛構字串資源");
        out
    }

    #[test]
    fn m2_bool_真bool形參true驅動if選中並從self票據讀值() {
        let out = run(BOOL_TRUE);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
        assert_eq!(out.expected_branch, Some(true));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_bool_真bool形參false不進早返葉並執行最後原Checker算式() {
        let out = run(&BOOL_TRUE.replace("owner.read(true)", "owner.read(false)"));
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!(out.expected_branch, Some(false));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count(), 1);
    }

    #[test]
    fn m2_bool_true時未選中溢位來源零效果() {
        let source = BOOL_TRUE.replace("return self.n+1",
            "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.branch_checked, Some(true));
    }

    #[test]
    fn m2_bool_false時已走到溢位必受控故障並真歸還self() {
        let source = BOOL_TRUE.replace("owner.read(true)", "owner.read(false)")
            .replace("return self.n+1",
                "return self.n+9223372036854775807");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.branch_checked, Some(false));
        assert!(!out.method_exited);
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_bool_入口故障前已綁定原bool形參但不會假記條件() {
        let plan = PreparedPrivateSelfMethod::prepare(BOOL_TRUE).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.branch_checked, None);
        assert!(out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn m2_bool_兩次重跑原工作圖不共用BorrowTicket或bool值槽() {
        let plan = PreparedPrivateSelfMethod::prepare(BOOL_TRUE).unwrap();
        let a = plan.execute().unwrap();
        let b = plan.execute().unwrap();
        assert_eq!((a.result, b.result), (Ok(7), Ok(7)));
        assert_eq!((a.branch_checked, b.branch_checked), (Some(true), Some(true)));
        assert!(a.has_balanced_self_borrow_trace());
        assert!(b.has_balanced_self_borrow_trace());
        assert!(a.ticket_returned && b.ticket_returned);
    }

    #[test]
    fn m2_bool_已核准形參動態值及DeclarationId遭篡改必在求值前拒絕() {
        let mut plan = PreparedPrivateSelfMethod::prepare(BOOL_TRUE).unwrap();
        let (id, value) = plan.method_bool_argument.unwrap();
        plan.method_bool_argument = Some((id, !value));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
        let mut plan = PreparedPrivateSelfMethod::prepare(BOOL_TRUE).unwrap();
        plan.method_branch_parameter = Some(DeclarationId(0));
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn m2_bool_原Checker拒絕int冒充bool及呼叫引數錯型別() {
        for source in [
            BOOL_TRUE.replace("fn read(flag:bool)", "fn read(flag:int)"),
            BOOL_TRUE.replace("owner.read(true)", "owner.read(5)"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不得在 Runtime 將 int 偷轉 bool：{source}");
        }
    }
}
