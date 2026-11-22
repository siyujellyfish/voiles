#[cfg(test)]
mod g18_v7_fastpath_graph_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e2_直接main呼叫原私有wrapper真UTF8幀可跨呼叫暫停恢復() {
        let plan = checked(concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"直接快路原資源🪼\"\n",
            "return await ready(6)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        assert!(plan.main_dynamic_call.is_none());
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("ready"));
        assert_eq!(live.retained_frames(), 1);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper"
        )).count(), 1);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "wrapper",
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_e2_快路多int實參沿原Checker宣告順序綁定並恢復await() {
        let plan = checked(concat!(
            "async fn increase(n:int)->int{ return n+1 }\n",
            "fn wrapper(first:int,second:int)->int{\n",
            "return await increase(first+second)\n}\n",
            "fn main()->int{ return wrapper(2,3) }\n",
        ));
        assert!(plan.main_dynamic_call.is_none());
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let bound = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::ParameterBound(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        // 真內層 async increase(n) 亦會新增自己的合法 ParameterBound，
        // 只取原 wrapper 的前兩筆對帳，不可把真正第三筆誤判成重複。
        assert_eq!(bound[..plan.callee_parameter_ids.len()],
            plan.callee_parameter_ids.iter().copied().zip([2,3]).collect::<Vec<_>>());
    }

    #[test]
    fn g18_v7_e3_直接main快路父資源暫停取消走原真scope退場() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 9 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"取消快路🪼\"\n",
            "return await ready()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(live.result().is_none());
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert!(live.result().is_none());
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "wrapper",
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready",
        )));
    }

    #[test]
    fn g18_v7_e4_直接main快路await後原運算溢位不生成成功Returned() {
        let plan = checked(concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"溢位仍須退場🪼\"\n",
            "return await over(9223372036854775807)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "wrapper",
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }
}

