#[cfg(test)]
mod g18_private_scalar_call_tests {
    use super::*;
    use crate::common_ir::NodeSpan;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src)
            .expect("真私有 Checker 已核准 Copyable int 參數應能進入動態框架")
    }

    fn literal_src() -> &'static str {
        "fn main()->int{\nreturn forward(7)\n}\nfn forward(value:int)->int{\nreturn value\n}\n"
    }

    // E5：真動態 BorrowSafety 安全點只在原 Checker 的
    // await Call 實際到達後簽發，並與具名 async 函式 EnterCall 對帳。
    #[test]
    fn g18_v7_e5_直接await確實留下與真async框架相鄰的安全點() {
        let src = "async fn ready(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await ready(6)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let marker = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "ready"
        )).expect("真正到達 await 才產生安全點");
        assert!(matches!(out.events.get(marker+1),
            Some(PrivateScalarCallEvent::FunctionEntered(name)) if name == "ready"));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_偽造刪除async等待事件不得冒充同步進框架() {
        let src = "async fn ready()->int{\nreturn 7\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        let mut out = plan.execute().unwrap();
        out.events.retain(|e| !matches!(e, PrivateScalarCallEvent::AwaitSafePoint(_)));
        // 舊的純框架平衡表只看 enter/exit，須另用原 Checker
        // async 來源身分對帳防止等待事件整筆消失。
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v7_e5_等待點改綁別的函式或重播都必須拒絕() {
        let src = "async fn ready()->int{\nreturn 7\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        let mut forged = plan.execute().unwrap();
        let mark = forged.events.iter_mut().find_map(|e| match e {
            PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name),
            _ => None,
        }).unwrap();
        *mark = "another".to_string();
        assert!(!forged.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(forged),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let mut replayed = plan.execute().unwrap();
        let pos = replayed.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )).unwrap();
        replayed.events.insert(pos, PrivateScalarCallEvent::AwaitSafePoint(
            "ready".to_string(),
        ));
        assert!(!replayed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e5_原同步普通函式不得假造await安全點() {
        let src = "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(7)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_跨函式借用來源由原Checker拒絕且真票據守門同樣拒絕() {
        let src = "async fn ready()->int{\nreturn 4\n}\nfn wrapper(source:str)->int{\nreturn await ready()\n}\nfn main()->int{\nvar source:str = \"真資源🪼\"\nreturn wrapper(source)\n}\n";
        // Checker 在 wrapper 獨立檢查階段已看見未證明歸還的
        // str 參數借用，故不能假裝此來源可進入真執行器。
        match PreparedPrivateScalarCall::prepare(src) {
            Err(PrivateScalarCallPrepareError::Check(problem)) => {
                assert!(problem.message.contains("VB15-01-A"));
            }
            _ => panic!("原 Checker 須比 runtime 更早拒絕跨等待借用"),
        }
        let fact = BorrowFact::new(DeclarationId(803), BorrowKind::SharedRead);
        let mut tracker = crate::borrow_safety::BorrowSafety::new();
        let ticket = tracker.acquire(fact.source, fact.kind).unwrap();
        assert_eq!(PreparedPrivateScalarCall::check_await_safe_point(&tracker, &[]),
            Err(PrivateScalarCallRuntimeError::Borrow(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            )));
        // 另一種真正來源：本框架 tracker 即使沒有借用，
        // 上一個動態框架傳入而尚未歸還的票據仍不可越過 await。
        let empty = crate::borrow_safety::BorrowSafety::new();
        assert_eq!(PreparedPrivateScalarCall::check_await_safe_point(
            &empty, &[(ticket, fact)],
        ), Err(PrivateScalarCallRuntimeError::Borrow(
            crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
        )));
        tracker.release(ticket).unwrap();
        assert!(PreparedPrivateScalarCall::check_await_safe_point(
            &tracker, &[],
        ).is_ok());
    }

    #[test]
    fn g18_v7_e5_真正str_move直接await不冒充借用且資源唯一清理() {
        let src = "async fn consume(value:str)->int{\nreturn 4\n}\nfn main()->int{\nvar source:str = \"真移交🪼\"\nreturn await consume(source.move())\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "consume"
        )).count(), 1);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 1);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 1);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e5_尚未能進入async函式的框架預算錯誤不可虛構安全點() {
        let src = "async fn ready()->int{\nreturn 4\n}\nfn wrapper()->int{\nreturn await ready()\n}\nfn main()->int{\nreturn wrapper()\n}\n";
        let plan = checked(src);
        let out = plan.execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 500,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )));
    }


    // V7／E2-A：來源真的執行到 await 前，協作排程才可暫用
    // 同一 Rust 執行緒；父函式 P16 資源與真框架一直由原工作持有。
    #[test]
    fn g18_v7_e2_真私有await在內層函式中讓出且父框架資源仍存活() {
        let main = checked(concat!(
            "async fn target(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\nvar hold:str = \"原工作🪼\"\nreturn await target(6)\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let other = checked(concat!(
            "async fn side()->int{ return 9 }\n",
            "fn main()->int{ return await side() }\n",
        ));
        let mut visited = Vec::<(String, Vec<crate::owned_str_runtime::ResourceEvent>)>::new();
        let mut scheduler = |name: &str, resources: &[crate::owned_str_runtime::ResourceEvent]| {
            visited.push((name.to_owned(), resources.to_vec()));
            let finished = other.execute().unwrap();
            assert_eq!(finished.result, Ok(9));
            assert!(finished.has_balanced_frame_trace());
        };
        let out = main.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].0, "target");
        assert!(matches!(visited[0].1.as_slice(),
            [crate::owned_str_runtime::ResourceEvent::Created(_)]));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "target"
        )).count(), 1);
        assert!(main.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_雙真正await只按動態左右順序發出協作切換() {
        let plan = checked(concat!(
            "async fn first(n:int)->int{ return n+1 }\n",
            "async fn second(n:int)->int{ return n+2 }\n",
            "fn main()->int{ return await first(2)+await second(3) }\n",
        ));
        let mut order = Vec::<String>::new();
        let mut scheduler = |name: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            order.push(name.to_owned());
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(order, ["first", "second"]);
        assert!(out.has_balanced_frame_trace());
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_未達等待點的框架預算耗盡絕不執行排程者() {
        let plan = checked(concat!(
            "async fn inner()->int{ return 3 }\n",
            "fn wrapper()->int{ return await inner() }\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let mut count = 0usize;
        let mut scheduler = |_: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            count += 1;
        };
        let out = plan.execute_with_budget_and_fault_scheduler(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 200 },
            false, Some(&mut scheduler),
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(count, 0);
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(_)
        )));
    }

    #[test]
    fn g18_v7_e2_普通同步呼叫不得觸發等待排程() {
        let plan = checked(concat!(
            "fn plain(n:int)->int{ return n+1 }\n",
            "fn main()->int{ return plain(5) }\n",
        ));
        let mut count = 0usize;
        let mut scheduler = |_: &str, _: &[crate::owned_str_runtime::ResourceEvent]| {
            count += 1;
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(count, 0);
        assert!(out.has_balanced_frame_trace());
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v7_e2_真await讓出後原執行溢位仍逐框架清理且不虛構返回() {
        let plan = checked(concat!(
            "async fn overflow(n:int)->int{ return n+1 }\n",
            "fn main()->int{\nvar hold:str = \"留存🪼\"\n",
            "return await overflow(9223372036854775807)\n}\n",
        ));
        let mut seen = Vec::new();
        let mut scheduler = |name: &str, events: &[crate::owned_str_runtime::ResourceEvent]| {
            seen.push((name.to_owned(), events.to_vec()));
        };
        let out = plan.execute_with_await_scheduler(&mut scheduler).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "overflow");
        assert!(matches!(seen[0].1.as_slice(),
            [crate::owned_str_runtime::ResourceEvent::Created(_)]));
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::UnwindStarted)));
        assert!(out.events.iter().all(|e|
            !matches!(e, PrivateScalarCallEvent::Returned(_))));
    }


    // V7／A1：原 Checker 已接受的直接 await，不在準備時執行；
    // 只在原 int 工作圖真正走到 await Call 才逐幀進入並退出。
    #[test]
    fn g18_v7_a1_原私有async直接await真int參數進框架返回() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(4)\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { awaited: true, .. })));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(4)
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "increase"
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(5)
        )));
        assert_eq!(out.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_v7_a1_真正零參數async直接await不能走舊同步快路() {
        let src = "async fn ready()->int{\nreturn 8\n}\nfn main()->int{\nreturn await ready()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { awaited: true, .. })));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )).count(), 1);
    }

    #[test]
    fn g18_v7_a1_兩個直接await按原運算式順序串行執行() {
        let src = "async fn left(n:int)->int{\nreturn n+1\n}\nasync fn right(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nreturn await left(2)+await right(3)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["left", "right"]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v7_a1_直接await內層溢位必受控回滾不偽造正常返回() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(9223372036854775807)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, returned: false }
            if name == "increase"
        )));
    }

    #[test]
    fn g18_v7_a1_竄改已編譯await身分即使來源相同仍拒絕執行() {
        let src = "async fn increase(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn await increase(4)\n}\n";
        let mut plan = checked(src);
        let Some(PrivateScalarReturnExpr::Call { awaited, .. }) = &mut plan.main_dynamic_call
            else { panic!("必須是已核准的直接等待工作計畫") };
        assert!(*awaited);
        *awaited = false;
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F2／R07：不變更 Checker 型別決策，只要求同框架
    // int、bool、str 借用／Move 都使用唯一的原 DeclarationId。
    #[test]
    fn g18_v1_f2_混合int_bool參數合法且遞迴重入各有獨立身分() {
        let src = "fn alpha(n:int,flag:bool)->int{\nif(flag){return beta(n,false)}\nreturn n\n}\nfn beta(n:int,flag:bool)->int{\nreturn alpha(n,flag)\n}\nfn main()->int{\nreturn alpha(7,true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
                | PrivateScalarCallEvent::BoolParameterBound(..)
        )).count(), 6);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f2_整數參數重複綁定即使原結果正確也拒絕() {
        let src = "fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(9)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).expect("原 Checker 的唯一整數參數");
        let forged = out.events[index].clone();
        out.events.insert(index + 1, forged);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f2_布林參數重播與冒充int同身分一律拒絕() {
        let src = "fn pick(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n\n}\nfn main()->int{\nreturn pick(true,4)\n}\n";
        let baseline = checked(src).execute().unwrap();
        assert_eq!(baseline.result, Ok(5));
        assert!(baseline.has_balanced_frame_trace());
        let index = baseline.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::BoolParameterBound(..)
        )).expect("原 Checker bool 參數");
        let mut replay = checked(src).execute().unwrap();
        replay.events.insert(index + 1, replay.events[index].clone());
        assert!(!replay.has_balanced_frame_trace());
        let mut spoof = baseline;
        let id = match spoof.events[index] {
            PrivateScalarCallEvent::BoolParameterBound(id, _) => id,
            _ => unreachable!("已確認 bool 參數"),
        };
        spoof.events.insert(index + 1,
            PrivateScalarCallEvent::ParameterBound(id, 8));
        assert!(!spoof.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f2_借用str參數不可偽造已存在的int宣告身分() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"真借用🪼\"\nreturn inspect(owner,3)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let int_id = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::ParameterBound(id, _) => Some(*id),
            _ => None,
        }).expect("真整數參數");
        let borrow = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::BorrowParameterBound(id, _) => Some(id),
            _ => None,
        }).expect("真 str 借用參數");
        *borrow = int_id;
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f2_跨框架str移交不可重綁另一個int參數身分() {
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owner:str = \"真移交🪼\"\nreturn inspect(owner.move(),4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let int_id = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::ParameterBound(id, _) => Some(*id),
            _ => None,
        }).expect("真整數參數");
        for event in &mut out.events {
            if let PrivateScalarCallEvent::ResourceTransferred { target, .. } = event {
                *target = int_id;
            }
        }
        let transferred = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::ResourceTransferred { .. }
        )).expect("真正跨框架移交");
        let owned = out.events.iter_mut().skip(transferred + 1).find_map(|event| match event {
            PrivateScalarCallEvent::ResourceOwnerBound(id, _) => Some(id),
            _ => None,
        }).expect("移交後真正子框架資源綁定");
        *owned = int_id;
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    // V1／F1：原 Checker 直接 Call、子呼叫及 main 的真正
    // int 返回值必須與最外層 Returned 事件相符，複合式不能誤判。
    #[test]
    fn g18_v1_f1_main直接呼叫的真返回值與最外層事件一致() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let prepared = checked(src);
        assert!(matches!(&prepared.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Call { .. })));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_偽造最外層Returned數值即使框架平衡也拒絕() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let prepared = checked(src);
        let mut out = prepared.execute().unwrap();
        let event = out.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("原 Checker 的直接返回事件");
        if let PrivateScalarCallEvent::Returned(value) = event {
            *value += 100;
        }
        // 原退出帳只核對有無返回，未曾交叉核對真正 main 結果。
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_多層返回只核對最外層不得誤用內層值() {
        let src = "fn leaf(n:int)->int{\nreturn n+1\n}\nfn wrap(n:int,flag:bool)->int{\nif(flag){return leaf(n)+2}\nreturn n\n}\nfn main()->int{\nreturn wrap(3,true)\n}\n";
        let prepared = checked(src);
        let baseline = prepared.execute().unwrap();
        assert_eq!(baseline.result, Ok(6));
        assert!(prepared.audit_graph_outcome(baseline).is_ok());
        let mut forged = prepared.execute().unwrap();
        let last = forged.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("外層 wrap 的真返回");
        if let PrivateScalarCallEvent::Returned(value) = last {
            *value += 1;
        }
        assert!(forged.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(forged),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_main兄弟呼叫複合算術不得誤把子返回當主返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+2}\nreturn n\n}\nfn main()->int{\nreturn choose(true,5)+choose(false,3)\n}\n";
        let prepared = checked(src);
        assert!(matches!(&prepared.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Addition(_, _))));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int直接呼叫沿同一原Checker結果真值稽核() {
        let prepared = checked(literal_src());
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int直接返回被改寫即使事件平衡也必拒絕() {
        let prepared = checked(literal_src());
        assert!(prepared.main_dynamic_call.is_none());
        let mut out = prepared.execute().unwrap();
        let event = out.events.iter_mut().rfind(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )).expect("原動態函式返回");
        if let PrivateScalarCallEvent::Returned(value) = event {
            *value = 777;
        }
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(prepared.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f1_舊式純int多層函式返回不得把子函式結果當主返回() {
        let src = "fn leaf(n:int)->int{\nreturn n+1\n}\nfn wrap(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn wrap(4)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [5, 7]);
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f1_舊式純int溢位無返回且仍正常回滾() {
        let src = "fn forward(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn forward(9223372036854775807)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_none());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
        assert!(out.has_balanced_frame_trace());
        assert!(prepared.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序() {
        let prepared = checked(literal_src());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            PrivateScalarCallEvent::ArgumentEvaluated(7),
            PrivateScalarCallEvent::FunctionEntered("forward".into()),
            PrivateScalarCallEvent::ParameterBound(prepared.parameter_id.expect("已驗證唯一 int 參數"), 7),
            PrivateScalarCallEvent::Returned(7),
            PrivateScalarCallEvent::FunctionExited {
                name: "forward".into(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nconst n:int = 8\nreturn identity(n)\n}\n";
        let prepared = checked(src);
        assert!(prepared.owner_id.is_some());
        assert_ne!(prepared.owner_id, prepared.parameter_id);
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nvar owner:int = 42\nreturn identity(owner)\n}\n";
        let prepared = checked(src);
        for _ in 0..3 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(42));
            assert_eq!(result.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_c_真正參數框架中途故障必清空且不冒充成功返回() {
        let prepared = checked(literal_src());
        let failure = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failure.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failure.parameter_slots_after_exit, 0);
        assert!(!failure.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failure.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "forward".into(), returned: false,
        }));
        let success = prepared.execute().unwrap();
        assert_eq!(success.result, Ok(7));
        assert_eq!(success.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_錯誤_copyable_int_實參型別及引數個數由原_checker_拒絕() {
        for src in [
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(true)\n}\n",
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(1,2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_)),), "{src}");
        }
    }

    #[test]
    fn g18_a_具名未初始化_int_無法假裝有函式實參值() {
        let src = "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int\nreturn forward(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_a_竄改已驗證參數_declarationid_和返回值必須零執行拒絕() {
        let mut prepared = checked(literal_src());
        prepared.parameter_id = Some(DeclarationId(0));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.argument = Some(999);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_竄改主函式呼叫跨度或原來源位元組不得建立任何參數框架() {
        let mut prepared = checked(literal_src());
        prepared.call_span = NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // G18 P18.1 第二批：函式內的算術不是靜態算好的常值，
    // 每個運算元都須從當次合法 Copyable int 參數槽取值。
    #[test]
    fn g18_a_真函式_copyable_int_參數參與加法且回傳動態值() {
        let src = "fn step(value:int)->int{\nreturn value+3\n}\nfn main()->int{\nreturn step(9)\n}\n";
        let plan = checked(src);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(result.events.contains(&PrivateScalarCallEvent::Returned(12)));
    }

    #[test]
    fn g18_a_真函式_copyable_int_參數多次引用每次都取同一實際值() {
        let src = "fn twice(value:int)->int{\nreturn value+value\n}\nfn main()->int{\nreturn twice(11)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(22));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算() {
        let src = "fn mix(value:int)->int{\nreturn (value+2)-(value-1)\n}\nfn main()->int{\nreturn mix(10)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(3));
    }

    #[test]
    fn g18_a_具名_copyable_int_實參傳入函式後參與真減法() {
        let src = "fn minus(value:int)->int{\nreturn value-3\n}\nfn main()->int{\nconst owner:int = 10\nreturn minus(owner)\n}\n";
        let plan = checked(src);
        assert!(plan.owner_id.is_some());
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_函式參數真加法溢位不能假造_return_且確實離域() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(!result.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(result.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "inc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_函式參數真減法溢位亦不可留下框架或偽造成功事件() {
        let src = "fn minus(value:int)->int{\nreturn value-9223372036854775807-2\n}\nfn main()->int{\nreturn minus(0)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }

    #[test]
    fn g18_a_竄改編譯後_true_arithmetic_運算樹必在執行前拒絕() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(2)\n}\n";
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Subtraction(
            Box::new(PrivateScalarReturnExpr::Parameter(prepared.parameter_id.expect("已驗證 int 參數"))),
            Box::new(PrivateScalarReturnExpr::Constant(1)),
        );
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_非法_bool_參與_int_加法與字面值不足均由原_checker_拒絕() {
        let bad_type = "fn wrong(value:int)->int{\nreturn value+true\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let bad_name = "fn wrong(value:int)->int{\nreturn value+missing\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_name),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }
    // P18.1／18.4：不是新的模型，而是原單參數真正 executor 的
    // 多 Parameter DeclarationId、TypedHIR、依序求值與退出擴充。
    fn multiple_source() -> &'static str {
        "fn combine(a:int,b:int)->int{\nreturn (a+b)-a\n}\nfn main()->int{\nreturn combine(7,9)\n}\n"
    }

    #[test]
    fn g18_a_雙_copyable_int_參數依原_checker_宣告順序逐一綁定() {
        let prepared = checked(multiple_source());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events[0], PrivateScalarCallEvent::ArgumentEvaluated(7));
        assert_eq!(outcome.events[1], PrivateScalarCallEvent::ArgumentEvaluated(9));
        assert!(matches!(outcome.events[2], PrivateScalarCallEvent::FunctionEntered(_)));
        assert_eq!(outcome.events[3], PrivateScalarCallEvent::ParameterBound(
            prepared.parameter_id.expect("已驗證 int 參數"), 7,
        ));
        assert_eq!(outcome.events[4], PrivateScalarCallEvent::ParameterBound(
            prepared.additional_arguments[0].0, 9,
        ));
        assert_ne!(prepared.parameter_id.expect("已驗證 int 參數"), prepared.additional_arguments[0].0);
    }

    #[test]
    fn g18_a_三個_int_參數在同一真框架內逐項參與原_hir_算式() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn (a-b)+c\n}\nfn main()->int{\nreturn blend(8,3,5)\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 3);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_同一個_const_int_持有者作兩個實參屬_copyable_不消耗() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst original:int = 21\nreturn sum(original,original)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.owner_id, prepared.additional_arguments[0].2);
        for _ in 0..3 {
            let outcome = prepared.execute().unwrap();
            assert_eq!(outcome.result, Ok(42));
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_多個具名_var_const_int_和常值實參按照原宣告身分傳遞() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn a+b+c\n}\nfn main()->int{\nconst first:int = 4\nvar second:int = 3\nreturn blend(first,1,second)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        assert_eq!(prepared.additional_arguments[1].2, Some(prepared.main_owners[1].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_多參數整數溢位不可回傳成功且所有動態參數清空() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(9223372036854775807,1)\n}\n";
        let prepared = checked(text);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_多參數真框架測試故障不殘留參數也不假冒_return() {
        let prepared = checked(multiple_source());
        let failed = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_a_擅改第二實參值或第二_parameter_declarationid_先驗後拒絕() {
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].1 = 100;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].0 = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.main_owners.push((DeclarationId(999), 1));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_多參數錯誤型別引數數量拒絕且未使用純值宣告合法() {
        for text in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,true)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(text),
                Err(PrivateScalarCallPrepareError::Check(_))), "{text}");
        }
        let unused = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar extra:int = 5\nreturn sum(1,2)\n}\n";
        let prepared = checked(unused);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
    }

    // P18.1／P18.3／P18.4：同一原 Checker／Parameter HIR 的真正多層具參數
    // 普通函式圖；每個呼叫邊皆有真正的進／出事件與獨立 int slot。
    fn graph_source() -> &'static str {
        "fn leaf(c:int)->int{\nreturn c+1\n}\nfn middle(a:int,b:int)->int{\nreturn leaf(a+b)\n}\nfn main()->int{\nreturn middle(3,4)\n}\n"
    }

    #[test]
    fn g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出() {
        let prepared = checked(graph_source());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 2);
        assert!(matches!(&result.events[0], PrivateScalarCallEvent::ArgumentEvaluated(3)));
        assert!(matches!(&result.events[1], PrivateScalarCallEvent::ArgumentEvaluated(4)));
        let trace = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(format!("+{name}")),
            PrivateScalarCallEvent::FunctionExited { name, returned } if *returned =>
                Some(format!("-{name}")),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(trace, ["+middle", "+leaf", "-leaf", "-middle"]);
    }

    #[test]
    fn g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值() {
        let src = "fn last(x:int)->int{\nreturn x-2\n}\nfn next(y:int)->int{\nreturn last(y+3)\n}\nfn first(z:int)->int{\nreturn next(z+4)\n}\nfn main()->int{\nreturn first(8)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "next", "last"]);
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["last", "next", "first"]);
    }

    #[test]
    fn g18_a_同一巢狀葉函式重複真呼叫各自返回且不共用參數槽() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn twice(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn twice(2,5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let values = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [3, 6, 9]);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )).count(), 2);
    }

    #[test]
    fn g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "outer"]);
    }

    #[test]
    fn g18_c_父框架故障時不可進入尚未求值的巢狀子函式() {
        let failed = checked(graph_source()).execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )));
        assert!(!failed.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failed.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "middle".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_巢狀函式來源或參數宣告身份被篡改零動態框架拒絕() {
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].parameters[0] = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].result = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.verified.plan.declarations.by_source.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出() {
        let src = "fn alpha(x:int)->int{\nreturn beta(x)\n}\nfn beta(y:int)->int{\nreturn alpha(y)\n}\nfn main()->int{\nreturn alpha(1)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["alpha", "beta", "alpha", "beta", "alpha"]);
        let exited = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exited, ["alpha", "beta", "alpha", "beta", "alpha"]);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕() {
        let bad_type = graph_source().replace("leaf(a+b)", "leaf(true)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let unknown = graph_source().replace("leaf(a+b)", "missing(a+b)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&unknown),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 8, max_steps: 500 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 8);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count(), 8);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_合法前向六層以上_copyable_int_函式真逐層返回() {
        let mut src = String::new();
        for i in 0..9 {
            let target = if i == 8 {
                "n+1".to_owned()
            } else {
                format!("f{}(n)", i+1)
            };
            src.push_str(&format!("fn f{i}(n:int)->int{{\nreturn {target}\n}}\n"));
        }
        src.push_str("fn main()->int{\nreturn f0(12)\n}\n");
        let outcome = checked(&src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 9);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 9);
    }

    #[test]
    fn g18_a_五個_copyable_int_真參數不應被內部三參數上界拒絕() {
        let src = "fn sum(a:int,b:int,c:int,d:int,e:int)->int{\nreturn a+b+c+d+e\n}\nfn main()->int{\nreturn sum(1,2,3,4,5)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(15));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ParameterBound(..))).count(), 5);
    }

    #[test]
    fn g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(9)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 200, max_steps: 8 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        let entered = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count();
        assert!(entered >= 2);
        assert_eq!(entered, exited);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_零框架或零步數預算不得偽造任何真函式進入() {
        let checked = checked(graph_source());
        for budget in [
            PrivateScalarExecutionBudget { max_call_frames: 0, max_steps: 100 },
            PrivateScalarExecutionBudget { max_call_frames: 100, max_steps: 0 },
        ] {
            let outcome = checked.execute_with_budget(budget).unwrap();
            assert!(outcome.result.is_err());
            assert!(outcome.events.is_empty());
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕() {
        let src = "fn unused(x:int)->int{\nreturn unused(x)\n}\nfn good(y:int)->int{\nreturn y+2\n}\nfn main()->int{\nreturn good(7)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 30 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(name) if name == "unused")).count(), 0);
    }

    #[test]
    fn g18_c_有界步數的兩個巢狀兄弟呼叫僅在實際出現時建立獨立框架() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn both(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn both(1,2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "leaf", "both"]);
    }


    #[test]
    fn g18_a_零參數前向葉函式與_copyable_int_父框架真返回() {
        let src = "fn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\nfn fixed()->int{\nreturn 7\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["plus", "fixed"]);
        let returned = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [7, 12]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_零參數函式重複呼叫不重用動態框架且保留真求值順序() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn add(n:int)->int{\nreturn fixed()+n+fixed()\n}\nfn main()->int{\nreturn add(5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["add", "fixed", "fixed"]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "fixed")).count(), 2);
    }

    #[test]
    fn g18_c_零參數自遞迴由真框架預算受控中止不虛構返回() {
        let src = "fn loop()->int{\nreturn loop()\n}\nfn entry(n:int)->int{\nreturn n+loop()\n}\nfn main()->int{\nreturn entry(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                returned: false, ..
            })).count(), 4);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_零參數子函式原宣告來源遭竄改必在動態執行前拒絕() {
        let src = "fn fixed()->int{\nreturn 7\n}\nfn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\n";
        let mut prepared = checked(src);
        let zero = prepared.nested_callees.iter_mut()
            .find(|f| f.name == "fixed").expect("原 Checker 的零參數函式");
        assert!(zero.parameters.is_empty());
        zero.parameters.push(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_巢狀兄弟呼叫每個實參求值完成立即紀錄且不重複() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a-b\n}\nfn outer(n:int)->int{\nreturn pair(leaf(n),leaf(n+2))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(-2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        // main 的 2、leaf(2) 的 2、pair 第一實參 3、
        // leaf(4) 的 4、pair 第二實參 5，必為原 Parser 的左至右順序。
        assert_eq!(arguments, [2, 2, 3, 4, 5]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf")).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")).count(), 1);
    }

    #[test]
    fn g18_c_第一實參完成後第二實參溢位仍保留真事件而不進入父呼叫() {
        let src = "fn first(x:int)->int{\nreturn x+1\n}\nfn bad(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a+b\n}\nfn outer(n:int)->int{\nreturn pair(first(n),bad(9223372036854775807))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 2, 3, 9223372036854775807]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 3);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited { .. })).count(), 3);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "outer")));
    }



    #[test]
    fn g18_a_main_零參數呼叫真正建立空參數框架並返回() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events, vec![
            PrivateScalarCallEvent::FunctionEntered("value".to_owned()),
            PrivateScalarCallEvent::Returned(9),
            PrivateScalarCallEvent::FunctionExited {
                name: "value".to_owned(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_main_零參數前向函式串接巢狀真返回() {
        let src = "fn first()->int{\nreturn second()\n}\nfn main()->int{\nreturn first()\n}\nfn second()->int{\nreturn 11\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "second"]);
        let returns = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [11, 11]);
    }

    #[test]
    fn g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出() {
        let src = "fn repeat()->int{\nreturn repeat()\n}\nfn main()->int{\nreturn repeat()\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 4);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_a_main_零參數來源被偽造一個參數必執行前拒絕() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let mut prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        prepared.parameter_id = Some(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.argument = Some(123);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_main_原_checker_括號常值引數真正進入雙參數框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum((5),(7))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 7]);
    }

    #[test]
    fn g18_a_main_多層括號具名_copyable_來源仍維持原宣告身分() {
        let src = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst original:int = 11\nreturn identity((((original))))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號布林與未知名稱不可充當合法_int_實參() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id((true))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id(((missing)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_數個未使用純值宣告與具名實參各自保留來源身分() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nvar input:int = 4\nvar another:int = 9\nreturn id(input)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert!(prepared.owner_id.is_some());
        for _ in 0..2 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(4));
            assert_eq!(result.parameter_slots_after_exit, 0);
            assert_eq!(result.events.iter().filter(|event| matches!(
                event, PrivateScalarCallEvent::ArgumentEvaluated(_)
            )).count(), 1);
        }
    }

    #[test]
    fn g18_a_未使用純值宣告遭竄改仍須執行前拒絕() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nreturn id(3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[0].1 = 999;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let changed = src.replace("unused:int = 8", "unused:int = 9");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&changed),
            Ok(_)));
        assert_ne!(prepared.original.as_ref(), changed.as_bytes());
    }



    #[test]
    fn g18_a_main_括號返回帶參數普通函式依原_checker_真求值() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn (((inc(7))))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(arguments, [7]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_多層括號零參數前向函式返回不虛構參數() {
        let src = "fn main()->int{\nreturn (((value())))\n}\nfn value()->int{\nreturn 23\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(23));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..))).count(), 0);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號返回的真內層呼叫跨度不得竄改() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(6)))\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.call_span = crate::common_ir::NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_括號返回錯型別與不存在函式由原_checker_拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(true)))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((missing(6)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_main_具名_copyable_int_多層括號初始化仍真傳入函式() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nconst base:int = ((((6))))\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 6);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_未使用的合法_group_int_初始化不應阻擋零參數函式() {
        let src = "fn fixed()->int{\nreturn 17\n}\nfn main()->int{\nvar unused:int = ((8))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 8);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(17));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_括號初始化為布林或缺失名稱仍由原_checker_拒絕() {
        for src in [
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((true))\nreturn id(owner)\n}\n",
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((missing))\nreturn id(owner)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_括號初始化持有者遭竄改不得建立動態框架() {
        let src = "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nvar owner:int = (((9)))\nreturn id(owner)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
        prepared.main_owners[0].1 = 90;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_已核准_copyable_int_名稱初始化串接真傳參() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst origin:int = 6\nvar copied:int = origin\nconst further:int = copied\nreturn inc(further)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert_eq!(prepared.main_owners.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [6, 6, 6]);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(6)
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_括號名稱初始化與未使用副本不消耗來源() {
        let src = "fn fixed()->int{\nreturn 13\n}\nfn main()->int{\nconst origin:int = ((9))\nvar unused:int = ((((origin))))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.main_owners[0].1, 9);
        assert_eq!(prepared.main_owners[1].1, 9);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_初始化來源錯型別或未宣告及尚未初始化仍靜態拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst ok:bool = true\nconst copy:int = ok\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst copy:int = missing\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar original:int\nconst copy:int = original\nreturn id(copy)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_被複製初始化持有者來源身分竄改必先拒絕() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst origin:int = 3\nconst copy:int = origin\nreturn id(copy)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[1].0 = DeclarationId(usize::MAX);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.main_owners[1].1 = 31;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_原_checker_算術實參於真動態框架依順序求值() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,7-4)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(6));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count(), 1);
    }

    #[test]
    fn g18_a_main_具名_copyable_算術與巢狀普通函式依真來源求值() {
        let src = "fn inc(v:int)->int{\nreturn v+1\n}\nfn combine(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{\nconst origin:int = 5\nreturn combine(inc(origin),inc(2+1))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["inc", "inc", "combine"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 6, 3, 4]);
    }

    #[test]
    fn g18_a_main_零參數葉函式作為複合實參須先真返回再綁定目標() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(fixed(),6)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["fixed", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [4, 6]);
    }

    #[test]
    fn g18_c_main_第二算術實參溢位保留前一完成事件且不可進入目標() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn sum(5+1,high+1)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [6]);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
                | PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_c_main_算術實參步數預算耗盡不會補寫未完成參數事件() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 6 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))));
    }

    #[test]
    fn g18_c_main_動態實參綁定後故障須先退出真根框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 2);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_a_main_動態算式已驗證工作計畫遭竄改必在執行前拒絕() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(99));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_動態實參非法型別或未知名稱仍須由原_checker_拒絕() {
        for src in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(true+2,3)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(2+missing,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_c_main_同名巢狀實參故障只能中斷真正外層函式框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(sum(1,2),3)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count();
        assert_eq!(enters, 2);
        let successes = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count();
        let failures = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "sum")).count();
        assert_eq!((successes, failures), (1, 1));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(3))).count(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
    }

    #[test]
    fn g18_c_main_不同名子函式先成功後故障只注入根函式綁定() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(4),2)\n}\n";
        let outcome = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned } =>
                Some((name.as_str(), *returned)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, [("inc", true), ("sum", false)]);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_動態實參的來源框架不占用唯一使用者函式預算() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_巢狀實參先返回後可合法復用唯一框架額度() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(1),3)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inc", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [1, 2, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "inc")).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count(), 1);
    }


    #[test]
    fn g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(1+2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer"]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "outer".into(), returned: false,
        }));
    }


    #[test]
    fn g18_a_main_普通函式返回值與常值相加由同一真動態工作堆疊完成() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(5)+2\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_兩個兄弟函式合成返回必照原_checker_左右順序執行() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let args = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 4]);
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [3, 5]);
    }

    #[test]
    fn g18_a_main_多層括號包含函式複合返回保持原_parser_int_來源() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn ((inc(3)+inc(4)))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
    }

    #[test]
    fn g18_c_main_子函式已真返回後最外層算術溢位不得冒充成功() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn id(high)+1\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(9223372036854775807)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "id"
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_複合返回工作計畫被竄改必在任何函式執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+3\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_複合函式返回與非法型別或未知名稱仍由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+true\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+missing\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_c_main_多兄弟複合返回不得假造單一根函式故障注入() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(8));
        assert!(matches!(prepared.execute_with_fault(true),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    #[test]
    fn g18_a_函式參數與多個純值區域初始化依_checker_順序真返回() {
        let src = "fn calc(n:int)->int{\nconst one:int = 1\nvar next:int = n+one\nreturn next+2\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 2);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_巢狀函式各自區域宣告不與父框架參數混用() {
        let src = "fn outer(n:int)->int{\nconst result:int = leaf(n+1)\nreturn result+1\n}\nfn leaf(x:int)->int{\nconst y:int = x+2\nreturn y\n}\nfn main()->int{\nreturn outer(3)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 1);
        assert_eq!(prepared.nested_callees.iter().find(|f| f.name == "leaf").unwrap().local_inits.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let names = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(names, ["outer", "leaf"]);
    }

    #[test]
    fn g18_a_真動態_main_實參進入帶區域宣告函式仍唯一返回() {
        let src = "fn calc(n:int)->int{\nconst extra:int = n+2\nreturn extra\n}\nfn main()->int{\nreturn calc(2+3)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_c_函式區域算術溢位必退出而不得虛構返回() {
        let src = "fn fail(n:int)->int{\nconst too_big:int = n+1\nreturn too_big\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "fail".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_局部整數宣告跨多個遞迴框架不得共用儲存位置() {
        let src = "fn repeat(n:int)->int{\nconst next:int = n+1\nreturn repeat(next)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {returned: false, ..})).count();
        assert_eq!((entered, exited), (4, 4));
    }

    #[test]
    fn g18_a_函式區域宣告工作計畫遭偽造必在執行前拒絕() {
        let src = "fn calc(n:int)->int{\nconst next:int = n+1\nreturn next\n}\nfn main()->int{\nreturn calc(2)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.root_local_inits[0].value = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_函式區域初始化錯誤型別與未知參照由原_checker_拒絕() {
        for src in [
            "fn calc(n:int)->int{\nconst x:int = true\nreturn n\n}\nfn main()->int{\nreturn calc(2)\n}\n",
            "fn calc(n:int)->int{\nconst x:int = missing\nreturn x\n}\nfn main()->int{\nreturn calc(2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_兩筆真區域初始化與逆序離域事件保持同源宣告身分() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nvar second:int = first+2\nreturn second\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let prepared = checked(src);
        let ids = prepared.root_local_inits.iter()
            .map(|init| init.declaration).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        let created = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        let ended = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(created, [(ids[0], 4), (ids[1], 6)]);
        assert_eq!(ended, [ids[1], ids[0]]);
        assert_eq!(out.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_第二局部初始化運算溢位只釋放已完成宣告() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nconst second:int = n+9223372036854775807\nreturn second\n}\nfn main()->int{\nreturn calc(1)\n}\n";
        let prepared = checked(src);
        let first_id = prepared.root_local_inits[0].declaration;
        let second_id = prepared.root_local_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first_id, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first_id)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second_id
        )));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "calc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域() {
        let src = "fn again(n:int)->int{\nconst local:int = n+1\nreturn again(local)\n}\nfn main()->int{\nreturn again(1)\n}\n";
        let prepared = checked(src);
        let id = prepared.root_local_inits[0].declaration;
        let out = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(actual, value) if *actual == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [2, 3, 4]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(actual) if *actual == id)
        ).count(), 3);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }


    #[test]
    fn g18_a_main_動態整數初始化應真執行後再傳入普通函式() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        assert!(prepared.main_owners.is_empty());
        let id = prepared.main_dynamic_inits[0].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_函式結果可先存入區域再用純值算術返回() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst result:int = inc(4)\nreturn result+1\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        let out = prepared.execute().unwrap();
        // inc(4) = 5，main 再加 1，真正結果為 6。
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let local = prepared.main_dynamic_inits[0].declaration;
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(local, 5)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))).count(), 1);
    }

    #[test]
    fn g18_a_main_前向零參數結果可作為初始化再跨局部複製() {
        let src = "fn main()->int{\nconst first:int = fixed()\nvar copy:int = first\nreturn inc(copy)\n}\nfn inc(n:int)->int{\nreturn n+1\n}\nfn fixed()->int{\nreturn 8\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(9));
        let inits = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(inits.len(), 2);
        assert_eq!(inits.iter().map(|(_, value)| *value).collect::<Vec<_>>(), [8, 8]);
    }

    #[test]
    fn g18_c_main_動態初始化中後筆溢位不得跳過前筆真離域() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst one:int = inc(1)\nconst too_big:int = 9223372036854775807+one\nreturn inc(one)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let first = prepared.main_dynamic_inits[0].declaration;
        let second = prepared.main_dynamic_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second
        )));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_真動態初始化計畫被篡改必在執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_inits[0].value = PrivateScalarReturnExpr::Constant(500);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_非法動態初始化型別及未宣告來源仍交由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = true+3\nreturn inc(base)\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = missing+3\nreturn inc(base)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_r01_r02_函式內多筆可寫純值賦值必依原_checker_語句次序() {
        let source = "fn calculate(n:int)->int{\nvar current:int = n+1\ncurrent = current+2\ncurrent = current+3\nreturn current\n}\nfn main()->int{\nreturn calculate(5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.root_local_inits.len(), 1);
        assert_eq!(checked.root_local_steps.len(), 3);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert_eq!(result.parameter_slots_after_exit, 0);
        let changes = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(key, value) if *key == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [8, 11]);
        assert!(result.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 6)));
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(key) if *key == id)).count(), 1);
    }

    #[test]
    fn g18_r01_r02_賦值右值真呼叫先完成後才寫入持有者() {
        let source = "fn bump(n:int)->int{\nreturn n+1\n}\nfn work(n:int)->int{\nvar value:int = n\nvalue = bump(value)\nreturn value+2\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(6));
        let returned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).unwrap();
        let assigned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(key, 4) if *key == id)).unwrap();
        assert!(returned < assigned);
    }

    #[test]
    fn g18_r01_r02_main_具名_copyable_int_多次更新後才進入普通函式() {
        let source = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar value:int = 2\nvalue = value+3\nreturn identity(value)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let id = checked.main_dynamic_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalAssigned(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_真巢狀函式可寫宣告不能跨動態框架共用() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\nreturn leaf(current)\n}\nfn leaf(x:int)->int{\nvar tmp:int = x+1\ntmp = tmp+1\nreturn tmp\n}\nfn main()->int{\nreturn work(1)\n}\n";
        let checked = checked(source);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        let writes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(id, v) => Some((*id, *v)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert_ne!(writes[0].0, writes[1].0);
        assert_eq!((writes[0].1, writes[1].1), (2, 4));
    }

    #[test]
    fn g18_r01_r02_賦值右值溢位不得補記後續賦值或假返回() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\ncurrent = current+9223372036854775807\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(actual, 4) if *actual == id)).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(..))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_const_重賦值與錯型別必由真_checker_先拒絕() {
        for source in [
            "fn work(n:int)->int{\nconst current:int = n\ncurrent = current+1\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nvar current:int = n\ncurrent = true\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nmissing = n+1\nreturn n\n}\nfn main()->int{\nreturn work(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r01_r02_已編譯賦值的_checker_身分遭竄改在執行前拒絕() {
        let source = "fn work(n:int)->int{\nvar value:int = n\nvalue = value+2\nreturn value\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::Assign { value, .. } = &mut checked.root_local_steps[1] {
            *value = PrivateScalarReturnExpr::Constant(99);
        } else {
            panic!("真 Checker 有第二筆可寫賦值");
        }
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_具名_bool_true_提前_return_不可再執行未達返回路徑() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        assert_eq!(plan.root_local_steps.len(), 2);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
    }

    #[test]
    fn g18_r03_具名_bool_false_不走提前_return_而正常執行後段() {
        let source = "fn choose(n:int)->int{\nconst flag:bool = false\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))));
    }

    #[test]
    fn g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = false\nflag = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_, true))));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
    }

    #[test]
    fn g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回() {
        let source = "fn choose(n:int)->int{\nvar first:bool = false\nvar second:bool = true\nif(first){\nreturn n+1\n}else if(second){\nreturn n+2\n}else{\nreturn n+3\n}\nreturn n+4\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        let checks = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6 | 7))));
    }

    #[test]
    fn g18_r03_巢狀子函式提前返回只退出該函式不跳過父框架返回() {
        let source = "fn leaf(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn outer(n:int)->int{\nconst result:int = leaf(n)\nreturn result+3\n}\nfn main()->int{\nreturn outer(4)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5 | 8))).count(), 2);
    }

    #[test]
    fn g18_r03_提前返回分支溢位只退出真的已初始化作用域() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nvar temp:int = n+1\nif(flag){\nreturn temp+9223372036854775807\n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolScopeEnded(_))));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(_))));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r03_未宣告_bool_及_const_bool_重綁由_checker_靜態拒絕() {
        for source in [
            "fn choose(n:int)->int{\nconst flag:bool = false\nflag = true\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(missing){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(n){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r03_篡改已編譯分支條件前須拒絕任何真框架() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        if let PrivateScalarLocalStep::EarlyReturn(branch) = &mut plan.root_local_steps[1] {
            branch.condition = PrivateScalarBoolExpr::Constant(false);
        } else {
            panic!("原 Checker 提供合法 if Return 計畫");
        }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r04_已核准_bool_true_函式實參與_int_返回共用真框架() {
        let source = "fn choose(flag:bool)->int{\nif(flag){return 4}\nreturn 7\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
        assert!(out.events.contains(&PrivateScalarCallEvent::BoolParameterBound(
            checked.callee_parameter_ids[0], true,
        )));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))));
    }

    #[test]
    fn g18_r04_已核准_bool_false_int_混合參數保持宣告順序和型別() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn main()->int{\nreturn choose(false,8)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true, false]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let order = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(false) => Some("bool"),
            PrivateScalarCallEvent::ArgumentEvaluated(8) => Some("int"),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(order, ["bool", "int"]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(..))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_r04_巢狀普通函式真_bool_參數不被誤讀成父整數槽() {
        let source = "fn inner(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn outer(flag:bool,n:int)->int{\nreturn inner(flag,n+2)\n}\nfn main()->int{\nreturn outer(true,4)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 2);
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer", "inner"]);
    }

    #[test]
    fn g18_r04_main_變動具名_bool_正值先更新再真正當實參() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+3}\nreturn n+4\n}\nfn main()->int{\nvar switched:bool = false\nswitched = true\nreturn choose(switched,5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_,true))));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
    }

    #[test]
    fn g18_r04_bool_混合參數進入後故障只退出本次真框架() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let checked = checked(source);
        let out = checked.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(_,5))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r04_bool_參數不是_int_隱含轉型_原_checker_型別優先拒絕() {
        for source in [
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(1,2)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true,false)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r04_bool_參數綁定靜態型別被竄改前必拒絕() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(6));
        checked.callee_parameter_is_bool[0] = false;
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r05_混合布林整數與巢狀提前返回動態事件帳簿必平衡() {
        let src = "fn leaf(n:int)->int{\nvar flag:bool = true\nvar x:int = n+1\nif(flag){return x}\nreturn n\n}\nfn parent(n:int,flag:bool)->int{\nvar other:int = leaf(n)\nif(flag){return other+2}\nreturn other+3\n}\nfn main()->int{\nreturn parent(3,true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionExited {returned: true,..}
        )).count(), 2);
    }

    #[test]
    fn g18_r05_動態整數溢位與真失敗退出仍保留合法平衡事件() {
        let src = "fn fail(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r05_遞迴重入相同靜態宣告身分不應誤判為重複清理() {
        let src = "fn repeat(n:int)->int{\nvar x:int = n\nx = x+1\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let out = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_偽造未建立區域的離域與錯順序均遭事件帳簿拒絕() {
        let x = DeclarationId(99);
        let y = DeclarationId(100);
        for events in [
            vec![PrivateScalarCallEvent::LocalScopeEnded(x)],
            vec![PrivateScalarCallEvent::LocalInitialized(x, 1)],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(y, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
                PrivateScalarCallEvent::LocalScopeEnded(y),
            ],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(x, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
            ],
            vec![PrivateScalarCallEvent::LocalAssigned(x, 5)],
        ] {
            let outcome = PrivateScalarCallOutcome {
                result: Ok(1), events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            };
            assert!(!outcome.has_balanced_frame_trace());
            assert!(matches!(PreparedPrivateScalarCall::audit_outcome(outcome),
                Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        }
    }

    #[test]
    fn g18_r05_虛構函式返回和錯誤退出標記不得通過一致性稽核() {
        let cases = [
            vec![PrivateScalarCallEvent::Returned(1)],
            vec![PrivateScalarCallEvent::FunctionExited {
                name: "leaf".into(), returned: false,
            }],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::FunctionExited {
                    name: "other".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: true,
                },
            ],
        ];
        for events in cases {
            assert!(!PrivateScalarCallOutcome {
                result: Ok(1), events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }

    #[test]
    fn g18_r05_布林與整數反向離域跨型別錯序不可冒充真清理() {
        let x = DeclarationId(100);
        let b = DeclarationId(200);
        let valid = vec![
            PrivateScalarCallEvent::FunctionEntered("fn".into()),
            PrivateScalarCallEvent::LocalInitialized(x, 3),
            PrivateScalarCallEvent::BoolInitialized(b, true),
            PrivateScalarCallEvent::Returned(3),
            PrivateScalarCallEvent::BoolScopeEnded(b),
            PrivateScalarCallEvent::LocalScopeEnded(x),
            PrivateScalarCallEvent::FunctionExited {
                name: "fn".into(), returned: true,
            },
        ];
        assert!(PrivateScalarCallOutcome {
            result: Ok(3), events: valid.clone(), resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut invalid = valid.clone();
        invalid.swap(4, 5);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: invalid, resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut missing = valid;
        missing.remove(4);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: missing, resource_events: Vec::new(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_純值事件帳簿不得將錯誤參數綁定或布林寫入當作合法框架() {
        for events in [
            vec![PrivateScalarCallEvent::BoolParameterBound(DeclarationId(1), true)],
            vec![PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), false)],
            vec![
                PrivateScalarCallEvent::FunctionEntered("f".into()),
                PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), true),
                PrivateScalarCallEvent::FunctionExited {name:"f".into(),returned:false},
            ],
        ] {
            assert!(!PrivateScalarCallOutcome {
                result: Err(PrivateScalarCallRuntimeError::StateMismatch),
                events, resource_events: Vec::new(), parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }


    #[test]
    fn g18_r03_if_已核准_int_可寫分支後仍執行真正尾端返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(6))).count(), 1);
    }

    #[test]
    fn g18_r03_if_false_未選中分支不得寫入而仍由原值返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\n}else if(false){\nvalue = value+100\n}else{\nvalue = value+2\n}\nreturn id(value)\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|ev| match ev {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::FunctionEntered(name) if name == "id")).count(), 1);
    }

    #[test]
    fn g18_r03_if_變更_bool_使後續獨立_if_重新讀取真動態值() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nif(true){\nflag = true\n}\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::BoolAssigned(_, true))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 2);
    }

    #[test]
    fn g18_r03_if_分支呼叫溢位不允許後續真賦值或假返回() {
        let src = "fn bump(n:int)->int{\nreturn n+1\n}\nfn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = bump(value)\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..)
                | PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r03_if_分支不能更改_const_或偷換型別_原_checker_拒絕() {
        for src in [
            "fn choose(n:int)->int{\nconst value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = false}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r03_if_分支原_checker_賦值工作計畫篡改於執行前拒絕() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::EarlyReturn(if_plan) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::AssignInt { value, .. } = &mut if_plan.when_true {
                *value = PrivateScalarReturnExpr::Constant(99);
            } else { panic!("真 Parser int 更新分支遺失"); }
        } else { panic!("真 Parser if 分支遺失"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_同一真_if_分支兩次_var_int_更新須照原_parser_先後返回() {
        for (flag, expected, updates) in [("true", 9, vec![5, 8]), ("false", 4, vec![])] {
            let src = format!(
                "fn choose(flag:bool,n:int)->int{{\nvar value:int = n\nif(flag){{\nvalue = value+2\nvalue = value+3\n}}\nreturn value+1\n}}\nfn main()->int{{\nreturn choose({},3)\n}}\n",
                flag,
            );
            let out = checked(&src).execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert!(out.has_balanced_frame_trace());
            let values = out.events.iter().filter_map(|event| match event {
                PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(values, updates);
        }
    }

    #[test]
    fn g18_r03_else_if_選中分支多筆更新不可執行其他兄弟分支() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else if(true){\nvalue = value+1\nvalue = value+2\n}else{\nvalue = value+50\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [4, 6]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
    }

    #[test]
    fn g18_r03_分支先更新_int_bool_後下個_if_必讀動態最新值() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nvar flag:bool = false\nif(true){\nvalue = value+2\nflag = true\n}\nif(flag){\nreturn value+1\n}\nreturn value+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_分支內先變更再提前_return_不得執行不可達尾端() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\nreturn value+2\n}\nreturn value+10\n}\nfn main()->int{\nreturn choose(true,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(7)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(15)
        )));
    }

    #[test]
    fn g18_r03_同分支第二更新溢位只保存第一個真更新並失敗退出() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_分支內常值錯誤賦值與真工作計畫竄改均須零額外執行() {
        let invalid = "fn choose(n:int)->int{\nconst x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let source = "fn choose(n:int)->int{\nvar x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
        if let PrivateScalarLocalStep::EarlyReturn(conditional) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::Sequence(actions) = &mut conditional.when_true {
                actions[1] = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(999),
                );
            } else { panic!("原 Parser 多筆分支序列未收錄"); }
        } else { panic!("原 Parser if 節點未收錄"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_真巢狀_if_葉提前返回保留雙層條件順序() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = true\nif(true){\nif(flag){\nreturn n+1\n}else{\nreturn n+2\n}\n}\nreturn n+3\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(6)
        )));
    }

    #[test]
    fn g18_r03_外層先更新_bool_後內層_if_須重讀最新值再寫入_int() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nvar value:int = n\nif(true){\nflag = true\nif(flag){\nvalue = value+2\n}else{\nvalue = value+99\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_外層_false_路徑內的_else_巢狀_if_只執行選中的葉() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else{\nif(false){\nvalue = value+20\n}else{\nvalue = value+3\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let changes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [7]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
    }

    #[test]
    fn g18_r03_巢狀提前_return_只退出內函式不截斷父函式算術() {
        let src = "fn leaf(n:int)->int{\nif(true){\nif(false){\nreturn n+10\n}else{\nreturn n+1\n}\n}\nreturn n+3\n}\nfn outer(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [3, 5]);
    }

    #[test]
    fn g18_r03_巢狀分支第二次_int_更新溢位真框架不補寫後續結果() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_巢狀條件非法名稱與工作計畫遭竄改必先拒絕() {
        let invalid = "fn choose(n:int)->int{\nif(true){\nif(missing){\nreturn n\n}\n}\nreturn n\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn choose(n:int)->int{\nif(true){\nif(true){\nreturn n+1\n}\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let mut prepared = checked(valid);
        assert_eq!(prepared.execute().unwrap().result, Ok(2));
        if let PrivateScalarLocalStep::EarlyReturn(plan) = &mut prepared.root_local_steps[0] {
            if let PrivateScalarBranchAction::NestedIf(inner) = &mut plan.when_true {
                inner.when_true = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(50),
                );
            } else { panic!("真巢狀 If 原來源丟失"); }
        } else { panic!("原 If 工作計畫丟失"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_普通函式_while_false_零迭代仍真正檢查且返回() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(false){\nvalue = value+1\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_逐輪重讀_bool_並僅更新到達的_int() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nvar flag:bool = true\nwhile(flag){\nvalue = value+2\nflag = false\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
    }

    #[test]
    fn g18_r03_普通函式_while_內_break_不執行後續寫入() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(true){\nvalue = value+1\nbreak\nvalue = value+20\n}\nreturn value\n}\nfn main()->int{\nreturn count(1)\n}\n";
        // 只接受原 Checker 核准的可達性，不自行放寬 break 後語句。
        match PreparedPrivateScalarCall::prepare(src) {
            Ok(plan) => {
                let out = plan.execute().unwrap();
                assert_eq!(out.result, Ok(2));
                assert!(out.has_balanced_frame_trace());
            }
            Err(PrivateScalarCallPrepareError::Check(_)) => {}
            Err(other) => panic!("原 Checker 接受但 while 執行器拒絕：{other:?}"),
        }
    }

    #[test]
    fn g18_r03_普通函式_while_步數預算耗盡不產生假返回() {
        let src = "fn count(n:int)->int{\nwhile(true){\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 30,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_真控制計畫篡改須執行前拒絕() {
        let src = "fn count(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nflag = false\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(1));
        if let PrivateScalarLocalStep::Control(PrivateScalarBranchAction::While {
            condition, ..
        }) = &mut prepared.root_local_steps[1] {
            *condition = PrivateScalarBoolExpr::Constant(false);
        } else { panic!("原 while 來源控制計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_普通函式_for_迭代器真值逐輪參與累加() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 2..5{\nvalue = value+i\n}\nreturn value\n}\nfn main()->int{\nreturn sum(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::LocalInitialized(_, v) if (2..5).contains(v) =>
                Some(*v), _ => None,
        }).collect::<Vec<_>>(), [3, 2, 3, 4]);
    }

    #[test]
    fn g18_r01_普通函式_for_空半開區間不虛構本體或迭代器() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 2..2{\nvalue = value+50\n}\nreturn value\n}\nfn main()->int{\nreturn sum(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_普通函式_for_break_只結束最近一輪並先離域() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 3..9{\nvalue = value+i\nbreak\nvalue = value+100\n}\nreturn value\n}\nfn main()->int{\nreturn sum(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(_))).count(), 2);
    }

    #[test]
    fn g18_r03_普通函式_for_continue_每輪清除同一宣告身份() {
        let src = "fn sum(n:int)->int{\nfor i in 0..3{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalInitialized(..))).count(), 3);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(..))).count(), 3);
    }

    #[test]
    fn g18_r02_普通函式_巢狀_for_同名迭代器用不同宣告身份() {
        let src = "fn sum(n:int)->int{\nvar value:int = n\nfor i in 1..2{\nfor i in 8..9{\nvalue = value+i\n}\n}\nreturn value\n}\nfn main()->int{\nreturn sum(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        // 外層 var value 的初值恰好也是 1；只按語意事件順序
        // 排除首個本地宣告，再核對兩個真正 for 迭代的來源身份。
        let locals = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::LocalInitialized(id, value) = event {
                Some((*id, *value))
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(locals.len(), 3);
        let iter = &locals[1..];
        assert_eq!(iter.len(), 2);
        assert_ne!(iter[0].0, iter[1].0);
        assert_eq!([iter[0].1, iter[1].1], [1, 8]);
    }

    #[test]
    fn g18_r03_普通函式_巨大合法_for_受真步數預算控制() {
        let src = "fn sum(n:int)->int{\nfor i in 0..9223372036854775807{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 32,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r03_普通函式_for_迭代身份計畫偽造必先拒絕() {
        let src = "fn sum(n:int)->int{\nfor i in 2..4{\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn sum(4)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(4));
        if let PrivateScalarLocalStep::Control(PrivateScalarBranchAction::ForRange {
            iterator, ..
        }) = &mut prepared.root_local_steps[0] {
            *iterator = DeclarationId(0);
        } else { panic!("原 for 迭代身份工作計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_分支局部_int_由真_checker_初始化並在離域前讀取() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst bonus:int = 4\nvalue = value+bonus\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let locals = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, _) => Some(*id), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(locals.len(), 2);
        let ended = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(ended, [locals[1], locals[0]]);
    }

    #[test]
    fn g18_r02_分支同名遮蔽只寫入區塊內宣告_外部值不變() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvar value:int = 8\nvalue = value+1\n}\nreturn value\n}\nfn main()->int{\nreturn choose(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        let initializers = out.events.iter().filter_map(|event| {
            if let PrivateScalarCallEvent::LocalInitialized(id, value) = event {
                Some((*id, *value))
            } else { None }
        }).collect::<Vec<_>>();
        assert_eq!(initializers.len(), 2);
        assert_ne!(initializers[0].0, initializers[1].0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalAssigned(
            initializers[1].0, 9,
        )));
    }

    #[test]
    fn g18_r02_巢狀條件使用分支局部_bool_真值且離域() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nconst allowed:bool = true\nif(allowed){\nvalue = value+3\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolInitialized(..)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolScopeEnded(..)
        )).count(), 1);
    }

    #[test]
    fn g18_r02_while_逐輪純值局部宣告配合_continue_逆序清除() {
        let src = "fn count(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nconst step:int = 2\nflag = false\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalInitialized(_, 2)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalScopeEnded(..)
        )).count(), 1);
    }

    #[test]
    fn g18_r02_for_每輪局部值及_iterator_各自離域後才下一輪() {
        let src = "fn count(n:int)->int{\nfor i in 0..3{\nconst local:int = i\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let mut active = Vec::new();
        for event in &out.events {
            match event {
                PrivateScalarCallEvent::LocalInitialized(id, _) => active.push(*id),
                PrivateScalarCallEvent::LocalScopeEnded(id) => assert_eq!(active.pop(), Some(*id)),
                _ => {}
            }
        }
        assert!(active.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalInitialized(..)
        )).count(), 6);
    }

    #[test]
    fn g18_r02_已離域局部名稱禁止逃逸且作用域計畫篡改拒絕() {
        let bad = "fn choose(n:int)->int{\nif(true){\nconst hidden:int = 5\n}\nreturn hidden\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "fn choose(n:int)->int{\nif(true){\nconst hidden:int = 5\n}\nreturn n\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        if let PrivateScalarLocalStep::EarlyReturn(branch) = &mut plan.root_local_steps[0] {
            if let PrivateScalarBranchAction::Scoped(actions) = &mut branch.when_true {
                actions[0] = PrivateScalarBranchAction::DeclareInt {
                    declaration: DeclarationId(0),
                    value: PrivateScalarReturnExpr::Constant(5),
                };
            } else { panic!("原 Checker 單局部區塊未建立離域計畫"); }
        } else { panic!("原 if 來源缺失"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_同一函式框架真_ownedstr_局部與_copyable_int_返回() {
        let src = "fn inc(n:int)->int{\nvar owned:str = \"內🪼\"\nreturn n+1\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_真_if_區塊_utf8_只在選中分支建立且退域釋放() {
        let src = "fn inc(n:int)->int{\nvar result:int = n\nif(true){\nvar owned:str = \"分支\"\nresult = result+2\n}\nif(false){\nvar never:str = \"不執行\"\n}\nreturn result\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_真_for_每輪_utf8_各有唯一_resourceid_並先釋放() {
        let src = "fn inc(n:int)->int{\nfor i in 0..3{\nvar owned:str = \"迭代🪼\"\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Released(1),
            E::Created(2), E::Released(2),
            E::Created(3), E::Released(3),
        ]);
    }

    #[test]
    fn g18_r04_真_while_continue_離域_utf8_不重用舊資源() {
        let src = "fn inc(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nvar owned:str = \"本輪\"\nflag = false\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_父子函式同時擁有真_utf8_返回後依堆疊逆序釋放() {
        let src = "fn child(n:int)->int{\nvar child_owner:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar parent_owner:str = \"父\"\nreturn child(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Created(2),
            E::Released(2), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_函式迴圈資源在步數預算耗盡仍全數真釋放() {
        let src = "fn inc(n:int)->int{\nwhile(true){\nvar owned:str = \"反覆\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn inc(4)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 6, max_steps: 30,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.resource_events.is_empty());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &out.resource_events,
        ));
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r04_真_ownedstr_帳簿偽造及未核准隱含複製皆先拒絕() {
        let src = "fn inc(n:int)->int{\nvar owned:str = \"原文\"\nreturn n\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        out.resource_events.pop();
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        let invalid = "fn inc(n:int)->int{\nvar a:str = \"原文\"\nvar b:str = a\nreturn n\n}\nfn main()->int{\nreturn inc(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_r04_真同函式_ownedstr_明確_move_單一_resourceid_不複製() {
        let src = "fn transfer(n:int)->int{\nvar original:str = \"🪼\"\nvar target:str = original.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_同函式_if_內_move_外層所有者成空_內層只釋放一次() {
        let src = "fn transfer(n:int)->int{\nvar original:str = \"外\"\nif(true){\nvar target:str = original.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_同函式_for_逐輪真_move_每次是新_resourceid() {
        let src = "fn transfer(n:int)->int{\nfor i in 0..2{\nvar source:str = \"迭代\"\nvar target:str = source.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
            E::Created(2), E::Moved(2), E::Released(2),
        ]);
    }

    #[test]
    fn g18_r04_父函式真_move_與子函式新_ownedstr_依動態順序退出() {
        let src = "fn child(n:int)->int{\nvar local:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar outer:str = \"父\"\nvar moved:str = outer.move()\nreturn child(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Created(2),
            E::Released(2), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_同函式_if_move_提前返回後不得遺失原_ResourceId() {
        let src = "fn transfer(n:int)->int{\nif(true){\nvar source:str = \"內\"\nvar target:str = source.move()\nreturn n+1\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Moved(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_不選中分支的_move_不能偽造已轉移事件() {
        let src = "fn transfer(n:int)->int{\nvar source:str = \"原本\"\nif(false){\nvar unused:str = source.move()\n}\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [E::Created(1), E::Released(1)]);
    }

    #[test]
    fn g18_r04_已移動來源再次_move_原_checker_拒絕且偽造來源身分先拒絕() {
        let invalid = "fn transfer(n:int)->int{\nvar source:str = \"內\"\nvar first:str = source.move()\nvar second:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn transfer(n:int)->int{\nvar source:str = \"內\"\nvar target:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let mut prepared = checked(valid);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        if let PrivateScalarLocalStep::DeclareStr {
            initializer: PrivateScalarStrInitializer::Move { declaration, .. }, ..
        } = &mut prepared.root_local_steps[1] {
            *declaration = DeclarationId(0);
        } else { panic!("原 .move() 已核准來源工作計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_原_checker_const_str_move_同函式保留目標不可寫性() {
        let src = "fn transfer(n:int)->int{\nvar source:str = \"🪼\"\nconst target:str = source.move()\nreturn n\n}\nfn main()->int{\nreturn transfer(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
        ]);
    }

    /// 不汙染 F00／P16.1 的 frozen owned_str_runtime::tests 清單；
    /// R04 新可寫性驗證收歸 G18 專用 Gate。
    #[test]
    fn g18_r04_const_move_來源所有權轉交後目標不可寫且_resourceid_只釋放一次() {
        use crate::owned_str_runtime::{OwnedStrRuntime, StrRuntimeError, ResourceEvent};
        let mut resources = OwnedStrRuntime::new();
        resources.declare("source", true, "🪼").unwrap();
        resources.move_named_as("source", "target", false).unwrap();
        assert_eq!(resources.read("source"), Err(StrRuntimeError::MovedSource));
        assert_eq!(resources.read("target"), Ok("🪼"));
        assert_eq!(resources.assign("target", "改寫"),
            Err(StrRuntimeError::ImmutableBinding));
        assert_eq!(resources.finish().unwrap(), [
            ResourceEvent::Created(1),
            ResourceEvent::Moved(1),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_父子真_utf8_資源事件必嵌入函式進退同序序列() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar inner:str = \"子\"\nreturn n+1\n}\nfn parent(n:int)->int{\nvar outer:str = \"父\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let paired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::OwnedResource(value) => Some(value.clone()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(paired, out.resource_events);
        let child_exit = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::FunctionExited { name, returned: true }
                if name == "leaf"
        )).unwrap();
        let child_release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(child_release < child_exit);
    }

    #[test]
    fn g18_r05_原_checker_真_move_資源編號不跨父子函式錯置() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn child(n:int)->int{\nvar child_owned:str = \"第二\"\nreturn n\n}\nfn parent(n:int)->int{\nvar source:str = \"第一\"\nconst target:str = source.move()\nreturn child(n)\n}\nfn main()->int{\nreturn parent(5)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Created(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r05_for_逐輪真_utf8_釋放不得延後至下一輪或函式退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nfor i in 0..3{\nvar local:str = \"輪\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(6)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Released(2),
            R::Created(3), R::Released(3),
        ]);
        let trace = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::OwnedResource(event) => Some(event.clone()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(trace, out.resource_events);
    }

    #[test]
    fn g18_r05_步數耗盡與真資源清理必先於失敗函式退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn looped(n:int)->int{\nvar persistent:str = \"持有\"\nwhile(true){\nvar local:str = \"迭代\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn looped(1)\n}\n";
        let out = checked(src).execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 28,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        let exit = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::FunctionExited { name, returned: false }
                if name == "looped"
        )).unwrap();
        let last_release = out.events.iter().rposition(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).unwrap();
        assert!(last_release < exit);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r05_偽造另一份平衡_resourceid_不得繞過真事件映射() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nvar owned:str = \"原\"\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        out.resource_events.extend([R::Created(88), R::Released(88)]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_偽造提前函式退出再釋放真_resourceid_必被拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn count(n:int)->int{\nvar owned:str = \"原\"\nreturn n\n}\nfn main()->int{\nreturn count(4)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { name, .. } if name == "count"
        )).unwrap();
        let release = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        let resource = out.events.remove(release);
        let exit_after_removal = if release < exit { exit - 1 } else { exit };
        out.events.insert(exit_after_removal + 1, resource);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r04_具名_const_str_與_int_真同框架借用票據先還才釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"來源🪼\"\nreturn inspect(owner,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let acquired = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanAcquired(_)
        )).unwrap();
        let released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let owner_released = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(acquired < released && released < owner_released);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )).count(), 1);
    }

    #[test]
    fn g18_r04_具名_var_str_獨占借用與_copyable_int_真返回() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn inspect(owner,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::BorrowParameterBound(_, BorrowFact {
                kind: BorrowKind::ExclusiveWrite, ..
            })
        )));
    }

    #[test]
    fn g18_r04_父函式局部_ownedstr_真正借用給子函式並續算() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn parent(n:int)->int{\nconst owner:str = \"父\"\nreturn leaf(owner,n)+2\n}\nfn main()->int{\nreturn parent(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r05_借用首實參後第二實參動態溢位須還真票據再清理_utf8() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn explode(n:int)->int{\nreturn n+9223372036854775807\n}\nfn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,explode(1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let loan_returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let resource_released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).unwrap();
        assert!(loan_returned < resource_released);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::BorrowParameterBound(_, _)
        )));
    }

    #[test]
    fn g18_r05_真_str_借用參數綁定後故障依原快照全數歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner,3)\n}\n";
        let out = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r04_真_checker_借用來源篡改或隱含複製仍拒絕() {
        let illegal = "fn inspect(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源\"\nvar copied:str = owner\nreturn inspect(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(illegal),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "fn inspect(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源\"\nreturn inspect(owner)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
        if let Some(PrivateScalarReturnExpr::Call { arguments, .. }) =
            &mut prepared.main_dynamic_call {
            if let PrivateScalarCallArgument::BorrowStr { fact, .. } = &mut arguments[0] {
                fact.source = DeclarationId(0);
            } else { panic!("原 BorrowFact 工作計畫未保存"); }
        } else { panic!("main 原 Checker 真呼叫工作計畫缺失"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r04_真函式_str_字面值覆寫只釋放被取代資源一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"舊\"\nowner = \"新🪼\"\nreturn n\n}\nfn main()->int{\nreturn run(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).count(), 1);
    }

    #[test]
    fn g18_r04_真函式_move_覆寫舊目標與來源重新初始化不複製() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar source:str = \"甲\"\nvar target:str = \"乙\"\ntarget = source.move()\nsource = \"重建\"\nreturn n\n}\nfn main()->int{\nreturn run(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Released(2),
            R::Created(3), R::Released(3), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_選中_if_字串覆寫原子提交_未選分支零資源事件() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"初始\"\nif(true){\nowner = \"選中\"\n}else{\nowner = \"不得建立\"\n}\nreturn n\n}\nfn main()->int{\nreturn run(6)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r02_普通函式_for_逐輪_str_覆寫即時釋放上一個_owner() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"原\"\nfor i in 0..3{\nowner = \"每輪\"\n}\nreturn n\n}\nfn main()->int{\nreturn run(7)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1),
            R::Created(3), R::Released(2), R::Created(4),
            R::Released(3), R::Released(4),
        ]);
    }

    #[test]
    fn g18_r03_函式提前返回先完成_str_覆寫再真離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn run(n:int)->int{\nvar owner:str = \"甲\"\nif(true){\nowner = \"乙\"\nreturn n+1\n}\nreturn 0\n}\nfn main()->int{\nreturn run(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
    }

    #[test]
    fn g18_r04_不可寫_str_隱含複製與已移動再次使用均由_checker_拒絕() {
        let invalid = [
            "fn run(n:int)->int{\nconst owner:str = \"原\"\nowner = \"新\"\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
            "fn run(n:int)->int{\nvar owner:str = \"原\"\nowner = 1\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
            "fn run(n:int)->int{\nvar source:str = \"原\"\nvar target:str = \"舊\"\ntarget = source.move()\ntarget = source.move()\nreturn n\n}\nfn main()->int{\nreturn run(1)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_r04_原_checker_str_move_真跨函式_唯一_utf8_交接() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar owned:str = \"所有權🪼\"\nreturn receive(owned.move(),4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|ev| matches!(ev,
            PrivateScalarCallEvent::ResourceTransferred {
                resource_id: 1, ..
            }
        )).count(), 1);
        assert!(!out.events.iter().any(|ev| matches!(ev,
            PrivateScalarCallEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn g18_r04_父函式局部_整筆_move_交給子函式同一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+1\n}\nfn parent(n:int)->int{\nconst local:str = \"父框架\"\nreturn leaf(local.move(),n)\n}\nfn main()->int{\nreturn parent(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_r04_同次真_call_獨立_move_及_const_str_借用各守原身分() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(moved:str,borrowed:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar first:str = \"移動\"\nconst second:str = \"借用\"\nreturn inspect(first.move(),second,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1),
            R::Released(1), R::Released(2),
        ]);
        let ticket = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let borrowed_release = out.events.iter().position(|event| matches!(event,
            PrivateScalarCallEvent::OwnedResource(R::Released(2))
        )).unwrap();
        assert!(ticket < borrowed_release);
    }

    #[test]
    fn g18_r05_跨框架_move_已完成綁定後故障由子框架釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar owned:str = \"中途失敗\"\nreturn receive(owned.move(),4)\n}\n";
        let out = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r05_第二實參溢位時_move_仍由原呼叫者唯一釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"求值失敗\"\nreturn receive(owned.move(),1+9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::FunctionEntered(name) if name == "receive"
        )));
    }

    #[test]
    fn g18_r04_跨函式_move_真來源被_cherker_禁止重複及隱含複製() {
        let invalid = [
            "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned,owned.move())\n}\n",
            "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nvar duplicate:str = owned\nreturn receive(owned.move(),1)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r05_真跨函式移動事件篡改來源或資源編號均拒絕() {
        let src = "fn receive(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar owned:str = \"來源\"\nreturn receive(owned.move(),3)\n}\n";
        let baseline = checked(src).execute().unwrap();
        assert!(baseline.has_balanced_frame_trace());
        let mut wrong_source = checked(src).execute().unwrap();
        let event = wrong_source.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { source, .. } = event {
            *source = DeclarationId(0);
        }
        assert!(!wrong_source.has_balanced_frame_trace());
        let mut wrong_id = checked(src).execute().unwrap();
        let event = wrong_id.events.iter_mut().find(|event| matches!(event,
            PrivateScalarCallEvent::ResourceTransferred { .. }
        )).unwrap();
        if let PrivateScalarCallEvent::ResourceTransferred { resource_id, .. } = event {
            *resource_id = 999;
        }
        assert!(!wrong_id.has_balanced_frame_trace());
    }


    #[test]
    fn g18_r02_函式var_int無初值在原checker准許後第一次賦值才建立值槽() {
        let src = "fn calc(n:int)->int{\nvar local:int\nlocal = n+4\nreturn local\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let declared = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))).unwrap();
        let initialized = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalInitialized(_, 7))).unwrap();
        let ended = out.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalScopeEnded(_))).unwrap();
        assert!(declared < initialized && initialized < ended);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r02_宣告未初始化int離域不虛構初始化值或回收資源() {
        let src = "fn calc(n:int)->int{\nvar spare:int\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(..)
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r02_函式var_bool無初值先合法賦值後用於提前return() {
        let src = "fn calc(n:int)->int{\nvar ready:bool\nready = true\nif(ready){\nreturn n+2\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolDeclaredEmpty(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolInitialized(_, true)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::BoolScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r02_同作用域未初始化與已初始化局部按宣告逆序離域() {
        let src = "fn calc(n:int)->int{\nvar pending:int\nconst later:int = 6\npending = n+1\nreturn later\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        let prepared = checked(src);
        let prefix = &prepared.root_local_steps;
        let a = match &prefix[0] {
            PrivateScalarLocalStep::DeclareEmptyInt(id) => *id,
            _ => panic!("原 Checker 無初值宣告來源不存在"),
        };
        let b = match &prefix[1] {
            PrivateScalarLocalStep::Declare(init) => init.declaration,
            _ => panic!("原 Checker 常值宣告來源不存在"),
        };
        assert_eq!(values, [b, a]);
    }

    #[test]
    fn g18_r02_分支var_int尚未初始化後更新不污染外層遮蔽() {
        let src = "fn calc(n:int)->int{\nvar result:int = n\nif(true){\nvar result:int\nresult = 15\n}\nreturn result\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let declare_empty = out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count();
        assert_eq!(declare_empty, 1);
    }

    #[test]
    fn g18_r03_for_逐輪var_int先保留空槽再第一次賦值與continue() {
        let src = "fn calc(n:int)->int{\nvar total:int = 0\nfor i in 0..3{\nvar step:int\nstep = i+2\ntotal = total+step\ncontinue\n}\nreturn total+n\n}\nfn main()->int{\nreturn calc(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )).count(), 0);
    }

    #[test]
    fn g18_r03_while_continue_未初始化bool與int只執行選中路徑() {
        let src = "fn calc(n:int)->int{\nvar active:bool\nactive = true\nvar count:int = n\nwhile(active){\nvar next:int\nnext = count+4\ncount = next\nactive = false\ncontinue\n}\nreturn count\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r03_初始化右值溢位時空局部只離域不造假值() {
        let src = "fn calc(n:int)->int{\nvar pending:int\npending = n+9223372036854775807\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_)
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(..)
                | PrivateScalarCallEvent::Returned(..)
        )));
    }

    #[test]
    fn g18_r01_main_var_int無初值賦值後真傳入普通函式() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar base:int\nbase = 4\nreturn inc(base)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 4)
        )));
    }

    #[test]
    fn g18_r02_無初值int_bool未合法賦值即讀由原checker拒絕() {
        let invalid = [
            "fn calc(n:int)->int{\nvar pending:int\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n",
            "fn calc(n:int)->int{\nvar flag:bool\nif(flag){return n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r02_未初始化來源計畫篡改類型執行前零事件拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:int\npending = n+1\nreturn pending\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.root_local_steps[0] =
            PrivateScalarLocalStep::DeclareEmptyBool(DeclarationId(0));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r05_偽造未初始化局部冒充已初始化或刪除真離域須拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:int\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let mut altered = checked(src).execute().unwrap();
        altered.events.retain(|e| !matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        ));
        assert!(!altered.has_balanced_frame_trace());
        let mut altered = checked(src).execute().unwrap();
        let at = altered.events.iter().position(|e|
            matches!(e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))
        ).unwrap();
        let id = match altered.events[at] {
            PrivateScalarCallEvent::LocalDeclaredEmpty(id) => id,
            _ => unreachable!(),
        };
        altered.events.insert(at+1, PrivateScalarCallEvent::LocalInitialized(id, 99));
        assert!(!altered.has_balanced_frame_trace());
    }



    #[test]
    fn g18_r02_真str無初值單純宣告離域零ResourceId() {
        let src = "fn calc(n:int)->int{\nvar spare:str\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(..)
        )));
    }

    #[test]
    fn g18_r02_真str無初值後字面賦值只建立一次並退出釋放() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"海月\"\nreturn n+1\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_r02_延遲str兩次賦值第二次原子替換釋放舊資源() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"先\"\npending = \"後\"\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Created(2), E::Released(1), E::Released(2),
        ]);
    }

    #[test]
    fn g18_r02_無初值str以已核准move首次接收原ResourceId() {
        let src = "fn calc(n:int)->int{\nvar source:str = \"唯一\"\nvar pending:str\npending = source.move()\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Moved(1), E::Released(1),
        ]);
    }

    #[test]
    fn g18_r03_未選分支的str空宣告與賦值不建立UTF8資源() {
        let src = "fn calc(n:int)->int{\nif(false){\nvar pending:str\npending = \"不可達\"\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r03_for_逐輪無初值str先賦值每輪唯一ResourceId清理() {
        let src = "fn calc(n:int)->int{\nfor i in 0..3{\nvar pending:str\npending = \"每輪\"\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        use crate::owned_str_runtime::ResourceEvent as E;
        assert_eq!(out.resource_events, [
            E::Created(1), E::Released(1),
            E::Created(2), E::Released(2),
            E::Created(3), E::Released(3),
        ]);
    }

    #[test]
    fn g18_r03_空str與真str共框架溢位僅清理確實建立的資源() {
        let src = "fn calc(n:int)->int{\nvar spare:str\nvar owned:str = \"已建\"\nreturn n+9223372036854775807\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_非法未初始化str讀取借用必由原Checker提前拒絕() {
        let invalid = [
            "fn receive(s:str,n:int)->int{return n}\nfn calc(n:int)->int{\nvar spare:str\nreturn receive(spare,n)\n}\nfn main()->int{return calc(3)}\n",
            "fn calc(n:int)->int{\nvar first:str\nvar other:str = first\nreturn n\n}\nfn main()->int{return calc(3)}\n",
        ];
        for src in invalid {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r02_延遲str來源編譯身分篡改必在動態執行前拒絕() {
        let src = "fn calc(n:int)->int{\nvar pending:str\npending = \"唯一\"\nreturn n\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        plan.root_local_steps[0] = PrivateScalarLocalStep::DeclareEmptyStr {
            declaration: DeclarationId(0), name: "pending".to_string(), mutable: true,
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r03_延遲str真字面賦值後借用參數先還票再釋放資源() {
        let src = "fn receive(s:str,n:int)->int{\nreturn n+1\n}\nfn calc(n:int)->int{\nvar pending:str\npending = \"借用\"\nreturn receive(pending,n)\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let repaid = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        let released = out.events.iter().rposition(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(
                crate::owned_str_runtime::ResourceEvent::Released(_)
            )
        )).unwrap();
        assert!(repaid < released);
        assert_eq!(out.resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
    }



    #[test]
    fn g18_r01_main_var首次賦值的普通函式來源必真執行() {
        let src = "fn increment(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar out:int\nout = increment(4)\nreturn out\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let declared = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalDeclaredEmpty(_))).unwrap();
        let entered = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "increment"
        )).unwrap();
        let initialized = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 5))).unwrap();
        assert!(declared < entered && entered < initialized);
    }

    #[test]
    fn g18_r01_main先返回葉函式後更新局部不得逆序求值() {
        let src = "fn leaf()->int{\nreturn 7\n}\nfn main()->int{\nvar value:int\nvalue = leaf()+3\nreturn value\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::Returned(7))).unwrap();
        let initialized = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LocalInitialized(_, 10))).unwrap();
        assert!(returned < initialized);
    }

    #[test]
    fn g18_r02_main同一var先以函式初次賦值再由函式重賦值() {
        let src = "fn a(n:int)->int{\nreturn n+2\n}\nfn b(n:int)->int{\nreturn n+3\n}\nfn main()->int{\nvar result:int\nresult = a(1)\nresult = b(result)\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) =>
                Some(name.as_str()), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["a", "b"]);
    }

    #[test]
    fn g18_r03_main函式賦值右值溢位不能假造成功初始化或第二個呼叫() {
        let src = "fn first(n:int)->int{\nreturn n+1\n}\nfn later()->int{\nreturn 3\n}\nfn main()->int{\nvar result:int\nresult = first(9223372036854775807)\nresult = later()\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "later"
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::LocalScopeEndedEmpty(_)
        )));
    }

    #[test]
    fn g18_r01_main賦值來源計畫遭竄改必先拒絕不得偽造執行() {
        let src = "fn leaf()->int{\nreturn 2\n}\nfn main()->int{\nvar value:int\nvalue = leaf()\nreturn value\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(2));
        if let PrivateScalarLocalStep::Assign { value, .. }
            = &mut plan.main_dynamic_steps[1] {
            *value = PrivateScalarReturnExpr::Constant(900);
        } else { panic!("原 Checker 的 main 右值真呼叫必須保留"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_r01_main_if選中分支的真函式呼叫不需出現在尾端return() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = inc(3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc"
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_if_false未選函式零事件且else選中另一真函式() {
        let src = "fn unused()->int{\nreturn 99\n}\nfn chosen()->int{\nreturn 7\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = unused()\n}else{\nresult = chosen()\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let enters = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["chosen"]);
    }

    #[test]
    fn g18_r03_main_while逐輪重讀條件內的真函式來源() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar active:bool = true\nvar result:int = 0\nwhile(active){\nresult = inc(result)\nactive = false\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_)
        )).count(), 2);
    }

    #[test]
    fn g18_r01_main_for迭代值在真函式參數中按半開區間逐輪傳入() {
        let src = "fn add(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar result:int = 0\nfor i in 1..4{\nresult = add(result,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "add"
        )).count(), 3);
    }

    #[test]
    fn g18_r03_main巢狀分支函式溢位不得執行後續else或虛構返回() {
        let src = "fn overflow(n:int)->int{\nreturn n+1\n}\nfn skipped()->int{\nreturn 9\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = overflow(9223372036854775807)\n}else{\nresult = skipped()\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "skipped"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_main巢狀控制真函式源計畫篡改必在執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nresult = inc(4)\n}\nreturn result\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(5));
        plan.main_dynamic_steps.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_r01_r03_main選中if內真utf8局部借用給函式後才離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"分支🪼\"\nresult = inspect(owner,4)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let borrowed = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).unwrap();
        let returned = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).unwrap();
        let released = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::OwnedResource(R::Released(1)))).unwrap();
        assert!(borrowed < returned && returned < released);
    }

    #[test]
    fn g18_r02_r03_main_if_false來源不建資源_else同名局部只取真分支() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar owner:str = \"不執行\"\nresult = inspect(owner,9)\n}else{\nvar owner:str = \"唯一🪼\"\nresult = inspect(owner,4)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
    }

    #[test]
    fn g18_r02_r03_main_for外層utf8每輪重借同源須逐次還票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"外層🪼\"\nvar result:int = 0\nfor i in 0..3{\nresult = result+inspect(owner,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
    }

    #[test]
    fn g18_r03_main_while內真字串借用隨continue先還票再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar active:bool = true\nvar result:int = 0\nwhile(active){\nvar owner:str = \"每輪🪼\"\nresult = inspect(owner,3)\nactive = false\ncontinue\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(_))).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_if借用首實參後第二實參溢位須回滾真票據與資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"錯誤🪼\"\nresult = inspect(owner,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }



    #[test]
    fn g18_r02_main_if內真str局部move給第二局部仍僅釋放原resourceid一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"獨占🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_for每輪str先move再借用不可複製同一個owner() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nfor i in 0..3{\nvar source:str = \"每輪\"\nvar target:str\ntarget = source.move()\nresult = result+inspect(target,i)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
            R::Created(3), R::Moved(3), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
    }

    #[test]
    fn g18_r03_main_if先move後呼叫次實參溢位同一資源須恰好釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"錯誤\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r02_main分支中已move來源再次借用必由原checker先拒絕() {
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar source:str = \"一次\"\nvar target:str\ntarget = source.move()\nresult = inspect(source,3)\n}\nreturn result\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01～R03：將真 UTF-8 所有權轉移、呼叫借用與 CFG 退出放在同一
    // 原 Checker 驗證的 main 工作堆疊；不是分別執行模型後拼湊結果。
    #[test]
    fn g18_r03_main_while_break前真move借用只建立清理首輪資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nvar active:bool = true\nwhile(active){\nvar source:str = \"首輪🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,2)\nbreak\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_for_continue每輪真move與借用先還票再走回邊() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar total:int = 0\nfor i in 0..3{\nvar source:str = \"回邊🪼\"\nvar target:str\ntarget = source.move()\ntotal = total+inspect(target,i)\ncontinue\n}\nreturn total\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
            R::Created(2), R::Moved(2), R::Released(2),
            R::Created(3), R::Moved(3), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 3);
        for id in [1, 2, 3] {
            let released = out.events.iter().position(|e| matches!(
                e, PrivateScalarCallEvent::OwnedResource(R::Released(actual)) if *actual == id
            )).unwrap();
            assert!(out.events[..released].iter().any(|e| matches!(
                e, PrivateScalarCallEvent::LoanReturned(_)
            )));
        }
    }

    #[test]
    fn g18_r03_main_if提前return真move借用且跳過後續資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn fallback(n:int)->int{\nreturn n\n}\nfn main()->int{\nif(true){\nvar source:str = \"提前🪼\"\nvar target:str\ntarget = source.move()\nreturn inspect(target,4)\n}\nreturn fallback(0)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "fallback"
        )));
    }

    #[test]
    fn g18_r01_main選中分支真字串借用嵌入第二層函式實參() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn increment(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"多層🪼\"\nresult = increment(inspect(owner,3))\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inspect", "increment"]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r03_main_if_false未選多層函式不造資源真else只借用一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn increment(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar owner:str = \"未選🪼\"\nresult = increment(inspect(owner,99))\n}else{\nvar owner:str = \"選中🪼\"\nresult = increment(inspect(owner,3))\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }


    // R01：其他合法宣告不是 main 所需的動態工作，不能強制把
    // 它的返回型別誤當 int 編譯；實際可達的所有層仍完整核對。
    #[test]
    fn g18_r01_未使用的合法bool輔助函式不得阻斷真int主呼叫() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn forward(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn forward(6)\n}\n";
        let plan = checked(src);
        assert!(plan.nested_callees.is_empty());
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "ready"
        )));
    }

    #[test]
    fn g18_r01_主函式到二層真葉函式仍收錄且跳過不相干bool宣告() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn leaf(n:int)->int{\nreturn n+1\n}\nfn relay(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn relay(5)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["leaf"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["relay", "leaf"]);
    }

    #[test]
    fn g18_r01_巢狀實參呼叫圖需收錄內外兩個int目標而略過無關函式() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn inner(n:int)->int{\nreturn n+2\n}\nfn outer(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn outer(inner(3))\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["inner"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inner", "outer"]);
    }

    #[test]
    fn g18_r01_主函式兄弟分支只執行選中函式而收錄兩者並略過無關bool() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn left(n:int)->int{\nreturn n+1\n}\nfn right(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = left(10)\n}else{\nresult = right(3)\n}\nreturn result\n}\n";
        let plan = checked(src);
        assert_eq!(plan.nested_callees.iter().map(|f| f.name.as_str())
            .collect::<Vec<_>>(), ["right"]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name)
                if name == "left" || name == "ready"
        )));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )).count(), 1);
    }

    #[test]
    fn g18_r01_可達圖身分遭竄改及bool誤用int回傳都須拒絕() {
        let src = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn leaf(n:int)->int{\nreturn n+1\n}\nfn relay(n:int)->int{\nreturn leaf(n)\n}\nfn main()->int{\nreturn relay(5)\n}\n";
        let mut plan = checked(src);
        plan.nested_callees.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let wrong = "fn ready(flag:bool)->bool{\nreturn flag\n}\nfn main()->int{\nreturn ready(true)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(wrong),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01：source 內可存在不會建構或執行的合法結構宣告。
    // 此切片只執行原 Checker 已准的真普通函式圖。
    #[test]
    fn g18_r01_無關int結構宣告不應阻斷main純函式執行() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(8)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.verified.structs.len(), 1);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.resource_events.is_empty());
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r01_未使用結構的合法self方法不可冒充真函式進入() {
        let src = "struct Archive{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn step(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nreturn step(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "get"
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_r01_無關結構的utf8預設欄位不得於main執行時建立資源() {
        let src = "struct Archive{\nvar title:str = \"未使用🪼\"\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(_)
        )));
    }

    #[test]
    fn g18_r02_旁觀結構宣告不影響真main分支字串借用與清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar title:str = \"旁觀\"\n}\nfn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(true){\nvar owner:str = \"唯一🪼\"\nresult = inspect(owner,3)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_))).count(), 1);
    }

    #[test]
    fn g18_r01_非法重複結構欄位仍由原checker拒絕() {
        let src = "struct Archive{\nvar count:int = 1\nvar count:int = 2\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(2)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_r01_來源已驗證的結構清單遭竄改必在執行前拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn step(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn step(4)\n}\n";
        let mut plan = checked(src);
        plan.verified.structs.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R03：跨 if／else-if／while／for 的真控制邊，必須在同一
    // 原 Checker 核准工作堆疊依實際進入路徑清理 Move／BorrowTicket。
    #[test]
    fn g18_r03_main_else_if選中分支str_move借用只建立一個resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nvar source:str = \"未選一\"\nresult = inspect(source,100)\n}else if(true){\nvar source:str = \"選中🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,3)\n}else{\nvar source:str = \"未選二\"\nresult = inspect(source,200)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_while內for_break只清理內層move借用且外層仍繼續() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar active:bool = true\nvar total:int = 0\nwhile(active){\nfor i in 0..3{\nvar source:str = \"內層🪼\"\nvar target:str\ntarget = source.move()\ntotal = total+inspect(target,i)\nbreak\n}\nactive = false\n}\nreturn total\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main_for內提前return完成真move借用且不進後續迭代() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn fallback(n:int)->int{\nreturn n\n}\nfn main()->int{\nfor i in 0..4{\nvar source:str = \"第一輪🪼\"\nvar target:str\ntarget = source.move()\nreturn inspect(target,i+5)\n}\nreturn fallback(0)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "fallback"
        )));
    }

    #[test]
    fn g18_r03_main_else_if真move後第二引數溢位須回滾票據和唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = 99\n}else if(true){\nvar source:str = \"錯誤🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(target,1+9223372036854775807)\n}\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }

    #[test]
    fn g18_r03_main_else_if已move來源再借用由原checker提前拒絕() {
        let src = "fn inspect(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nvar result:int = 0\nif(false){\nresult = 1\n}else if(true){\nvar source:str = \"單一🪼\"\nvar target:str\ntarget = source.move()\nresult = inspect(source,3)\n}\nreturn result\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }


    // R01：main 尾端只是合法 int 常值，前置語句裡的函式
    // 一樣必須真進入與真返回，不能以最後的常值掩蓋它的錯誤。
    #[test]
    fn g18_r01_main前置真呼叫後尾端int常值return必執行呼叫() {
        let src = "fn calc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar ignored:int = calc(3)\nreturn 9\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "calc"
        )));
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(4)
        )));
    }

    #[test]
    fn g18_r01_main真賦值函式後尾端括號整數常值仍須執行() {
        let src = "fn calc(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar output:int\noutput = calc(5)\nreturn (11)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "calc"
        )).count(), 1);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(7)
        )));
    }

    #[test]
    fn g18_r03_main分支真str借用函式後返回常值應先歸還再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nif(true){\nvar owner:str = \"先執行🪼\"\nvar ignored:int = inspect(owner,3)\n}\nreturn 17\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(17));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )).count(), 1);
    }

    #[test]
    fn g18_r03_main前置函式溢位不可被最後return常值掩蓋() {
        let src = "fn crash(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar unused:int = crash(9223372036854775807)\nreturn 9\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "crash"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(9)
        )));
    }

    #[test]
    fn g18_r01_main前置兩函式依序執行後才允許返回無關常值() {
        let src = "fn first(n:int)->int{\nreturn n+1\n}\nfn second(n:int)->int{\nreturn n+2\n}\nfn main()->int{\nvar a:int = first(2)\nvar b:int = second(a)\nreturn 0\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        let entered: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["first", "second"]);
    }

    #[test]
    fn g18_r01_main前置呼叫尾端常值計畫遭竄改必先拒絕() {
        let src = "fn calc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar unused:int = calc(3)\nreturn 9\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        plan.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_不含真函式呼叫的main常值仍不得偽造函式證據() {
        let src = "fn unused(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn 9\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    // R03：原 Checker 不會為所有分支已返回的不可達末端 return
    // 簽發 int Expression HIR；不准因此執行假的末端返回。
    #[test]
    fn g18_r03_main_if_else全分支真返回且末端不可達常值不需假hir() {
        let src = "fn left()->int{\nreturn 2\n}\nfn right()->int{\nreturn 3\n}\nfn main()->int{\nif(true){\nreturn left()\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let plan = checked(src);
        assert!(plan.verified.plan.scalar_hir.expression(plan.call_span).is_none());
        assert!(matches!(&plan.main_dynamic_call, Some(PrivateScalarReturnExpr::Unreachable)));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "left"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
    }

    #[test]
    fn g18_r03_main_if_else假分支不進且不可達末端return零事件() {
        let src = "fn left()->int{\nreturn 5\n}\nfn right()->int{\nreturn 7\n}\nfn main()->int{\nvar use_left:bool = false\nif(use_left){\nreturn left()\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let calls: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(calls, ["right"]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
    }

    #[test]
    fn g18_r03_main全分支返回時選中str借用完成還票與資源清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(s:str,n:int)->int{\nreturn n+1\n}\nfn right()->int{\nreturn 999\n}\nfn main()->int{\nif(true){\nvar owner:str = \"提前🪼\"\nreturn inspect(owner,3)\n}else{\nreturn right()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "right"
        )));
    }

    #[test]
    fn g18_r03_main全分支提前返回溢位不可退回不可達常值() {
        let src = "fn crash(n:int)->int{\nreturn n+1\n}\nfn safe()->int{\nreturn 3\n}\nfn main()->int{\nif(true){\nreturn crash(9223372036854775807)\n}else{\nreturn safe()\n}\nreturn 99\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(99)
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "safe"
        )));
    }

    #[test]
    fn g18_r03_main不可達尾端計畫遭改為可執行常值須拒絕() {
        let src = "fn calc()->int{\nreturn 3\n}\nfn other()->int{\nreturn 4\n}\nfn main()->int{\nif(true){\nreturn calc()\n}else{\nreturn other()\n}\nreturn 99\n}\n";
        let mut plan = checked(src);
        assert!(matches!(plan.main_dynamic_call, Some(PrivateScalarReturnExpr::Unreachable)));
        plan.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(99));
        assert!(matches!(plan.execute(), Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R02：跨框架同名的 str 局部是不同 Checker DeclarationId；
    // 真資源核心必保留原本詞彙身分，不得因文字同名交錯釋放。
    #[test]
    fn g18_r02_父子函式同名str各自建立並依框架反向釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"子層🪼\"\nreturn n+2\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層🪼\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn mid(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(_, _)
        )).count(), 2);
    }

    #[test]
    fn g18_r02_三層move資源與兩層旁觀str不同id各自唯一清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nvar spare:str = \"葉層\"\nreturn n+1\n}\nfn mid(item:str,n:int)->int{\nvar spare:str = \"中層\"\nreturn leaf(item.move(),n)\n}\nfn main()->int{\nvar original:str = \"根🪼\"\nreturn mid(original.move(),3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Created(2),
            R::Moved(1), R::Created(3), R::Released(3),
            R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceTransferred { resource_id: 1, .. }
        )).count(), 2);
    }

    #[test]
    fn g18_r02_借用中的父框架str與子框架新同名局部不能互相冒充() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(item:str,n:int)->int{\nvar owner:str = \"子層獨立\"\nreturn n+1\n}\nfn main()->int{\nconst owner:str = \"父層仍在借用\"\nreturn inspect(owner,7)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ResourceOwnerBound(_, 2)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_子框架同名str後溢位必先釋放子層再清理父層() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"子層錯誤\"\nreturn n+9223372036854775807\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層持有\"\nreturn leaf(n)\n}\nfn main()->int{\nreturn mid(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r02_重入同一葉函式的同名str每次重新配置且不污染父框架() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(n:int)->int{\nvar owner:str = \"每次不同id\"\nreturn n+1\n}\nfn mid(n:int)->int{\nvar owner:str = \"父層仍持有\"\nvar first:int = leaf(n)\nvar second:int = leaf(first)\nreturn second\n}\nfn main()->int{\nreturn mid(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2),
            R::Created(3), R::Released(3), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf"
        )).count(), 2);
    }

    #[test]
    fn g18_r02_借用str參數不得因子函式可見同名文字擅自move() {
        let src = "fn leaf(item:str)->int{\nreturn 1\n}\nfn mid(item:str)->int{\nreturn leaf(item.move())\n}\nfn main()->int{\nvar owner:str = \"不可轉借\"\nreturn mid(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))
                | Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    // R01：主函式尾端若已由 Checker 完整證明不可達，未執行
    // 的其他合法函式不得被強制當作純 int 動態圖的一環。
    #[test]
    fn g18_r01_main全分支提前返回不應編譯尾端不支援的結構函式() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nif(true){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert_eq!(plan.callee_name, "chosen");
        assert!(!plan.nested_callees.iter().any(|p| p.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_r03_main_false分支真函式不被不可達結構呼叫阻塞() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nvar select:bool = false\nif(select){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert!(!plan.nested_callees.iter().any(|p| p.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let entered: Vec<_> = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["other"]);
    }


    // V1／F4：以下均由原 Parser／Checker 已核准的真來源出發，
    // 只有無 str 欄位的結構才交給 P16 MixedStructRuntime 真執行。
    // V1／R01：用與原 Checker 同源的真正 main→函式工作圖驗證
    // Copyable 結構欄位在呼叫實參、賦值、跨框架與故障退出的順序。
    // V1-F3：來源為原 Checker TypedHIR 認可的 bool 欄位；
    // 在真正的 if／while 和普通函式 bool 實參位置動態讀取 P16 持有值。
    // V1-F3：真 Parser/Checker 的 for／while 控制與 Copyable 結構
    // int/bool 欄位混合，逐輪動態求值；不可達邊不得冒充函式進入。
    #[test]
    fn g18_v1_f3_真結構bool欄位三輪for內呼叫函式有序累積() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 3\n}\nfn step(x:int)->int{\nreturn x+2\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nfor i in 0..3{\nif(meter.enabled){\ntotal = total+step(meter.count)\n}\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(15));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "step"
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f3_真結構欄位while中呼叫後break不重複執行() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 4\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nwhile(meter.enabled){\ntotal = next(meter.count)\nbreak\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_真結構欄位零次for永不進入待呼叫函式() {
        let src = "struct Meter{\nvar count:int = 4\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 8\nfor i in 0..0{\ntotal = next(meter.count)\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )));
    }

    #[test]
    fn g18_v1_f3_真結構欄位for條件continue不執行後續呼叫() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 5\n}\nfn next(x:int)->int{\nreturn x+1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nfor i in 0..2{\nif(meter.enabled){\ncontinue\n}\ntotal = next(meter.count)\n}\nreturn total\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(0));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "next"
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool真欄位控制if選中及未選中不造假框架() {
        let src = "struct Meter{\nvar enabled:bool = true\nvar count:int = 5\n}\nfn yes()->int{\nreturn 7\n}\nfn no()->int{\nreturn 3\n}\nfn route()->int{\nvar note:Meter = Meter(enabled=false)\nif(note.enabled){\nreturn yes()\n}else{\nreturn no()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "yes"
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位真實參在子函式if判定() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn yes()->int{\nreturn 10\n}\nfn no()->int{\nreturn 2\n}\nfn choose(flag:bool)->int{\nif(flag){\nreturn yes()\n}\nreturn no()\n}\nfn route()->int{\nvar meter:Meter = Meter(enabled=false)\nreturn choose(meter.enabled)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::BoolArgumentEvaluated(false)
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool真欄位while每輪判定且break有序離域() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one(x:int)->int{\nreturn x\n}\nfn route()->int{\nvar meter:Meter = Meter()\nvar total:int = 0\nwhile(meter.enabled){\ntotal = total+1\nbreak\n}\nreturn one(total)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ConditionalChecked(true)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_結構bool_false_while零迭代且保留真函式圖() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one(x:int)->int{\nreturn x\n}\nfn route()->int{\nvar meter:Meter = Meter(enabled=false)\nvar total:int = 8\nwhile(meter.enabled){\ntotal = total+1\n}\nreturn one(total)\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ConditionalChecked(false)
        )));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位型別不符原Checker直接拒絕() {
        let src = "struct Meter{\nvar count:int = 9\n}\nfn yes()->int{\nreturn 7\n}\nfn route()->int{\nvar meter:Meter = Meter()\nif(meter.count){\nreturn yes()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f3_結構bool欄位編譯身分竄改必須零執行拒絕() {
        let src = "struct Meter{\nvar enabled:bool = true\n}\nfn one()->int{\nreturn 1\n}\nfn route()->int{\nvar meter:Meter = Meter()\nif(meter.enabled){\nreturn one()\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(1));
        let Some(PrivateScalarLocalStep::EarlyReturn(branch)) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::EarlyReturn(_))) else {
            panic!("原 Checker 真 if 工作圖必須存在");
        };
        branch.condition = PrivateScalarBoolExpr::CopyStructBoolField {
            declaration: DeclarationId(0),
            owner: "meter".to_owned(), field: "enabled".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F3：所有 int 欄位寫入均由真正 Checker 允許的
    // var 實例／var 欄位與 TypedHIR RHS 簽發，P16 真值更新後才留下事件。
    // V1／F3：bool 欄位只在真 P16 持有者修改後可改變
    // 同一原 Checker 合法的 if／while 動態控制判斷。
    // V1／F3：補 #660 未涵蓋的來源交叉，沿用同一 Checker/P16 真 runtime。
    #[test]
    fn g18_v1_f3_同一函式if內同名結構遮蔽更新後外層真值不變() {
        let source = "struct Meter{\nvar count:int = 1\nvar ready:bool = false\n}\nfn run()->int{\nvar note:Meter = Meter(count=3)\nif(true){\nvar note:Meter = Meter(count=9)\nnote.count = 11\nnote.ready = true\nif(note.ready){\nnote.count = note.count+1\n}\n}\nreturn note.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f3_bool欄位真來源作為右值並保持先讀後寫() {
        let src = "struct Flags{\nvar left:bool = false\nvar right:bool = true\n}\nfn route()->int{\nvar f:Flags = Flags()\nf.left = f.right\nf.right = false\nif(f.left){\nreturn 9\n}\nreturn 0\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 2);
    }


    #[test]
    fn g18_v1_f3_bool欄位只寫所選if葉而且可傳實參() {
        let src = "struct Flag{\nvar ready:bool = false\n}\nfn choose(value:bool)->int{\nif(value){\nreturn 5\n}\nreturn 2\n}\nfn route(flag:bool)->int{\nvar f:Flag = Flag()\nif(flag){\nf.ready = true\n}else{\nf.ready = false\n}\nreturn choose(f.ready)\n}\nfn main()->int{\nreturn route(false)+route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 2);
    }


    #[test]
    fn g18_v1_f3_bool欄位for_continue與while_break真回邊() {
        let src = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nfor i in 0..2{\nf.ready = true\ncontinue\n}\nwhile(f.ready){\nf.ready = false\nbreak\n}\nif(f.ready){\nreturn 99\n}\nreturn 4\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).count(), 3);
    }


    #[test]
    fn g18_v1_f3_bool欄位非法const及錯型別必原Checker拒絕() {
        let const_owner = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nconst f:Flag = Flag()\nf.ready = true\nreturn 0\n}\nfn main()->int{return route()}\n";
        let const_field = "struct Flag{\nconst ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nf.ready = true\nreturn 0\n}\nfn main()->int{return route()}\n";
        let wrong_type = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar f:Flag = Flag()\nf.ready = 1\nreturn 0\n}\nfn main()->int{return route()}\n";
        for src in [const_owner, const_field, wrong_type] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    // V1／F4：使用原 Checker direct_moves 的唯一轉移配對，
    // 真 P16 整筆 move 後目標才可以讀，原來源已不可讀。
    #[test]
    fn g18_v1_f4_純結構整筆move真值唯一轉交而無第二建構() {
        let src = "struct Marker{\nvar count:int = 6\nvar enabled:bool = true\n}\nfn route()->int{\nvar source:Marker = Marker()\nvar target:Marker = source.move()\nreturn target.count+1\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_純結構兩次move鏈只交付最後唯一持有者() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nvar third:Marker = second.move()\nreturn third.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f4_純結構真分支整筆move早返且完成雙局部離域() {
        let src = "struct Marker{\nvar count:int = 8\n}\nfn route(flag:bool)->int{\nvar source:Marker = Marker()\nif(flag){\nvar target:Marker = source.move()\nreturn target.count\n}\nreturn 2\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        // Checker 接受但實際沒選到的同一條控制路徑，
        // 不得提前發出 MoveBound 或少清理來源結構。
        let not_selected = src.replace("route(true)", "route(false)");
        let skipped = checked(&not_selected).execute().unwrap();
        assert_eq!(skipped.result, Ok(2));
        assert!(skipped.has_balanced_frame_trace());
        assert!(skipped.resource_events.is_empty());
        assert!(!skipped.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )));
        assert_eq!(skipped.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_move後再次移動相同原結構原Checker必拒絕() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nvar illegal:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_move動態計畫DeclarationId遭改寫必先拒絕() {
        let src = "struct Marker{\nvar count:int = 6\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
        let Some(PrivateScalarLocalStep::MoveCopyStruct { source, .. }) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::MoveCopyStruct { .. })) else {
            panic!("原 Checker 真 move 工作應存在");
        };
        *source = DeclarationId(0);
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F4：含 const str 的整筆結構移動不重新建立 UTF-8，
    // 原 P16 真 OwnedUtf8 必須保有 ResourceId 並由新持有者唯一釋放。
    // V1-F4：原 Checker 真具名 var str 完整結構以同一
    // P16 SharedLedger Move，移後仍可依既存原子欄位覆寫核准路徑更新。
    #[test]
    fn g18_v1_f4_具名varstr結構整筆move原resourceid唯一轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"未選預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"真值🪼\")\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr移動前後兩次真覆寫各id只釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"不執行預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"初始\")\nfirst.data = \"移動前\"\nvar second:Marker = first.move()\nsecond.data = \"移動後🪼\"\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Moved(2),
            R::Created(3), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 2);
    }


    // V1-F4／R07：P16 真資源欄位次序是結構定義順序，不是名稱字典序；
    // 欄位原子換值後仍必以最新 ResourceId 逐欄 Move。
    #[test]
    fn g18_v1_f4_倒序欄名替換後整筆move仍依P16欄位順序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"預設甲\"\nvar alpha:str = \"預設乙\"\nvar count:int = 8\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"甲\",alpha=\"乙\")\nold.zeta = \"新甲🪼\"\nvar next:Marker = old.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3), R::Released(1),
            R::Moved(3), R::Moved(2), R::Released(2), R::Released(3),
        ]);
    }

    #[test]
    fn g18_v1_f4_雙欄move事件即使雙帳同步調序仍須拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"甲\"\nvar alpha:str = \"乙\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"真甲\",alpha=\"真乙\")\nvar next:Marker = old.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let inline: Vec<_> = out.events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateScalarCallEvent::OwnedResource(R::Moved(_)))
                .then_some(index)).collect();
        let ledger: Vec<_> = out.resource_events.iter().enumerate()
            .filter_map(|(index, event)|
                matches!(event, R::Moved(_)).then_some(index)).collect();
        assert_eq!(inline.len(), 2);
        assert_eq!(ledger.len(), 2);
        out.events.swap(inline[0], inline[1]);
        out.resource_events.swap(ledger[0], ledger[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_v1_f4_雙欄連續兩次整筆move仍保持欄位原次序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar zeta:str = \"甲\"\nvar alpha:str = \"乙\"\nvar count:int = 6\n}\nfn route()->int{\nvar old:Marker = Marker(zeta=\"真甲\",alpha=\"真乙\")\nvar middle:Marker = old.move()\nvar next:Marker = middle.move()\nreturn next.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Moved(1), R::Moved(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_雙具名varstr整筆move保持雙resourceid及逆序退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar first:str = \"忽略甲\"\nvar second:str = \"忽略乙\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker(first=\"甲\",second=\"乙🪼\")\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr與conststr整筆move後僅var可覆寫() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar data:str = \"預設\"\nconst label:str = \"不可寫\"\nvar count:int = 9\n}\nfn route()->int{\nvar old:Marker = Marker(data=\"可移🪼\")\nvar new:Marker = old.move()\nnew.data = \"新值\"\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Created(3), R::Released(1), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名varstr整筆move僅真條件葉有資源移動() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 4\nvar data:str = \"預設\"\n}\nfn route(flag:bool)->int{\nvar old:Marker = Marker(data=\"真值\")\nif(flag){\nvar new:Marker = old.move()\nreturn new.count\n}\nreturn old.count\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let selected = checked(src).execute().unwrap();
        assert_eq!(selected.result, Ok(4));
        assert!(selected.has_balanced_frame_trace());
        assert_eq!(selected.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        let false_src = src.replace("route(true)", "route(false)");
        let not_selected = checked(&false_src).execute().unwrap();
        assert_eq!(not_selected.result, Ok(4));
        assert!(not_selected.has_balanced_frame_trace());
        assert_eq!(not_selected.resource_events, [R::Created(1), R::Released(1)]);
    }

    #[test]
    fn g18_v1_f4_具名varstr整筆move後再讀來源原Checker必拒絕() {
        let src = "struct Marker{\nvar count:int = 7\nvar data:str = \"預設\"\n}\nfn route()->int{\nvar first:Marker = Marker(data=\"源值\")\nvar second:Marker = first.move()\nreturn first.count\n}\nfn main()->int{\nreturn route()\n}\n";
        assert!(PreparedPrivateScalarCall::prepare(src).is_err());
    }

    #[test]
    fn g18_v1_f4_conststr結構整筆move真資源唯一轉交() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"真資源🪼\"\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { .. }
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_雙conststr整筆move兩個id不複製並反序退出() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nvar count:int = 5\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(1), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_conststr整筆move在真條件分支才發出moved() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"分支資源\"\n}\nfn route(flag:bool)->int{\nvar first:Marker = Marker()\nif(flag){\nvar second:Marker = first.move()\nreturn second.count\n}\nreturn first.count\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )).count(), 1);
        let src_false = src.replace("route(true)", "route(false)");
        let absent = checked(&src_false).execute().unwrap();
        assert_eq!(absent.result, Ok(7));
        assert!(absent.has_balanced_frame_trace());
        assert_eq!(absent.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!absent.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructMovedBound { .. }
        )));
    }

    #[test]
    fn g18_v1_f4_conststr整筆move後溢位仍真釋放唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"回滾🪼\"\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count+9223372036854775807\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Moved(1), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_conststr整筆move資源事件竄改及刪除必拒絕() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Marker{\nvar count:int = 7\nconst data:str = \"防偽\"\n}\nfn route()->int{\nvar old:Marker = Marker()\nvar new:Marker = old.move()\nreturn new.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let index = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Moved(_))
        )).expect("原共享帳已發出真 Moved");
        let original = out.events[index].clone();
        out.events[index] = PrivateScalarCallEvent::OwnedResource(R::Moved(999));
        assert!(!out.has_balanced_frame_trace());
        out.events[index] = original;
        out.events.remove(index);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_可寫str預設結構整筆move維持唯一resourceid() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Marker{\nvar count:int = 6\nvar data:str = \"安全\"\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);

    }

    #[test]
    fn g18_v1_f4_無str預設的部分初始化整筆move仍保守拒絕() {
        // 真 Checker 先拒絕未初始化 var str 整筆 Move，
        // 不能為了命中 Runtime Unsupported 降低靜態安全標準。
        let src = "struct Marker{\nvar count:int = 6\nvar data:str\n}\nfn route()->int{\nvar first:Marker = Marker()\nvar second:Marker = first.move()\nreturn second.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let result = PreparedPrivateScalarCall::prepare(src);
        assert!(matches!(&result, Err(PrivateScalarCallPrepareError::Check(_))),
            "原 Checker 須先拒絕不完整來源");

    }

    #[test]
    fn g18_v1_f3_bool結構欄位寫入後if重新讀取最新值() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nif(item.enabled){\nreturn 9\n}\nreturn 2\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, name, false)
                if name == "enabled"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool欄位while首輪更新後反向邊真結束() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nvar total:int = 0\nwhile(item.enabled){\ntotal = total+1\nitem.enabled = false\n}\nreturn total\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::ConditionalChecked(false)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool結構欄位只有選中分支真更新() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nif(false){\nitem.enabled = false\n}else{\nitem.enabled = true\n}\nif(item.enabled){\nreturn 8\n}\nreturn 2\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, _, true)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(_, _, false)
        )));
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f3_bool結構寫入後溢位要正常單次離域() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 9223372036854775807+1\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_bool欄位const持有者或const欄位原Checker拒絕() {
        let owner = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nconst item:Flag = Flag()\nitem.enabled = false\nreturn 1\n}\nfn main()->int{\nreturn step()\n}\n";
        let field = "struct Flag{\nconst enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 1\n}\nfn main()->int{\nreturn step()\n}\n";
        for src in [owner, field] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f3_bool欄位偽造更新DeclarationId必拒絕() {
        let src = "struct Flag{\nvar enabled:bool = true\n}\nfn step()->int{\nvar item:Flag = Flag()\nitem.enabled = false\nreturn 3\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let mut changed = out;
        let target = changed.events.iter_mut().find(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructBoolFieldAssigned(..)
        )).expect("真 P16 bool 欄位事件");
        if let PrivateScalarCallEvent::CopyStructBoolFieldAssigned(id, _, _) = target {
            *id = DeclarationId(0);
        }
        assert!(!changed.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f3_真struct_int欄位寫入後讀回更新值() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = note.count+4\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, field, 7)
                if field == "count"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_分支只寫入選中路徑的真struct欄位() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step(flag:bool)->int{\nvar note:Meter = Meter()\nif(flag){\nnote.count = 9\n}else{\nnote.count = 2\n}\nreturn note.count\n}\nfn main()->int{\nreturn step(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, field, 2)
                if field == "count"
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(_, _, 9)
        )));
    }

    #[test]
    fn g18_v1_f3_真struct可寫欄位在callee故障後仍須一次逆序離域() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 9\nreturn note.count+9223372036854775807\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f3_const結構或const欄位的寫入原Checker必拒絕() {
        let const_owner = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nconst note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let const_field = "struct Meter{\nconst count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        for src in [const_owner, const_field] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f3_偽造struct欄位更新來源身分必在稽核拒絕() {
        let src = "struct Meter{\nvar count:int = 3\n}\nfn step()->int{\nvar note:Meter = Meter()\nnote.count = 8\nreturn note.count\n}\nfn main()->int{\nreturn step()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        let mut forged = out;
        let field_event = forged.events.iter_mut().find(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructIntFieldAssigned(..)
        )).expect("原 P16 真欄位寫入事件");
        if let PrivateScalarCallEvent::CopyStructIntFieldAssigned(id, _, _) = field_event {
            *id = DeclarationId(0);
        }
        assert!(!forged.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f1_f3_main結構欄位作為真函式實參依動態值求出() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nvar note:Archive = Archive(count=8)\nreturn plus(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f1_f3_main結構欄位函式初始化賦值與返回有序執行() {
        let src = "struct Archive{\nvar count:int = 5\n}\nfn plus(x:int)->int{\nreturn x+2\n}\nfn main()->int{\nvar note:Archive = Archive()\nvar result:int = 0\nresult = plus(note.count)\nreturn result\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f1_f3_父子同名結構幀中讀欄位恢復父值不串位() {
        let src = "struct Archive{\nvar count:int = 1\n}\nfn leaf(x:int)->int{\nreturn x+1\n}\nfn mid()->int{\nvar note:Archive = Archive(count=9)\nreturn leaf(note.count)\n}\nfn main()->int{\nvar note:Archive = Archive(count=3)\nreturn mid()+leaf(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f1_f3_被呼叫者溢位先退出後清理main真結構() {
        let src = "struct Archive{\nvar count:int = 9223372036854775807\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nvar note:Archive = Archive()\nreturn plus(note.count)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    // V1／F4：原 Checker 的具名純值來源必須按 DeclarationId
    // 真正讀取動態 frame，而不是以初始字面值假造欄位。
    // V1／F4：只有原 Checker 接受的 Copyable int/bool 真欄位才能
    // 作另一結構的具名值；建構完成後不可偷偷保留來源別名。
    #[test]
    fn g18_v1_f4_具名int欄位真值快照不受來源後續更新污染() {
        let source = "struct Source{\nvar count:int = 3\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\na.count = 7\nvar b:Target = Target(count=a.count)\na.count = 9\nreturn b.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool來源欄位真快照決定後續分支() {
        let source = "struct Flag{\nvar ready:bool = false\n}\nfn route()->int{\nvar a:Flag = Flag()\na.ready = true\nvar b:Flag = Flag(ready=a.ready)\na.ready = false\nif(b.ready){\nreturn 5\n}\nreturn 2\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名雙欄位由另一真結構讀int_bool混合參數() {
        let source = "struct Source{\nvar count:int = 3\nvar ready:bool = true\n}\nstruct Target{\nvar count:int = 0\nvar ready:bool = false\n}\nfn route(flag:bool)->int{\nvar a:Source = Source()\na.count = 9\na.ready = flag\nvar b:Target = Target(count=a.count,ready=a.ready)\nif(b.ready){\nreturn b.count\n}\nreturn 1\n}\nfn main()->int{\nreturn route(true)+route(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 4);
    }

    #[test]
    fn g18_v1_f4_具名欄位來源型別不合由原Checker先拒絕() {
        let source = "struct Source{\nvar ready:bool = true\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\nvar b:Target = Target(count=a.ready)\nreturn b.count\n}\nfn main()->int{return route()}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(source),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_具名真欄位來源身分竄改執行前拒絕() {
        let source = "struct Source{\nvar count:int = 3\n}\nstruct Target{\nvar count:int = 0\n}\nfn route()->int{\nvar a:Source = Source()\na.count = 7\nvar b:Target = Target(count=a.count)\na.count = 9\nreturn b.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
        let Some(PrivateScalarLocalStep::DeclareCopyStruct { plan: named, .. }) =
            plan.root_local_steps.iter_mut().find(|step| match step {
                PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                    plan.named.iter().any(|(name, _)| name == "count"),
                _ => false,
            }) else { panic!("已驗證真欄位來源工作應存在"); };
        named.named[0].1 = PrivateScalarCopyStructSeed::IntField {
            declaration: DeclarationId(0),
            owner: "a".to_owned(),
            field: "count".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F4：原 Checker 的具名 int 加減保留當下真局部與真結構欄位，
    // 交由已驗證 P16 建構求值；動態溢位不得留下半個資源結構。
    // V1／F1/F3/F4：具名 Copyable int 的真函式 Call 必須經
    // 同一顯式工作堆疊執行，所有 RHS 完成後 P16 才能原子建構。
    // V1／F2×F4：具名建構中的真普通函式可借用／移動 str，
    // 但 ResourceId／BorrowTicket 一律先完成原函式退出才允許 P16 建構。
    // V1／F3×F4：同一 Checker 已編譯 Call 不得因 for/while 反向邊
    // 被提前執行；每輪完整 P16 作用域、借用票據獨立反向歸還。
    #[test]
    fn g18_v1_f3_f4_具名函式for三輪建構並continue逐輪反序離域() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar sum:int = 0\nfor i in 0..3{\nvar note:Meter = Meter(count=plus(i))\nsum = sum+note.count\ncontinue\n}\nreturn sum\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 3);
    }

    #[test]
    fn g18_v1_f3_f4_具名函式while真回邊只執行動態首輪() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar value:int = 0\nvar active:bool = true\nwhile(active){\nvar note:Meter = Meter(count=plus(value))\nvalue = note.count\nactive = false\n}\nreturn value\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f3_f4_空for具名callee與P16資源均零效果() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nfor i in 0..0{\nvar note:Meter = Meter(count=plus(i))\n}\nreturn 5\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f3_f4_具名函式for每輪真借同一str但P16按輪獨立() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(item:str,x:int)->int{\nreturn x+1\n}\nfn run()->int{\nconst owner:str = \"逐輪借用🪼\"\nvar sum:int = 0\nfor i in 0..2{\nvar note:Meter = Meter(count=plus(owner,i))\nsum = sum+note.count\n}\nreturn sum\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(source).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f2_f4_具名呼叫借str真票據先歸還後建構且資源唯一() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst owner:str = \"原資料🪼\"\nvar note:Meter = Meter(count=inspect(owner,4))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).expect("callee 回票");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("具名函式完畢後才建構");
        assert!(released < constructed);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_具名呼叫真str_move先轉唯一資源且返回後建構() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn consume(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nvar owner:str = \"整筆轉交🪼\"\nvar note:Meter = Meter(count=consume(owner.move(),3))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Moved(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(1))
        )).expect("唯一原資源在 callee 離域");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("P16 真建構");
        assert!(released < constructed);
    }

    #[test]
    fn g18_v1_f2_f4_雙str具名函式依序借還且兩個resourceid不混() {
        let src = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst a:str = \"甲🪼\"\nconst b:str = \"乙🪼\"\nvar note:Pair = Pair(first=inspect(a,2),second=inspect(b,6))\nreturn note.first+note.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_第一具名借用已成功後第二呼叫溢位仍不得建構() {
        let src = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn overflow(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nconst owner:str = \"保留🪼\"\nvar maximum:int = 9223372036854775807\nvar note:Pair = Pair(first=inspect(owner,3),second=overflow(maximum))\nreturn note.first+note.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute().unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f2_f4_具名已取得str借用但子呼叫預算耗盡必先還票() {
        let src = "struct Meter{\nvar count:int = 0\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn build()->int{\nconst owner:str = \"框架預算🪼\"\nvar note:Meter = Meter(count=inspect(owner,2))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 300 }
        ).unwrap();
        use crate::owned_str_runtime::ResourceEvent as R;
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanAcquired(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
    }

    #[test]
    fn g18_v1_f4_具名int真呼叫先返回後才建構P16唯一值() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn add(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar base:int = 8\nvar note:Meter = Meter(count=add(base))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["build", "add"]);
    }

    #[test]
    fn g18_v1_f4_兩個具名int函式RHS依原書寫順序進出() {
        let source = "struct Pair{\nvar first:int = 0\nvar second:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn double(x:int)->int{\nreturn x+x\n}\nfn build()->int{\nvar pair:Pair = Pair(first=plus(2),second=double(4))\nreturn pair.first+pair.second\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["build", "plus", "double"]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名加減內兩層函式真工作圖與結果() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar note:Meter = Meter(count=plus(plus(3))+2)\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名函式發生溢位時零結構提交且真逐幀回滾() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar max:int = 9223372036854775807\nvar note:Meter = Meter(count=plus(max))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_未選控制葉具名函式零執行零結構() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn build(flag:bool)->int{\nif(flag){\nvar note:Meter = Meter(count=plus(7))\nreturn note.count\n}\nreturn 4\n}\nfn main()->int{\nreturn build(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "plus"
        )));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f4_具名函式來源計畫遭竄改不得偷偷替換回傳() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn add(x:int)->int{\nreturn x+1\n}\nfn build()->int{\nvar base:int = 8\nvar note:Meter = Meter(count=add(base))\nreturn note.count\n}\nfn main()->int{\nreturn build()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "count")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("真具名函式來源計畫");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::IntCall(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Int(9));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_bool括號預設真值由P16建構後讀取() {
        let src = "struct Meter{\nvar enabled:bool = (true)\n}\nfn run()->int{\nvar note:Meter = Meter()\nif(note.enabled){\nreturn 10\n}\nreturn 1\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_bool雙重括號預設與具名覆蓋不錯置() {
        let src = "struct Meter{\nvar enabled:bool = ((false))\n}\nfn run()->int{\nvar first:Meter = Meter()\nvar second:Meter = Meter(enabled=true)\nif(first.enabled){\nreturn 1\n}\nif(second.enabled){\nreturn 6\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_bool括號預設錯型別原Checker拒絕() {
        let src = "struct Meter{\nvar enabled:bool = (1)\n}\nfn run()->int{\nvar note:Meter = Meter()\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_bool括號預設計畫變造不許偽造欄位() {
        let src = "struct Meter{\nvar enabled:bool = (true)\n}\nfn run()->int{\nvar note:Meter = Meter()\nif(note.enabled){\nreturn 4\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        let default = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.fields.iter_mut().find(|(field, _, _, _)| field == "enabled")
                    .map(|(_, _, seed, _)| seed),
            _ => None,
        }).expect("原 P16 bool 預設來源");
        assert!(matches!(default, Some(ApprovedSeed::Bool(true))));
        *default = Some(ApprovedSeed::Bool(false));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名bool括號讀取更新後局部真值() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar flag:bool = false\nflag = true\nvar note:Meter = Meter(enabled=(flag))\nif(note.enabled){\nreturn 9\n}\nreturn 1\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名bool雙括號跨結構讀取即時值() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar seed:Meter = Meter(enabled=false)\nseed.enabled = true\nvar note:Meter = Meter(enabled=((seed.enabled)))\nif(note.enabled){\nreturn 5\n}\nreturn 2\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool括號非法型別原Checker先拒絕() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar note:Meter = Meter(enabled=(1))\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_具名bool括號來源計畫變造執行前拒絕() {
        let src = "struct Meter{\nvar enabled:bool = false\n}\nfn run()->int{\nvar flag:bool = true\nvar note:Meter = Meter(enabled=(flag))\nif(note.enabled){\nreturn 3\n}\nreturn 0\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "enabled")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("原 Checker 真具名 bool Group");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::BoolGrouped(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Bool(true));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名int多層算式讀更新後局部而非宣告時常值() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar source:int = 4\nsource = source+3\nvar note:Meter = Meter(count=(source+2))\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名int算式跨真正可寫結構欄位加減() {
        let source = "struct Meter{\nvar count:int = 1\n}\nfn compute()->int{\nvar left:Meter = Meter(count=6)\nleft.count = 9\nvar right:Meter = Meter(count=(left.count-2)+5)\nreturn right.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名bool與動態int算式一併交付真P16快照() {
        let source = "struct Meter{\nvar count:int = 0\nvar enabled:bool = false\n}\nfn choose(flag:bool)->int{\nvar source:int = 3\nvar note:Meter = Meter(count=source+4,enabled=flag)\nsource = 99\nif(note.enabled){\nreturn note.count\n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名動態int溢位不得建立半個結構或遺留資源() {
        let source = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar maximum:int = 9223372036854775807\nvar note:Meter = Meter(count=maximum+1)\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名int算式原Checker拒絕錯型別且動態計畫防偽() {
        let invalid = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar ready:bool = true\nvar note:Meter = Meter(count=ready+1)\nreturn note.count\n}\nfn main()->int{return compute()}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let src = "struct Meter{\nvar count:int = 0\n}\nfn compute()->int{\nvar source:int = 4\nsource = source+3\nvar note:Meter = Meter(count=(source+2))\nreturn note.count\n}\nfn main()->int{\nreturn compute()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let seed = plan.root_local_steps.iter_mut().find_map(|step| match step {
            PrivateScalarLocalStep::DeclareCopyStruct { plan, .. } =>
                plan.named.iter_mut().find(|(name, _)| name == "count")
                    .map(|(_, seed)| seed),
            _ => None,
        }).expect("應找到原 Checker 具名動態算式");
        assert!(matches!(seed, PrivateScalarCopyStructSeed::IntArithmetic(_)));
        *seed = PrivateScalarCopyStructSeed::Static(ApprovedSeed::Int(7));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_具名int取最後動態局部值不重播宣告值() {
        let source = "struct Meter{\nvar count:int = 1\n}\nfn route()->int{\nvar n:int = 5\nn = 9\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名bool來源由真函式參數動態分支決定() {
        let source = "struct Meter{\nvar count:int = 3\nvar ready:bool = false\n}\nfn route(flag:bool)->int{\nvar note:Meter = Meter(ready=flag)\nif(note.ready){\nreturn note.count+5\n}\nreturn note.count\n}\nfn main()->int{\nreturn route(true)+route(false)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名原局部遮蔽後使用本層Checker身分值() {
        let source = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:int = 3\nif(true){\nvar n:int = 11\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nreturn n\n}\nfn main()->int{\nreturn route()\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f4_具名int_bool錯型別與未初始化原Checker先拒絕() {
        let wrong_type = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:bool = true\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{return route()}\n";
        let uninitialized = "struct Meter{\nvar count:int = 2\n}\nfn route()->int{\nvar n:int\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{return route()}\n";
        for src in [wrong_type, uninitialized] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_v1_f4_具名局部來源DeclarationId篡改執行前拒絕() {
        let src = "struct Meter{\nvar count:int = 1\n}\nfn route()->int{\nvar n:int = 5\nn = 9\nvar note:Meter = Meter(count=n)\nreturn note.count\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        let Some(PrivateScalarLocalStep::DeclareCopyStruct { plan: named, .. }) =
            plan.root_local_steps.iter_mut().find(|step|
                matches!(step, PrivateScalarLocalStep::DeclareCopyStruct { .. })) else {
            panic!("原 Checker 真具名 struct 工作不存在");
        };
        named.named[0].1 = PrivateScalarCopyStructSeed::IntBound(DeclarationId(0));
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_真P16結構int欄位讀取而非硬編預設值() {
        let src = "struct Meter{\nvar count:int = 7\nvar flag:bool = true\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.count+3\n}\nfn main()->int{\nreturn getter()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_具名覆蓋保留真欄位來源不執行錯誤預設() {
        let src = "struct Meter{\nvar count:int = 9223372036854775807+1\n}\nfn getter()->int{\nvar note:Meter = Meter(count=9)\nreturn note.count\n}\nfn main()->int{\nreturn getter()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
    }

    #[test]
    fn g18_v1_f4_同名Copyable結構跨兩次函式呼叫不錯用欄位值() {
        let src = "struct Meter{\nvar count:int = 1\n}\nfn first()->int{\nvar note:Meter = Meter(count=4)\nreturn note.count\n}\nfn second()->int{\nvar note:Meter = Meter(count=9)\nreturn note.count\n}\nfn main()->int{\nreturn first()+second()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(13));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_欄位錯誤型別或不存在原Checker即拒絕() {
        let invalid_type = "struct Meter{\nvar flag:bool = true\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.flag\n}\nfn main()->int{\nreturn getter()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let missing = "struct Meter{\nvar count:int = 7\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.unknown\n}\nfn main()->int{\nreturn getter()\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(missing),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_v1_f4_真欄位編譯身分遭篡改執行前必拒絕() {
        let src = "struct Meter{\nvar count:int = 7\n}\nfn getter()->int{\nvar note:Meter = Meter()\nreturn note.count\n}\nfn main()->int{\nreturn getter()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
        plan.return_expr = PrivateScalarReturnExpr::CopyStructIntField {
            declaration: DeclarationId(0),
            owner: "note".to_owned(), field: "count".to_owned(),
        };
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1-F4：原 Checker 核准 const str 結構的真正 UTF-8 所有權，
    // 同一 OwnedStrRuntime 帳中按建構先後配置 ResourceId。
    #[test]
    fn g18_v1_f4_同層先conststr結構後字面str按真建立反序離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"結構\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nvar tail:str = \"晚建🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層雙str結構先於普通str三資源真反序() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst first:str = \"甲\"\nconst second:str = \"乙\"\nvar count:int = 5\n}\nfn run()->int{\nvar item:Archive = Archive()\nconst tail:str = \"丙\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層後建varstr兩次字面覆寫仍先於結構離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 9\nconst label:str = \"先建\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nvar tail:str = \"舊\"\ntail = \"新🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(2), R::Released(3), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層後建str借用先還票再釋放字串與結構() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 4\nconst label:str = \"較早\"\n}\nfn receive(s:str)->int{\nreturn 6\n}\nfn run()->int{\nvar item:Archive = Archive()\nconst tail:str = \"借用者\"\nreturn receive(tail)+item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).expect("BorrowTicket 真歸還");
        let released = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::OwnedResource(R::Released(_))
        )).expect("真資源退出");
        assert!(returned < released);
    }

    #[test]
    fn g18_v1_f4_真if局部先結構後str兩資源按所選葉釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 3\nconst label:str = \"分支\"\n}\nfn run()->int{\nif(true){\nvar item:Archive = Archive()\nvar tail:str = \"內層🪼\"\n}\nreturn 8\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    // V1-F4：原 Checker 准許的具名 var str 字面值，與不可寫
    // str 欄位使用 P16 同一真 OwnedUtf8 資源帳，不放行預設 var str。
    // V1-F4：所有替換只能在原 Checker 合法的 var str 欄位上，
    // 真共享帳先建新 OwnedUtf8 再釋放舊 ResourceId。
    #[test]
    fn g18_v1_f4_str欄位寫入事件重複插入不得假裝二次提交() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let i = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).expect("真欄位寫入事件");
        let duplicate = out.events[i].clone();
        out.events.insert(i, duplicate);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_str欄位寫入確認遺失不許冒充正常退出() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        out.events.retain(|event| !matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        ));
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_str欄位寫入確認早於新resource綁定必拒絕() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        let i = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).expect("真寫入確認");
        let j = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { resource_id: 2, .. }
        )).expect("新資源真綁定");
        out.events.swap(i, j);
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_for兩輪重用原DeclarationId仍逐輪驗真str欄位() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar total:int = 0\nfor i in 0..2{\nvar item:Archive = Archive(body=\"初值\")\nitem.body = \"覆寫🪼\"\ntotal = total+item.count\n}\nreturn total\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
            R::Created(3), R::Created(4), R::Released(3), R::Released(4),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_具名varstr欄位真字面覆寫建立新id釋放舊id() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 7\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"舊🪼\")\nitem.body = \"新🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1), R::Released(2),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructStrFieldAssigned(_, field, 2)
                if field == "body"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_同一varstr欄位雙次更新每id只真釋放一次() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 3\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"初始\")\nitem.body = \"第一次\"\nitem.body = \"第二次🪼\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(1),
            R::Created(3), R::Released(2), R::Released(3),
        ]);
    }

    #[test]
    fn g18_v1_f4_if真分支更新varstr假分支零新資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 5\n}\nfn run(flag:bool)->int{\nvar item:Archive = Archive(body=\"原值\")\nif(flag){\nitem.body = \"選中🪼\"\n}\nreturn item.count\n}\nfn main()->int{\nreturn run(false)+run(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Released(1),
            R::Created(2), R::Created(3), R::Released(2), R::Released(3),
        ]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructStrFieldAssigned(..)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_conststr欄位寫入由原Checker拒絕不得產生資源() {
        let src = "struct Archive{\nconst label:str = \"不可寫\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive()\nitem.label = \"非法\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        assert!(PreparedPrivateScalarCall::prepare(src).is_err());
    }

    #[test]
    fn g18_v1_f4_竄改str欄位寫入後resourceid不得通過退出稽核() {
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"原值\")\nitem.body = \"新值\"\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(1));
        assert!(out.has_balanced_frame_trace());
        let id = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::CopyStructStrFieldAssigned(_, _, resource_id) =>
                Some(resource_id),
            _ => None,
        }).expect("原 Checker 真 str 欄位寫入身分");
        *id = 999;
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_具名varstr欄位以真utf8覆蓋未執行預設() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\nvar data:str = \"未選預設\"\n}\nfn run()->int{\nvar item:Archive = Archive(data=\"具名🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(event,
            PrivateScalarCallEvent::CopyStructResourceOwnerBound { field, resource_id: 1, .. }
                if field == "data"
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_雙具名varstr欄位由同一真帳逆序離域() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar first:str = \"預設甲\"\nvar second:str = \"預設乙\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive(first=\"甲🪼\",second=\"乙🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr和conststr交錯仍共用唯一resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst header:str = \"固定\"\nvar body:str = \"舊值\"\nvar count:int = 3\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"新值\")\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
    }

    #[test]
    fn g18_v1_f4_具名varstr結構完成前先等待int函式真返回() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"未選\"\nvar count:int = 1\n}\nfn increase(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"動態🪼\",count=increase(5))\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(6)
        )).expect("具名 int 的真函式已返回");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("P16 真建構");
        assert!(returned < constructed);
    }

    #[test]
    fn g18_v1_f4_具名varstr建立後溢位仍須單一真資源回收() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar body:str = \"預設\"\nvar count:int = 1\n}\nfn run()->int{\nvar item:Archive = Archive(body=\"回滾🪼\")\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
    }

    // V1/F4：可寫預設 str 與具名欄位共用唯一 P16 Shared Ledger；
    // 明確驗證原 Checker 全欄位依序求值、唯一 ResourceId 與故障退回。
    #[test]
    fn g18_v1_f4_兩個未覆蓋varstr預設依宣告次序建立並逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"甲🪼\"\nvar second:str = \"乙🪼\"\nvar count:int = 12\n}\nfn read()->int{\nvar item:Parcel = Parcel()\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_具名str覆蓋預設與另一varstr預設只建立兩筆真資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"舊預設不能執行\"\nvar second:str = \"合法預設\"\nvar count:int = 5\n}\nfn read()->int{\nvar item:Parcel = Parcel(first=\"具名替代🪼\")\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_建立預設varstr後真整數溢位必回滾唯一資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Parcel{\nvar first:str = \"回滾🪼\"\nvar count:int = 1\n}\nfn fail()->int{\nvar item:Parcel = Parcel()\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn fail()\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert!(out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted,
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_),
        )));
        assert!(plan.audit_graph_outcome(out).is_ok());
    }

    #[test]
    fn g18_v1_f4_真預設str建構函式編譯計畫被刪除須零執行拒絕() {
        let src = "struct Parcel{\nvar first:str = \"合法🪼\"\nvar count:int = 3\n}\nfn read()->int{\nvar item:Parcel = Parcel()\nreturn item.count\n}\nfn main()->int{\nreturn read()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(3));
        plan.root_local_steps.clear();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_f4_未具名varstr預設由p16真建立與釋放() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar body:str = \"未授權預設\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);

    }

    #[test]
    fn g18_v1_f4_conststr具名int函式真返回後才建立資源結構() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 0\nconst label:str = \"真資源🪼\"\n}\nfn plus(x:int)->int{\nreturn x+1\n}\nfn run()->int{\nvar item:Archive = Archive(count=plus(6))\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        let inner_return = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::Returned(7)
        )).expect("真 plus 返回");
        let constructed = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).expect("真 P16 結構建立");
        assert!(inner_return < constructed);
    }

    #[test]
    fn g18_v1_f4_單一結構兩個conststr欄位唯一id各自逆序釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nconst first:str = \"甲\"\nconst second:str = \"乙🪼\"\nvar count:int = 2\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(2));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructResourceOwnerBound { .. }
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_結構欄位resourceid竄改不能通過同一退出稽核() {
        let src = "struct Archive{\nvar count:int = 3\nconst label:str = \"真身分\"\n}\nfn run()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn run()\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        let link = out.events.iter_mut().find_map(|event| match event {
            PrivateScalarCallEvent::CopyStructResourceOwnerBound { resource_id, .. } =>
                Some(resource_id),
            _ => None,
        }).expect("真結構字串 ResourceId 連結");
        *link = 999;
        assert!(!out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_v1_f4_conststr結構共用唯一真resourceid與逆序退出() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"繁體🪼\"\n}\nfn idle()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_兩個conststr結構唯一id按完成反序清理() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"預設\"\n}\nfn idle()->int{\nvar first:Archive = Archive(label=\"甲🪼\")\nvar second:Archive = Archive(label=\"乙🪼\")\nreturn first.count+second.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Created(2),
            crate::owned_str_runtime::ResourceEvent::Released(2),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 2);
    }

    #[test]
    fn g18_v1_f4_conststr結構建立後整數溢位應釋放真資源() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"錯誤清理🪼\"\n}\nfn idle()->int{\nvar item:Archive = Archive()\nreturn item.count+9223372036854775807\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_未選分支conststr結構零建立零清理() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"不可達🪼\"\n}\nfn chosen()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn chosen()\n}\nreturn 4\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "chosen"
        )));
    }

    #[test]
    fn g18_v1_f4_跨兩函式conststr結構各用獨立resourceid() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"重入🪼\"\n}\nfn first()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn second()->int{\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn first()+second()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
            crate::owned_str_runtime::ResourceEvent::Created(2),
            crate::owned_str_runtime::ResourceEvent::Released(2),
        ]);
    }

    #[test]
    fn g18_v1_f4_同層普通str與含str結構交錯安全拒絕且資源回收() {
        let src = "struct Archive{\nvar count:int = 7\nconst label:str = \"安全限制\"\n}\nfn idle()->int{\nvar title:str = \"先建立🪼\"\nvar item:Archive = Archive()\nreturn item.count\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StateMismatch));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, vec![
            crate::owned_str_runtime::ResourceEvent::Created(1),
            crate::owned_str_runtime::ResourceEvent::Released(1),
        ]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
    }

    #[test]
    fn g18_v1_f4_原v0可達純int結構局部真建構並反序離域() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["route", "idle"]);
    }

    #[test]
    fn g18_v1_f4_未選分支純結構與資源均零建構() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )));
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_f4_純結構建構後int溢位逐框架離域不留資源() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 9223372036854775807+1\n}\nfn main()->int{\nreturn idle()\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructConstructed(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::CopyStructScopeEnded(_)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn g18_v1_f4_可達普通函式真建立預設str不虛構雙帳() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"保留🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn main()->int{\nreturn idle()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);

    }

    #[test]
    fn g18_r01_main可達預設str結構函式真正執行() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn main()->int{\nreturn idle()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(91));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_r01_main不可達來源圖被手工加入未支援函式須拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar record:Archive = Archive()\nreturn 91\n}\nfn chosen()->int{\nreturn 4\n}\nfn other()->int{\nreturn 5\n}\nfn main()->int{\nif(true){\nreturn chosen()\n}else{\nreturn other()\n}\nreturn idle()\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.callee_name = "idle".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R01：已核准的普通函式亦可能在所有葉分支已返回後
    // 保留不可達尾端呼叫；不能把它當成必要動態子圖。
    #[test]
    fn g18_r01_普通函式兩葉return後不可達結構函式不污染圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 5\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.callee_name, "route");
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        assert!(matches!(prepared.return_expr, PrivateScalarReturnExpr::Unreachable));
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_r03_多層普通函式選false真返回不能進不可達tail() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn first()->int{\nreturn 3\n}\nfn second()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn first()\n}else{\nreturn second()\n}\nreturn idle()\n}\nfn bridge(flag:bool)->int{\nreturn route(flag)\n}\nfn main()->int{\nreturn bridge(false)\n}\n";
        let plan = checked(src);
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let entered = out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["bridge", "route", "second"]);
    }

    #[test]
    fn g18_r04_普通函式提前return真str借用不可達結構零資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn inspect(item:str,n:int)->int{\nreturn n+1\n}\nfn fallback()->int{\nreturn 8\n}\nfn route()->int{\nif(true){\nvar owner:str = \"內🪼\"\nreturn inspect(owner,5)\n}else{\nreturn fallback()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route()\n}\n";
        let plan = checked(src);
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::LoanReturned(_)
        )).count(), 1);
        assert!(!out.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    // V1／F4：原 Checker 合法的零迭代本體不在執行圖內。
    // 只可以略過真正不可達的 work；有正迭代的結構局部仍須拒絕。
    // V1/F3：跨三層普通函式的子呼叫集合來自同一份
    // 原 Checker 已簽發的工作計畫，零迭代中的合法構造函式
    // 不可回流成新的子函式依賴。
    #[test]
    fn g18_v1_三層真工作計畫只收錄動態可能呼叫的函式() {
        let source = "struct Archive{\nvar count:int = 7\n}\nfn hidden()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn leaf()->int{\nreturn 9\n}\nfn mid()->int{\nfor i in 0..0{\nvar phantom:int = hidden()\n}\nreturn leaf()\n}\nfn top()->int{\nreturn mid()\n}\nfn main()->int{\nreturn top()\n}\n";
        let plan = checked(source);
        assert_eq!(plan.callee_name, "top");
        let names: Vec<_> = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect();
        assert_eq!(names, ["leaf", "mid"]);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "hidden"
        )));
    }

    #[test]
    fn g18_v1_普通函式工作計畫須保留兩個動態可能的分支呼叫() {
        let source = "fn left()->int{\nreturn 6\n}\nfn right()->int{\nreturn 8\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn 0\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let plan = checked(source);
        let names: Vec<_> = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect();
        assert_eq!(names, ["left", "right"]);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "left"
        )));
    }

    #[test]
    fn g18_v1_工作計畫閉包竄改巢狀被呼叫者不得通過來源稽核() {
        let source = "fn leaf()->int{\nreturn 9\n}\nfn mid()->int{\nreturn leaf()\n}\nfn top()->int{\nreturn mid()\n}\nfn main()->int{\nreturn top()\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(9));
        plan.nested_callees.pop();
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_v1_原checker空for本體結構函式不污染可達呼叫閉包() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..0{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_原checker_false_while結構函式不污染可達呼叫閉包() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nwhile(false){\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        assert!(!prepared.nested_callees.iter().any(|c| c.name == "idle"));
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert!(result.has_balanced_frame_trace());
        assert!(!result.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_正迭代for真呼叫預設str結構函式() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..1{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_空迭代閉包被竄改新增可達目標須零執行拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route()->int{\nfor i in 0..0{\nvar result:int = idle()\n}\nreturn good()\n}\nfn main()->int{\nreturn route()\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
        prepared.nested_callees.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    // V1／F3：同一原 Parser／Checker 已證的 Return 須切斷後續工作圖。
    // 不可達函式不得占用 frame／OwnedStr ResourceId，也不可把合法
    // 但未接通的結構執行路徑誤判成此選中分支的必要功能。

    // V1／F3：root main 也須由原 Checker 已編譯的控制工作圖
    // 列出可能的子呼叫；不再以原 AST 遞迴掃描推定執行可達。

    #[test]
    fn g18_v1_main空for內可達性被原Checker排除不污染呼叫圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn main()->int{\nfor i in 0..0{\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|entry| entry.name.as_str()).collect::<Vec<_>>();
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main_false_while內結構函式零建構且返回真目標() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 11\n}\nfn main()->int{\nwhile(false){\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|entry| entry.name.as_str()).collect::<Vec<_>>();
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main真迭代可達結構函式實際進框架() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar local:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn main()->int{\nfor i in 0..1{\nvar phantom:int = idle()\n}\nreturn good()\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_頂層普通函式全葉返回後局部不可達不再錯編() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 8\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nconst skipped:int = idle()\nreturn 0\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        assert!(matches!(&plan.return_expr, PrivateScalarReturnExpr::Unreachable));
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.resource_events.is_empty());
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_main全葉返回後局部不可達不再污染首入口() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 8\n}\nfn main()->int{\nif(false){\nreturn left()\n}else{\nreturn right()\n}\nconst skipped:int = idle()\nreturn 0\n}\n";
        let plan = checked(src);
        assert_eq!(plan.callee_name, "left");
        assert!(matches!(&plan.main_dynamic_call,
            Some(PrivateScalarReturnExpr::Unreachable)));
        assert!(!plan.nested_callees.iter().any(|c| c.name == "idle"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert!(outcome.has_balanced_frame_trace());
        assert!(outcome.resource_events.is_empty());
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_頂層非全葉返回仍真執行可達str結構局部() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nconst live:int = idle()\nreturn live\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_v1_區塊已返回後結構函式不得進入工作圖() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\nvar skipped:int = idle()\n}\nreturn 2\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect::<Vec<_>>();
        for target in ["good"] {
            assert!(names.contains(&target), "已核准的可達呼叫不能遺失：{target}");
        }
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_巢狀全分支返回後不可達建構不得污染兄弟返回() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 5\n}\nfn right()->int{\nreturn 8\n}\nfn fallback()->int{\nreturn 12\n}\nfn route(flag:bool)->int{\nif(flag){\nif(false){\nreturn left()\n}else{\nreturn right()\n}\nvar skipped:int = idle()\n}\nreturn fallback()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let plan = checked(src);
        let names = plan.nested_callees.iter()
            .map(|callee| callee.name.as_str()).collect::<Vec<_>>();
        for target in ["left","right","fallback"] {
            assert!(names.contains(&target), "已核准的可達呼叫不能遺失：{target}");
        }
        assert!(!names.contains(&"idle"));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.has_balanced_frame_trace());
        assert!(out.resource_events.is_empty());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));
    }

    #[test]
    fn g18_v1_未知條件編譯所有可達str來源而只執行選中邊() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar x:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 6\n}\nfn route(flag:bool)->int{\nif(flag){\nvar value:int = idle()\nreturn value\n}\nreturn good()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, []);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )));

    }

    #[test]
    fn g18_r01_原v0可達預設str結構函式真返回() {
        // 原 Parser／Checker 真來源在 P16 同一 OwnedStr ledger 上執行。
        use crate::owned_str_runtime::ResourceEvent as R;
                let src = "struct Archive{\nvar count:int = 7\nvar resource:str = \"待整合🪼\"\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn good()->int{\nreturn 7\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn good()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(false)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(99));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [R::Created(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "idle"
        )).count(), 1);

    }

    #[test]
    fn g18_r01_普通函式不可達閉包計畫被增添目標須在執行前拒絕() {
        let src = "struct Archive{\nvar count:int = 7\n}\nfn idle()->int{\nvar r:Archive = Archive()\nreturn 99\n}\nfn left()->int{\nreturn 4\n}\nfn right()->int{\nreturn 5\n}\nfn route(flag:bool)->int{\nif(flag){\nreturn left()\n}else{\nreturn right()\n}\nreturn idle()\n}\nfn main()->int{\nreturn route(true)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        plan.nested_callees.clear();
        assert!(matches!(plan.execute(), Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    // R05：真回傳與故障退出均按原顯式工作堆疊反向還票。
    // 平衡但錯序的事件不能被當成 Checker 已核准執行證據。
    #[test]
    fn g18_r05_雙str實參真借用必反序歸還且不提前釋放資源() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst left:str = \"甲🪼\"\nconst right:str = \"乙🪼\"\nreturn inspect(left,right,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn g18_r05_雙str正常返回交換兩張票據歸還次序必拒絕() {
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst left:str = \"甲\"\nconst right:str = \"乙\"\nreturn inspect(left,right,5)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let returns: Vec<_> = out.events.iter().enumerate()
            .filter_map(|(index, event)| matches!(
                event, PrivateScalarCallEvent::LoanReturned(_)
            ).then_some(index)).collect();
        assert_eq!(returns.len(), 2);
        out.events.swap(returns[0], returns[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_雙str實參後第三引數溢位反序歸還再釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"第一\"\nconst second:str = \"第二\"\nreturn inspect(first,second,1+9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Released(2), R::Released(1),
        ]);
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::UnwindStarted)));
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inspect"
        )));
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn g18_r05_失敗回滾雙str平衡但錯序還票仍必拒絕() {
        let src = "fn inspect(a:str,b:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst first:str = \"第一\"\nconst second:str = \"第二\"\nreturn inspect(first,second,1+9223372036854775807)\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let returns: Vec<_> = out.events.iter().enumerate()
            .filter_map(|(index, event)| matches!(
                event, PrivateScalarCallEvent::LoanReturned(_)
            ).then_some(index)).collect();
        assert_eq!(returns.len(), 2);
        out.events.swap(returns[0], returns[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_跨子呼叫前後的兩張借用票據各按真返回逆序歸還() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+1\n}\nfn parent(s:str,n:int)->int{\nreturn n+2\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,3))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let acquired: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        let returned: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        let entered: Vec<_> = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(entered, ["leaf", "parent"]);
    }

    // G18 R05：覆蓋 BorrowSafety 的獨立子集原子交易，
    // 不擴增 F00 已凍結的 borrow_safety::tests:: 原 24 項來源。
    #[test]
    fn g18_r05_巢狀函式子集歸票不能提前解除外層活躍票據() {
        let mut tracker = BorrowSafety::new();
        let outer = DeclarationId(8041);
        let inner = DeclarationId(8042);
        let first = tracker.acquire(outer, BorrowKind::SharedRead).unwrap();
        let second = tracker.acquire(inner, BorrowKind::SharedRead).unwrap();
        tracker.finish_selected_verified(&[second], &[second.fact()]).unwrap();
        assert_eq!(
            tracker.check_direct_access(outer),
            Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed),
        );
        assert_eq!(tracker.check_direct_access(inner), Ok(()));
        assert_eq!(
            tracker.check_ticket_access(first, BorrowKind::SharedRead), Ok(outer),
        );
        tracker.finish_scope_verified(&[first], &[first.fact()]).unwrap();
        assert_eq!(tracker.check_direct_access(outer), Ok(()));
    }

    #[test]
    fn g18_r05_子集歸票靜態來源失配必原子拒絕保留所有票據() {
        let mut tracker = BorrowSafety::new();
        let first = tracker.acquire(
            DeclarationId(8061), BorrowKind::SharedRead,
        ).unwrap();
        let second = tracker.acquire(
            DeclarationId(8062), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(
            tracker.finish_selected_verified(&[second], &[first.fact()]),
            Err(crate::borrow_safety::BorrowSafetyError::SnapshotMismatch),
        );
        assert_eq!(
            tracker.finish_selected_verified(&[second], &[]),
            Err(crate::borrow_safety::BorrowSafetyError::SnapshotMismatch),
        );
        for ticket in [first, second] {
            assert_eq!(
                tracker.check_ticket_access(ticket, BorrowKind::SharedRead),
                Ok(ticket.fact().source),
            );
        }
        tracker.finish_scope_verified(
            &[first, second], &[first.fact(), second.fact()],
        ).unwrap();
    }

    #[test]
    fn g18_r05_子集重複票據不得部分歸還其餘合法借用() {
        let mut tracker = BorrowSafety::new();
        let original = tracker.acquire(
            DeclarationId(8081), BorrowKind::SharedRead,
        ).unwrap();
        let remaining = tracker.acquire(
            DeclarationId(8082), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(
            tracker.finish_selected_verified(
                &[original, original], &[original.fact(), original.fact()],
            ),
            Err(crate::borrow_safety::BorrowSafetyError::InvalidRelease),
        );
        assert_eq!(
            tracker.check_ticket_access(original, BorrowKind::SharedRead),
            Ok(original.fact().source),
        );
        assert_eq!(
            tracker.check_ticket_access(remaining, BorrowKind::SharedRead),
            Ok(remaining.fact().source),
        );
        tracker.finish_scope_verified(
            &[original, remaining], &[original.fact(), remaining.fact()],
        ).unwrap();
    }



    // R05：外層未 Enter 的借用票據不能先於正在退出的內層
    // 借用／資源返還。只有真正已進入的 frame 按由內向外順序卸責。
    #[test]
    fn g18_r05_外層待呼叫借用與內層函式溢位須先還內票再還外票() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(s:str,n:int)->int{\nvar local:str = \"子框架🪼\"\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外票\"\nconst inner:str = \"內票\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Created(3),
            R::Released(3), R::Released(2), R::Released(1),
        ]);
        let acquired = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanAcquired(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(acquired.len(), 2);
        assert_eq!(returned, acquired.into_iter().rev().collect::<Vec<_>>());
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "parent"
        )));
    }

    #[test]
    fn g18_r05_內層真move溢位先清理子資源外層待借用才歸還() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn leaf(item:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外票\"\nvar inner:str = \"移動🪼\"\nreturn parent(outer,leaf(inner.move(),1))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.resource_events, [
            R::Created(1), R::Created(2), R::Moved(2),
            R::Released(2), R::Released(1),
        ]);
        let child_exit = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false,
            } if name == "leaf"
        )).unwrap();
        let loan_returned = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::LoanReturned(_)
        )).unwrap();
        assert!(child_exit < loan_returned);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "parent"
        )));
    }

    #[test]
    fn g18_r05_巢狀回滾交換內外票據歸還順序須拒絕() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().enumerate().filter_map(|(index, event)|
            matches!(event, PrivateScalarCallEvent::LoanReturned(_)).then_some(index)
        ).collect::<Vec<_>>();
        assert_eq!(returned.len(), 2);
        out.events.swap(returned[0], returned[1]);
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(PreparedPrivateScalarCall::audit_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }

    #[test]
    fn g18_r05_不在任何框架內的偽造待歸票不得掩蓋溢位() {
        let src = "fn leaf(s:str,n:int)->int{\nreturn n+9223372036854775807\n}\nfn parent(s:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst outer:str = \"外\"\nconst inner:str = \"內\"\nreturn parent(outer,leaf(inner,1))\n}\n";
        let mut out = checked(src).execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        // 偽造歸票即使附在正式 UnwindStarted 後也沒有真簽發來源，
        // 應由共用票據及來源雙帳同時拒絕。
        let token = out.events.iter().find_map(|event| match event {
            PrivateScalarCallEvent::LoanReturned(ticket) => Some(*ticket),
            _ => None,
        }).expect("必有真內層票據");
        let unwind = out.events.iter().position(|event| matches!(
            event, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        out.events.insert(unwind + 1, PrivateScalarCallEvent::LoanReturned(token));
        assert!(!out.has_balanced_frame_trace());
    }


}
