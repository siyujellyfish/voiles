#[cfg(test)]
mod g18_v7_terminal_task_retirement_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"終態資源帳不可丟棄🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )).expect("必須來自原 Parser／Checker 核准的真正資源來源")
    }

    #[test]
    fn g18_v7_e3_完成與預算故障退休必移交R05原結果且工作身分不回繞() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let success = queue.submit(&plan).unwrap();
        let failed = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((success, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((failed, PrivateScalarGraphPoll::Completed)));
        queue.wake(success).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((success, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.jobs.len(), 2);

        let PrivateScalarRetiredJob::Completed(failure) =
            queue.retire_terminal(failed).unwrap() else {
                panic!("受控故障必保留真錯誤帳");
            };
        assert_eq!(failure.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(failure.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &failure.resource_events,
        ));
        let PrivateScalarRetiredJob::Completed(ok) =
            queue.retire_terminal(success).unwrap() else {
                panic!("正常完成必保留真結果");
            };
        assert_eq!(ok.result, Ok(11));
        assert!(ok.has_balanced_frame_trace());
        assert!(matches!(ok.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.status(success), None);
        assert!(matches!(queue.retire_terminal(success),
            Err(PrivateScalarCooperativeError::UnknownTask)));
        assert_eq!(queue.drive_order().len(), 3);
        let fresh = queue.submit(&plan).unwrap();
        assert_eq!(fresh, failed + 1);
        assert_eq!(queue.shutdown().unwrap(), [fresh]);
    }

    #[test]
    fn g18_v7_e3_準備與暫停不得退休取消後真ResourceId證據隨工作移交() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let suspended = queue.submit(&plan).unwrap();
        let prepared = queue.submit(&plan).unwrap();
        assert!(matches!(queue.retire_terminal(prepared),
            Err(PrivateScalarCooperativeError::WrongState)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((suspended, PrivateScalarGraphPoll::Suspended)));
        assert!(matches!(queue.retire_terminal(suspended),
            Err(PrivateScalarCooperativeError::WrongState)));
        assert_eq!(queue.ready_ids(), [prepared]);
        queue.cancel(suspended).unwrap();
        queue.cancel(prepared).unwrap();

        let PrivateScalarRetiredJob::Cancelled(Some(cancelled)) =
            queue.retire_terminal(suspended).unwrap() else {
                panic!("真已執行框架的取消必交付原資源事件");
            };
        assert!(matches!(cancelled.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert!(!cancelled.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(matches!(queue.retire_terminal(prepared).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        assert!(queue.ready_ids().is_empty());
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.cancellation_order(), &[suspended, prepared]);
        assert_eq!(queue.drive_one().unwrap(), None);
    }

    #[test]
    fn g18_v7_e3_未有原R05驗證的Failed不得被退休以掩蓋失敗() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        // 模擬引擎在正式結果產生之前即被阻斷：
        // Failed 標籤不得冒充已產生成功／失敗退出帳。
        queue.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Failed;
        assert!(matches!(queue.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(queue.ready_ids(), [id]);
        assert_eq!(queue.jobs.len(), 1);
        assert!(queue.outcome(id).is_none());
    }
}

#[cfg(test)]
mod g18_v7_cumulative_budget_tests {
    use super::*;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("必須是原 Parser／Checker 核准來源，不藉新測試放寬文法")
    }

    fn owned_source() -> &'static str {
        concat!(
            "async fn ready(n:int)->int{ return n+1 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"同一預算真持有者🪼\"\n",
            "return await ready(7)\n",
            "}\n",
            "fn main()->int{ return wrapper() }\n",
        )
    }

    #[test]
    fn g18_v7_d1801_原真已執行工作跨await總步數不因wake重置() {
        let plan = checked(owned_source());
        let mut reference = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let consumed = reference.checkpoint.as_ref().unwrap().steps;
        assert!(consumed > 0);
        // 在原預先測得的「抵達 await 前」真正已執行步數上，
        // 僅再允許一個 EnterCall；後續子函式真正步驟必受控拒絕。
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 128, max_steps: consumed + 1,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(
            queue.jobs[&id].continuation.checkpoint.as_ref().unwrap().steps,
            consumed,
        );
        let before = queue.retained_resources(id).unwrap().to_vec();
        assert!(matches!(before.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "wrapper",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_d1801_兩個真正await的第二個安全點與第三次poll仍共用總步數() {
        let plan = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{\n",
            "var hold:str = \"多安全點同一預算🪼\"\n",
            "return await first()+await second()\n",
            "}\n",
        ));
        let mut reference = PrivateScalarGraphContinuation::new(&plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let first_steps = reference.checkpoint.as_ref().unwrap().steps;
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let second_steps = reference.checkpoint.as_ref().unwrap().steps;
        assert!(second_steps > first_steps);

        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 128, max_steps: second_steps + 1,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(queue.pending_await(id), Some("second"));
        assert_eq!(
            queue.jobs[&id].continuation.checkpoint.as_ref().unwrap().steps,
            second_steps,
        );
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(outcome.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.resource_events));
        assert_eq!(outcome.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "first",
        )).count(), 1);
        assert_eq!(queue.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Completed),
        ]);
    }

    #[test]
    fn g18_v7_d1801_同一原Checker來源兩份FIFO工作各自獨立預算與資源帳() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(owned_source()).unwrap();
        let plan = catalog.plan(program).unwrap();
        let mut reference = PrivateScalarGraphContinuation::new(plan).unwrap();
        assert_eq!(reference.poll().unwrap(), PrivateScalarGraphPoll::Suspended);
        let steps = reference.checkpoint.as_ref().unwrap().steps;

        let mut queue = PrivateScalarCooperativeQueue::new();
        let limited = queue.submit_program_with_budget(
            &catalog, program, PrivateScalarExecutionBudget {
                max_call_frames: 128, max_steps: steps + 1,
            },
        ).unwrap();
        let normal = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((limited, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((normal, PrivateScalarGraphPoll::Suspended)));
        queue.wake(limited).unwrap();
        queue.wake(normal).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((limited, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(),
            Some((normal, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(limited).unwrap().result,
            Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(queue.outcome(normal).unwrap().result, Ok(8));
        for id in [limited, normal] {
            let result = queue.outcome(id).unwrap();
            assert!(result.has_balanced_frame_trace());
            assert!(matches!(result.resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(x),
                crate::owned_str_runtime::ResourceEvent::Released(y),
            ] if x == y));
        }
        assert_eq!(queue.ready_ids(), []);
    }

    #[test]
    fn g18_v7_d1801_零環境預算與未知登錄身分不消耗FIFO工作ID() {
        let plan = checked(owned_source());
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(owned_source()).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        let before = queue.ready_ids();
        for budget in [
            PrivateScalarExecutionBudget { max_call_frames: 0, max_steps: 100 },
            PrivateScalarExecutionBudget { max_call_frames: 100, max_steps: 0 },
        ] {
            assert_eq!(queue.submit_with_budget(&plan, budget),
                Err(PrivateScalarCooperativeError::Engine));
            assert_eq!(queue.ready_ids(), before);
            assert_eq!(queue.jobs.len(), 1);
        }
        assert_eq!(queue.submit_program_with_budget(
            &catalog, program + 500, PrivateScalarExecutionBudget::default(),
        ), Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.ready_ids(), before);
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.cancel(id).unwrap();
        assert_eq!(queue.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
        ]);
    }

    #[test]
    fn g18_v7_d1801_函式框架環境預算限制在真正await前拒絕且釋放父資源() {
        let plan = checked(owned_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        assert_eq!(queue.pending_await(id), None);
        let outcome = queue.outcome(id).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(outcome.has_balanced_frame_trace());
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(x),
            crate::owned_str_runtime::ResourceEvent::Released(y),
        ] if x == y));
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::AwaitSafePoint(_),
        )));
    }
}

