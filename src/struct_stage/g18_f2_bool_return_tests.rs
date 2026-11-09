// G18 F2：真正的同步 bool 返回只能進入 bool 參數路徑，不得當成 int。
// 源自唯一原 Parser／Checker TypedHIR；沿用原 F1/F2 真動態框架與 R05 退出帳。
#[cfg(test)]
mod g18_f2_bool_return_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_return_原checker真bool葉函式作為int函式的布林引數() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn choose(flag:bool,n:int)->int{
if(flag){
return n+2
}
return n
}
fn main()->int{
return choose(ready(true),5)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(7));
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["ready", "choose"]);
        assert_eq!(out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolReturned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>(), [true]);
        assert_eq!(out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>(), [7]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn f2_bool_return_false與true真正返回維持獨立型別與求值順序() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
if(flag){
return n+10
}
return n-1
}
fn main()->int{
return select(ready(false),5)+select(ready(true),5)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(19));
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolReturned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [false, true]);
        let bool_params = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolParameterBound(_, value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(bool_params, [false, false, true, true]);
    }

    #[test]
    fn f2_bool_return_無參數真bool函式與原checker零參數分類() {
        let source = r#"fn enabled()->bool{
return true
}
fn choose(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return choose((enabled()),4)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_return_兩層bool實參函式由真工作棧逐層退出() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(ready(true)),3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(4));
        let exited = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, .. } => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exited, ["ready", "ready", "pick"]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 2);
    }

    #[test]
    fn f2_bool_return_父函式字串借用在bool子工作結束後安全歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn inspect(item:str,flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
const owner:str = "外部🪼"
return inspect(owner,ready(true),3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(returned < released);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_return_不得冒用整數返回或竄改bool函式原始計畫() {
        let bad = r#"fn ready(flag:bool)->bool{
return flag
}
fn main()->int{
return ready(true)
}
"#;
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n
}
return 0
}
fn main()->int{
return pick(ready(true),3)
}
"#;
        let mut prepared = PreparedPrivateScalarCall::prepare(source).unwrap();
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        let leaf = prepared.nested_callees.iter_mut()
            .find(|fun| fun.name == "ready").unwrap();
        leaf.result = PrivateScalarReturnExpr::Constant(1);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }
}