#[cfg(test)]
mod g18_v7_real_graph_cancel_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e3_真父子函式已建兩份UTF8資源後取消須逆序真釋放() {
        let plan = checked(concat!(
            "async fn target()->int{ return 4 }\n",
            "fn inner()->int{\n",
            "var child:str = \"內🪼\"\n",
            "return await target()\n}\n",
            "fn outer()->int{\n",
            "var parent:str = \"外🪼\"\n",
            "return inner()\n}\n",
            "fn main()->int{ return outer()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("target"));
        assert_eq!(live.retained_frames(), 3);
        let (parent, child) = match live.retained_resources() {
            [crate::owned_str_runtime::ResourceEvent::Created(parent),
             crate::owned_str_runtime::ResourceEvent::Created(child)] => (*parent, *child),
            _ => panic!("原父子已執行真 UTF-8 必各持唯一 ResourceId"),
        };
        assert_ne!(parent, child);
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert!(live.result().is_none());
        assert_eq!(live.retained_frames(), 0);
        assert_eq!(live.pending_await(), None);
        assert_eq!(evidence.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(parent),
            crate::owned_str_runtime::ResourceEvent::Created(child),
            crate::owned_str_runtime::ResourceEvent::Released(child),
            crate::owned_str_runtime::ResourceEvent::Released(parent),
        ]);
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
                | PrivateScalarCallEvent::Returned(_),
        )));
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_v7_e3_第一次真await已完成第二次暫停取消不得重執行第一個() {
        let plan = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{ return await first()+await second() }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("second"));
        let evidence = live.cancel_suspended().unwrap();
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "first"
        )).count(), 1);
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "second"
        )));
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "first"
        )).count(), 1);
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "second"
        )));
        assert!(evidence.resource_events.is_empty());
    }

    #[test]
    fn g18_v7_e3_尚未執行與已完成的真工作不能冒稱安全點取消() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 8 }\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        live.cancel_suspended().unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(matches!(live.poll(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        let mut completed = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(completed.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(completed.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert!(matches!(completed.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(completed.result().unwrap().result, Ok(8));
    }

    #[test]
    fn g18_v7_e3_暫停後意外新增真BorrowTicket時取消拒絕並保留原框架() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 4 }\n",
            "fn main()->int{\n",
            "var own:str = \"不能提前釋放🪼\"\n",
            "return await ready()+0\n}\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let original = live.retained_resources().to_vec();
        let source = DeclarationId(987);
        let ticket = live.checkpoint.as_mut().unwrap()
            .frames[0].borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert!(matches!(live.cancel_suspended(),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Suspended);
        assert_eq!(live.retained_resources(), original.as_slice());
        assert_eq!(live.retained_frames(), 1);
        live.checkpoint.as_mut().unwrap()
            .frames[0].borrow_tracker.release(ticket).unwrap();
        let evidence = live.cancel_suspended().unwrap();
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(live.status, PrivateDeferredAwaitStatus::Cancelled);
    }

    #[test]
    fn g18_v7_e3_暫停取消只留下真退場事件不釋放未啟動async子函式資源() {
        let plan = checked(concat!(
            "async fn ready()->int{\n",
            "var never:str = \"不得建構\"\n",
            "return 6\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(live.retained_resources().is_empty());
        let evidence = live.cancel_suspended().unwrap();
        assert!(evidence.resource_events.is_empty());
        assert!(evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!evidence.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
        assert!(live.result().is_none());
    }
}

#[cfg(test)]
mod g18_v7_real_graph_checkpoint_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src).unwrap()
    }

    #[test]
    fn g18_v7_e2_原私有函式真UTF8資源與顯式工作棧跨Rust呼叫暫停後原點恢復() {
        let plan = checked(concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"父框架已執行🪼\"\n",
            "return await ready(6)\n}\n",
            "fn main()->int{ return wrapper()+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("ready"));
        assert_eq!(live.retained_frames(), 2);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert!(live.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper",
        )));
        assert!(!live.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
        assert!(live.result().is_none());

        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready"
        )).count(), 1);
        assert_eq!(live.retained_frames(), 0);
        assert_eq!(live.pending_await(), None);
        assert!(plan.audit_graph_outcome(PrivateScalarCallOutcome {
            result: out.result.clone(), events: out.events.clone(),
            resource_events: out.resource_events.clone(),
            parameter_slots_after_exit: out.parameter_slots_after_exit,
        }).is_ok());
    }

    #[test]
    fn g18_v7_e2_原Checker兩個真await可跨三次呼叫從不同指令位置接續() {
        let plan = checked(concat!(
            "async fn left(n:int)->int{ return n+1 }\n",
            "async fn right(n:int)->int{ return n+2 }\n",
            "fn main()->int{ return await left(2)+await right(3) }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("left"));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(live.pending_await(), Some("right"));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let awaited = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(awaited, ["left", "right"]);
        assert!(matches!(live.poll(),
            Err(PrivateScalarCallPrepareError::Unsupported(
                "已結束的私有工作不得重複 poll",
            )),
        ));
    }

    #[test]
    fn g18_v7_e2_私有真框架暫停後能先執行另一工作再喚醒原工作() {
        let first = checked(concat!(
            "async fn ready()->int{ return 13 }\n",
            "fn main()->int{ return await ready() }\n",
        ));
        let second = checked(concat!(
            "async fn other()->int{ return 21 }\n",
            "fn main()->int{ return await other() }\n",
        ));
        let mut one = PrivateScalarGraphContinuation::new(&first).unwrap();
        let mut two = PrivateScalarGraphContinuation::new(&second).unwrap();
        assert_eq!(one.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(two.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert_eq!(two.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(two.result().unwrap().result, Ok(21));
        assert!(one.result().is_none());
        assert_eq!(one.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(one.result().unwrap().result, Ok(13));
        assert!(one.result().unwrap().has_balanced_frame_trace());
        assert!(two.result().unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e2_真私有await暫停後溢位仍只清理原P16資源不虛構返回() {
        let plan = checked(concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn main()->int{\n",
            "var hold:str = \"原框架錯誤也須釋放🪼\"\n",
            "return await over(9223372036854775807)\n}\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        assert!(matches!(live.retained_resources(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        let out = live.result().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_e2_同步原Checker工作不產生虛假的可暫停await() {
        let plan = checked(concat!(
            "fn forward(n:int)->int{ return n+4 }\n",
            "fn main()->int{ return forward(2)+0 }\n",
        ));
        let mut live = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(live.poll().unwrap(), PrivateScalarGraphPoll::Completed);
        assert_eq!(live.result().unwrap().result, Ok(6));
        assert_eq!(live.pending_await(), None);
        assert!(!live.result().unwrap().events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
    }

    #[test]
    fn g18_v7_e2_原Checker未授權跨await借用不能建立動態續行() {
        let source = concat!(
            "async fn ready()->int{ return 4 }\n",
            "fn wrapper(source:str)->int{ return await ready() }\n",
            "fn main()->int{\n",
            "var source:str = \"仍有效🪼\"\n",
            "return wrapper(source)\n}\n",
        );
        match PreparedPrivateScalarCall::prepare(source) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 必須先拒絕，不能建立真續行"),
        }
    }
}

#[cfg(test)]
mod g18_v7_ancestor_borrow_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    #[test]
    fn g18_v7_e5_外層真BorrowTicket未還不能因內層無票據而進await() {
        let mut parent = crate::borrow_safety::BorrowSafety::new();
        let child = crate::borrow_safety::BorrowSafety::new();
        let source = DeclarationId(501);
        let ticket = parent.acquire(source, BorrowKind::SharedRead).unwrap();
        let empty: &[(crate::borrow_safety::BorrowTicket, BorrowFact)] = &[];
        // 原先只檢查最內層 child 將錯誤放行；新的全框架安全點必拒。
        assert!(PreparedPrivateScalarCall::check_await_safe_point(&child, empty).is_ok());
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain([
                (&parent, empty), (&child, empty),
            ]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )),
        );
        assert_eq!(parent.check_ticket_access(ticket, BorrowKind::SharedRead), Ok(source));
        parent.release(ticket).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point_chain([
            (&parent, empty), (&child, empty),
        ]).is_ok());
    }

    #[test]
    fn g18_v7_e5_外層傳入仍有效的BorrowFact不能跨過內層await() {
        let parent = crate::borrow_safety::BorrowSafety::new();
        let child = crate::borrow_safety::BorrowSafety::new();
        let mut authority = crate::borrow_safety::BorrowSafety::new();
        let fact = BorrowFact::new(DeclarationId(502), BorrowKind::SharedRead);
        let ticket = authority.acquire(fact.source, fact.kind).unwrap();
        let incoming = [(ticket, fact)];
        let empty: &[(crate::borrow_safety::BorrowTicket, BorrowFact)] = &[];
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain([
                (&parent, incoming.as_slice()), (&child, empty),
            ]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )),
        );
        authority.finish_selected_verified(&[ticket], &[fact]).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point_chain([
            (&parent, empty), (&child, empty),
        ]).is_ok());
    }

    #[test]
    fn g18_v7_e5_空函式鏈不能被當成已驗證await安全點() {
        let empty: [(&crate::borrow_safety::BorrowSafety,
            &[(crate::borrow_safety::BorrowTicket, BorrowFact)]); 0] = [];
        assert_eq!(
            PreparedPrivateScalarCall::check_await_safe_point_chain(empty),
            Err(PrivateScalarCallRuntimeError::StateMismatch),
        );
    }

    #[test]
    fn g18_v7_e5_真Checker三層函式皆無借用可正常直接await() {
        let source = concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn inner(n:int)->int{ return await ready(n) }\n",
            "fn outer(n:int)->int{ return inner(n) }\n",
            "fn main()->int{ return outer(8) }\n",
        );
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert!(outcome.has_balanced_frame_trace());
        assert_eq!(outcome.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready",
        )).count(), 1);
        assert!(plan.audit_graph_outcome(outcome).is_ok());
    }

    #[test]
    fn g18_v7_e5_原Checker仍須拒絕未證歸還的跨await字串借用() {
        let source = concat!(
            "async fn ready()->int{ return 6 }\n",
            "fn inner(source:str)->int{ return await ready() }\n",
            "fn outer(source:str)->int{ return inner(source) }\n",
            "fn main()->int{\n",
            "var source:str = \"有效借用🪼\"\n",
            "return outer(source)\n}\n",
        );
        match PreparedPrivateScalarCall::prepare(source) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 必須在動態工作圖建立前拒絕借用跨 await"),
        }
    }
}

