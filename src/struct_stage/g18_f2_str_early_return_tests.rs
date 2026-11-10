// G18／P18.1-F2：str 字面／具名明確 Move 的合法提前 return
// 沿原 Checker、TypedHIR、P16 真 OwnedUtf8／ResourceId 同一圖驗收。
#[cfg(test)]
mod g18_f2_str_early_return_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let result = plan.execute().unwrap();
        assert!(result.has_balanced_frame_trace(), "F2 提前 str return 必須真動態平衡");
        assert_eq!(result.parameter_slots_after_exit, 0);
        result
    }

    #[test]
    fn f2_str_early_真if提前字面返回與未選中尾端不同資源() {
        for flag in ["true", "false"] {
            let src = format!(r#"fn make(flag:bool)->str{{
if(flag){{
return "提前🪼"
}}
return "尾端🪼"
}}
fn main()->int{{
const received:str = make({flag})
return 0
}}
"#);
            let out = run(&src);
            assert_eq!(out.result, Ok(0));
            assert_eq!(out.resource_events, [
                R::Created(1), R::Returned(1), R::Released(1),
            ]);
            assert_eq!(out.events.iter().filter(|event| matches!(
                event, PrivateScalarCallEvent::StrReturned(1)
            )).count(), 1);
        }
    }

    #[test]
    fn f2_str_early_完整if_else兩葉返回尾端不可達不得配置第三字串() {
        let src = r#"fn make(flag:bool)->str{
if(flag){
return "左側"
}else{
return "右側"
}
return "不可達"
}
fn main()->int{
const one:str = make(false)
const two:str = make(true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1),
            R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(_)
        )).count(), 2);
    }

    #[test]
    fn f2_str_early_提前返回前已配置兩字串須依逆序釋放旁觀資源() {
        let src = r#"fn make(flag:bool)->str{
const first:str = "第一"
const second:str = "第二"
if(flag){
return "第三"
}
return "第四"
}
fn main()->int{
const result:str = make(true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3), R::Returned(3),
            R::Released(2), R::Released(1), R::Released(3),
        ]);
    }

    #[test]
    fn f2_str_early_具名move提前轉交同一resourceid沒有重新建構() {
        let src = r#"fn make(flag:bool)->str{
var own:str = "唯一🪼"
if(flag){
return own.move()
}
return own.move()
}
fn main()->int{
const first:str = make(true)
const second:str = make(false)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1),
            R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn f2_str_early_先真借用str實參再提前返回借用票據必歸還() {
        let src = r#"fn make(source:str,flag:bool)->str{
if(flag){
return "新資源"
}
return "另一份"
}
fn main()->int{
const keep:str = "原資源"
const result:str = make(keep,true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn f2_str_early_分支前整數溢位只能清理已建資源不得假返回() {
        let src = r#"fn make(flag:bool)->str{
const original:str = "待回滾"
const n:int = 9223372036854775807+1
if(flag){
return "不能建立"
}
return "不能建立"
}
fn main()->int{
const result:str = make(true)
return 0
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::StrReturned(_)
                | PrivateScalarCallEvent::StrReturnAccepted(..)
        )));
    }

    #[test]
    fn f2_str_early_原checker拒絕提前返回錯誤型別及非法str複製() {
        let mismatch = r#"fn make(flag:bool)->str{
if(flag){
return 9
}
return "合法"
}
fn main()->int{
const result:str = make(true)
return 0
}
"#;
        let implicit_copy = r#"fn make(flag:bool)->str{
const owned:str = "禁止"
if(flag){
return owned
}
return "合法"
}
fn main()->int{
const result:str = make(true)
return 0
}
"#;
        for src in [mismatch, implicit_copy] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))
            ));
        }
    }

    #[test]
    fn f2_str_early_竄改提前返回接受resourceid即使平衡也不能冒領() {
        let src = r#"fn make(flag:bool)->str{
if(flag){
return "不能冒領"
}
return "尾端"
}
fn main()->int{
const result:str = make(true)
return 0
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let accepted = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::StrReturnAccepted(..)
        )).unwrap();
        if let PrivateScalarCallEvent::StrReturnAccepted(owner, _) = out.events[accepted] {
            out.events[accepted] = PrivateScalarCallEvent::StrReturnAccepted(owner, 9999);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
