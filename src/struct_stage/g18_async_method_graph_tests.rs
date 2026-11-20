#[cfg(test)]
mod g18_async_method_graph_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent::{Created, Moved, Released};

    const SOURCE: &str = r#"struct Counter{
var n:int = 2
async fn add(x:int)->int{
self.n = self.n+x
return self.n
}
async fn read()->int{ return self.n }
}
async fn work(payload:str)->int{
var consumed:str = payload.move()
var holder:Counter = Counter()
var guard:str = "子框架"
var total:int = await holder.add(3)
return total+await holder.read()
}
fn main()->int{
var outer:str = "父框架"
var payload:str = "移交"
return await work(payload.move())
}
"#;
    fn plan(src: &str) -> PreparedPrivateScalarCall { PreparedPrivateScalarCall::prepare(src).unwrap() }

    #[test]
    fn a1_普通等待與兩種方法共用可喚醒框架() {
        let checked = plan(SOURCE);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&checked).unwrap();
        for name in ["work","add","read"] {
            assert_eq!(queue.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            assert_eq!(queue.pending_await(id),Some(name));
            assert_eq!(queue.drive_one().unwrap(),None);
            queue.wake(id).unwrap();
        }
        assert_eq!(queue.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Completed)));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result,Ok(10));
        assert_eq!(out.resource_events,[Created(1),Created(2),Moved(2),Moved(2),Created(3),Released(3),Released(2),Released(1)]);
        let loans: Vec<_> = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { site, ticket, .. } if site.awaited => Some(ticket.fact().kind),
            _ => None,
        }).collect();
        assert_eq!(loans,[BorrowKind::ExclusiveWrite,BorrowKind::SharedRead]);
        let eager = checked.execute().unwrap();
        assert_eq!(eager.result,out.result);
        assert_eq!(eager.resource_events,out.resource_events);
        assert_eq!(eager.events.len(),out.events.len());
    }

    #[test]
    fn a1_兩工作在方法安全點按喚醒順序續行() {
        let checked = plan(SOURCE);
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = q.submit(&checked).unwrap();
        let b = q.submit(&checked).unwrap();
        for id in [a,b] { assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended))); }
        for id in [b,a] { q.wake(id).unwrap(); }
        for id in [b,a] {
            assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            assert_eq!(q.pending_await(id),Some("add"));
        }
        assert_eq!(q.shutdown().unwrap().len(),2);
    }

    #[test]
    fn a2_各安全點取消時真移交與逆序離域() {
        let checked = plan(SOURCE);
        for stop in 0..=3 {
            let mut q = PrivateScalarCooperativeQueue::new();
            let id = q.submit(&checked).unwrap();
            for poll in 0..stop {
                if poll>0 {q.wake(id).unwrap();}
                assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            }
            q.cancel(id).unwrap();
            if stop == 0 {
                assert!(q.cancellation(id).is_none());
                assert!(q.retained_resources(id).unwrap().is_empty());
                continue;
            }
            let c = q.cancellation(id).unwrap();
            let expected = match stop {
                0 => vec![],
                1 => vec![Created(1),Created(2),Released(2),Released(1)],
                _ => vec![Created(1),Created(2),Moved(2),Moved(2),Created(3),Released(3),Released(2),Released(1)],
            };
            assert_eq!(c.resource_events,expected);
            let methods=c.events.iter().filter(|e| matches!(e,PrivateScalarCallEvent::SelfRead {..})).count();
            assert_eq!(methods,usize::from(stop==3));
            assert!(q.wake(id).is_err());
        }
    }

    #[test]
    fn a2_安全點收據與方法來源不得重播() {
        let checked=plan(SOURCE);
        let mut out=checked.execute().unwrap();
        let points: Vec<_>=out.events.iter().enumerate().filter_map(|(i,e)|
            matches!(e,PrivateScalarCallEvent::MethodAwaitSafePoint(_)).then_some(i)).collect();
        assert_eq!(points.len(),2);
        out.events.swap(points[0],points[1]);
        assert!(checked.audit_graph_outcome(out).is_err());
        let overflow=plan(&SOURCE.replace("holder.add(3)","holder.add(9223372036854775807)")).execute().unwrap();
        assert_eq!(overflow.result,Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(overflow.resource_events.ends_with(&[Released(3),Released(2),Released(1)]));
        assert!(!overflow.events.iter().any(|e|matches!(e,PrivateScalarCallEvent::SelfRead{site,..} if site.name=="read")));
    }

    #[test]
    fn a2_錯換方法等待來源須在整批取消前拒絕() {
        let checked=plan(SOURCE);
        let mut q=PrivateScalarCooperativeQueue::new();
        let a=q.submit(&checked).unwrap();
        let b=q.submit(&checked).unwrap();
        for id in [a,b] {
            assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            q.wake(id).unwrap();
        }
        for id in [a,b] { assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended))); }
        q.jobs.get_mut(&b).unwrap().continuation.checkpoint.as_mut().unwrap().awaiting_name = "read".into();
        let resources=q.retained_resources(a).unwrap().to_vec();
        assert!(q.shutdown().is_err());
        assert_eq!(q.retained_resources(a).unwrap(),resources);
        assert!(q.cancellation_order.is_empty());
        assert_eq!(q.status(a),Some(PrivateDeferredAwaitStatus::Suspended));
    }

    #[test]
    fn a1_借用不能跨等待且續行不重設預算() {
        let borrowed=SOURCE.replace("var consumed:str = payload.move()","const copy:int = 1");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&borrowed),Err(PrivateScalarCallPrepareError::Check(_))));
        let checked=plan(SOURCE);
        let mut saw_method_limit=false;
        for max_steps in 1..95 {
            let mut q=PrivateScalarCooperativeQueue::new();
            let id=q.submit_with_budget(&checked, PrivateScalarExecutionBudget{max_steps,max_call_frames:8}).unwrap();
            loop {
                let (_,poll)=q.drive_one().unwrap().unwrap();
                if poll==PrivateScalarGraphPoll::Completed { break; }
                q.wake(id).unwrap();
            }
            let out=q.outcome(id).unwrap();
            if out.result==Err(PrivateScalarCallRuntimeError::StepBudget)
                && out.events.iter().any(|e|matches!(e,PrivateScalarCallEvent::SelfRead{site,..} if site.awaited)) {
                assert!(out.has_balanced_frame_trace());
                saw_method_limit=true;
            }
        }
        assert!(saw_method_limit);
    }

    #[test]
    fn a2_另一合法工作的完成收據不可用於退休() {
        let first=plan(SOURCE);
        let second=plan(&SOURCE.replace("holder.add(3)", "holder.add(4)"));
        let mut q=PrivateScalarCooperativeQueue::new();
        let a=q.submit(&first).unwrap();
        let b=q.submit(&second).unwrap();
        for _ in 0..3 {
            for id in [a,b] { assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended))); }
            for id in [a,b] { q.wake(id).unwrap(); }
        }
        for id in [a,b] { assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Completed))); }
        let other=q.outcome(b).unwrap().clone();
        assert_eq!(other.result,Ok(12));
        q.jobs.get_mut(&a).unwrap().continuation.outcome=Some(other);
        assert!(q.shutdown().is_err());
        assert!(q.retire_terminal(a).is_err());
        assert_eq!(q.status(b),Some(PrivateDeferredAwaitStatus::Succeeded));
    }

    #[test]
    fn a1_布林與字串等待結果回到原呼叫端() {
        let src=r#"struct Counter{
var enabled:bool = true
async fn ready()->bool{ return self.enabled }
async fn text()->str{ return "返回資源" }
}
fn use_text(s:str)->int{ return 8 }
fn work()->int{
var h:Counter = Counter()
if(await h.ready()){
var text:str = await h.text()
return use_text(text)
}
return 0
}
fn main()->int{ return work()+0 }
"#;
        let checked=plan(src);
        let mut q=PrivateScalarCooperativeQueue::new();
        let id=q.submit(&checked).unwrap();
        for target in ["ready","text"] {
            assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            assert_eq!(q.pending_await(id),Some(target));
            q.wake(id).unwrap();
        }
        assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Completed)));
        let out=q.outcome(id).unwrap();
        assert_eq!(out.result,Ok(8));
        assert_eq!(out.resource_events.first(),Some(&Created(1)));
        assert_eq!(out.resource_events.last(),Some(&Released(1)));
    }
}
