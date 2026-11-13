// G18／P18.1-F3：原 Checker 已核准的普通函式出現於
// 初始化、賦值、return、if/else、for/while 的同一真動態工作圖。
// 不新增可執行語法，也不將尚未訪問的分支視為已經產生資源。
#[cfg(test)]
mod g18_f3_call_site_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "F3 真動態函式圖／借用退出不平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    fn entered(out: &PrivateScalarCallOutcome) -> Vec<&str> {
        out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect()
    }

    #[test]
    fn f3_sites_主函式同一圖按初始化賦值分支與返回順序完成四呼叫() {
        let out = run(r#"fn increment(n:int)->int{
return n+1
}
fn jump(n:int)->int{
return n+2
}
fn main()->int{
var value:int = increment(2)
value = jump(value)
if(true){
value = increment(value)
}
return jump(value)
}
"#);
        assert_eq!(out.result, Ok(8));
        assert_eq!(entered(&out), ["increment", "jump", "increment", "jump"]);
        let returned = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::Returned(n) => Some(*n), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [3, 5, 6, 8]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn f3_sites_未選if葉不呼叫而選中else借用完成先還票後離域() {
        let out = run(r#"fn inspect(item:str,n:int)->int{
return n+1
}
fn unused(n:int)->int{
return n+100
}
fn main()->int{
var answer:int = 0
if(false){
answer = unused(100)
}else{
const owner:str = "選中🪼"
answer = inspect(owner,4)
}
return answer
}
"#);
        assert_eq!(out.result, Ok(5));
        assert_eq!(entered(&out), ["inspect"]);
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(returned < released, "選葉的真借用必先歸還再退出持有者");
    }

    #[test]
    fn f3_sites_for三輪逐次呼叫並於每輪退出真utf8與借用() {
        let out = run(r#"fn inspect(item:str,n:int)->int{
return n+1
}
fn main()->int{
var total:int = 0
for i in 0..3{
const item:str = "逐輪🪼"
total = total+inspect(item,i)
}
return total
}
"#);
        assert_eq!(out.result, Ok(6));
        assert_eq!(entered(&out), ["inspect", "inspect", "inspect"]);
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Released(2),
            R::Created(3), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 3);
    }

    #[test]
    fn f3_sites_while真首輪呼叫後break不得重放返回或第二輪() {
        let out = run(r#"fn value(n:int)->int{
return n+2
}
fn main()->int{
var active:bool = true
var total:int = 0
while(active){
total = value(5)
break
}
return total
}
"#);
        assert_eq!(out.result, Ok(7));
        assert_eq!(entered(&out), ["value"]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::Returned(7)
        )).count(), 1);
    }

    #[test]
    fn f3_sites_選中if的被呼叫者溢位後未到達後續函式() {
        let out = run(r#"fn fail(n:int)->int{
return n+1
}
fn fallback()->int{
return 9
}
fn main()->int{
if(true){
return fail(9223372036854775807)
}
return fallback()
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(entered(&out), ["fail"]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(9)
        )));
    }

    #[test]
    fn f3_sites_改寫已編譯主函式動態返回圖即使框架平衡仍先拒絕() {
        let source = r#"fn step(n:int)->int{
return n+1
}
fn main()->int{
var value:int = step(2)
return value
}
"#;
        let mut plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let original = plan.execute().unwrap();
        assert_eq!(original.result, Ok(3));
        assert!(original.has_balanced_frame_trace());
        plan.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }
}