#[cfg(test)]
mod g18_v7_private_deferred_await_tests {
    use super::*;

    fn src(value: i64) -> String {
        format!(
            "async fn ready()->int{{ return {value} }}\n\
             fn main()->int{{ return await ready() }}\n"
        )
    }

    #[test]
    fn g18_v7_a1_私有Checker工作可跨呼叫保存直到真喚醒才執行() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(23)).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(queue.execution_order().is_empty());
        assert!(queue.jobs[&id].outcome.is_none());
        queue.suspend(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(queue.jobs[&id].plan.is_some());
        assert!(queue.execution_order().is_empty());
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Ok(23));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready",
        )));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(queue.execution_order(), &[id]);
        assert!(queue.jobs[&id].plan.is_none());
    }

    #[test]
    fn g18_v7_a1_兩個私有Checked工作只依真喚醒順序執行() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let first = queue.prepare(&src(1)).unwrap();
        let second = queue.prepare(&src(2)).unwrap();
        queue.suspend(first).unwrap();
        queue.suspend(second).unwrap();
        assert_eq!(queue.wake(second).unwrap().result, Ok(2));
        assert_eq!(queue.status(first), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.wake(first).unwrap().result, Ok(1));
        assert_eq!(queue.execution_order(), &[second, first]);
    }

    #[test]
    fn g18_v7_a1_取消已暫停私有工作不執行任何函式事件() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(12)).unwrap();
        queue.suspend(id).unwrap();
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.jobs[&id].plan.is_none());
        assert!(queue.jobs[&id].outcome.is_none());
        assert!(queue.execution_order().is_empty());
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::Cancelled));
        assert_eq!(queue.cancel(id), Err(PrivateDeferredAwaitError::AlreadyFinished));
    }

    #[test]
    fn g18_v7_a1_真正私有async局部UTF8只在喚醒後建構並唯一釋放() {
        let source = concat!(
            "async fn ready()->int{\n",
            "var hold:str = \"真正🪼UTF-8\"\n",
            "return 27\n}\n",
            "fn main()->int{ return await ready() }\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(source).unwrap();
        queue.suspend(id).unwrap();
        assert!(queue.jobs[&id].outcome.is_none());
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Ok(27));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
    }

    #[test]
    fn g18_v7_a2_私有直接等待真運算溢位後錯誤與資源單次退出() {
        let source = concat!(
            "async fn overflow()->int{\n",
            "var hold:str = \"出錯也要清理🪼\"\n",
            "return 9223372036854775807+1\n}\n",
            "fn main()->int{ return await overflow() }\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(source).unwrap();
        queue.suspend(id).unwrap();
        let outcome = queue.wake(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(queue.execution_order(), &[id]);
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
    }

    #[test]
    fn g18_v7_a1_有前置main資源不得假裝成已保存的等待框架() {
        let source = concat!(
            "async fn ready()->int{ return 3 }\n",
            "fn main()->int{\n",
            "var hold:str = \"不准偷偷跳過\"\n",
            "return await ready()\n}\n",
        );
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        assert!(matches!(queue.prepare(source),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
        assert!(queue.execution_order().is_empty());
    }

    #[test]
    fn g18_v7_a1_複合await與普通同步呼叫不能冒充直接等待() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let addition = concat!(
            "async fn ready()->int{ return 3 }\n",
            "fn main()->int{ return await ready()+1 }\n",
        );
        assert!(matches!(queue.prepare(addition),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        let sync = concat!(
            "fn plain()->int{ return 7 }\n",
            "fn main()->int{ return plain() }\n",
        );
        assert!(matches!(queue.prepare(sync),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn g18_v7_a1_靜態非法來源不得建立工作或消耗工作身分() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let invalid = concat!(
            "async fn ready()->int{ return true }\n",
            "fn main()->int{ return await ready() }\n",
        );
        assert!(matches!(queue.prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let id = queue.prepare(&src(7)).unwrap();
        assert_eq!(id, 1);
        assert!(queue.execution_order().is_empty());
        assert_eq!(queue.suspend(100), Err(PrivateDeferredAwaitError::UnknownTask));
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
        queue.cancel(id).unwrap();
    }

    #[test]
    fn g18_v7_a1_已封存私有來源竄改不能執行而可安全取消() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(17)).unwrap();
        queue.suspend(id).unwrap();
        queue.jobs.get_mut(&id).unwrap().plan.as_mut().unwrap().original[0] = b'!';
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(queue.execution_order().is_empty());
        assert!(queue.jobs[&id].outcome.is_none());
        // T3／V7：舊測試只驗醒來前拒絕、卻容許直接
        // 取消污染來源以抹掉原計畫。新的取消前來源守門
        // 必須保留污染證據；還原真 Checker 工作後才可取消。
        assert_eq!(queue.cancel(id),
            Err(PrivateDeferredAwaitError::SourceChanged));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert!(queue.jobs[&id].plan.is_some());
        queue.jobs.get_mut(&id).unwrap().plan =
            Some(PreparedPrivateScalarCall::prepare(&src(17)).unwrap());
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.jobs[&id].plan.is_none());
    }

    #[test]
    fn g18_v7_a1_真工作不允許重複暫停或重複喚醒() {
        let mut queue = PrivateCheckedDeferredAwaitQueue::new();
        let id = queue.prepare(&src(9)).unwrap();
        queue.suspend(id).unwrap();
        assert_eq!(queue.suspend(id), Err(PrivateDeferredAwaitError::WrongState));
        assert_eq!(queue.wake(id).unwrap().result, Ok(9));
        assert_eq!(queue.wake(id).err(), Some(PrivateDeferredAwaitError::WrongState));
        assert_eq!(queue.cancel(id), Err(PrivateDeferredAwaitError::AlreadyFinished));
        assert_eq!(queue.execution_order(), &[id]);
    }
}
