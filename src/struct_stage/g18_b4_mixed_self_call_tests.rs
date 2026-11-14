// G18／P18.4-B4：原 Checker 普通 fn 在同步 self 方法借用期間的真巢狀框架。
// 只接通可證零參數、單一 int 常值返回；不代表 B4 完整方法／CFG 閉合。
#[cfg(test)]
mod g18_b4_mixed_self_call_tests {
    use super::*;

    const MIXED: &str = r#"struct Counter{
var value:int = 7
fn read()->int{
const from_helper:int = helper()
return from_helper
}
}
fn helper()->int{
return 11
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    #[test]
    fn b4_真正普通helper在同步self借用存活時巢狀執行返回() {
        let plan = PreparedPrivateSelfMethod::prepare(MIXED).unwrap();
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert_eq!(out.ordinary_calls_completed, 1);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(), "純 int 不能冒充真 UTF8 ResourceEvent");
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(out.method_local_slots_after_exit, 0);
        let enters = out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count();
        let returns = out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).count();
        let exits = out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).count();
        assert_eq!((enters, returns, exits), (2, 2, 2));
        let first_loan = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let second_enter = out.exit_events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered).then_some(index)
        ).nth(1).unwrap();
        let returned_ticket = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(first_loan < second_enter && second_enter < returned_ticket);
    }

    #[test]
    fn b4_同步方法注入失敗必在內層普通函式未進入前還self票據() {
        let plan = PreparedPrivateSelfMethod::prepare(MIXED).unwrap();
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.ordinary_calls_completed, 0);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::FunctionEntered
        )).count(), 1);
        assert_eq!(out.exit_events.iter().filter(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn b4_原Checker普通函式返回工作計畫遭竄改不可偽造真結果() {
        let mut plan = PreparedPrivateSelfMethod::prepare(MIXED).unwrap();
        plan.method_helper = Some(PrivateSelfApprovedHelper {
            name: "helper".to_owned(),
            parameters: Vec::new(),
            returned: PrivateScalarReturnExpr::Constant(999),
        });
        assert!(matches!(
            plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)
        ));
    }

    #[test]
    fn b4_同步self兩次執行各自產生新真票據與巢狀返回() {
        let plan = PreparedPrivateSelfMethod::prepare(MIXED).unwrap();
        let first = plan.execute().unwrap();
        let second = plan.execute().unwrap();
        assert!(first.has_balanced_self_borrow_trace());
        assert!(second.has_balanced_self_borrow_trace());
        assert_eq!((first.result, second.result), (Ok(11), Ok(11)));
        assert_eq!(first.ordinary_calls_completed, 1);
        assert_eq!(second.ordinary_calls_completed, 1);
    }

    #[test]
    fn b4_普通函式不存在或方法返回型別不符仍由原checker拒絕() {
        for source in [
            MIXED.replace("fn helper()->int{\nreturn 11\n}\n", ""),
            MIXED.replace("fn helper()->int{\nreturn 11", "fn helper()->bool{\nreturn true"),
        ] {
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
            ), "不應以 Unsupported 隱藏原 Checker 的正式拒絕：{source}");
        }
    }

    #[test]
    fn b4_尚未證明的普通函式參數與可溢位計畫仍保守拒絕() {
        for source in [
            // 零至二個已核准 int 形參現已接通；三個仍保守拒絕。
            MIXED.replace("from_helper:int = helper()", "from_helper:int = helper(2,3,4)")
                .replace("fn helper()->int{", "fn helper(a:int,b:int,c:int)->int{")
                .replace("return 11\n}", "return a+b+c\n}"),
            // 原 Checker 合法的 i64 加減溢位改由 Runtime 真 Unwind；
            // 這裡只保留超出 i64 詞彙範圍的靜態非法反例。
            MIXED.replace("return 11\n}", "return 9223372036854775808\n}"),
        ] {
            // 原 Checker 能直接拒絕時，比執行器 Unsupported 更早
            // 且更強；兩者都必須在沒有動態執行前拒絕來源。
            assert!(matches!(
                PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Unsupported(_))
                    | Err(PrivateSelfMethodPrepareError::Check(_))
            ), "未完整動態驗證的普通函式來源不可直接放行：{source}");
        }
    }

    #[test]
    fn b4_造假普通函式漏返回或重播來源即使數量相似亦應拒絕() {
        let plan = PreparedPrivateSelfMethod::prepare(MIXED).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        let position = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.remove(position);
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = plan.execute().unwrap();
        out.ordinary_calls_completed += 1;
        assert!(!out.has_balanced_self_borrow_trace());
    }
}
