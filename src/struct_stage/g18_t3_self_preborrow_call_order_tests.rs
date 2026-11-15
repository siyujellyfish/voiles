// G18／P18.5-T3／P18.4-B4／P18.6-M3：main 普通函式
// 的靜態原 Checker Call 數與動態借 self 前退出時序對帳。
// 只移植同一三筆 Enter／Returned／Exited、不變更票據或
// 結果仍須拒絕「方法先借票，main 普通 helper 後進入」。
#[cfg(test)]
mod g18_t3_self_preborrow_call_order_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var value:int = 7
fn read(n:int)->int{
return n
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

    fn run(source:&str)->PrivateSelfMethodOutcome {
        PreparedPrivateSelfMethod::prepare(source)
            .expect("須經原 Parser/Checker 核准同步 self 與 main helper")
            .execute().expect("原私有執行器真動態結果")
    }

    fn relocate_helper(
        out:&mut PrivateSelfMethodOutcome,
        ordinal:usize,
        insert_into_method:bool,
    ) {
        let start=out.exit_events.iter().enumerate().filter_map(
            |(i,event)|matches!(
                event,PrivateSharedCallExitEvent::FunctionEntered
            ).then_some(i)
        ).nth(ordinal).expect("main helper 真 Enter");
        assert!(matches!(
            out.exit_events.get(start+1),
            Some(PrivateSharedCallExitEvent::Returned)
        ));
        assert!(matches!(
            out.exit_events.get(start+2),
            Some(PrivateSharedCallExitEvent::FunctionExited{returned:true})
        ));
        let moved=out.exit_events.drain(start..start+3).collect::<Vec<_>>();
        if insert_into_method {
            let loan=out.exit_events.iter().position(|event|matches!(
                event,PrivateSharedCallExitEvent::LoanAcquired(_)
            )).unwrap();
            let method_enter=out.exit_events.iter().enumerate().find_map(
                |(index,event)| (index>loan && matches!(
                    event,PrivateSharedCallExitEvent::FunctionEntered
                )).then_some(index)
            ).unwrap();
            for (offset,event) in moved.into_iter().enumerate() {
                out.exit_events.insert(method_enter+1+offset,event);
            }
        } else {
            out.exit_events.extend(moved);
        }
    }

    #[test]
    fn t3_preborrow_原Checker唯一main_helper真先Returned再借self() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(5));
        assert_eq!(out.expected_pre_method_calls,1);
        assert!(out.has_balanced_self_borrow_trace());
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn t3_preborrow_兩份main同步helper與方法內純helper各自原位進退() {
        let src=SOURCE.replace("return n\n}","const m:int = helper(n)\nreturn m\n}")
            .replace(
                "const before:int = helper(3)\nreturn owner.read(before)",
                "const a:int = helper(1)\nconst b:int = helper(a)\nreturn owner.read(b)",
            );
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.expected_pre_method_calls,2);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(3,3));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_main第二helper真溢位不得要求其後方法已借票() {
        let src=SOURCE.replace(
            "const before:int = helper(3)\nreturn owner.read(before)",
            "const before:int = helper(3)\nconst overflow:int = helper(9223372036854775807)\nreturn owner.read(overflow)",
        );
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered);
        assert_eq!(out.expected_pre_method_calls,2);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(2,1));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_self入口注入失敗仍必保留main_helper真正退出與借票歸還() {
        let plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let out=plan.execute_with_fault(true).unwrap();
        assert_eq!(out.expected_pre_method_calls,1);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_移動完整main_helper三事件至方法退出後泛用帳仍平衡須拒絕() {
        let mut out=run(SOURCE);
        relocate_helper(&mut out,0,false);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events,out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_移動main_helper三事件到self持票方法中仍不得歸錯框架() {
        let mut out=run(SOURCE);
        relocate_helper(&mut out,0,true);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events,out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_兩個main_helper只移第二個到方法內仍須核對原全數() {
        let src=SOURCE.replace(
            "const before:int = helper(3)\nreturn owner.read(before)",
            "const a:int = helper(1)\nconst b:int = helper(a)\nreturn owner.read(b)",
        );
        let mut out=run(&src);
        assert_eq!(out.expected_pre_method_calls,2);
        relocate_helper(&mut out,1,true);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events,out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn t3_preborrow_方法真溢位且借票無變仍不得將main_helper移入方法() {
        let src=SOURCE.replace("return n\n}",
            "return n+9223372036854775807\n}");
        let mut out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_self_borrow_trace());
        relocate_helper(&mut out,0,true);
        assert!(private_shared_call_exit_balanced(
            &out.exit_events,out.result.is_ok(),
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }
}
