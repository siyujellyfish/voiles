#[cfg(test)]
mod g18_b4_obligation_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent::{Created, Moved, Released};

    const SOURCE: &str = r#"fn fold(x:int, flag:bool)->int{
var n:int = x
if(flag){
for j in 0..2{ n = n+j }
}
return n
}
struct Counter{
var n:int = 2
fn update(delta:int, flag:bool, watch:str, owned:str)->int{
self.n = fold(self.n+delta,flag)
return self.n
}
fn read()->int{ return self.n }
}
fn run(flag:bool)->int{
var owner:Counter = Counter()
var other:Counter = Counter(n=10)
const watch:str = "借入"
var value:int = 0
for i in 0..2{
value = owner.update(i,flag,watch,"移交")
}
return owner.read()+other.read()
}
fn main()->int{ return run(true) }
"#;

    fn prepare(src: &str) -> PreparedPrivateScalarCall { PreparedPrivateScalarCall::prepare(src).unwrap() }

    #[test]
    fn b4_01_兩輪獨占寫入及跨持有者共享讀取() {
        for (flag, result, updates) in [("true",15,[3,5]),("false",13,[2,3])] {
            let src = SOURCE.replace("run(true)", &format!("run({flag})"));
            let out = prepare(&src).execute().unwrap();
            assert_eq!(out.result, Ok(result));
            let calls: Vec<_> = out.events.iter().filter_map(|e| match e {
                PrivateScalarCallEvent::SelfRead { site, ticket, result: Ok(PrivateMethodValue::Int(n)), .. } =>
                    Some((site.owner.as_str(), ticket.fact(), *n)), _ => None,
            }).collect();
            assert_eq!(calls.len(), 4);
            assert_eq!([calls[0].2,calls[1].2], updates);
            assert_eq!(calls[0].1.kind, BorrowKind::ExclusiveWrite);
            assert_eq!(calls[0].1, calls[1].1);
            assert_eq!(calls[2].1.kind, BorrowKind::SharedRead);
            assert_eq!(calls[2].1.source, calls[1].1.source);
            assert_ne!(calls[3].1.source, calls[2].1.source);
            assert_eq!((calls[2].0, calls[3].0, calls[3].2), ("owner","other",10));
        }
    }

    #[test]
    fn b4_02_方法參數借票與逐輪移交使用實際資源() {
        let out = prepare(SOURCE).execute().unwrap();
        assert_eq!(out.resource_events, [Created(1),Created(2),Moved(2),Released(2),
            Created(3),Moved(3),Released(3),Released(1)]);
        let mut count = 0;
        for event in &out.events {
            if let PrivateScalarCallEvent::SelfRead { site, trace, ticket, argument_tickets, owned_arguments, .. } = event {
                if site.name != "update" { continue; }
                count += 1;
                let [arg] = argument_tickets.as_slice() else { panic!("缺少借入參數票"); };
                assert_eq!(arg.fact().kind, BorrowKind::SharedRead);
                assert_ne!(arg.fact().source, ticket.fact().source);
                assert!(trace.ends_with(&[PrivateSharedCallExitEvent::LoanReturned(*ticket),
                    PrivateSharedCallExitEvent::LoanReturned(*arg), PrivateSharedCallExitEvent::FunctionExited { returned: true }]));
                assert_eq!(owned_arguments.len(), 1);
                assert_eq!(owned_arguments[0].2, count+1);
            }
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn b4_03_同一來源借後移動與不可寫接收者均在靜態拒絕() {
        for src in [SOURCE.replace("const watch:str","var watch:str").replace("watch,\"移交\"", "watch,watch.move()"),
                    SOURCE.replace("var owner:Counter","const owner:Counter"),
                    SOURCE.replace("owner.update(i,flag", "owner.update(flag,i")] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(&src), Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn b4_04_第二輪子框架溢位截斷後繼共享呼叫() {
        let src = SOURCE.replace("var n:int = 2","var n:int = 9223372036854775805");
        let out = prepare(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        let calls: Vec<_> = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { result, helpers, .. } => Some((*result, helpers.last().unwrap().result)),
            _ => None,
        }).collect();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].0.is_ok() && calls[1].0.is_err() && calls[1].1.is_err());
        assert_eq!(out.resource_events.last(), Some(&Released(1)));
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn b4_05_共用預算在借票前與持票後均安全退出() {
        let plan = prepare(SOURCE);
        let mut before = false;
        let mut inside = false;
        for max_steps in 1..160 {
            let out = plan.execute_with_budget(PrivateScalarExecutionBudget { max_steps, max_call_frames: 12 }).unwrap();
            if out.result != Err(PrivateScalarCallRuntimeError::StepBudget) { continue; }
            assert!(out.has_balanced_frame_trace());
            before |= out.resource_events == [Created(1), Released(1)] &&
                !out.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead { .. }));
            inside |= out.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead { helpers, argument_tickets, .. }
                if argument_tickets.len() == 1 && helpers.last().is_some_and(|h| h.result == Err(PrivateScalarCallRuntimeError::StepBudget))));
        }
        assert!(before && inside);
    }

    #[test]
    fn b4_06_方法與參數還票次序交換須拒絕() {
        let plan = prepare(SOURCE);
        let mut out = plan.execute().unwrap();
        let trace = out.events.iter_mut().find_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { trace, argument_tickets, .. } if !argument_tickets.is_empty() => Some(trace),
            _ => None,
        }).unwrap();
        let n = trace.len();
        trace.swap(n-3,n-2);
        assert!(matches!(plan.audit_graph_outcome(out), Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn b4_07_原始來源及不同接收者呼叫位置不可互換() {
        let mut plan = prepare(SOURCE);
        let mut out = plan.execute().unwrap();
        let mut reads = out.events.iter_mut().filter_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { site, .. } if site.name == "read" => Some(site), _ => None,
        });
        let first = reads.next().unwrap();
        let second = reads.next().unwrap();
        std::mem::swap(&mut first.call_span, &mut second.call_span);
        assert!(matches!(plan.audit_graph_outcome(out), Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        plan.verified.source.bytes[0] ^= 1;
        assert!(matches!(plan.execute(), Err(PrivateScalarCallPrepareError::SourceChanged)));
    }
}
