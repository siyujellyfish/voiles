// G18／P18.1-F2：經正式 Checker 的同步 str 直接轉發中斷矩陣。
// 以可配置真執行步數掃描「Returned 前、Returned 後、Forward 前後」
// 每個動態切點；不能用模擬字串或合成借用票據取代實際執行。
#[cfg(test)]
mod g18_f2_str_forward_budget_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let prepared = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = prepared.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 str 多框架雙帳不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_forward_budget_多層跨框架每個步數截斷均真資源清理不丟失() {
        let source = r#"fn make()->str{
return "步數可控"
}
fn middle()->str{
return make()
}
fn relay()->str{
return middle()
}
fn main()->int{
const received:str = relay()
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut interrupted_after_str_return = false;
        let mut interrupted_after_forward = false;
        let mut successful = false;
        for max_steps in 1..100 {
            let out = plan.execute_with_budget(PrivateScalarExecutionBudget {
                max_call_frames: 128, max_steps,
            }).unwrap();
            assert!(out.has_balanced_frame_trace(), "F2 step={max_steps} R05 失衡");
            assert_eq!(out.parameter_slots_after_exit, 0);
            let created = out.resource_events.iter().filter(|event|
                matches!(event, R::Created(1))).count();
            let released = out.resource_events.iter().filter(|event|
                matches!(event, R::Released(1))).count();
            assert_eq!(created, released, "F2 step={max_steps} UTF8 洩漏");
            if matches!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget)) {
                let returns = out.events.iter().filter(|event|
                    matches!(event, PrivateScalarCallEvent::StrReturned(1))).count();
                interrupted_after_str_return |= returns >= 1 && released == 1;
                interrupted_after_forward |= returns >= 2 && released == 1;
                assert!(out.events.iter().any(|event| matches!(
                    event, PrivateScalarCallEvent::UnwindStarted,
                )));
            } else {
                assert_eq!(out.result, Ok(0));
                successful = true;
            }
        }
        assert!(successful && interrupted_after_str_return && interrupted_after_forward);
    }

    #[test]
    fn f2_forward_budget_先返回後經中層參數再轉發同resourceid() {
        let out = run(r#"fn make()->str{
return "鏈🪼"
}
fn identity(value:str)->str{
return value.move()
}
fn relay()->str{
return identity(make())
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Moved(1),
            R::Returned(1), R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )).count(), 1);
    }

    #[test]
    fn f2_forward_budget_兩個獨立字串參數其中一筆直接返回且另一筆先釋放() {
        let out = run(r#"fn first()->str{
return "第一"
}
fn second()->str{
return "第二"
}
fn choose(left:str,right:str)->str{
return left.move()
}
fn relay()->str{
return choose(first(),second())
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Created(2), R::Returned(2),
            R::Moved(1), R::Moved(2), R::Returned(1), R::Released(2),
            R::Returned(1), R::Released(1),
        ]);
    }

    #[test]
    fn f2_forward_budget_返回函式外層括號必保留真checker及唯一id() {
        let out = run(r#"fn make()->str{
return "括號來源"
}
fn relay()->str{
return (make())
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Returned(1), R::Released(1),
        ]);
    }

    #[test]
    fn f2_forward_budget_未命中if不得提前建立或呼叫原str函式() {
        let out = run(r#"fn make()->str{
return "未到達"
}
fn relay(flag:bool)->str{
if (flag) {
return make()
}
return "備用"
}
fn main()->int{
const received:str = relay(false)
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "make"
        )));
    }

    #[test]
    fn f2_forward_budget_未進入真正子函式前的暫存字面值加溢位逐框架清理() {
        let out = run(r#"fn make(value:str,n:int)->str{
return value.move()
}
fn relay()->str{
return make("暫存已建",9223372036854775807+1)
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "make"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(_)
        )));
    }

    #[test]
    fn f2_forward_budget_重播第二次真returned事件必遭來源雙帳防偽拒絕() {
        let source = r#"fn make()->str{
return "只能同一次"
}
fn relay()->str{
return make()
}
fn main()->int{
const received:str = relay()
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().rposition(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Returned(1))
        )).unwrap();
        out.events.insert(index + 1, PrivateScalarCallEvent::OwnedResource(R::Returned(1)));
        out.resource_events.insert(2, R::Returned(1));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn f2_forward_budget_竄改第二次真strreturned的resourceid必遭R05拒絕() {
        let source = r#"fn make()->str{
return "身分"
}
fn relay()->str{
return make()
}
fn main()->int{
const received:str = relay()
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().rposition(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(1)
        )).unwrap();
        out.events[index] = PrivateScalarCallEvent::StrReturned(999);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    // F2：兩個不同的 str-return 暫存均已建立，最後一個 int 實參
    // 溢位時不得偷渡 callee；返回後尚未轉入參數的 ResourceId 必逆序退出。
    #[test]
    fn f2_forward_budget_雙字串已返回第三實參溢位逆序回收而不入被呼叫者() {
        let out = run(r#"fn first()->str{
return "先有"
}
fn second()->str{
return "後有"
}
fn consume(left:str,right:str,n:int)->str{
return left.move()
}
fn main()->int{
const received:str = consume(first(),second(),9223372036854775807+1)
return 0
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "consume"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::StrTemporaryTransferred { .. }
        )));
    }

    // 對應相同真實來源的「每個中斷點」，不在 Rust 測試合成事件或資源。
    #[test]
    fn f2_forward_budget_雙字串前置實參逐步數截斷均無遺漏與重複釋放() {
        let source = r#"fn first()->str{
return "步數先"
}
fn second()->str{
return "步數後"
}
fn consume(left:str,right:str,n:int)->str{
return left.move()
}
fn main()->int{
const received:str = consume(first(),second(),9223372036854775807+1)
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut reached_both = false;
        let mut reached_overflow = false;
        for max_steps in 1..160 {
            let out = plan.execute_with_budget(PrivateScalarExecutionBudget {
                max_call_frames: 128, max_steps,
            }).unwrap();
            assert!(out.has_balanced_frame_trace(), "F2 step={max_steps} 逐幀退出失衡");
            assert_eq!(out.parameter_slots_after_exit, 0);
            assert!(matches!(out.result,
                Err(PrivateScalarCallRuntimeError::StepBudget)
                | Err(PrivateScalarCallRuntimeError::IntegerOverflow)
            ), "F2 step={max_steps} 不得成功略過最後溢位");
            reached_overflow |= matches!(out.result,
                Err(PrivateScalarCallRuntimeError::IntegerOverflow));
            let mut created = std::collections::BTreeMap::<u64, usize>::new();
            let mut released = std::collections::BTreeMap::<u64, usize>::new();
            for event in &out.resource_events {
                match event {
                    R::Created(id) => { *created.entry(*id).or_default() += 1; }
                    R::Released(id) => { *released.entry(*id).or_default() += 1; }
                    _ => {}
                }
            }
            assert_eq!(created, released, "F2 step={max_steps} id 清理錯位");
            assert!(created.values().all(|count| *count == 1),
                "F2 step={max_steps} 重複資源建立");
            if created.len() == 2 {
                reached_both = true;
                let releases: Vec<_> = out.resource_events.iter().filter_map(|event|
                    match event { R::Released(id) => Some(*id), _ => None }
                ).collect();
                assert_eq!(releases, [2, 1], "F2 step={max_steps} 必須先退較新暫存");
            }
            assert!(!out.events.iter().any(|event| matches!(
                event, PrivateScalarCallEvent::FunctionEntered(name) if name == "consume"
            )), "F2 step={max_steps} 溢位前不得進入被呼叫者");
        }
        assert!(reached_both && reached_overflow, "必須真正覆蓋雙資源與算術溢位");
    }

    // 同時有兩個真實的暫存參數時，不可把第二個參數身分偽造為第一個。
    #[test]
    fn f2_forward_budget_雙實參將第二個轉交目標竄改成第一個即使id正確仍拒絕() {
        let source = r#"fn first()->str{
return "第一個"
}
fn second()->str{
return "第二個"
}
fn consume(left:str,right:str)->str{
return left.move()
}
fn main()->int{
const received:str = consume(first(),second())
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        let positions = out.events.iter().enumerate().filter_map(|(i, event)| {
            if matches!(event, PrivateScalarCallEvent::StrTemporaryTransferred { .. }) {
                Some(i)
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(positions.len(), 2);
        let first_target = match &out.events[positions[0]] {
            PrivateScalarCallEvent::StrTemporaryTransferred { target, .. } => *target,
            _ => unreachable!(),
        };
        let second_id = match &out.events[positions[1]] {
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id, .. } => *resource_id,
            _ => unreachable!(),
        };
        out.events[positions[1]] = PrivateScalarCallEvent::StrTemporaryTransferred {
            target: first_target, resource_id: second_id,
        };
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    // ResourceId 互換即使仍有兩個合法 Created/Released，也不得當成同一來源的返回。
    #[test]
    fn f2_forward_budget_雙來源第二個str返回偽冒第一個id必遭稽核拒絕() {
        let source = r#"fn first()->str{
return "來源甲"
}
fn second()->str{
return "來源乙"
}
fn consume(left:str,right:str)->str{
return left.move()
}
fn main()->int{
const received:str = consume(first(),second())
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        let second_return = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(2)
        )).expect("第二個真函式必有自己的 str 返回身分");
        out.events[second_return] = PrivateScalarCallEvent::StrReturned(1);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }
}
