// G18／P18.1-F2：原 Checker 已核准同步 str 呼叫作另一函式直接 return。
// 原內層 Returned 與外層第二次 Returned 必須是同一 OwnedUtf8／ResourceId。
#[cfg(test)]
mod g18_f2_str_forward_return_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let prepared = PreparedPrivateScalarCall::prepare(source).unwrap();
        let outcome = prepared.execute().unwrap();
        assert!(outcome.has_balanced_frame_trace(), "F2 直接 str call-return 雙帳不平衡");
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        outcome
    }

    #[test]
    fn f2_forward_str_兩層直接函式返回保留唯一utf8資源() {
        let out = run(r#"fn make()->str{
return "中文🪼"
}
fn relay()->str{
return make()
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
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(1)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).count(), 1);
    }

    #[test]
    fn f2_forward_str_三層直接返回不複製資源id也不丟失動態框架() {
        let out = run(r#"fn make()->str{
return "三層"
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
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Returned(1),
            R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(1)
        )).count(), 3);
    }

    #[test]
    fn f2_forward_str_直接返回結果再作下一函式str實參同id() {
        let out = run(r#"fn make()->str{
return "交給第三人"
}
fn relay()->str{
return make()
}
fn consume(value:str)->int{
return 8
}
fn main()->int{
const answer:int = consume(relay())
return answer
}
"#);
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Returned(1),
            R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(e,
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )).count(), 1);
    }

    #[test]
    fn f2_forward_str_混合純值形參向內轉交不虛構返回資源() {
        let out = run(r#"fn make(n:int,flag:bool)->str{
return "純值輸入"
}
fn relay(n:int)->str{
return make(n,true)
}
fn main()->int{
const received:str = relay(4)
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Returned(1), R::Released(1),
        ]);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolArgumentEvaluated(true)
        )));
    }

    #[test]
    fn f2_forward_str_提前if命中直接返回子函式不建未達字面() {
        let out = run(r#"fn make()->str{
return "到達"
}
fn relay(flag:bool)->str{
if (flag) {
return make()
}
return "沒到達"
}
fn main()->int{
const received:str = relay(true)
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(true)
        )).count(), 1);
    }

    #[test]
    fn f2_forward_str_內層建資源後溢位必逐框架回滾不捏造returned() {
        let out = run(r#"fn fail()->str{
const created:str = "尚未返回"
const overflow:int = 9223372036854775807+1
return created.move()
}
fn relay()->str{
return fail()
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {returned: false, ..}
        )).count(), 2);
    }

    #[test]
    fn f2_forward_str_前置算式溢位零callee與零資源返回() {
        let out = run(r#"fn make(n:int)->str{
return "不可達"
}
fn relay()->str{
return make(9223372036854775807+1)
}
fn main()->int{
const received:str = relay()
return 0
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, []);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "make"
        )));
    }

    #[test]
    fn f2_forward_str_偽造第二次returned的另一資源及錯型別原checker拒絕() {
        let source = r#"fn make()->str{
return "不可偽造"
}
fn relay()->str{
return make()
}
fn main()->int{
const received:str = relay()
return 0
}
"#;
        let prepared = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = prepared.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateScalarCallEvent::OwnedResource(R::Returned(1)))
                .then_some(index)
        ).collect::<Vec<_>>();
        assert_eq!(returned.len(), 2);
        out.events[returned[1]] = PrivateScalarCallEvent::OwnedResource(R::Returned(9999));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        assert!(matches!(PreparedPrivateScalarCall::prepare(r#"fn make()->int{
return 1
}
fn relay()->str{
return make()
}
fn main()->int{
const received:str = relay()
return 0
}
"#), Err(PrivateScalarCallPrepareError::Check(_))));
    }
}
