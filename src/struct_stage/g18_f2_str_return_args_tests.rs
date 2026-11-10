// G18/P18.1-F2：原 Checker 真 str 返回與 int/bool/str 混合實參，
// 在單一 OwnedUtf8/ResourceId 與 HC14 BorrowTicket 動態帳下驗收。
#[cfg(test)]
mod g18_f2_str_return_args_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 真 str 返回工作圖不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_str_args_整數布林混型別正確求值才交付真utf8() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn create(n:int,flag:bool)->str{
return "混型別🪼"
}
fn main()->int{
const received:str = create(3,true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        let arg = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::ArgumentEvaluated(3)
        )).unwrap();
        let flag = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolArgumentEvaluated(true)
        )).unwrap();
        let entry = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "create"
        )).unwrap();
        let exit = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(1)
        )).unwrap();
        let accepted = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).unwrap();
        assert!(arg < flag && flag < entry && entry < exit && exit < accepted);
    }

    #[test]
    fn f2_str_args_先真借用外部字串後再接收返回資源且兩者各自釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn create(source:str,n:int,flag:bool)->str{
return "新返回🪼"
}
fn main()->int{
const input:str = "原資源"
const output:str = create(input,4,true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 2)
        )).count(), 1);
    }

    #[test]
    fn f2_str_args_移動參數再從原函式返回同一resourceid不許二次建立() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn handoff(source:str,flag:bool)->str{
return source.move()
}
fn main()->int{
var original:str = "唯一持有者🪼"
const received:str = handoff(original.move(),true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred {
                resource_id: 1, ..
            }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).count(), 1);
    }

    #[test]
    fn f2_str_args_整數子函式先真返回後作為str返回實參() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn plus(n:int)->int{
return n+1
}
fn create(n:int)->str{
return "真的utf8"
}
fn main()->int{
const output:str = create(plus(3))
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        let names = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(names, ["plus", "create"]);
    }

    #[test]
    fn f2_str_args_借用後續整數溢位不進callee不假str返回仍還票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn create(source:str,n:int)->str{
return "不應建立"
}
fn main()->int{
const original:str = "保留"
const output:str = create(original,9223372036854775807+1)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_args_移動意圖後續溢位不得提前提交資源轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn create(source:str,n:int)->str{
return "不應建立"
}
fn main()->int{
var original:str = "仍由原框架"
const output:str = create(original.move(),9223372036854775807+1)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { .. }
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_args_原checker拒絕整數字串實參錯置及不合法複製() {
        let wrong_type = r#"fn create(n:int)->str{
return "x"
}
fn main()->int{
const owner:str = "x"
const received:str = create(owner)
return 0
}
"#;
        let implicit_copy = r#"fn create(source:str)->str{
return source
}
fn main()->int{
const owner:str = "x"
const received:str = create(owner)
return 0
}
"#;
        for src in [wrong_type, implicit_copy] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))
            ));
        }
    }

    #[test]
    fn f2_str_args_偽造返回接受resourceid即使進退筆數平衡也必拒絕() {
        let src = r#"fn create(n:int,flag:bool)->str{
return "不可偽造"
}
fn main()->int{
const received:str = create(2,true)
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut outcome = plan.execute().unwrap();
        let idx = outcome.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(..)
        )).unwrap();
        if let PrivateScalarCallEvent::StrReturnAccepted(id, _) = outcome.events[idx] {
            outcome.events[idx] = PrivateScalarCallEvent::StrReturnAccepted(id, 99999);
        }
        assert!(!outcome.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(outcome),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }
}
