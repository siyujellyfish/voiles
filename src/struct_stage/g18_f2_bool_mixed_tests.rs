// G18 F2：bool 函式已核准 int/bool 混合形式參數與獨立 bool 返回。
// 不引入任何 int/bool 隱式轉型；使用原 Checker/TypedHIR 真動態框架。
#[cfg(test)]
mod g18_f2_bool_mixed_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_mixed_同步bool葉函式可綁定int與bool後正確返回() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(3,true),4)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ParameterBound(_, 3)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_mixed_多個混型別參數按真左到右順序綁定並用於if() {
        let source = r#"fn ready(first:bool,n:int,last:bool)->bool{
return last
}
fn calc(n:int)->int{
if(ready(false,n+2,true)){
return n+1
}
return n
}
fn main()->int{
return calc(4)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        let seen = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert!(seen.contains(&6));
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn f2_bool_mixed_可寫bool局部兩次由帶整數參數函式完成返回後更新() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
var state:bool = ready(n+1,false)
state = ready(n+2,true)
if(state){
return n+3
}
return n
}
fn main()->int{
return calc(2)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, false)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_mixed_整數子函式可先真返回並作為布林函式的int實參() {
        let source = r#"fn inc(n:int)->int{
return n+1
}
fn ready(n:int,flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(inc(3),true),4)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inc", "ready", "pick"]);
    }

    #[test]
    fn f2_bool_mixed_布林葉函式返回另一帶int實參布林函式結果() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn relay(flag:bool)->bool{
return ready(4,flag)
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(relay(true),3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 2);
    }

    #[test]
    fn f2_bool_mixed_整數實參溢位不可進入布林函式或偽造布林返回() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
return n
}
fn main()->int{
return pick(ready(9223372036854775807+1,true),3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(_)
        )));
    }

    #[test]
    fn f2_bool_mixed_非法int與bool位置互換由原checker先拒絕() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
return n
}
fn main()->int{
return pick(ready(true,3),4)
}
"#;
        assert!(matches!(PreparedPrivateScalarCall::prepare(source),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn f2_bool_mixed_竄改已編譯形式參數型別即使返回bool仍在執行前拒絕() {
        let source = r#"fn ready(n:int,flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
return n
}
fn main()->int{
return pick(ready(3,true),4)
}
"#;
        let mut plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        let target = plan.nested_callees.iter_mut()
            .find(|fun| fun.name == "ready").unwrap();
        target.parameter_is_bool[0] = true;
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }
}
