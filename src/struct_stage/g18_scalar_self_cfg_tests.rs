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
    fn 方法由分支移出單一字串並逆序清理其餘形參() {
        use crate::owned_str_runtime::ResourceEvent::*;
        let src = r#"struct Holder{
var n:int = 1
fn pick(left:str,right:str,flag:bool)->str{
if(flag){ return left.move() }
return right.move()
}
}
fn relay()->str{
var owner:Holder = Holder()
var a:str = "甲"
var b:str = "乙"
return owner.pick(a.move(),b.move(),true)
}
fn inspect(s:str)->int{ return 4 }
fn main()->int{ return inspect(relay()) }
"#;
        for binding in ["var got:str = ", "var got:str\ngot = "] {
            let program = src.replace("return owner.pick(a.move(),b.move(),true)",
                &format!("{binding}owner.pick(a.move(),b.move(),true)\nreturn got.move()"));
            let outcome = PreparedPrivateScalarCall::prepare(&program).unwrap().execute().unwrap();
            assert_eq!(outcome.result, Ok(4));
            assert!(outcome.has_balanced_frame_trace());
        }
        for (flag, kept, dropped) in [("true",1,2),("false",2,1)] {
            let program = src.replace("b.move(),true)", &format!("b.move(),{flag})"));
            let mut outcome = PreparedPrivateScalarCall::prepare(&program).unwrap().execute().unwrap();
            assert_eq!(outcome.result, Ok(4));
            assert!(outcome.has_balanced_frame_trace());
            assert!(outcome.resource_events.starts_with(&[Created(1),Created(2),Moved(1),Moved(2),
                Returned(kept),Released(dropped),Returned(kept)]));
            for event in &mut outcome.events {
                if let PrivateScalarCallEvent::SelfRead { result, .. } = event {
                    *result = Ok(PrivateMethodValue::Str(dropped));
                }
            }
            assert!(!outcome.has_balanced_frame_trace());
        }
        let borrowed = src.replace("pick(a.move(),b.move(),true)", "pick(a,b.move(),true)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&borrowed), Err(PrivateScalarCallPrepareError::Check(_))));
        let overflow = src.replace("if(flag){", "self.n = self.n + 9223372036854775807\nif(flag){");
        let failed = PreparedPrivateScalarCall::prepare(&overflow).unwrap().execute().unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(failed.resource_events, [Created(1), Created(2), Moved(1), Moved(2), Released(2), Released(1)]);
        assert!(failed.has_balanced_frame_trace());
    }

    #[test]
    fn 持票方法複合加減逐次讀取當下欄位且溢位歸票() {
        let src = r#"struct Counter{
var n:int = 3
var delta:int = 2
fn tick(x:int, watch:str)->int{
self.n = (self.n + x) - self.delta
return self.n + self.delta
}
}
fn run()->int{
var owner:Counter = Counter()
const s:str = "追蹤"
var total:int = 0
for i in 0..3{
total = total + owner.tick(5,s)
}
return total
}
fn main()->int{ return run() }
"#;
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(33));
        assert!(out.has_balanced_frame_trace());
        let overflow = src.replace("tick(5,s)", "tick(9223372036854775807,s)");
        let out = PreparedPrivateScalarCall::prepare(&overflow).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let bad = src.replace("var delta:int = 2", "var delta:bool = false");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&bad), Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn 方法整筆移動來源與形參交叉同名仍逐筆清理() {
        use crate::owned_str_runtime::ResourceEvent::*;
        let src = r#"struct Counter{
var n:int = 1
fn put(b:str,a:str,watch:str,x:int)->int{
self.n = x+1
return self.n
}
}
fn run()->int{
var owner:Counter = Counter()
var a:str = "甲"
var b:str = "乙"
const s:str = "觀察"
return owner.put(a.move(),b.move(),s,1)
}
fn main()->int{ return run() }
"#;
        for (program, expected) in [(src.to_owned(), Ok(2)),(src.replace("s,1)", "s,9223372036854775807)"),
            Err(PrivateScalarCallRuntimeError::IntegerOverflow))] {
            let mut out = PreparedPrivateScalarCall::prepare(&program).unwrap().execute().unwrap();
            assert_eq!(out.result, expected);
            assert!(out.has_balanced_frame_trace());
            assert_eq!(out.resource_events, [Created(1),Created(2),Created(3),Moved(1),Moved(2),
                Released(2),Released(1),Released(3)]);
            for event in &mut out.events {
                if let PrivateScalarCallEvent::SelfRead { owned_arguments, .. } = event {
                    owned_arguments.swap(0,1);
                }
            }
            assert!(!out.has_balanced_frame_trace());
        }
        let before = src.replace("s,1)", "s,9223372036854775807+1)");
        let failed = PreparedPrivateScalarCall::prepare(&before).unwrap().execute().unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(failed.resource_events, [Created(1),Created(2),Created(3),Released(3),Released(2),Released(1)]);
        let duplicate = src.replace("b.move()", "a.move()");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&duplicate), Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn 方法混合字串借用與整數寫入於正常及溢位皆歸票() {
        let src = SOURCE.replace("fn read()", "fn put(label:str,x:int)->int{\nself.n = x+1\nreturn self.n\n}\nfn read()")
            .replace("var total:int = 0", "const s:str = \"甲\"\nvar total:int = 0")
            .replace("total = total + owner.read()", "total = total + owner.put(s,i)");
        let mixed = src.replace("label:str,x:int", "first:str,last:str,x:int")
            .replace("var total:int = 0", "var t:str = \"乙\"\nvar total:int = 0")
            .replace("owner.put(s,i)", "owner.put(s,t,i)");
        let both = PreparedPrivateScalarCall::prepare(&mixed).unwrap().execute().unwrap();
        let tickets = both.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::SelfRead { argument_tickets, .. } if !argument_tickets.is_empty() => Some(argument_tickets),
            _ => None,
        }).unwrap();
        assert_eq!(tickets.iter().map(|t| t.fact().kind).collect::<Vec<_>>(),
            [BorrowKind::SharedRead, BorrowKind::ExclusiveWrite]);
        assert_eq!(both.resource_events.len(), 4);
        assert_eq!(both.result, Ok(9));
        let conflict = mixed.replace("owner.put(s,t,i)", "owner.put(t,t,i)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&conflict), Err(PrivateScalarCallPrepareError::Check(_))));
        let before_self = src.replace("owner.put(s,i)", "owner.put(s,9223372036854775807+1)");
        let failed = PreparedPrivateScalarCall::prepare(&before_self).unwrap().execute().unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(failed.has_balanced_frame_trace());
        assert!(!failed.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead { .. })));
        for (case, expected) in [(src.clone(), Ok(9)),
            (src.replace("owner.put(s,i)", "owner.put(s,9223372036854775807)"), Err(PrivateScalarCallRuntimeError::IntegerOverflow))] {
            let plan = PreparedPrivateScalarCall::prepare(&case).unwrap();
            let mut out = plan.execute().unwrap();
            assert_eq!(out.result, expected);
            assert!(out.has_balanced_frame_trace());
            assert_eq!(out.resource_events.len(), 2);
            let tickets = out.events.iter_mut().find_map(|e| match e {
                PrivateScalarCallEvent::SelfRead { argument_tickets, .. } if !argument_tickets.is_empty() => Some(argument_tickets),
                _ => None,
            }).unwrap();
            tickets.clear();
            assert!(!out.has_balanced_frame_trace());
        }
    }

    #[test]
    fn 巢狀方法分支依每輪欄位真值寫入與提前返回() {
        let src = SOURCE.replace("fn read()", r#"var ready:bool = true
fn choose(flag:bool,x:int)->int{
if(flag){
if(self.ready){
self.ready = false
self.n = x
return self.n
}else{
self.n = 8
}
}else{
self.n = 12
}
return self.n
}
fn read()"#).replace("total = total + owner.read()", "total = total + owner.choose(flag,i)");
        let owned = src.replace("var n:int = 3", "var n:int = 3\nvar text:str = \"甲\"");
        let plan = PreparedPrivateScalarCall::prepare(&owned).unwrap();
        let mut failed_in_method = false;
        for max_steps in 1..80 {
            let out = plan.execute_with_budget(PrivateScalarExecutionBudget { max_call_frames: 8, max_steps }).unwrap();
            if out.events.iter().any(|e| matches!(e, PrivateScalarCallEvent::SelfRead {
                result: Err(PrivateScalarCallRuntimeError::StepBudget), .. })) {
                assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
                assert_eq!(out.resource_events.len(), 2);
                assert!(out.has_balanced_frame_trace());
                failed_in_method = true;
                break;
            }
        }
        assert!(failed_in_method);
        for (flag, expected) in [("true",24), ("false",36)] {
            let case = src.replace("run(true)", &format!("run({flag})"));
            let out = PreparedPrivateScalarCall::prepare(&case).unwrap().execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert!(out.has_balanced_frame_trace());
        }
    }

    #[test]
    fn 布林實參在方法內決定提前返回的欄位() {
        let src = SOURCE.replace("fn read()", "var other:int = 10\nfn pick(flag:bool)->int{\nif(flag){ return self.other }\nreturn self.n\n}\nfn read()")
            .replace("total = total + owner.read()", "total = total + owner.pick(flag)");
        for (flag, expected) in [("true", 33), ("false", 9)] {
            let case = src.replace("run(true)", &format!("run({flag})"));
            let out = PreparedPrivateScalarCall::prepare(&case).unwrap().execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert!(out.has_balanced_frame_trace());
        }
    }

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
            PrivateScalarCallEvent::SelfRead { result: Ok(PrivateMethodValue::Int(value)), .. } => Some(*value),
            _ => None,
        }).collect();
        assert_eq!(reads, [0, 1, 2, 2]);

        let helper = src.replace("fn run(", "fn plus(x:int)->int{ return x+1 }\nfn run(")
            .replace("owner.set(9,i)", "owner.set(9,plus(i))");
        let out = PreparedPrivateScalarCall::prepare(&helper).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.events.iter().filter(|e| matches!(e, PrivateScalarCallEvent::FunctionEntered(name)
            if name == "plus")).count(), 3);
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
                site.body.clear();
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
                PrivateScalarCallEvent::SelfRead { result: Ok(PrivateMethodValue::Int(3)), .. })).count(), calls);
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
