// G18／F2：已核准 bool 回傳可作原普通函式的真局部初始化及更新。
// 只沿用原 Checker TypedHIR、既有動態 LIFO 框架與 P16/HC14 資源帳。
#[cfg(test)]
mod g18_f2_bool_local_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_local_真bool返回初始化const局部再作條件選擇() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
const allowed:bool = ready(true)
if(allowed){
return n+2
}
return n
}
fn main()->int{
return calc(3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(5));
        let bool_return = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).unwrap();
        let local = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, true)
        )).unwrap();
        assert!(bool_return < local);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_local_真bool返回更新可寫局部並讀取最新值() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
var flag:bool = false
flag = ready(true)
if(flag){
return n+1
}
return n
}
fn main()->int{
return calc(8)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, false)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_local_未初始化var布林由真正子函式返回後才首次綁定() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
var flag:bool
flag = ready(false)
if(flag){
return n+1
}
return n
}
fn main()->int{
return calc(4)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(4));
        let declared = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolDeclaredEmpty(_)
        )).unwrap();
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(false)
        )).unwrap();
        let initialized = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, false)
        )).unwrap();
        assert!(declared < returned && returned < initialized);
    }

    #[test]
    fn f2_bool_local_雙層巢狀bool函式先返回才真建立局部值() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
const flag:bool = ready(ready(true))
if(flag){
return n+1
}
return n
}
fn main()->int{
return calc(2)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(3));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_local_布林函式可直接返回另一個已核准的布林子函式() {
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn relay(flag:bool)->bool{
return ready(flag)
}
fn calc(n:int)->int{
const flag:bool = relay(true)
if(flag){
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
        let names = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(names, ["calc", "relay", "ready"]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 2);
    }

    #[test]
    fn f2_bool_local_資源持有者與bool局部順序清理不混淆() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn ready(flag:bool)->bool{
return flag
}
fn calc(n:int)->int{
var owner:str = "真資源🪼"
var flag:bool = ready(true)
if(flag){
return n+1
}
return n
}
fn main()->int{
return calc(3)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let bool_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEnded(_)
        )).unwrap();
        let resource_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(bool_end < resource_end);
    }

    #[test]
    fn f2_bool_local_整數冒充bool初始值由原checker拒絕() {
        let source = r#"fn step(n:int)->int{
return n+1
}
fn calc(n:int)->int{
const flag:bool = step(n)
if(flag){
return n
}
return 0
}
fn main()->int{
return calc(2)
}
"#;
        assert!(matches!(
            PreparedPrivateScalarCall::prepare(source),
            Err(PrivateScalarCallPrepareError::Check(_))
        ));
    }
}
