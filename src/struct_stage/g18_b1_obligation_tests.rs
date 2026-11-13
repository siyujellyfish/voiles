// G18／P18.4-B1：Copyable int／bool 的原 Checker 真呼叫契約。
// 不經第二份 Parser／Checker、不建立額外資源身分；每筆都實跑真 Runtime。
#[cfg(test)]
mod g18_b1_obligation_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as R;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let outcome = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(outcome.has_balanced_frame_trace(), "B1 真來源框架／資源稽核失衡");
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        outcome
    }

    #[test]
    fn b1_原值重複傳入兩次普通函式仍可在主函式讀回() {
        let out = run(r#"fn plus(n:int)->int{
return n+1
}
fn main()->int{
const origin:int = 4
return plus(origin)+plus(origin)+origin
}
"#);
        assert_eq!(out.result, Ok(14));
        let args = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [4, 4]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_同一Copyable整數填入三個不同參數不移動() {
        let out = run(r#"fn sum(a:int,b:int,c:int)->int{
return a+b+c
}
fn main()->int{
const origin:int = 3
return sum(origin,origin,origin)+origin
}
"#);
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 3);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_布林局部更新後兩次傳參各讀當前Copyable值() {
        let out = run(r#"fn choose(flag:bool)->int{
if(flag){
return 5
}
return 2
}
fn main()->int{
var current:bool = false
var first:int = choose(current)
current = true
return first+choose(current)
}
"#);
        assert_eq!(out.result, Ok(7));
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [false, true]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_混型別形參依書寫順序且不消耗主函式整數() {
        let out = run(r#"fn choose(flag:bool,x:int,y:int)->int{
if(flag){
return x
}
return y
}
fn main()->int{
const origin:int = 5
return choose(true,origin,1)+choose(false,origin,2)+origin
}
"#);
        assert_eq!(out.result, Ok(12));
        let bools = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        let ints = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(bools, [true, false]);
        assert_eq!(ints, [5, 1, 5, 2]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_兄弟巢狀函式共用Copyable來源但框架互不污染() {
        let out = run(r#"fn twice(n:int)->int{
return n+n
}
fn combine(left:int,right:int)->int{
return left+right
}
fn main()->int{
const origin:int = 4
return combine(twice(origin),twice(origin))+origin
}
"#);
        assert_eq!(out.result, Ok(20));
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["twice", "twice", "combine"]);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_for逐輪Copyable來源與迭代純值不消耗() {
        let out = run(r#"fn add(a:int,b:int)->int{
return a+b
}
fn main()->int{
const origin:int = 4
var total:int = 0
for i in 0..3{
total = total+add(origin,i)
}
return total+origin
}
"#);
        assert_eq!(out.result, Ok(19));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "add"
        )).count(), 3);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn b1_共享str借用與Copyable來源雙次呼叫先還票後離域() {
        let out = run(r#"fn inspect(item:str,n:int)->int{
return n+1
}
fn main()->int{
const item:str = "借用同源"
const origin:int = 4
return inspect(item,origin)+inspect(item,origin)+origin
}
"#);
        assert_eq!(out.result, Ok(14));
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let taken = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(taken.len(), 2);
        assert_eq!(taken, returned);
        let last_return = out.events.iter().rposition(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let release = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(last_return < release);
    }

    #[test]
    fn b1_str整筆Move與Copyable來源不會連帶移走整數() {
        let out = run(r#"fn consume(item:str,n:int)->int{
return n+1
}
fn main()->int{
var item:str = "整筆轉交"
const origin:int = 8
return consume(item.move(),origin)+origin
}
"#);
        assert_eq!(out.result, Ok(17));
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 1);
    }

    #[test]
    fn b1_子函式算術溢位不冒稱正常返回或留下參數槽() {
        let out = run(r#"fn overflow(n:int)->int{
return n+1
}
fn main()->int{
const origin:int = 9223372036854775807
return overflow(origin)
}
"#);
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )));
    }

    #[test]
    fn b1_未初始化與錯型別原Checker必拒絕() {
        for source in [
            r#"fn use_value(n:int)->int{
return n
}
fn main()->int{
var missing:int
return use_value(missing)
}
"#,
            r#"fn use_value(n:int)->int{
return n
}
fn main()->int{
const wrong:bool = true
return use_value(wrong)
}
"#,
        ] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ), "必須由原 Checker 拒絕：{source}");
        }
    }

    #[test]
    fn b1_真參數身分重播即使數值一致也不得交付() {
        let source = r#"fn plus(n:int)->int{
return n+1
}
fn main()->int{
return plus(3)
}
"#;
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let mut out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let position = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).unwrap();
        let repeated = out.events[position].clone();
        out.events.insert(position + 1, repeated);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(
            plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
