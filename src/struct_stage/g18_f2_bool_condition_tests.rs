// G18 F2：原 Checker 真 bool-return 函式直接作 if／else-if／while 條件。
// 同一顯式動態工作棧、真正返回前不能偷選控制邊／建立未選資源。
#[cfg(test)]
mod g18_f2_bool_condition_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let out = PreparedPrivateScalarCall::prepare(source).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace(), "真條件返回與資源退出未平衡");
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }

    #[test]
    fn f2_bool_cond_if真函式返回true才執行選中分支() {
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn choose(n:int)->int{
if(ready(true)){
return n+2
}
return n
}
fn main()->int{
return choose(3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(true)
        )).unwrap();
        let checked = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(true)
        )).unwrap();
        assert!(returned < checked);
    }

    #[test]
    fn f2_bool_cond_if真函式返回false不執行未選分支() {
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn choose(n:int)->int{
if(ready(false)){
return 99
}
return n
}
fn main()->int{
return choose(4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(false)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(99)
        )));
    }

    #[test]
    fn f2_bool_cond_else_if按原順序真執行前兩個條件() {
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn choose(n:int)->int{
if(ready(false)){
return n+100
}else if(ready(true)){
return n+2
}else{
return n+20
}
return n+300
}
fn main()->int{
return choose(3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        let results = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolReturned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(results, [false, true]);
        let conditions = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(conditions, [false, true]);
    }

    #[test]
    fn f2_bool_cond_else_if首分支已選中時不可多執行後續條件() {
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn choose(n:int)->int{
if(ready(true)){
return n+1
}else if(ready(false)){
return n+100
}else{
return n+200
}
return n
}
fn main()->int{
return choose(3)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(_)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_cond_while每輪重呼叫真bool函式而非重用快照() {
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn count(n:int)->int{
var again:bool = true
var value:int = 0
while(ready(again)){
value = value+1
again = false
}
return value+n
}
fn main()->int{
return count(4)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(5));
        let values = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::BoolReturned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [true, false]);
        let checks = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, false]);
    }

    #[test]
    fn f2_bool_cond_while第一次false不得建立循環本體資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn count(n:int)->int{
while(ready(false)){
var local:str = "不可執行"
break
}
return n
}
fn main()->int{
return count(6)
}
"#;
        let out = run(src);
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.resource_events, Vec::<R>::new());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::BoolReturned(false)
        )).count(), 1);
    }

    #[test]
    fn f2_bool_cond_迴圈預算耗盡仍清理父框架真utf8而不假成功() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = r#"fn ready(flag:bool)->bool{
return flag
}
fn looped(n:int)->int{
var owned:str = "終止回收🪼"
while(ready(true)){
continue
}
return n
}
fn main()->int{
return looped(3)
}
"#;
        let out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute_with_budget(PrivateScalarExecutionBudget {
                max_call_frames: 12, max_steps: 44,
            }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn f2_bool_cond_整數函式不能冒充bool條件由原checker拒絕() {
        let src = r#"fn step(n:int)->int{
return n+1
}
fn choose(n:int)->int{
if(step(n)){
return n
}
return 0
}
fn main()->int{
return choose(2)
}
"#;
        assert!(matches!(
            PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))
        ));
    }
}
