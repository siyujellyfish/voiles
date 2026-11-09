// G18／P18.1-F1：只驗收已核准的函式圖、來源拒絕與動態進退。
// 原 Parser／Checker 是唯一前端；本組不建立獨立解析器、資源帳或外部入口。
#[cfg(test)]
mod g18_f1_obligation_tests {
    use super::*;

    fn run(source: &str) -> PrivateScalarCallOutcome {
        let prepared = PreparedPrivateScalarCall::prepare(source).unwrap();
        let outcome = prepared.execute().unwrap();
        assert!(outcome.has_balanced_frame_trace(), "真函式框架／資源帳不平衡");
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        outcome
    }

    fn entered(outcome: &PrivateScalarCallOutcome) -> Vec<&str> {
        outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect()
    }

    fn exited(outcome: &PrivateScalarCallOutcome) -> Vec<&str> {
        outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, .. } => Some(name.as_str()),
            _ => None,
        }).collect()
    }

    #[test]
    fn f1_前向宣告與直接呼叫真返回() {
        let source = r#"fn main()->int{
return outer(3)
}
fn outer(n:int)->int{
return inner(n+1)
}
fn inner(n:int)->int{
return n+2
}
"#;
        let outcome = run(source);
        assert_eq!(outcome.result, Ok(6));
        assert_eq!(entered(&outcome), ["outer", "inner"]);
        assert_eq!(exited(&outcome), ["inner", "outer"]);
    }

    #[test]
    fn f1_同層兄弟與巢狀實參按來源順序進退() {
        let source = r#"fn left(n:int)->int{
return n+1
}
fn right(n:int)->int{
return n+2
}
fn join(a:int,b:int)->int{
return a+b
}
fn main()->int{
return join(left(3),right(5))
}
"#;
        let outcome = run(source);
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(entered(&outcome), ["left", "right", "join"]);
        assert_eq!(exited(&outcome), ["left", "right", "join"]);
        let returned = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [4, 7, 11]);
    }

    #[test]
    fn f1_相互呼叫雙向均有真動態到達與停止路徑() {
        let source = r#"fn left(flag:bool)->int{
if(flag){
return right(false)
}
return 11
}
fn right(flag:bool)->int{
if(flag){
return left(false)
}
return 17
}
fn main()->int{
return left(true)+right(true)
}
"#;
        let outcome = run(source);
        assert_eq!(outcome.result, Ok(28));
        assert_eq!(entered(&outcome), ["left", "right", "right", "left"]);
        assert_eq!(exited(&outcome), ["right", "left", "left", "right"]);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 0);
    }

    #[test]
    fn f1_巢狀真資源先釋放子框架再釋放父框架() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn leaf(n:int)->int{
var child:str = "子🪼"
return n+1
}
fn outer(n:int)->int{
var parent:str = "父"
return leaf(n)
}
fn main()->int{
return outer(2)
}
"#;
        let outcome = run(source);
        assert_eq!(outcome.result, Ok(3));
        assert_eq!(entered(&outcome), ["outer", "leaf"]);
        assert_eq!(exited(&outcome), ["leaf", "outer"]);
        assert_eq!(outcome.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn f1_內層溢位不能偽造成功且外層資源完整退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let source = r#"fn child(n:int)->int{
return n+1
}
fn outer(n:int)->int{
var resource:str = "應回收"
return child(n)
}
fn main()->int{
return outer(9223372036854775807)
}
"#;
        let outcome = run(source);
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(entered(&outcome), ["outer", "child"]);
        assert_eq!(exited(&outcome), ["child", "outer"]);
        assert_eq!(outcome.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn f1_不存在函式錯型別與未初始化實參由原checker拒絕() {
        for source in [
            r#"fn main()->int{
return missing(2)
}
"#,
            r#"fn correct(n:int)->int{
return n
}
fn main()->int{
return correct(true)
}
"#,
            r#"fn correct(n:int)->int{
return n
}
fn main()->int{
var x:int
return correct(x)
}
"#,
        ] {
            assert!(matches!(
                PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))
            ), "非法原始來源不得繞過 Checker：{source}");
        }
    }

    #[test]
    fn f1_來源函式事件遭竄改即使數目平衡也不可交付() {
        let source = r#"fn leaf(n:int)->int{
return n+1
}
fn main()->int{
return leaf(4)
}
"#;
        let mut outcome = run(source);
        let entered = outcome.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).unwrap();
        outcome.events[entered] = PrivateScalarCallEvent::FunctionEntered("fake".to_owned());
        assert!(!outcome.has_balanced_frame_trace());
        assert!(matches!(
            PreparedPrivateScalarCall::audit_outcome(outcome),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        ));
    }
}
