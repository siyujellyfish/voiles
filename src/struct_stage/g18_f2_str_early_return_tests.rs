// G18/P18.1-F2：原 Checker 的 str-return 函式在 if／else-if／else
// 合法提前返回真 OwnedUtf8，同一資源事件與正常／故障逐框架退出。
#[cfg(test)]
mod g18_f2_str_early_return_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 str 提前返回的真正框架／資源事件不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_str_early_if真分支只建立與返回選定的utf8() {
        let source = r#"fn pick(flag:bool)->str{
if(flag){
return "先回傳🪼"
}
return "尾端"
}
fn main()->int{
const selected:str = pick(true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(1)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::OwnedResource(R::Created(2)))));
    }

    #[test]
    fn f2_str_early_if假分支回到尾端真move維持唯一resourceid() {
        let source = r#"fn pick(flag:bool)->str{
var original:str = "原生唯一"
if(flag){
return "不能選"
}
return original.move()
}
fn main()->int{
const selected:str = pick(false)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, 1)
        )).count(), 1);
    }

    #[test]
    fn f2_str_early_else_if完整三分支僅各自真返回且逆序釋放() {
        let source = r#"fn pick(a:bool,b:bool)->str{
if(a){
return "甲🪼"
}else if(b){
return "乙🪼"
}else{
return "丙🪼"
}
return "不可達"
}
fn main()->int{
const first:str = pick(false,true)
const second:str = pick(false,false)
const third:str = pick(true,false)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1),
            R::Created(2), R::Returned(2),
            R::Created(3), R::Returned(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
        let returned = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::StrReturned(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [1, 2, 3]);
    }

    #[test]
    fn f2_str_early局部move提前返回與旁觀資源各自唯一釋放() {
        let source = r#"fn pick(flag:bool)->str{
const neighbor:str = "留在函式"
var payload:str = "移交給父框架"
if(flag){
return payload.move()
}
return "另外返回"
}
fn main()->int{
const selected:str = pick(true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Returned(2),
            R::Released(1), R::Released(2),
        ]);
        let exit = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited { name, returned: true }
            if name == "pick"
        )).unwrap();
        let release_neighbor = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(release_neighbor < exit);
    }

    #[test]
    fn f2_str_early內層條件真返回僅選擇原checker分支() {
        let source = r#"fn pick(outer:bool,inner:bool)->str{
if(outer){
if(inner){
return "內層真"
}else{
return "內層假"
}
}
return "外層假"
}
fn main()->int{
const a:str = pick(true,false)
const b:str = pick(false,true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
        )).count(), 2);
    }

    #[test]
    fn f2_str_early溢位不執行未達分支返回且真清理已建str() {
        let source = r#"fn pick(flag:bool)->str{
const already:str = "已建立"
const failed:int = 9223372036854775807+1
if(flag){
return "不應建立"
}
return "未返回"
}
fn main()->int{
const selected:str = pick(true)
return 0
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_early原checker拒絕分支錯型別與隱含str複製() {
        for source in [
            r#"fn pick(flag:bool)->str{
if(flag){
return 1
}
return "合法"
}
fn main()->int{
const selected:str = pick(true)
return 0
}
"#,
            r#"fn pick(flag:bool)->str{
const value:str = "不可複製"
if(flag){
return value
}
return "合法"
}
fn main()->int{
const selected:str = pick(true)
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
    fn f2_str_early篡改提前返回resourceid即使資源數相等仍拒絕() {
        let source = r#"fn pick(flag:bool)->str{
if(flag){
return "不可偽造"
}
return "另一條"
}
fn main()->int{
const selected:str = pick(true)
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let idx = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrReturned(_)
        )).unwrap();
        out.events[idx] = PrivateScalarCallEvent::StrReturned(99999);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
