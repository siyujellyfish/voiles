// G18／P18.1-F2：原 Checker 巢狀 str-return 真 OwnedUtf8 直接交參。
// 必須驗收同一 ResourceId 與正式 R05 軌跡；不聲稱 F2 已全部閉合。
#[cfg(test)]
mod g18_f2_nested_str_temporary_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "巢狀 str 臨時值跨框架雙帳必須閉合");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_nested_str_單字串內層返回原id直接交付外層返回沒有複製() {
        let out = run(r#"fn make()->str{
return "第一個🪼"
}
fn pass(value:str)->str{
return value.move()
}
fn main()->int{
const received:str = pass(make())
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Moved(1),
            R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(e,
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(e,
            PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).count(), 1);
    }

    #[test]
    fn f2_nested_str_原int返回混合str臨時與純值形參() {
        let out = run(r#"fn make()->str{
return "獨立str"
}
fn length(value:str,n:int)->int{
return n+2
}
fn main()->int{
const answer:int = length(make(),5)
return answer
}
"#);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Moved(1), R::Released(1),
        ]);
        assert!(out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )));
    }

    #[test]
    fn f2_nested_str_原bool返回接收同一真str臨時資源() {
        let out = run(r#"fn make()->str{
return "布林驗收"
}
fn accept(value:str)->bool{
return true
}
fn main()->int{
const checked:bool = accept(make())
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(e,
            PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 1);
    }

    #[test]
    fn f2_nested_str_兩個獨立子返回以原形參順序交付唯一id() {
        let out = run(r#"fn first()->str{
return "第一"
}
fn second()->str{
return "第二"
}
fn choose(left:str,right:str)->str{
return left.move()
}
fn main()->int{
const received:str = choose(first(),second())
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Created(2), R::Returned(2),
            R::Moved(1), R::Moved(2), R::Returned(1),
            R::Released(2), R::Released(1),
        ]);
        let ids = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id, .. } =>
                Some(*resource_id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn f2_nested_str_內層已return但外層後序實參溢位零外層enter且釋放() {
        let out = run(r#"fn make()->str{
return "尚未交接"
}
fn consume(value:str,n:int)->int{
return n
}
fn main()->int{
const answer:int = consume(make(),9223372036854775807+1)
return answer
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::FunctionEntered(name) if name == "consume"
        )));
        assert!(!out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::StrTemporaryTransferred { .. }
        )));
    }

    #[test]
    fn f2_nested_str_進入外層後故障逐框架只釋放唯一資源() {
        let out = run(r#"fn make()->str{
return "須逐幀清理"
}
fn fail(value:str)->str{
const overflow:int = 9223372036854775807+1
return value.move()
}
fn main()->int{
const received:str = fail(make())
return 0
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Moved(1), R::Released(1),
        ]);
        assert!(out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::UnwindStarted
        )));
        assert!(out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::FunctionExited { name, returned: false }
                if name == "fail"
        )));
    }

    #[test]
    fn f2_nested_str_偽造暫存交付來源資源id被真trace拒絕() {
        let src = r#"fn make()->str{
return "不能偽造"
}
fn consume(value:str)->int{
return 9
}
fn main()->int{
const answer:int = consume(make())
return answer
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|e| matches!(e,
            PrivateScalarCallEvent::StrTemporaryTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::StrTemporaryTransferred { target, .. } = out.events[index] {
            out.events[index] = PrivateScalarCallEvent::StrTemporaryTransferred {
                target, resource_id: 999_999,
            };
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn f2_nested_str_原checker拒絕錯誤結果型別與非所有權實參() {
        for src in [
            r#"fn wrong()->int{
return 1
}
fn take(value:str)->int{
return 2
}
fn main()->int{
return take(wrong())
}
"#,
            r#"fn make()->str{
return "值"
}
fn take(value:str)->int{
return 2
}
fn main()->int{
return take(make().move())
}
"#,
        ] {
            assert!(PreparedPrivateScalarCall::prepare(src).is_err());
        }
    }
}
