#[cfg(test)]
mod g18_scalar_self_cfg_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 3
fn read()->int{ return self.n }
}
fn run(flag:bool)->int{
var owner:Counter = Counter()
var total:int = 0
for i in 0..3{
total = total + owner.read()
}
if(flag){
return total + owner.read()
}
return total
}
fn main()->int{ return run(true) }
"#;

    #[test]
    fn 建構具名實參的方法讀取依序還票且溢位不提交() {
        for (expr, expected, built) in [
            ("owner.read()", Ok(3), 2),
            ("(owner.read()+owner.read())", Ok(6), 2),
            ("owner.read()+9223372036854775807", Err(PrivateScalarCallRuntimeError::IntegerOverflow), 1),
        ] {
            let src = SOURCE.split("fn run").next().unwrap().to_owned()
                + &format!("fn run()->int{{\nvar owner:Counter = Counter()\nvar second:Counter = Counter(n={expr})\nreturn second.n\n}}\n")
                + "fn main()->int{ return run() }\n";
            let out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
            assert_eq!(out.result, expected);
            assert_eq!(out.events.iter().filter(|e| matches!(e,
                PrivateScalarCallEvent::CopyStructConstructed(_))).count(), built);
            assert_eq!(out.events.iter().filter(|e| matches!(e,
                PrivateScalarCallEvent::SelfRead { .. })).count(), expr.matches("owner.read()").count());
            assert!(out.has_balanced_frame_trace());
        }
    }

    #[test]
    fn 方法來源收據與重播票據分別拒絕() {
        let plan = PreparedPrivateScalarCall::prepare(SOURCE).unwrap();
        let mut out = plan.execute().unwrap();
        for event in &mut out.events {
            if let PrivateScalarCallEvent::SelfRead { site, .. } = event {
                site.field = "forged".to_owned();
                break;
            }
        }
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let mut out = plan.execute().unwrap();
        let mut calls = out.events.iter_mut().filter(|event|
            matches!(event, PrivateScalarCallEvent::SelfRead { .. }));
        let first = calls.next().unwrap().clone();
        *calls.next().unwrap() = first;
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn 同一實參序列中普通字串借票與方法票獨立歸還() {
        let src = SOURCE.split("fn run").next().unwrap().to_owned()
            + "fn combine(s:str,n:int)->int{ return n }\n"
            + "fn run()->int{\nvar owner:Counter = Counter()\nconst s:str = \"甲\"\nreturn combine(s,owner.read())\n}\n"
            + "fn main()->int{ return run() }\n";
        let out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert_eq!(out.resource_events.len(), 2);
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn 普通函式的迴圈與提前返回依動態次序借票() {
        for (flag, expected, calls) in [("true", 12, 4), ("false", 9, 3)] {
            let src = SOURCE.replace("run(true)", &format!("run({flag})"));
            let out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert_eq!(out.events.iter().filter(|event| matches!(event,
                PrivateScalarCallEvent::SelfRead { value: 3, .. })).count(), calls);
            assert!(out.has_balanced_frame_trace());
        }
    }

    #[test]
    fn 步數耗盡仍清理持有者且不重播方法() {
        let src = SOURCE.replace("var n:int = 3","var n:int = 3\nvar label:str = \"甲\"");
        let plan = PreparedPrivateScalarCall::prepare(&src).unwrap();
        let mut observed = false;
        for max_steps in 1..100 {
            let out = plan.execute_with_budget(PrivateScalarExecutionBudget { max_call_frames: 8, max_steps }).unwrap();
            if out.result == Err(PrivateScalarCallRuntimeError::StepBudget)
                && out.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead { .. })) {
                assert!(out.has_balanced_frame_trace());
                assert_eq!(out.resource_events.len(), 2);
                observed = true;
                break;
            }
        }
        assert!(observed);
    }
}
