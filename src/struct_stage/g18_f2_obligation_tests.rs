// G18／P18.1-F2：原 Checker 已核准 int/bool/str 混合實參、返回與資源退出。
#[cfg(test)]
mod g18_f2_obligation_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 真動態退出證據不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_純值布林整數同一函式按來源順序求值並重複讀取() {
        let source = r#"fn choose(flag:bool,n:int,delta:int)->int{
if(flag){
return n+delta
}
return n-delta
}
fn main()->int{
const n:int = 4
var flag:bool = false
return choose(flag,n,1)+choose(true,n,2)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "choose"
        )).count(), 2);
        let bools = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(value) => Some(*value),
            _ => None
        }).collect::<Vec<_>>();
        assert_eq!(bools, [false, true]);
        let ints = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None
        }).collect::<Vec<_>>();
        assert_eq!(ints, [4, 1, 4, 2]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn f2_字串共享借用布林與整數雙次傳入均各自歸票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn inspect(item:str,flag:bool,n:int)->int{
if(flag){
return n+2
}
return n-1
}
fn main()->int{
const owner:str = "共用🪼"
return inspect(owner,true,5)+inspect(owner,false,5)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(11));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acquired = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired);
    }

    #[test]
    fn f2_同一呼叫字串整筆移動加另一字串借用與布林整數不混帳() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn take(moved:str,observed:str,flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
var moved:str = "轉交"
const watched:str = "旁觀"
return take(moved.move(),watched,true,5)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        let loan_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let owner_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(loan_end < owner_end);
    }

    #[test]
    fn f2_純值布林選擇不得改變不同參數來源身分() {
        let source = r#"fn choose(flag:bool,low:int,high:int)->int{
if(flag){
return high
}
return low
}
fn main()->int{
var pick:bool = true
const a:int = 2
const b:int = 9
var value:int = choose(pick,a,b)
pick = false
return value+choose(pick,a,b)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Ok(11));
        let bindings = out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolParameterBound(..)
        )).count();
        assert_eq!(bindings, 2);
    }

    #[test]
    fn f2_借用後續整數實參溢位由呼叫者真還票且不進函式() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn inspect(item:str,flag:bool,n:int)->int{
return n
}
fn main()->int{
const watched:str = "尚未進入"
return inspect(watched,true,9223372036854775807+1)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
        let acquired = out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count();
        let returned = out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count();
        assert_eq!((acquired, returned), (1, 1));
    }

    #[test]
    fn f2_移動後續整數溢位不能提前轉交也不能重複釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn receive(item:str,flag:bool,n:int)->int{
return n
}
fn main()->int{
var owned:str = "尚未移交"
return receive(owned.move(),false,9223372036854775807+1)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "receive"
        )));
    }

    #[test]
    fn f2_成功移入子框架後溢位由目前所有者完成完整資源清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn receive(item:str,flag:bool,n:int)->int{
var local:str = "子框架"
if(flag){
return n+9223372036854775807
}
return n
}
fn main()->int{
var owned:str = "原持有者"
return receive(owned.move(),true,1)
}
"#;
        let out = run(source);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Created(2),
            R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn f2_跨形參錯型別與借移同源衝突由原checker拒絕() {
        for source in [
            r#"fn choose(flag:bool,n:int)->int{
return n
}
fn main()->int{
return choose(3,true)
}
"#,
            r#"fn receive(item:str,n:int)->int{
return n
}
fn main()->int{
var owner:str = "原"
return receive(owner.move(),owner)
}
"#,
            r#"fn receive(item:str,n:int)->int{
return n
}
fn main()->int{
var owner:str = "原"
var duplicate:str = owner
return receive(owner.move(),3)
}
"#,
        ] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ), "非法參數不得繞過 Checker：{source}");
        }
    }

    #[test]
    fn f2_跨資源借用票據事件偽造即使計數不變也不得交付() {
        let source = r#"fn inspect(item:str,flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
const observed:str = "不可偽造"
return inspect(observed,true,2)
}
"#;
        let mut out = run(source);
        let index = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanResourceLinked(..)
        )).unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(ticket, _) = out.events[index].clone() {
            out.events[index] = PrivateScalarCallEvent::LoanResourceLinked(ticket, 99999);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
