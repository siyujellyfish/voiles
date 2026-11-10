// G18／F2：已核准同步 bool-return 的 str 真借用與 Move，
// 保留原 Lexer／Checker／TypedHIR→HC14 BorrowTicket→P16 ResourceId 唯一責任。
#[cfg(test)]
mod g18_f2_bool_str_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F2 bool+str 真資源退出未平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_str_共享借用bool返回後真歸票且持有者最後釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(s:str,flag:bool,n:int)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
const owner:str = "🪼借用"
return select(ready(owner,true,3),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acq = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        let ret = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acq.len(), 1);
        assert_eq!(ret, acq);
        let bool_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).unwrap();
        let loan_end = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let release = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(bool_end < loan_end && loan_end < release);
    }

    #[test]
    fn f2_bool_str_明確Move只轉交一次真ResourceId且子框架負責釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(s:str,flag:bool,n:int)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
var owner:str = "🪼移動"
return select(ready(owner.move(),true,3),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred {
                resource_id: 1, ..
            }
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn f2_bool_str_同一布林返回呼叫可混合兩str獨立借用與int_bool() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(a:str,n:int,b:str,flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
if(flag){
return n+1
}
return n
}
fn main()->int{
const first:str = "甲"
const second:str = "乙"
return select(ready(first,3,second,true),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn f2_bool_str_借用後續int求值溢位不可錯造函式成功或遺失借用() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(s:str,n:int,flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
return n
}
fn main()->int{
const owner:str = "早借用"
return select(ready(owner,9223372036854775807+1,true),3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(_)
        )));
    }

    #[test]
    fn f2_bool_str_移動後續int失敗不能提前轉交ResourceId() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(s:str,n:int,flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
return n
}
fn main()->int{
var owner:str = "尚未移入"
return select(ready(owner.move(),9223372036854775807+1,true),3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
    }

    #[test]
    fn f2_bool_str_不同來源Move與共享借用同批必維持各自責任() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(moved:str,watched:str,flag:bool,n:int)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
return n+1
}
fn main()->int{
var moved:str = "轉交"
const watched:str = "觀測"
return select(ready(moved.move(),watched,true,3),4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { .. }
        )).count(), 1);
    }

    #[test]
    fn f2_bool_str_使用str作bool型別實參仍由原Checker拒絕() {
        let src = r#"fn ready(s:str,flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
return n
}
fn main()->int{
const owner:str = "不可變布林"
return select(ready(owner,owner),3)
}
"#;
        assert!(matches!(
            PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))
        ));
    }

    #[test]
    fn f2_bool_str_篡改來源ResourceId或BorrowTicket身分即使筆數平衡也拒絕() {
        let src = r#"fn ready(s:str,flag:bool)->bool{
return flag
}
fn select(flag:bool,n:int)->int{
return n
}
fn main()->int{
const owner:str = "受控資源"
return select(ready(owner,true),3)
}
"#;
        let mut out = run(src);
        let i = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanResourceLinked(..)
        )).unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(ticket, _) = out.events[i].clone() {
            out.events[i] = PrivateScalarCallEvent::LoanResourceLinked(ticket, 32767);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
