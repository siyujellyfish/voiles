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
}
