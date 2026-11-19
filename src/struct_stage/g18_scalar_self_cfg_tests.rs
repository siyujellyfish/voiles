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
    fn 方法內算術溢位先還獨占票再清理呼叫者() {
        let src = SOURCE.replace("fn read()", "fn add(a:int,b:int)->int{\nself.n = a+b\nreturn self.n\n}\nfn read()")
            .replace("var n:int = 3", "var n:int = 3\nvar label:str = \"甲\"")
            .replace("total = total + owner.read()", "total = owner.add(9223372036854775807,i)");
        let mut out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events.len(), 2);
        assert!(out.has_balanced_frame_trace());
        let trace = out.events.iter_mut().find_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { result: Err(_), trace, .. } => Some(trace),
            _ => None,
        }).unwrap();
        trace.swap(2,3);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn 迴圈實參以當下值綁入方法形參() {
        let src = SOURCE.replace("fn read()", "fn set(first:int,last:int)->int{\nself.n = last\nreturn self.n\n}\nfn read()")
            .replace("total = total + owner.read()", "total = total + owner.set(9,i)");
        let out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let reads: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::SelfRead { result: Ok(value), .. } => Some(*value),
            _ => None,
        }).collect();
        assert_eq!(reads, [0, 1, 2, 2]);

        let grouped = src.replace("owner.set(9,i)", "owner.set(9,(i+2)-1)");
        let values = PreparedPrivateScalarCall::prepare(&grouped).unwrap().execute().unwrap();
        assert_eq!(values.result, Ok(9));
        let before_loan = src.replace("owner.set(9,i)", "owner.set(9,9223372036854775807+1)");
        let failed = PreparedPrivateScalarCall::prepare(&before_loan).unwrap().execute().unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(!failed.events.iter().any(|event| matches!(event, PrivateScalarCallEvent::SelfRead { .. })));

        let wrong_type = src.replace("owner.set(9,i)", "owner.set(true,i)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&wrong_type),
            Err(PrivateScalarCallPrepareError::Check(_))));

        let overflow = src.replace("var n:int = 3", "var n:int = 3\nvar label:str = \"甲\"")
            .replace("total + owner.set(9,i)", "owner.set(9,i) + 9223372036854775807");
        let failed = PreparedPrivateScalarCall::prepare(&overflow).unwrap().execute().unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(failed.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::SelfRead { .. })).count(), 2);
        assert_eq!(failed.resource_events.len(), 2);
        assert!(failed.has_balanced_frame_trace());
    }

    #[test]
    fn 迴圈內獨占方法寫入在歸票後仍可共享讀取() {
        let src = SOURCE.replace("fn read()", "fn set()->int{\nself.n = 8\nreturn self.n\n}\nfn read()")
            .replace("total = total + owner.read()", "total = total + owner.set()");
        let out = PreparedPrivateScalarCall::prepare(&src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(32));
        let loans: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::SelfRead { ticket, .. } => Some(ticket.fact().kind),
            _ => None,
        }).collect();
        assert_eq!(loans, [BorrowKind::ExclusiveWrite, BorrowKind::ExclusiveWrite,
            BorrowKind::ExclusiveWrite, BorrowKind::SharedRead]);
        assert!(out.has_balanced_frame_trace());
        let invalid = src.replace("var n:int", "const n:int");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&invalid), Err(PrivateScalarCallPrepareError::Check(_))));
    }

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
                PrivateScalarCallEvent::SelfRead { result: Ok(3), .. })).count(), calls);
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
