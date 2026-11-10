// G18/P18.1-F2：原 Checker 真 str-return 的多 ResourceId 與 BorrowTicket
// 同一次呼叫的正常/故障/防偽；不替未核准巢狀 str 參數補定新語意。
#[cfg(test)]
mod g18_f2_str_multi_resource_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let outcome = plan.execute().unwrap();
        assert!(outcome.has_balanced_frame_trace(), "F2 多資源真呼叫圖雙帳不平衡");
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        outcome
    }

    #[test]
    fn f2_str_multi_雙獨立唯讀字串借用再接收第三個唯一返回資源() {
        let source = r#"fn make(first:str,second:str,n:int)->str{
return "新資源🪼"
}
fn main()->int{
const first:str = "來源甲"
const second:str = "來源乙"
const received:str = make(first,second,5)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Returned(3), R::Released(3), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 3)
        )).count(), 1);
    }

    #[test]
    fn f2_str_multi_另一個資源借用時真move並返回同一id沒有重配() {
        let source = r#"fn handoff(payload:str,observer:str,flag:bool)->str{
return payload.move()
}
fn main()->int{
var original:str = "唯一🪼"
const observer:str = "旁觀者"
const received:str = handoff(original.move(),observer,true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Returned(1), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred {
                resource_id: 1, ..
            }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn f2_str_multi_前序借用和move意圖後續溢位須全部回滾不提前移交() {
        let source = r#"fn handoff(observer:str,payload:str,n:int)->str{
return payload.move()
}
fn main()->int{
const observer:str = "唯讀"
var payload:str = "不得提前移動"
const received:str = handoff(observer,payload.move(),9223372036854775807+1)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { .. }
                | PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_multi_兩張borrowticket後續溢位以逆序還票且零callee() {
        let source = r#"fn make(first:str,second:str,n:int)->str{
return "不可到達"
}
fn main()->int{
const one:str = "甲"
const two:str = "乙"
const received:str = make(one,two,9223372036854775807+1)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "make"
        )));
    }

    #[test]
    fn f2_str_multi_同一旁觀資源跨兩次返回借用票據不殘留() {
        let source = r#"fn make(observer:str,n:int)->str{
return "另建UTF8"
}
fn main()->int{
const observer:str = "長期存活"
const first:str = make(observer,1)
const second:str = make(observer,2)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Returned(2),
            R::Created(3), R::Returned(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn f2_str_multi_函式進入後溢位必歸還外票再釋放各自擁有者() {
        let source = r#"fn fail(payload:str,observer:str,n:int)->str{
const temporary:str = "內層已建立"
const overflow:int = 9223372036854775807+1
return payload.move()
}
fn main()->int{
var payload:str = "真轉交"
const observer:str = "旁觀借用"
const received:str = fail(payload.move(),observer,3)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Created(3), R::Released(3), R::Released(1), R::Released(2),
        ]);
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let release = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(3))
        )).unwrap();
        assert!(returned < release);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_multi_同源重複獨占借用及隱含拷貝原checker先拒絕() {
        for source in [
            r#"fn make(a:str,b:str)->str{
return "新"
}
fn main()->int{
var original:str = "不得同借"
const received:str = make(original,original)
return 0
}
"#,
            r#"fn make(source:str)->str{
return source
}
fn main()->int{
const original:str = "不可拷貝"
const received:str = make(original)
return 0
}
"#,
        ] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ));
        }
    }

    #[test]
    fn f2_str_multi_偽造另一resourceid的借用連結即使計數不變也拒絕() {
        let source = r#"fn make(first:str,second:str)->str{
return "第三份"
}
fn main()->int{
const first:str = "甲"
const second:str = "乙"
const received:str = make(first,second)
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let idx = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanResourceLinked(..)
        )).unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(ticket, id) = out.events[idx] {
            out.events[idx] =
                PrivateScalarCallEvent::LoanResourceLinked(ticket, if id == 1 { 2 } else { 1 });
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }
}
