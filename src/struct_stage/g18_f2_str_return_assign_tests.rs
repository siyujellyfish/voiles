// G18／P18.1-F2：原 Checker 核准未初始化 var str 首次接收 str-return，
// 既有已初始化 str 的函式返回覆寫維持 P16 靜態保守拒絕。
#[cfg(test)]
mod g18_f2_str_return_assign_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 首次 str-return 賦值雙帳須平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_str_assign_未初始化var帶整數布林形參真接受唯一utf8資源() {
        let source = r#"fn make(n:int,flag:bool)->str{
return "第二份🪼"
}
fn main()->int{
var output:str
output = make(2,true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Released(1),
        ]);
        let argument = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(2)
        )).unwrap();
        let flag = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::BoolArgumentEvaluated(true)
        )).unwrap();
        let accepted = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).unwrap();
        assert!(argument < flag && flag < accepted);
    }

    #[test]
    fn f2_str_assign_未初始化var接管str返回同一id並於退出唯一釋放() {
        let source = r#"fn make()->str{
return "延後賦值"
}
fn main()->int{
var output:str
output = make()
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).count(), 1);
    }

    #[test]
    fn f2_str_assign_已初始化var函式覆寫及重複覆寫仍由原checker拒絕() {
        let first = r#"fn make()->str{
return "真返回"
}
fn main()->int{
var output:str = "舊"
output = make()
return 0
}
"#;
        let again = r#"fn make()->str{
return "真返回"
}
fn main()->int{
var output:str
output = make()
output = make()
return 0
}
"#;
        for source in [first, again] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ));
        }
    }

    #[test]
    fn f2_str_assign_借用實參後溢位不進callee不交付且原資源唯一清理() {
        let source = r#"fn make(source:str,n:int)->str{
return "不可建"
}
fn main()->int{
const input:str = "原來源"
var output:str
output = make(input,9223372036854775807+1)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_assign_const與錯誤型別仍由原checker拒絕不得建立資源() {
        let immutable = r#"fn make()->str{
return "合法返回"
}
fn main()->int{
const output:str = "不可改"
output = make()
return 0
}
"#;
        let wrong_type = r#"fn make()->int{
return 4
}
fn main()->int{
var output:str
output = make()
return 0
}
"#;
        for source in [immutable, wrong_type] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ));
        }
    }

    #[test]
    fn f2_str_assign_偽造首次接收resourceid即使釋放數平衡也須拒絕() {
        let source = r#"fn make()->str{
return "新"
}
fn main()->int{
var output:str
output = make()
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let idx = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::StrReturnAccepted(..)
        )).unwrap();
        if let PrivateScalarCallEvent::StrReturnAccepted(id, _) = out.events[idx] {
            out.events[idx] = PrivateScalarCallEvent::StrReturnAccepted(id, 9999);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
