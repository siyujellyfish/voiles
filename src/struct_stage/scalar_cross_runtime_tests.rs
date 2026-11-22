/// R06：跨各個原 Checker 已核准執行器的**真動態結果**整合回歸。
/// 不假設各路徑已共享同一 runtime，亦不把 int 離域等同 OwnedStr Release。
#[cfg(test)]
mod g18_r06_cross_runtime_tests {
    use super::*;
    use crate::owned_str_runtime::{self, ResourceEvent};

    fn scalar() -> &'static str {
        "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n"
    }

    fn borrow() -> &'static str {
        "fn measure(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源🪼\"\nreturn measure(owner)\n}\n"
    }

    fn move_borrow() -> &'static str {
        "fn consume(owned:str,watch:str)->int{\nvar local:str = owned.move()\nreturn 11\n}\nfn main()->int{\nvar source:str = \"唯一🪼\"\nconst observer:str = \"借用\"\nreturn consume(source.move(),observer)\n}\n"
    }

    fn method() -> &'static str {
        "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n"
    }

    fn recursive() -> &'static str {
        "fn descend(flag:bool,payload:str)->int{\nvar local:str = \"層🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root)\n}\n"
    }

    #[test]
    fn g18_r06_混合布林整數與真借用成功返回各守門須同時成立() {
        let pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(6));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap().execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert_eq!(borrowed.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
        // int/bool ScopeEvent 不會也不能憑空創造 OwnedStr 事件。
        assert!(!pure.events.is_empty());
    }

    #[test]
    fn g18_r06_同_call_整筆移動借用和同步_self_不混淆資源責任() {
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert_eq!(mixed.result, Ok(11));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert_eq!(mixed.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 2);
        let own = PreparedPrivateSelfMethod::prepare(method())
            .unwrap().execute().unwrap();
        assert_eq!(own.result, Ok(7));
        assert!(own.has_balanced_self_borrow_trace());
        assert!(own.events.is_empty());
        assert!(mixed.ticket_returned && own.ticket_returned);
    }

    #[test]
    fn g18_r06_合法遞迴真資源與普通借用在預算耗盡時皆不得假返回() {
        let plan = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap();
        let exhausted = plan.execute(PrivateRecursiveStrBudget {
            max_call_frames: 2, max_steps: 80,
        }).unwrap();
        assert_eq!(exhausted.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(exhausted.has_balanced_resource_frame_trace());
        assert!(exhausted.loans_returned);
        assert!(owned_str_runtime::event_trace_balanced(&exhausted.events));
        assert!(exhausted.frames.iter().all(|event| !matches!(
            event, PrivateRecursiveFrameEvent::Exited { returned: true, .. }
        )));
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(borrowed.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(!borrowed.call_exited);
    }

    #[test]
    fn g18_r06_不同執行器的正常失敗不得丟失已建資源或複製假返回() {
        let pure = PreparedPrivateScalarCall::prepare(
            "fn inc(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n"
        ).unwrap().execute().unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|ev| matches!(
            ev, PrivateScalarCallEvent::Returned(_)
        )));
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(!method.method_exited);
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow()).unwrap()
            .execute_with_fault(Some(3)).unwrap();
        assert_eq!(mixed.result, Err(PrivateMixedOwnershipRunError::InjectedFailure));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert!(mixed.ticket_returned);
    }

    #[test]
    fn g18_r06_跨模型偽造離域資源票據任一項必在交付稽核失敗() {
        let mut pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap()
            .execute().unwrap();
        pure.events.push(PrivateScalarCallEvent::LocalScopeEnded(DeclarationId(0)));
        assert!(!pure.has_balanced_frame_trace());
        let mut borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        borrowed.ticket_returned = false;
        assert!(!borrowed.has_balanced_borrow_resource_trace());
        let mut mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        mixed.events.clear();
        assert!(!mixed.has_balanced_mixed_resource_trace());
        let mut self_method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute().unwrap();
        self_method.events.extend([
            ResourceEvent::Created(77), ResourceEvent::Released(77),
        ]);
        assert!(!self_method.has_balanced_self_borrow_trace());
        let mut nested = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap()
            .execute(PrivateRecursiveStrBudget {
                max_call_frames: 4, max_steps: 100,
            }).unwrap();
        assert!(nested.has_balanced_resource_frame_trace());
        nested.loans_returned = false;
        assert!(!nested.has_balanced_resource_frame_trace());
    }
    #[test]
    fn g18_r06_真_while_for_局部_算術與_str_借用雙執行器結果皆須平衡() {
        let src = "fn calculate(n:int)->int{\nvar result:int = n\nvar active:bool = true\nwhile(active){\nconst step:int = 2\nresult = result+step\nactive = false\n}\nfor i in 1..3{\nconst copied:int = i\nresult = result+copied\n}\nreturn result\n}\nfn main()->int{\nreturn calculate(3)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(8));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 6);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert_eq!(borrowed.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r06_巢狀同名遮蔽與同步_self_不同資源帳簿均有真退出() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvar value:int = 9\nif(true){\nvalue = value+2\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        let ids = pure.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LocalInitialized(id, _) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.ticket_returned);
    }

    #[test]
    fn g18_r06_迴圈_break_先釋放局部與遞迴資源耗盡各守真出口() {
        let src = "fn choose(n:int)->int{\nfor i in 1..8{\nconst local:int = i\nbreak\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 2);
        let recursive = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap()
            .execute(PrivateRecursiveStrBudget {
                max_call_frames: 2, max_steps: 90,
            }).unwrap();
        assert_eq!(recursive.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(recursive.has_balanced_resource_frame_trace());
        assert!(recursive.loans_returned);
    }

    #[test]
    fn g18_r06_區塊算術溢位與_str_借用中途故障均禁止虛構返回() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst increment:int = 9223372036854775807\nvalue = value+increment\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(borrowed.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(!borrowed.call_exited);
    }

    #[test]
    fn g18_r06_純值區塊離域偽造及_move_borrow_資源偽造皆失敗() {
        let src = "fn choose(n:int)->int{\nif(true){\nconst local:int = 3\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let mut pure = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        pure.events.insert(0, PrivateScalarCallEvent::LocalScopeEnded(DeclarationId(0)));
        assert!(!pure.has_balanced_frame_trace());
        let mut mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert!(mixed.has_balanced_mixed_resource_trace());
        mixed.ticket_returned = false;
        assert!(!mixed.has_balanced_mixed_resource_trace());
    }

    #[test]
    fn g18_r06_合法迴圈中提前返回清理_另一條資源模型仍歸還票據() {
        let src = "fn choose(n:int)->int{\nfor i in 1..3{\nconst temporary:int = i\nreturn temporary+n\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(5));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )).count(), 1);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(borrowed.ticket_returned);
    }

    #[test]
    fn g18_r06_巢狀_while_預算耗盡與_self_故障無假成功返回() {
        let src = "fn choose(n:int)->int{\nwhile(true){\nconst temporary:int = 2\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute_with_budget(PrivateScalarExecutionBudget {
                max_call_frames: 5, max_steps: 32,
            }).unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(!method.method_exited);
    }

    #[test]
    fn g18_r06_靜態逃逸拒絕不得取代_true_runtime_安全交叉驗證() {
        let invalid = "fn choose(n:int)->int{\nfor i in 0..3{\nconst hidden:int = i\n}\nreturn hidden\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn choose(n:int)->int{\nfor i in 0..3{\nconst hidden:int = i\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn choose(4)\n}\n";
        let pure = PreparedPrivateScalarCall::prepare(valid).unwrap()
            .execute().unwrap();
        assert_eq!(pure.result, Ok(4));
        assert!(pure.has_balanced_frame_trace());
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert_eq!(mixed.result, Ok(11));
        assert!(mixed.has_balanced_mixed_resource_trace());
    }

    #[test]
    fn g18_r06_同一函式圖兩張_const_str_共享票據來源相同但序號不同() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst shared:str = \"共享🪼\"\nreturn inspect(shared,shared,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let tickets = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(tickets.len(), 2);
        assert_ne!(tickets[0], tickets[1]);
        assert_eq!(tickets[0].fact(), tickets[1].fact());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r06_兩個獨立_var_str_同一次真呼叫各有獨占票據() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn inspect(first,second,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let facts = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::BorrowParameterBound(_, fact) = event {
                Some(*fact)
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].kind, BorrowKind::ExclusiveWrite);
        assert_eq!(facts[1].kind, BorrowKind::ExclusiveWrite);
        assert_ne!(facts[0].source, facts[1].source);
    }

    #[test]
    fn g18_r06_同一真函式堆疊先_move_後借另一_source_資源不混淆() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar moved:str = \"搬移\"\nconst observed:str = \"觀察\"\nconst destination:str = moved.move()\nreturn inspect(observed,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).unwrap();
        assert!(returned < released);
    }

    #[test]
    fn g18_r06_for_重複同一外層_str_真借用逐輪歸還() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn count(n:int)->int{\nconst source:str = \"外\"\nvar result:int = n\nfor i in 0..2{\nresult = inspect(source,result)\n}\nreturn result\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r06_未選中的_if_真借用不簽發任何動態票據() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn choose(n:int)->int{\nconst source:str = \"外\"\nif(false){\nreturn inspect(source,n)\n}\nreturn n\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
                | PrivateScalarCallEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r05_r06_偽造借用函式已退出後才歸還票據必拒絕() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"外\"\nreturn inspect(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, .. }
                if name == "inspect"
        )).unwrap();
        let loan = out.events.remove(returned);
        let new_exit = if returned < exit { exit - 1 } else { exit };
        out.events.insert(new_exit + 1, loan);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r06_同一_while_重借外層_const_str_逐次歸還才進下一輪() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nconst source:str = \"重借🪼\"\nvar value:int = n\nvar active:bool = true\nwhile(active){\nvalue = observe(source,value)\nactive = false\n}\nreturn observe(source,value)\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acquired = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count();
        let returned = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count();
        assert_eq!((acquired, returned), (2, 2));
    }

    #[test]
    fn g18_r06_真借用已取得但子呼叫框架預算耗盡仍全數原子歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nconst source:str = \"呼叫預算\"\nreturn observe(source,n)\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute_with_budget(
            PrivateScalarExecutionBudget {max_call_frames: 1, max_steps: 100}
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        let acq = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count();
        let ret = out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count();
        assert_eq!(acq, ret);
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r06_借用後第二_int_實參溢位_真票據歸還先於_resourceid_釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nconst source:str = \"溢位\"\nreturn observe(source,n+9223372036854775807)\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let loan = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(loan < released);
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
    }

    #[test]
    fn g18_r05_r06_重複歸還同一票據必由同序帳簿拒絕() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn observe(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let loan = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let duplicate = out.events[loan].clone();
        out.events.insert(loan + 1, duplicate);
        assert!(!out.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r05_r06_每張真借用票據必連至實際_utf8_resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nreturn inspect(first,second,4)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let linked = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanResourceLinked(ticket, id) =>
                Some((*ticket, *id)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(linked.len(), 2);
        assert_ne!(linked[0].0, linked[1].0);
        assert_eq!([linked[0].1, linked[1].1], [1, 2]);
        for (ticket, id) in linked {
            let acquired = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanAcquired(found) if found == &ticket
            )).unwrap();
            let joined = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanResourceLinked(found, linked)
                    if found == &ticket && *linked == id
            )).unwrap();
            let returned = out.events.iter().position(|event| matches!(event,
                PrivateScalarCallEvent::LoanReturned(found) if found == &ticket
            )).unwrap();
            assert!(acquired < joined && joined < returned);
        }
    }

    #[test]
    fn g18_r05_r06_偽造借用未歸還便先釋放真來源資源必拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn inspect(source,4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        // 將唯一真正 Released 提前至票據取得後、被呼叫者進入前。
        // ResourceEvent 序列本身仍平衡，舊稽核只看兩套獨立帳簿會漏判。
        let release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        let event = out.events.remove(release);
        let linked = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).unwrap();
        out.events.insert(linked + 1, event);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_票據資源身分偽造重複綁定或缺失均拒絕() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"來源\"\nreturn inspect(source,4)\n}\n";
        let original = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(original.has_balanced_frame_trace());
        let position = original.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, _)
        )).unwrap();

        let mut wrong_id = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(_, resource) =
            &mut wrong_id.events[position] {
            *resource = 999;
        } else { panic!("缺少原票據連結"); }
        assert!(!wrong_id.has_balanced_frame_trace());

        let mut duplicate = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        let duplicate_link = duplicate.events[position].clone();
        duplicate.events.insert(position + 1, duplicate_link);
        assert!(!duplicate.has_balanced_frame_trace());

        let mut missing = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        missing.events.remove(position);
        assert!(!missing.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r05_r06_同框架兩個存活_resourceid_借用不得錯綁他人() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"甲\"\nconst second:str = \"乙\"\nreturn inspect(first,4)\n}\n";
        let baseline = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(baseline.result, Ok(4));
        assert!(baseline.has_balanced_frame_trace());
        assert_eq!(baseline.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        // 真來源是 first / ResourceId 1。假裝其連結是同樣活躍的
        // second / ResourceId 2：僅驗證 ResourceId 存在仍會誤放行。
        let mut wrong = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        let linked = wrong.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).unwrap();
        if let PrivateScalarCallEvent::LoanResourceLinked(_, resource) =
            &mut wrong.events[linked] {
            *resource = 2;
        } else { panic!("缺少真連結"); }
        assert!(!wrong.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(wrong),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_str_覆寫後再借新_resourceid_票據須對應最新擁有者() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nvar owner:str = \"舊\"\nowner = \"新\"\nreturn inspect(owner,n)\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 2)
        )).count(), 1);
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(returned < release);
    }

    #[test]
    fn g18_r06_真_move_覆寫後借目標仍指同一原始_resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nvar source:str = \"甲\"\nvar target:str = \"乙\"\ntarget = source.move()\nreturn inspect(target,n)\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::LoanResourceLinked(_, 1)
        )).count(), 1);
    }

    #[test]
    fn g18_r05_r06_偽造_str_覆寫後仍由舊資源認領必拒絕() {
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = \"新\"\nreturn n\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceOwnerBound(_, resource) = &mut out.events[index] {
            *resource = 1; // 舊資源早已 Released，不能被同一 DeclarationId 復活
        } else { panic!("缺少真擁有者轉移事件"); }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r06_str_覆寫後整數溢位仍釋放目前唯一擁有者() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = \"新\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::Returned(_)
        )));
    }


    #[test]
    fn g18_r06_三層真函式同一_resourceid_連續跨框架_move_僅一次清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn mid(item:str,n:int)->int{\nreturn leaf(item.move(),n)\n}\nfn main()->int{\nvar owner:str = \"三層\"\nreturn mid(owner.move(),5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_r06_函式_for_每輪新_str_跨函式_move_返回後即時釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(item:str,n:int)->int{\nreturn n+1\n}\nfn run(n:int)->int{\nvar result:int = n\nfor i in 0..2{\nvar owner:str = \"逐輪\"\nresult = consume(owner.move(),result)\n}\nreturn result\n}\nfn main()->int{\nreturn run(3)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r06_同次呼叫先借再_move_不同真來源_各有正確出口() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(borrowed:str,moved:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst read:str = \"讀\"\nvar transfer:str = \"移\"\nreturn inspect(read,transfer.move(),2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(returned < released);
    }

    #[test]
    fn g18_r06_移動實參已準備但_callbudget_耗盡不得假轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn parent(n:int)->int{\nvar owner:str = \"預算\"\nreturn leaf(owner.move(),n)\n}\nfn main()->int{\nreturn parent(1)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 }
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
    }

    #[test]
    fn g18_r05_r06_偽造跨框架_move_目標宣告身分須拒絕() {
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned.move(),2)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let event = out.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { target, .. } = event {
            *target = DeclarationId(0);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_原_checker_VB14_借用別名缺乏重借證明不得進真子呼叫() {
        let invalid = [
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nreturn leaf(item,n)\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nreturn leaf(item,n)\n}\nfn main()->int{\nvar owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
            "fn leaf(item:str,n:int)->int{\nreturn n\n}\nfn bridge(item:str,n:int)->int{\nvar value:int = n\nfor i in 0..2{\nvalue = leaf(item,value)\n}\nreturn value\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn bridge(owner,2)\n}\n",
        ];
        for src in invalid {
            let Err(PrivateScalarCallPrepareError::Check(diagnostic)) =
                PreparedPrivateScalarCall::prepare(src) else {
                panic!("VB14 必須由正式 Checker 拒絕，不能交由 runtime 猜測");
            };
            assert!(format!("{diagnostic:?}").contains("VB14"));
        }
    }

    #[test]
    fn g18_r06_原_checker_准許真持有者_move_給子函式不受_VB14_錯誤擴張() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn bridge(n:int)->int{\nvar owned:str = \"合法\"\nreturn leaf(owned.move(),n)\n}\nfn main()->int{\nreturn bridge(5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_r06_存活借用票據期間偽造同源跨框架_move_仍須拒絕() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"不可移\"\nreturn observe(owner,3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let ticket = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )).unwrap();
        let source = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(ticket.fact().source),
            _ => None,
        }).unwrap();
        // 借用函式已進入，來源 LoanResourceLinked 仍有效；
        // 即使 forged transfer 指向存在的 ResourceId，也不可認領。
        out.events.insert(ticket + 1, PrivateScalarCallEvent::ResourceTransferred {
            source, target: DeclarationId(0), resource_id: 1,
        });
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r06_同一真_call_雙_resourceid_整筆_move_依原參數順序交付逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙🪼\"\nreturn consume(first.move(),second.move(),5)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let handoffs = out.events.iter().filter_map(|ev| match ev {
            PrivateScalarCallEvent::ResourceTransferred { resource_id, .. } =>
                Some(*resource_id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(handoffs, [1, 2]);
    }

    #[test]
    fn g18_r06_雙_move_後續_int_實參動態溢位不可提前移交任何資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn consume(first.move(),second.move(),1+9223372036854775807)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|ev| matches!(ev,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
    }

    #[test]
    fn g18_r06_雙_resourceid_跨三層函式_每層重新綁定但零複製零洩漏() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn bridge(a:str,b:str,n:int)->int{\nreturn leaf(a.move(),b.move(),n)\n}\nfn main()->int{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nreturn bridge(first.move(),second.move(),2)\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Moved(1), R::Moved(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r06_同一來源重複兩次_move_必由_checker_阻止進入真框架() {
        let src = "fn consume(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"甲\"\nreturn consume(first.move(),first.move(),2)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_r06_成功函式未真正_return_不得提前歸還其入參票據() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let before = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::Returned(3)
        )).unwrap();
        let after = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(before < after);
        let premature = out.events.remove(after);
        out.events.insert(before, premature);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_函式失敗還入參票據必先留下真逐框架回滾起點() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nconst owner:str = \"失敗\"\nreturn inspect(owner,1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let unwind = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(unwind < returned);
        out.events.remove(unwind);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_偽造成功路徑開始回滾或失敗路徑重複開始均須拒絕() {
        let valid = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"真\"\nreturn inspect(owner,2)\n}\n";
        let mut completed = PreparedPrivateScalarCall::prepare(valid).unwrap()
            .execute().unwrap();
        assert!(completed.has_balanced_frame_trace());
        completed.events.insert(0, PrivateScalarCallEvent::UnwindStarted);
        assert!(!completed.has_balanced_frame_trace());

        let fails = "fn inspect(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nconst owner:str = \"真\"\nreturn inspect(owner,1)\n}\n";
        let mut failed = PreparedPrivateScalarCall::prepare(fails).unwrap()
            .execute().unwrap();
        assert!(failed.has_balanced_frame_trace());
        let at = failed.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        failed.events.insert(at + 1, PrivateScalarCallEvent::UnwindStarted);
        assert!(!failed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_無借用的失敗回滾不可遺漏_unwindstarted() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn calculate(n:int)->int{\nvar owner:str = \"真資源\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn calculate(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let count = out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::UnwindStarted)
        ).count();
        assert_eq!(count, 1);
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        ));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_成功結果不能以未返回的函式退出事件偽造() {
        let src = "fn calculate(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn calculate(5)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::Returned(_)
        ));
        for event in &mut out.events {
            if let PrivateScalarCallEvent::FunctionExited { returned, .. } = event {
                *returned = false;
            }
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r05_r06_未進入子框架的借用失敗不能先於回滾起點還票() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn run(n:int)->int{\nconst source:str = \"尚未進入\"\nreturn observe(source,n+9223372036854775807)\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(unwind < returned);
        // 保留所有資源事件及票據數量，只把還票移到正式失敗起點之前。
        let forged = out.events.remove(returned);
        out.events.insert(unwind, forged);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_已成功歸還的前序借用不受後續溢位回滾誤封鎖() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"先成功\"\nreturn observe(source,1)+9223372036854775807\n}\n";
        let out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let success = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(1)
        )).unwrap();
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        assert!(success < returned && returned < unwind);
    }


    #[test]
    fn g18_r05_r06_正式回滾後不得補造新的條件計算事件() {
        let src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        out.events.insert(at + 1, PrivateScalarCallEvent::ConditionalChecked(true));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_r06_正式回滾後即使局部事件平衡也不能重新初始化() {
        let src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap().execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let fake = DeclarationId(0);
        out.events.insert(at + 1, PrivateScalarCallEvent::LocalInitialized(fake, 7));
        out.events.insert(at + 2, PrivateScalarCallEvent::LocalScopeEnded(fake));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }


    #[test]
    fn g18_r05_r06_普通函式與同步self真呼叫均須通過共用框架票據進退帳() {
        let scalar_src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"真正借用\"\nreturn observe(source,4)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(4));
        assert!(scalar.has_balanced_frame_trace());
        let self_src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(self_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r05_r06_普通函式與同步self錯誤均須真回滾且拒絕偽造退出() {
        let scalar_src = "fn fail(n:int)->int{\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn fail(1)\n}\n";
        let mut scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(scalar.has_balanced_frame_trace());
        scalar.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        ));
        assert!(!scalar.has_balanced_frame_trace());
        let self_src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let plan = PreparedPrivateSelfMethod::prepare(self_src).unwrap();
        let mut method = plan.execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        method.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        ));
        assert!(!method.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_r06_普通函式真借用票據不得早於返回或晚於離域() {
        let src = "fn observe(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst source:str = \"因果\"\nreturn observe(source,4)\n}\n";
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut early = plan.execute().unwrap();
        assert!(early.has_balanced_frame_trace());
        let position = early.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let event = early.events.remove(position);
        let returned = early.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).unwrap();
        early.events.insert(returned, event);
        assert!(!early.has_balanced_frame_trace());

        let mut late = plan.execute().unwrap();
        let position = late.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let event = late.events.remove(position);
        let exit = late.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: true, .. }
        )).unwrap();
        late.events.insert(exit + 1, event);
        assert!(!late.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r06_普通函式與同步self方法int局部均沿用原_checker_真正求值且離域() {
        let scalar_src = "fn calc(a:int)->int{\nconst local:int = a+4\nreturn local-2\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(7));
        assert!(scalar.has_balanced_frame_trace());

        let method_src = "struct Counter{\nvar count:int = 7\nfn calc(a:int)->int{\nconst local:int = a+4\nreturn local-2\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(5)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert_eq!(method.method_local_events.len(), 2);
        assert_eq!(method.method_local_slots_after_exit, 0);
        assert!(method.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )));
    }

    #[test]
    fn g18_r06_普通函式與同步self局部溢位均保留真失敗回滾與乾淨離域() {
        let scalar_src = "fn calc(a:int)->int{\nconst first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(scalar.has_balanced_frame_trace());
        let method_src = "struct Counter{\nvar count:int = 7\nfn calc(a:int)->int{\nconst first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(5)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.method_entered && !method.method_exited && method.ticket_returned);
        assert_eq!(method.method_local_events.len(), 2);
        assert_eq!(method.method_local_slots_after_exit, 0);
    }


    #[test]
    fn g18_r05_r06_真普通函式int局部與同步self共用離域帳阻擋假初始化錯序() {
        let src = "fn calculate(n:int)->int{\nconst temp:int = n+2\nreturn temp\n}\nfn main()->int{\nreturn calculate(4)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, _)
        )).unwrap();
        let forged = out.events.remove(at);
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )).unwrap();
        out.events.insert(returned + 1, forged);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_r06_普通函式bool局部亦納入共用真退出帳的局部來源責任() {
        let src = "fn choose(n:int)->int{\nconst flag:bool = false\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut out = PreparedPrivateScalarCall::prepare(src).unwrap()
            .execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let at = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEnded(_)
        )).unwrap();
        out.events.remove(at);
        assert!(!out.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r06_同源int普通函式與方法局部只有方法真checker前綴可交付() {
        let scalar_src = "fn calc(n:int)->int{\nconst local:int = n+1\nreturn local\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(scalar_src)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(4));
        assert!(scalar.has_balanced_frame_trace());

        let self_src = "struct Sample{\nvar x:int = 1\nfn calc(n:int)->int{\nconst local:int = n+1\nreturn local\n}\n}\nfn main()->int{\nconst value:Sample = Sample()\nreturn value.calc(3)\n}\n";
        let plan = PreparedPrivateSelfMethod::prepare(self_src).unwrap();
        let mut method = plan.execute().unwrap();
        assert_eq!(method.result, Ok(4));
        assert!(method.has_balanced_self_borrow_trace());
        // 同一成功值與相同事件數量仍不能替換原 Checker
        // 簽發的方法內局部 DeclarationId 來源。
        method.expected_method_local_ids = vec![DeclarationId(0)];
        assert!(!method.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r06_普通函式與同步self方法均以真var_int更新通過共用離域帳() {
        let ordinary = "fn calc(a:int,b:int)->int{\nvar amount:int = a\namount = amount+b\nreturn amount\n}\nfn main()->int{\nreturn calc(3,4)\n}\n";
        let scalar = PreparedPrivateScalarCall::prepare(ordinary)
            .unwrap().execute().unwrap();
        assert_eq!(scalar.result, Ok(7));
        assert!(scalar.has_balanced_frame_trace());
        assert!(scalar.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 7)
        )));
        let method_src = "struct Sample{\nvar x:int = 1\nfn calc(a:int,b:int)->int{\nvar amount:int = a\namount = amount+b\nreturn amount\n}\n}\nfn main()->int{\nconst value:Sample = Sample()\nreturn value.calc(3,4)\n}\n";
        let method = PreparedPrivateSelfMethod::prepare(method_src)
            .unwrap().execute().unwrap();
        assert_eq!(method.result, Ok(7));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(method.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::LocalAssigned(_)
        )));
    }


}
