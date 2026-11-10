// G18／F2：原 Checker 核准 bool 返回的多語句本體、提前返回與 P16 資源。
// 與 int-return 共用 Checked Source→TypedHIR→顯式工作棧，不引入隱含型別轉換。
#[cfg(test)]
mod g18_f2_bool_body_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 多語句 bool-return 真資源／框架帳不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_body_真布林函式局部宣告後返回當前值() {
        let src = r#"fn ready(flag:bool)->bool{
const local:bool = flag
return local
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(true),3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(4));
        let inited = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, true)
        )).unwrap();
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).unwrap();
        assert!(inited < returned);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_body_真布林局部可寫更新以最後值作返回() {
        let src = r#"fn ready(flag:bool)->bool{
var local:bool = false
local = flag
return local
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+2
}
return n
}
fn main()->int{
return pick(ready(true),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_body_真函式前置int與str局部必在bool返回後逆序清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(flag:bool,n:int)->bool{
const first:str = "第一份🪼"
const second:str = "第二份🪼"
const adjusted:int = n+1
return flag
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(true,3),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let bool_return = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).unwrap();
        let release = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(bool_return < release);
    }

    #[test]
    fn f2_bool_body_提前if返回true與尾端false按真動態分支求值() {
        let src = r#"fn ready(flag:bool)->bool{
if(flag){
return true
}
return false
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(true),2)+pick(ready(false),2)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolReturned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>(), [true, false]);
    }

    #[test]
    fn f2_bool_body_完整if_else兩葉已返回的尾端不可達不造假執行() {
        let src = r#"fn ready(flag:bool)->bool{
if(flag){
return true
}else{
return false
}
return true
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(false),2)+pick(ready(true),2)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolReturned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn f2_bool_body_提前返回前建立真utf8不管選哪側都只釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(flag:bool)->bool{
const source:str = "及時清理"
if(flag){
return true
}
return false
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(true),2)+pick(ready(false),2)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(_)
        )).count(), 2);
    }

    #[test]
    fn f2_bool_body_前置bool子函式真返回更新局部後由另一bool返回() {
        let src = r#"fn leaf(flag:bool)->bool{
return flag
}
fn ready(flag:bool)->bool{
var stored:bool = leaf(false)
stored = leaf(flag)
return stored
}
fn pick(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
return pick(ready(true),2)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(3));
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolReturned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>(), [false, true, true]);
    }

    #[test]
    fn f2_bool_body_提前布林返回內混入整數必由原checker拒絕() {
        let src = r#"fn ready(flag:bool)->bool{
if(flag){
return 3
}
return false
}
fn pick(flag:bool,n:int)->int{
return n
}
fn main()->int{
return pick(ready(true),3)
}
"#;
        assert!(matches!(
            PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))
        ));
    }

    #[test]
    fn f2_bool_body_本體int運算溢位不得假BoolReturned且真清理str() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(flag:bool)->bool{
const source:str = "失敗退出"
const large:int = 9223372036854775807+1
return flag
}
fn pick(flag:bool,n:int)->int{
return n
}
fn main()->int{
return pick(ready(true),2)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(_)
        )));
    }
}
