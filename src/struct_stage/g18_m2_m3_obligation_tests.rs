#[cfg(test)]
mod g18_m2_m3_obligation_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent::{Created, Moved, Released};

    const METHODS: &str = r#"struct Meter{
var n:int = 3
var enabled:bool = true
fn apply(x:int,flag:bool,view:str,payload:str)->int{
if(flag){
if(self.enabled){
self.enabled = false
self.n = self.n+x
return self.n
}
}
self.n = self.n-1
return self.n
}
fn read()->int{ return self.n }
}
fn run()->int{
var a:Meter = Meter()
var b:Meter = Meter(n=9)
const view:str = "來源"
var p:str = "實參"
var one:int = a.apply(2,true,view,p.move())
var two:int = a.apply(3,true,view,"第二次")
var three:int = b.apply(4,false,view,"第三次")
return a.read()+b.read()
}
fn main()->int{ return run() }
"#;

    fn plan(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn m2_交替持有者的寫入分支與參數資源() {
        let out = plan(METHODS).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.resource_events, [Created(1),Created(2),Moved(2),Released(2),
            Created(3),Moved(3),Released(3),Created(4),Moved(4),Released(4),Released(1)]);
        let receipts: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::SelfRead { site, branches, argument_tickets, ticket, .. } =>
                Some((&site.name, branches.as_slice(), argument_tickets.len(), ticket.fact())),
            _ => None,
        }).collect();
        assert_eq!(receipts.len(), 5);
        assert_eq!(receipts[0].1, [true,true]);
        assert_eq!(receipts[1].1, [true,false]);
        assert_eq!(receipts[2].1, [false]);
        assert!(receipts[..3].iter().all(|entry| entry.2 == 1 && entry.3.kind == BorrowKind::ExclusiveWrite));
        assert!(receipts[3..].iter().all(|entry| entry.2 == 0 && entry.3.kind == BorrowKind::SharedRead));
        assert_ne!(receipts[0].3.source, receipts[2].3.source);
    }

    #[test]
    fn m2_故障退出與變造持有者() {
        let overflow = METHODS.replace("Meter(n=9)", "Meter(n=0-9223372036854775807-1)");
        let out = plan(&overflow).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events.last(), Some(&Released(1)));
        assert!(!out.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead { site, .. } if site.name=="read")));
        let checked = plan(METHODS);
        let mut out = checked.execute().unwrap();
        let mut sites = out.events.iter_mut().filter_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { site, .. } if site.name=="read" => Some(site), _ => None,
        });
        std::mem::swap(&mut sites.next().unwrap().owner, &mut sites.next().unwrap().owner);
        assert!(checked.audit_graph_outcome(out).is_err());
        assert!(matches!(PreparedPrivateScalarCall::prepare(&METHODS.replace("var b:Meter","const b:Meter")),Err(PrivateScalarCallPrepareError::Check(_))));
    }

    const GRAPH: &str = r#"fn adjust(x:int)->int{
var result:int = x
for i in 0..2{ result = result+i }
return result
}
struct Counter{
var n:int = 1
fn apply(x:int)->int{
self.n = adjust(self.n+x)
return self.n
}
}
fn inner(flag:bool)->int{
const guard:str = "內"
var holder:Counter = Counter()
var n:int = 2
for i in 0..3{
n = n+1
if(flag){
n = holder.apply(adjust(n))
}
}
return n
}
fn outer(flag:bool)->int{
const guard:str = "外"
return inner(flag)
}
fn main()->int{ return outer(true) }
"#;

    #[test]
    fn m3_求參與方法回傳共用迴圈中的真框架() {
        for (flag, expected, calls) in [("true",33,3),("false",5,0)] {
            let src = GRAPH.replace("outer(true)", &format!("outer({flag})"));
            let out = plan(&src).execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert_eq!(out.resource_events,[Created(1),Created(2),Released(2),Released(1)]);
            let methods = out.events.iter().filter(|e| matches!(e, PrivateScalarCallEvent::SelfRead { helpers, .. } if helpers.len()==1)).count();
            let args = out.events.iter().filter(|e| matches!(e, PrivateScalarCallEvent::FunctionEntered(name) if name=="adjust")).count();
            assert_eq!((args,methods),(calls,calls));
        }
    }

    #[test]
    fn m3_中途故障逆序離域及子呼叫來源重驗() {
        let src = GRAPH.replace("var n:int = 2","var n:int = 2305843009213693952");
        let out = plan(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.resource_events,[Created(1),Created(2),Released(2),Released(1)]);
        let exits: Vec<_> = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } => Some(name.as_str()), _ => None,
        }).collect();
        assert_eq!(exits, ["inner","outer"]);
        let checked = plan(GRAPH);
        let mut out = checked.execute().unwrap();
        let helper = out.events.iter_mut().find_map(|e| match e {
            PrivateScalarCallEvent::SelfRead { helpers, .. } => helpers.first_mut(), _ => None,
        }).unwrap();
        helper.result=Ok(PrivateMethodValue::Int(-100));
        assert!(checked.audit_graph_outcome(out).is_err());
    }
}
