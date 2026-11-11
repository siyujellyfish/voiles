// G18/P18.1-F2：原 Lexer 的字串常值直接作為 str 實參。
// 檢查無具名持有者、真 ResourceId、逐幀清理與 trace 來源防偽。
#[cfg(test)]
mod g18_f2_literal_str_temporary_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 原 Lexer str 常值跨框架帳須閉合");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_literal_arg_直接utf8字面作str形參原id移交並真正返回() {
        let out = run(r#"fn pass(value:str)->str{
return value.move()
}
fn main()->int{
const received:str = pass("字面🪼")
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Returned(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrLiteralTemporaryCreated(1)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )).count(), 1);
    }

    #[test]
    fn f2_literal_arg_int返回接收字面並在子框架清理() {
        let out = run(r#"fn consume(value:str,n:int)->int{
return n+3
}
fn main()->int{
const answer:int = consume("參數",4)
return answer
}
"#);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn f2_literal_arg_bool返回接收字面沒有整數偽裝() {
        let out = run(r#"fn confirm(value:str)->bool{
return true
}
fn main()->int{
const accepted:bool = confirm("同意")
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )));
    }

    #[test]
    fn f2_literal_arg_兩個不同字面資源按實參原序移交且只各釋放一次() {
        let out = run(r#"fn choose(left:str,right:str)->str{
return left.move()
}
fn main()->int{
const chosen:str = choose("來源甲","來源乙")
return 0
}
"#);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Returned(1), R::Released(2), R::Released(1),
        ]);
        let ids = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::StrTemporaryTransferred { resource_id, .. } =>
                Some(*resource_id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn f2_literal_arg_完成字面後序溢位零子框架零轉交但必釋放() {
        let out = run(r#"fn consume(value:str,n:int)->int{
return n
}
fn main()->int{
const answer:int = consume("僅暫存",9223372036854775807+1)
return answer
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::StrTemporaryTransferred { .. }
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "consume"
        )));
    }

    #[test]
    fn f2_literal_arg_被呼叫端溢位仍從真子框架單次清理() {
        let out = run(r#"fn fail(value:str)->int{
const overflow:int = 9223372036854775807+1
return 7
}
fn main()->int{
const answer:int = fail("已交付")
return answer
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited { name, returned: false }
                if name == "fail"
        )));
    }

    #[test]
    fn f2_literal_arg_字面先交內層返回再交外層始終同一resourceid() {
        let out = run(r#"fn echo(value:str)->str{
return value.move()
}
fn consume(payload:str)->int{
return 7
}
fn main()->int{
const answer:int = consume(echo("鏈式資源"))
return answer
}
"#);
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Returned(1),
            R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrTemporaryTransferred { resource_id: 1, .. }
        )).count(), 2);
    }

    #[test]
    fn f2_literal_arg_偽造常值暫存id及移除來源事件必遭真稽核拒絕() {
        let src = r#"fn consume(value:str)->int{
return 3
}
fn main()->int{
const answer:int = consume("不可偽造")
return answer
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrLiteralTemporaryCreated(_)
        )).unwrap();
        out.events[at] = PrivateScalarCallEvent::StrLiteralTemporaryCreated(999_999);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let mut removed = plan.execute().unwrap();
        let idx = removed.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrLiteralTemporaryCreated(_)
        )).unwrap();
        removed.events.remove(idx);
        assert!(!removed.has_balanced_frame_trace());
    }
}