#[cfg(test)]
mod g18_v7_loop_body_await_checkpoint_tests {
    use super::*;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("必須為原 Lexer／Parser／Checker 已核准的原有 await／迴圈組合")
    }

    fn for_source() -> &'static str {
        concat!(
            "async fn tick(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var total:int = 0\n",
            "for i in 0..3{\n",
            "var local:str = \"每輪真資源🪼\"\n",
            "total = total+await tick(i)\n",
            "}\n",
            "return total\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        )
    }

    #[test]
    fn g18_v7_e2_原Checker_for迴圈本體三次真await逐輪checkpoint而不重複建立資源() {
        let plan = checked(for_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        for iteration in 0..3 {
            assert_eq!(queue.drive_one().unwrap(), Some((
                id, PrivateScalarGraphPoll::Suspended,
            )));
            assert_eq!(queue.pending_await(id), Some("tick"));
            let events = queue.retained_resources(id).unwrap();
            assert_eq!(events.iter().filter(|e| matches!(
                e, crate::owned_str_runtime::ResourceEvent::Created(_),
            )).count(), iteration + 1);
            assert_eq!(events.iter().filter(|e| matches!(
                e, crate::owned_str_runtime::ResourceEvent::Released(_),
            )).count(), iteration);
            assert_eq!(queue.drive_one().unwrap(), None);
            queue.wake(id).unwrap();
        }
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 3);
        assert_eq!(out.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "tick",
        )).count(), 3);
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "worker",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_for真第二輪await取消只清理當前局部而不重建前輪() {
        let plan = checked(for_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        let before = queue.retained_resources(id).unwrap();
        assert_eq!(before.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 2);
        assert_eq!(before.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 1);
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        let evidence = queue.cancellation(id).unwrap();
        assert!(crate::owned_str_runtime::event_trace_balanced(&evidence.resource_events));
        assert_eq!(evidence.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_),
        )).count(), 2);
        assert_eq!(evidence.resource_events.iter().filter(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Released(_),
        )).count(), 2);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "tick",
        )).count(), 1);
        assert_eq!(evidence.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "tick",
        )).count(), 1);
        assert!(queue.drive_order().iter().all(|(task, _)| *task == id));
    }

    #[test]
    fn g18_v7_e2_原Checker_while本體await後continue真局部資源不重建() {
        let plan = checked(concat!(
            "async fn next(n:int)->int{ return n+2 }\n",
            "fn worker()->int{\n",
            "var total:int = 4\n",
            "var active:bool = true\n",
            "while(active){\n",
            "var local:str = \"while真輪🪼\"\n",
            "active = false\n",
            "total = await next(total)\n",
            "continue\n",
            "}\n",
            "return total\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        assert_eq!(queue.pending_await(id), Some("next"));
        assert!(matches!(queue.retained_resources(id), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Completed,
        )));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert!(matches!(out.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(out.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "next",
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_原Checker_while本體await被取消後不執行後續continue或再迭代() {
        let plan = checked(concat!(
            "async fn next()->int{ return 5 }\n",
            "fn worker()->int{\n",
            "var active:bool = true\n",
            "while(active){\n",
            "var local:str = \"未退出的真局部🪼\"\n",
            "active = false\n",
            "var computed:int = await next()\n",
            "continue\n",
            "}\n",
            "return 7\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((
            id, PrivateScalarGraphPoll::Suspended,
        )));
        let original = queue.retained_resources(id).unwrap().to_vec();
        assert!(matches!(original.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ]));
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        let cancellation = queue.cancellation(id).unwrap();
        assert!(matches!(cancellation.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert!(!cancellation.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::FunctionEntered(name) if name == "next",
        )));
        assert_eq!(queue.drive_one().unwrap(), None);
        assert!(queue.outcome(id).is_none());
    }
}

#[cfg(test)]
mod g18_v7_cooperative_shutdown_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(source: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(source)
            .expect("原 Parser／Checker 真正核准的 V7 來源")
    }

    fn owned_parent_source() -> &'static str {
        concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"真正已建立的持有者🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )
    }

    #[test]
    fn g18_v7_e3_三工作安全關閉須逆登錄順序取消且各自真資源歸零() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&plan).unwrap();
        let c = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.ready_ids(), [c]);
        assert!(queue.retained_resources(a).unwrap().iter().any(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )));
        assert!(queue.retained_resources(b).unwrap().iter().any(|e| matches!(
            e, crate::owned_str_runtime::ResourceEvent::Created(_)
        )));
        assert_eq!(queue.shutdown().unwrap(), [c, b, a]);
        assert_eq!(queue.cancellation_order(), &[c, b, a]);
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.drive_order(), &[
            (a, PrivateScalarGraphPoll::Suspended),
            (b, PrivateScalarGraphPoll::Suspended),
        ]);
        assert_eq!(queue.drive_one().unwrap(), None);
        for id in [a, b] {
            assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
            assert!(queue.outcome(id).is_none());
            assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(first),
                crate::owned_str_runtime::ResourceEvent::Released(last),
            ] if first == last));
        }
        assert!(queue.cancellation(c).is_none());
        assert_eq!(queue.shutdown().unwrap(), []);
        assert_eq!(queue.cancellation_order(), &[c, b, a]);
    }

    #[test]
    fn g18_v7_e3_任何待關閉工作仍持真BorrowTicket整批預檢必零狀態變更() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [a]);
        let before_a = queue.retained_resources(a).unwrap().to_vec();
        let before_b = queue.retained_resources(b).unwrap().to_vec();
        let tracker = &mut queue.jobs.get_mut(&b).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0].borrow_tracker;
        let ticket = tracker.acquire(
            DeclarationId(1881), BorrowKind::SharedRead,
        ).unwrap();
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [a]);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(a).unwrap(), before_a.as_slice());
        assert_eq!(queue.retained_resources(b).unwrap(), before_b.as_slice());
        queue.jobs.get_mut(&b).unwrap().continuation.checkpoint.as_mut()
            .unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [b, a]);
        assert_eq!(queue.ready_ids(), []);
        for id in [a, b] {
            assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
                crate::owned_str_runtime::ResourceEvent::Created(x),
                crate::owned_str_runtime::ResourceEvent::Released(y),
            ] if x == y));
        }
    }

    #[test]
    fn g18_v7_e3_真遞迴兩層UTF8框架停在await後關閉須內層優先釋放() {
        let plan = checked(concat!(
            "async fn terminal()->int{ return 17 }\n",
            "fn descend(flag:bool)->int{\n",
            "var own:str = \"遞迴每層🪼\"\n",
            "if(flag){\nreturn descend(false)\n}\n",
            "return await terminal()\n}\n",
            "fn main()->int{ return descend(true)+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("terminal"));
        assert_eq!(queue.jobs.get(&id).unwrap().continuation.retained_frames(), 3);
        let retained = queue.retained_resources(id).unwrap();
        let [crate::owned_str_runtime::ResourceEvent::Created(outer),
             crate::owned_str_runtime::ResourceEvent::Created(inner)] = retained else {
            panic!("真正兩層遞迴持有者應已建立且尚未釋放");
        };
        assert_ne!(outer, inner);
        let (outer, inner) = (*outer, *inner);
        assert_eq!(queue.shutdown().unwrap(), [id]);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.cancellation(id).unwrap().resource_events, [
            crate::owned_str_runtime::ResourceEvent::Created(outer),
            crate::owned_str_runtime::ResourceEvent::Created(inner),
            crate::owned_str_runtime::ResourceEvent::Released(inner),
            crate::owned_str_runtime::ResourceEvent::Released(outer),
        ]);
        let evidence = queue.cancellation(id).unwrap();
        assert!(!evidence.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "terminal"
        )));
        assert_eq!(evidence.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false,
            } if name == "descend"
        )).count(), 2);
    }

    #[test]
    fn g18_v7_e2_三次for真UTF8迭代已清理後才允許await暫停並安全關閉() {
        let plan = checked(concat!(
            "async fn done(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var root:str = \"仍持有的根🪼\"\n",
            "var total:int = 0\n",
            "for i in 0..3{\n",
            "var local:str = \"每輪已退出🪼\"\n",
            "total = total+i\n",
            "}\n",
            "return await done(total)\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("done"));
        let state = queue.retained_resources(id).unwrap();
        assert_eq!(state.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 4);
        assert_eq!(state.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 3);
        assert_eq!(queue.shutdown().unwrap(), [id]);
        let events = &queue.cancellation(id).unwrap().resource_events;
        assert_eq!(events.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 4);
        assert_eq!(events.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 4);
        assert!(crate::owned_str_runtime::event_trace_balanced(events));
        assert!(!queue.cancellation(id).unwrap().events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name)
                if name == "done")));
    }

    #[test]
    fn g18_v7_e2_while真動態迴圈清理後等待由同一工作恢復不重跑迭代() {
        let plan = checked(concat!(
            "async fn done(n:int)->int{ return n+1 }\n",
            "fn worker()->int{\n",
            "var root:str = \"while根🪼\"\n",
            "var active:bool = true\n",
            "var total:int = 4\n",
            "while(active){\n",
            "var local:str = \"while本輪🪼\"\n",
            "total = total+2\n",
            "active = false\n",
            "continue\n",
            "}\n",
            "return await done(total)\n",
            "}\n",
            "fn main()->int{ return worker()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("done"));
        let before = queue.retained_resources(id).unwrap();
        assert_eq!(before.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Created(_)
        )).count(), 2);
        assert_eq!(before.iter().filter(|event| matches!(
            event, crate::owned_str_runtime::ResourceEvent::Released(_)
        )).count(), 1);
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        let out = queue.outcome(id).unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.resource_events));
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::AwaitSafePoint(name) if name == "done"
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "worker"
        )).count(), 1);
    }

    #[test]
    fn g18_v7_e3_關閉保留已完成工作真Outcome只取消其餘Ready或Suspended() {
        let sync = checked(concat!(
            "fn plain()->int{ return 8 }\n",
            "fn main()->int{ return plain()+0 }\n",
        ));
        let async_plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let done = queue.submit(&sync).unwrap();
        let suspended = queue.submit(&async_plan).unwrap();
        let not_started = queue.submit(&async_plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((done, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(), Some((suspended, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.ready_ids(), [not_started]);
        assert_eq!(queue.shutdown().unwrap(), [not_started, suspended]);
        assert_eq!(queue.status(done), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(queue.outcome(done).unwrap().result, Ok(8));
        assert!(queue.outcome(done).unwrap().has_balanced_frame_trace());
        assert!(queue.cancellation(done).is_none());
        assert_eq!(queue.status(not_started), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.status(suspended), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.cancellation(not_started).is_none());
        assert!(matches!(queue.cancellation(suspended).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(x),
            crate::owned_str_runtime::ResourceEvent::Released(y),
        ] if x == y));
        assert!(queue.ready_ids().is_empty());
    }

    #[test]
    fn g18_v7_e3_尚有Running工作關閉保守拒絕且保持整批FIFO() {
        let plan = checked(owned_parent_source());
        let mut queue = PrivateScalarCooperativeQueue::new();
        let first = queue.submit(&plan).unwrap();
        let second = queue.submit(&plan).unwrap();
        queue.jobs.get_mut(&second).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Running;
        assert_eq!(queue.shutdown(), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.ready_ids(), [first, second]);
        assert!(queue.cancellation_order().is_empty());
        queue.jobs.get_mut(&second).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Prepared;
        assert_eq!(queue.shutdown().unwrap(), [second, first]);
        assert!(queue.ready_ids().is_empty());
    }

    #[test]
    fn g18_v7_e3_尚未開始的三份來源關閉不得產生任何真Runtime事件() {
        let source = checked(concat!(
            "async fn ready()->int{\n",
            "var resource:str = \"未執行不得建立\"\n",
            "return 14\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&source).unwrap();
        let b = queue.submit(&source).unwrap();
        let c = queue.submit(&source).unwrap();
        assert_eq!(queue.shutdown().unwrap(), [c, b, a]);
        assert!(queue.drive_order().is_empty());
        for id in [a, b, c] {
            assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
            assert!(queue.cancellation(id).is_none());
            assert!(queue.outcome(id).is_none());
            assert!(queue.retained_resources(id).unwrap().is_empty());
        }
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.shutdown().unwrap(), []);
    }
}

#[cfg(test)]
mod g18_v7_checked_program_catalog_tests {
    use super::*;

    fn source(n: i64) -> String {
        format!(
            "async fn ready()->int{{ return {n} }}\n\
             fn main()->int{{ return await ready()+0 }}\n"
        )
    }

    #[test]
    fn g18_v7_a1_原Checker來源由登錄持有且FIFO續行引用同一真計畫身分() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(&source(17)).unwrap();
        assert_eq!(program, 1);
        assert_eq!(catalog.len(), 1);
        let checked_ptr = catalog.plan(program).unwrap() as *const _;
        let mut queue = PrivateScalarCooperativeQueue::new();
        let task = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(task, 1);
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
        assert_eq!(queue.status(task), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(task), Some("ready"));
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
        queue.wake(task).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(task).unwrap().result, Ok(17));
        assert!(queue.outcome(task).unwrap().has_balanced_frame_trace());
        assert!(std::ptr::eq(
            checked_ptr, queue.jobs.get(&task).unwrap().continuation.plan,
        ));
    }

    #[test]
    fn g18_v7_a1_靜態非法來源零身分消耗且原Checker合法bool動態圖完整保留() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let invalid = concat!(
            "async fn ready()->int{ return true }\n",
            "fn main()->int{ return await ready()+0 }\n",
        );
        assert!(matches!(catalog.prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        assert_eq!(catalog.len(), 0);
        // 原 Checker 合法的 bool 參數呼叫已經採真 main_dynamic_call；
        // 不可因為另有 int-only 直接快路的保守界線，誤拒完整動態圖。
        let bool_graph = concat!(
            "fn choose(flag:bool)->int{\n",
            "if(flag){return 4}\nreturn 7\n}\n",
            "fn main()->int{ return choose(true) }\n",
        );
        let flag_id = catalog.prepare(bool_graph).unwrap();
        assert_eq!(flag_id, 1);
        assert!(catalog.plan(flag_id).unwrap().main_dynamic_call.is_some());
        let second = catalog.prepare(&source(6)).unwrap();
        assert_eq!(second, 2);
        assert_eq!(catalog.len(), 2);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let flag_task = queue.submit_program(&catalog, flag_id).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((flag_task, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(flag_task).unwrap().result, Ok(4));
        assert!(queue.outcome(flag_task).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e2_未知來源登錄ID不得消耗FIFO工作身分或其他已核准佇列() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(&source(8)).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        assert_eq!(queue.submit_program(&catalog, 500),
            Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.jobs.len(), 0);
        assert_eq!(queue.ready_ids(), []);
        let task = queue.submit_program(&catalog, program).unwrap();
        assert_eq!(task, 1);
        assert_eq!(queue.submit_program(&catalog, 500),
            Err(PrivateScalarCooperativeError::UnknownProgram));
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.ready_ids(), [task]);
        assert_eq!(queue.drive_one().unwrap(), Some((task, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(task).unwrap();
        assert_eq!(queue.status(task), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn g18_v7_e2_同一份原Checker登錄來源可產生兩份相互獨立的真續行() {
        let source = concat!(
            "async fn ready()->int{ return 33 }\n",
            "fn wrapper()->int{\n",
            "var own:str = \"同源各有真Runtime🪼\"\n",
            "return await ready()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        );
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let program = catalog.prepare(source).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit_program(&catalog, program).unwrap();
        let b = queue.submit_program(&catalog, program).unwrap();
        assert_ne!(a, b);
        assert!(std::ptr::eq(
            queue.jobs.get(&a).unwrap().continuation.plan,
            queue.jobs.get(&b).unwrap().continuation.plan,
        ));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert!(matches!(queue.retained_resources(a), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        assert!(matches!(queue.retained_resources(b), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        queue.cancel(a).unwrap();
        queue.wake(b).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.outcome(a).is_none());
        assert_eq!(queue.outcome(b).unwrap().result, Ok(33));
        assert!(matches!(queue.cancellation(a).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(matches!(queue.outcome(b).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(queue.outcome(b).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e3_登錄來源真溢位及取消分別沿原Runtime清理不偽造成功() {
        let failed = concat!(
            "async fn over(n:int)->int{ return n+1 }\n",
            "fn main()->int{\n",
            "var own:str = \"原溢位也清理🪼\"\n",
            "return await over(9223372036854775807)+0\n}\n",
        );
        let cancelled = concat!(
            "async fn no()->int{ return 9 }\n",
            "fn main()->int{\n",
            "var own:str = \"取消前還在🪼\"\n",
            "return await no()+0\n}\n",
        );
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let failure_program = catalog.prepare(failed).unwrap();
        let cancel_program = catalog.prepare(cancelled).unwrap();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit_program(&catalog, failure_program).unwrap();
        let b = queue.submit_program(&catalog, cancel_program).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        let outcome = queue.outcome(a).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(outcome.has_balanced_frame_trace());
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(matches!(outcome.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(matches!(queue.cancellation(b).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Cancelled));
    }

    #[test]
    fn g18_v7_a1_來源登錄身分耗盡時原計畫不回繞也不污染先前工作() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let original = catalog.prepare(&source(4)).unwrap();
        assert_eq!(original, 1);
        catalog.next_id = u64::MAX;
        assert!(matches!(catalog.prepare(&source(8)),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
        assert_eq!(catalog.len(), 1);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_program(&catalog, original).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(4));
    }

    #[test]
    fn g18_v7_a1_排程器完成後釋放借用原登錄仍能增加下一份真來源() {
        let mut catalog = PrivateScalarCheckedProgramCatalog::new();
        let one = catalog.prepare(&source(5)).unwrap();
        {
            let mut queue = PrivateScalarCooperativeQueue::new();
            let id = queue.submit_program(&catalog, one).unwrap();
            assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
            queue.wake(id).unwrap();
            assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
            assert_eq!(queue.outcome(id).unwrap().result, Ok(5));
        }
        // Rust 編譯期保證：仍被續行借用時 catalog 不可變更；
        // 所有相關工作自然結束／離域後才重新取得唯一可變存取。
        let two = catalog.prepare(&source(12)).unwrap();
        assert_eq!(two, 2);
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit_program(&catalog, two).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(12));
    }
}

#[cfg(test)]
mod g18_v7_cooperative_graph_queue_tests {
    use super::*;
    use crate::ownership_ledger::DeclarationId;

    fn checked(text: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(text)
            .expect("必須使用原 Parser／Checker 真正核准的私有來源")
    }

    #[test]
    fn g18_v7_e2_真動態工作三份FIFO交錯後依明確喚醒次序完成() {
        let left = checked(concat!(
            "async fn left()->int{ return 7 }\n",
            "fn wrapper()->int{\n",
            "var parent:str = \"佇列內原資源🪼\"\n",
            "return await left()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let right = checked(concat!(
            "async fn right()->int{ return 11 }\n",
            "fn main()->int{ return await right()+0 }\n",
        ));
        let sync = checked(concat!(
            "fn simple()->int{ return 19 }\n",
            "fn main()->int{ return simple()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&left).unwrap();
        let b = queue.submit(&right).unwrap();
        let c = queue.submit(&sync).unwrap();
        assert_eq!((a, b, c), (1, 2, 3));
        assert_eq!(queue.ready_ids(), [a, b, c]);
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Prepared));
        assert!(queue.drive_order().is_empty());
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("left"));
        assert!(matches!(queue.retained_resources(a), Some([
            crate::owned_str_runtime::ResourceEvent::Created(_),
        ])));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.drive_one().unwrap(), Some((c, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(c).unwrap().result, Ok(19));
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.drive_one().unwrap(), None);
        assert!(queue.outcome(a).is_none());
        queue.wake(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [b, a]);
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(11));
        let finished = queue.outcome(a).unwrap();
        assert_eq!(finished.result, Ok(7));
        assert!(finished.has_balanced_frame_trace());
        assert!(matches!(finished.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert_eq!(queue.drive_order(), &[
            (a, PrivateScalarGraphPoll::Suspended),
            (b, PrivateScalarGraphPoll::Suspended),
            (c, PrivateScalarGraphPoll::Completed),
            (b, PrivateScalarGraphPoll::Completed),
            (a, PrivateScalarGraphPoll::Completed),
        ]);
    }

    #[test]
    fn g18_v7_e2_多await工作在不同真指令位置反覆yield且其他工作可插入() {
        let multi = checked(concat!(
            "async fn first()->int{ return 2 }\n",
            "async fn second()->int{ return 3 }\n",
            "fn main()->int{ return await first()+await second() }\n",
        ));
        let other = checked(concat!(
            "async fn other()->int{ return 40 }\n",
            "fn main()->int{ return await other()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&multi).unwrap();
        let b = queue.submit(&other).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("first"));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(a), Some("second"));
        assert_eq!(queue.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        queue.wake(b).unwrap();
        queue.wake(a).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(40));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(a).unwrap().result, Ok(5));
        let awaited = queue.outcome(a).unwrap().events.iter().filter_map(|event|
            match event {
                PrivateScalarCallEvent::AwaitSafePoint(name) => Some(name.as_str()),
                _ => None,
            }
        ).collect::<Vec<_>>();
        assert_eq!(awaited, ["first", "second"]);
        assert!(queue.outcome(a).unwrap().has_balanced_frame_trace());
    }

    #[test]
    fn g18_v7_e3_已醒來尚待執行的真父框架取消會移除FIFO並真釋放資源() {
        let active = checked(concat!(
            "async fn never()->int{ return 8 }\n",
            "fn wrapper()->int{\n",
            "var own:str = \"原工作唯一持有🪼\"\n",
            "return await never()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        ));
        let next = checked(concat!(
            "fn ready()->int{ return 12 }\n",
            "fn main()->int{ return ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&active).unwrap();
        let b = queue.submit(&next).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.ready_ids(), [b, a]);
        queue.cancel(a).unwrap();
        assert_eq!(queue.ready_ids(), [b]);
        assert_eq!(queue.status(a), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(queue.outcome(a).is_none());
        assert_eq!(queue.cancellation_order(), &[a]);
        let evidence = queue.cancellation(a).unwrap();
        assert!(matches!(evidence.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(id),
            crate::owned_str_runtime::ResourceEvent::Released(done),
        ] if id == done));
        assert!(!evidence.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "never",
        )));
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(b).unwrap().result, Ok(12));
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.cancel(a), Err(PrivateScalarCooperativeError::WrongState));
    }

    #[test]
    fn g18_v7_e5_喚醒前及已排隊後新增真祖先票據都不得偷消耗checkpoint() {
        let plan = checked(concat!(
            "async fn work()->int{ return 5 }\n",
            "fn main()->int{\n",
            "var hold:str = \"不許跨等待借用🪼\"\n",
            "return await work()+0\n}\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        let before = queue.retained_resources(id).unwrap().to_vec();
        let source = DeclarationId(974);
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(queue.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert!(queue.ready_ids().is_empty());
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), before.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        queue.wake(id).unwrap();
        assert_eq!(queue.ready_ids(), [id]);
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(queue.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [id]);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), before.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(id).unwrap().result, Ok(5));
        assert!(matches!(queue.outcome(id).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
    }

    #[test]
    fn g18_v7_e3_取消前真祖先票據未還需保留已喚醒FIFO與原ResourceId() {
        let plan = checked(concat!(
            "async fn async_fn()->int{ return 4 }\n",
            "fn main()->int{\nvar own:str = \"待取消🪼\"\n",
            "return await async_fn()+0\n}\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(), Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        let original = queue.retained_resources(id).unwrap().to_vec();
        let ticket = queue.jobs.get_mut(&id).unwrap()
            .continuation.checkpoint.as_mut().unwrap().frames[0]
            .borrow_tracker.acquire(DeclarationId(975), BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(queue.cancel(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [id]);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(queue.retained_resources(id).unwrap(), original.as_slice());
        queue.jobs.get_mut(&id).unwrap().continuation.checkpoint
            .as_mut().unwrap().frames[0].borrow_tracker.release(ticket).unwrap();
        queue.cancel(id).unwrap();
        assert_eq!(queue.ready_ids(), []);
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.cancellation_order(), &[id]);
        assert!(matches!(queue.cancellation(id).unwrap().resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
    }

    #[test]
    fn g18_v7_e2_普通同步工作不得醒來且重複喚醒不得重複入FIFO() {
        let plan = checked(concat!(
            "async fn target()->int{ return 9 }\n",
            "fn main()->int{ return await target()+0 }\n",
        ));
        let sync = checked(concat!(
            "fn plain()->int{ return 3 }\n",
            "fn main()->int{ return plain()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let a = queue.submit(&plan).unwrap();
        let b = queue.submit(&sync).unwrap();
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.wake(b), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Suspended)));
        queue.wake(a).unwrap();
        assert_eq!(queue.wake(a), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.ready_ids(), [b, a]);
        assert_eq!(queue.drive_one().unwrap(), Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.wake(b), Err(PrivateScalarCooperativeError::WrongState));
        assert_eq!(queue.drive_one().unwrap(), Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.outcome(a).unwrap().result, Ok(9));
        assert_eq!(queue.drive_order().len(), 3);
    }

    #[test]
    fn g18_v7_e3_未啟動工作取消必零真動態事件且無未知工作副作用() {
        let plan = checked(concat!(
            "async fn ready()->int{\nvar never:str = \"不得建構\"\n",
            "return 9\n}\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        assert_eq!(queue.wake(812), Err(PrivateScalarCooperativeError::UnknownTask));
        assert_eq!(queue.cancel(812), Err(PrivateScalarCooperativeError::UnknownTask));
        let id = queue.submit(&plan).unwrap();
        assert_eq!(queue.ready_ids(), [id]);
        queue.cancel(id).unwrap();
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(queue.ready_ids(), []);
        assert!(queue.cancellation(id).is_none());
        assert!(queue.outcome(id).is_none());
        assert!(queue.retained_resources(id).unwrap().is_empty());
        assert_eq!(queue.drive_one().unwrap(), None);
        assert_eq!(queue.drive_order(), &[]);
        assert_eq!(queue.cancellation_order(), &[id]);
    }

    #[test]
    fn g18_v7_e2_工作身分耗盡必拒絕且不污染原待執行FIFO() {
        let plan = checked(concat!(
            "async fn ready()->int{ return 1 }\n",
            "fn main()->int{ return await ready()+0 }\n",
        ));
        let mut queue = PrivateScalarCooperativeQueue::new();
        let valid = queue.submit(&plan).unwrap();
        // 無法在真執行中憑空發放 2^64-1 份工作；測試直接
        // 篡改發號器時，E2 原 Submitted 來源守恆必先拒絕，
        // 不得以 IdExhausted 掩蓋損壞的工作發行歷史。
        queue.next_id = u64::MAX;
        assert_eq!(queue.submit(&plan), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.cancel(valid), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.ready_ids(), [valid]);
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.drive_order(), &[]);
        // 還原唯一真 submit 的已發行 ID 後，原任務可正常
        // 暫停與安全取消；u64 checked_add 的真正溢位分支不改。
        queue.next_id = valid;
        assert_eq!(queue.drive_one().unwrap(), Some((valid, PrivateScalarGraphPoll::Suspended)));
        queue.cancel(valid).unwrap();
        assert_eq!(queue.status(valid), Some(PrivateDeferredAwaitStatus::Cancelled));
    }
}
