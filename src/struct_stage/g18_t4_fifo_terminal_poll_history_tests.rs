// P18.5-T4／V7-E2：真 Checker FIFO 工作終態不可只核對最後一次
// poll；原工作 ID 的所有歷史必為零至多次 Suspended、最後最多
// 一次 Completed。Completed 後續 poll／重複完成均是偽造 R05 終態。
#[cfg(test)]
mod g18_t4_fifo_terminal_poll_history_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn value()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var hold:str = \"T4 FIFO 真終態歷史🪼\"\n",
            "return await value()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("必須由原 Lexer/Parser/Checker 建立的真 async P16 資源工作")
    }

    fn completed<'a>(
        queue: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall,
    ) -> u64 {
        let id = queue.submit(plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        queue.wake(id).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(queue.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        id
    }

    #[test]
    fn t4_history_真Suspended後唯一Completed仍可正常退休並移交原P16結果() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = completed(&mut q, &plan);
        assert_eq!(q.drive_order(), &[
            (id, PrivateScalarGraphPoll::Suspended),
            (id, PrivateScalarGraphPoll::Completed),
        ]);
        let PrivateScalarRetiredJob::Completed(value) = q.retire_terminal(id).unwrap()
            else { panic!("須移交原真結果與 R05/P16 資源退場"); };
        assert_eq!(value.result, Ok(11));
        assert!(value.has_balanced_frame_trace());
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &value.resource_events,
        ));
    }

    #[test]
    fn t4_history_真已完成工作後再寫入Completed必拒絕且不可丟棄結果() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = completed(&mut q, &plan);
        q.drive_order.push((id, PrivateScalarGraphPoll::Completed));
        let before = q.drive_order().to_vec();
        assert_eq!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.drive_order(), before);
        assert!(q.jobs.contains_key(&id));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_history_真Completed後偽造Suspended即使最後不是完成也必拒絕() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = completed(&mut q, &plan);
        q.drive_order.push((id, PrivateScalarGraphPoll::Suspended));
        assert_eq!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.jobs.len(), 1);
    }

    #[test]
    fn t4_history_真受控CallBudget錯誤Completed只一次仍可合法退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        let PrivateScalarRetiredJob::Completed(value) = q.retire_terminal(id).unwrap()
            else { panic!("原 Checker 預算錯誤仍須交付失敗退出證據"); };
        assert_eq!(value.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(value.has_balanced_frame_trace());
    }

    #[test]
    fn t4_history_已Failed後重複Completed不能掩蓋原受控錯誤() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit_with_budget(&plan, PrivateScalarExecutionBudget {
            max_call_frames: 1, max_steps: 100_000,
        }).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        q.drive_order.push((id, PrivateScalarGraphPoll::Completed));
        assert_eq!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Failed));
        assert_eq!(q.outcome(id).unwrap().result,
            Err(PrivateScalarCallRuntimeError::CallBudget));
    }

    #[test]
    fn t4_history_合法已暫停後取消的歷史只有Suspended可安全退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        q.cancel(id).unwrap();
        assert_eq!(q.cancellation_order(), &[id]);
        let PrivateScalarRetiredJob::Cancelled(Some(evidence)) =
            q.retire_terminal(id).unwrap()
            else { panic!("已執行暫停後取消須有真反序資源清理"); };
        let audited = PrivateScalarCallOutcome {
            result: Err(PrivateScalarCallRuntimeError::StateMismatch),
            events: evidence.events,
            resource_events: evidence.resource_events,
            parameter_slots_after_exit: 0,
        };
        assert!(audited.has_balanced_frame_trace());
    }

    #[test]
    fn t4_history_已真取消工作若歷史插入Completed不得用後續Suspended掩飾() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        q.cancel(id).unwrap();
        q.drive_order.push((id, PrivateScalarGraphPoll::Completed));
        q.drive_order.push((id, PrivateScalarGraphPoll::Suspended));
        assert_eq!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert!(q.cancellation(id).is_some());
    }

    #[test]
    fn t4_history_shutdown先查所有終態完整歷史失敗不得部分取消他人() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let terminal = completed(&mut q, &plan);
        let pending = q.submit(&plan).unwrap();
        q.drive_order.push((terminal, PrivateScalarGraphPoll::Completed));
        let before_order = q.drive_order().to_vec();
        let before_ready = q.ready_ids();
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(pending), Some(PrivateDeferredAwaitStatus::Prepared));
        assert_eq!(q.status(terminal), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.drive_order(), before_order);
        assert_eq!(q.ready_ids(), before_ready);
        assert!(q.cancellation_order().is_empty());
        // 復原遭竄改的終態後，才能按原 FIFO 取消仍待執行工作。
        q.drive_order.pop();
        assert_eq!(q.shutdown().unwrap(), [pending]);
        assert_eq!(q.status(pending), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.cancellation_order(), &[pending]);
    }
}
