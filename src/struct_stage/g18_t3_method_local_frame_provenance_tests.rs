// G18／P18.5-T3／P18.4-B4／P18.6-M3：原 Checker 方法
// 局部不能被移入 main 的普通函式或 self 借用期間的子函式。
// 同樣數量、DeclarationId、真 LoanTicket 與完全平衡的
// SharedCallExit，仍必須驗證局部所屬的**唯一 self 方法框架**。
#[cfg(test)]
mod g18_t3_method_local_frame_provenance_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
const in_method:int = helper(n)
return in_method
}
}
fn helper(n:int)->int{
return n+2
}
fn main()->int{
var owner:Counter = Counter()
const before:int = helper(3)
return owner.read(before)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        PreparedPrivateSelfMethod::prepare(source)
            .expect("只能由原 Checker 核准的方法與普通函式圖")
            .execute()
            .expect("真方法執行必保留 HC14／R05")
    }

    /// 不增刪動態局部事件，只將原方法整個局部進退簿
    /// 原順序搬到指定普通函式內；SharedExit 原有的
    /// 一般函式平衡會接受這種語意上錯誤的移植。
    fn transplant_locals_to_enter(
        out: &mut PrivateSelfMethodOutcome, enter_ordinal: usize,
    ) {
        let mut moved = Vec::<PrivateSharedCallExitEvent>::new();
        out.exit_events.retain(|event| match event {
            PrivateSharedCallExitEvent::LocalInitialized(_)
            | PrivateSharedCallExitEvent::LocalAssigned(_)
            | PrivateSharedCallExitEvent::LocalScopeEnded(_) => {
                moved.push(event.clone());
                false
            }
            _ => true,
        });
        assert!(!moved.is_empty());
        let enter = out.exit_events.iter().enumerate()
            .filter_map(|(i, event)| matches!(
                event, PrivateSharedCallExitEvent::FunctionEntered
            ).then_some(i))
            .nth(enter_ordinal).expect("原 Checker 真函式已進入");
        for (offset, event) in moved.into_iter().enumerate() {
            out.exit_events.insert(enter + 1 + offset, event);
        }
    }

    #[test]
    fn t3_self_frame_原main_helper先真返回再借self內局部成功可驗() {
        let out = run(SOURCE);
        assert_eq!(out.result, Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(2,2));
        assert!(out.has_balanced_self_borrow_trace());
        let self_loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let local=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LocalInitialized(_)
        )).unwrap();
        let loan_end=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(self_loan<local && local<loan_end);
    }

    #[test]
    fn t3_self_frame_同main兩次真helper後仍只有唯一self局部歸屬() {
        let src=SOURCE.replace(
            "const before:int = helper(3)\nreturn owner.read(before)",
            "const a:int = helper(1)\nconst b:int = helper(a)\nreturn owner.read(b)",
        );
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(3,3));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_方法進入即故障時不得偽造未執行局部但票據須真退() {
        let plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let out=plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.method_local_events.is_empty());
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_方法局部真初始化後Return溢位仍保留逆序清理() {
        let src=SOURCE.replace("return in_method",
            "return in_method+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_移到main普通helper同數量局部與有效self票仍必拒絕() {
        let mut out=run(SOURCE);
        transplant_locals_to_enter(&mut out,0);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events, out.result.is_ok(),
        ),"僅一般幀／票據平衡無法證明局部歸屬");
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_移到self內巢狀普通helper同數量局部仍必拒絕() {
        let mut out=run(SOURCE);
        // ordinal 0 是 main helper，1 是 self 方法，
        // ordinal 2 才是 self 方法借票期間進入的 helper。
        transplant_locals_to_enter(&mut out,2);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events, out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_初始化重賦與退域三事件移到main_helper不得冒充() {
        let src=SOURCE.replace(
            "const in_method:int = helper(n)\nreturn in_method",
            "var in_method:int = helper(n)\nin_method = helper(in_method)\nreturn in_method",
        );
        let mut out=run(&src);
        assert_eq!(out.result,Ok(9));
        assert!(out.has_balanced_self_borrow_trace());
        transplant_locals_to_enter(&mut out,0);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events, out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_self_frame_真失敗Unwind與票序完全不改但清理被移至main亦拒絕() {
        let src=SOURCE.replace("return in_method",
            "return in_method+9223372036854775807");
        let mut out=run(&src);
        assert!(out.has_balanced_self_borrow_trace());
        transplant_locals_to_enter(&mut out,0);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events, out.result.is_ok(),
        ));
        assert!(out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )));
        assert!(!out.has_balanced_self_borrow_trace());
    }
}
