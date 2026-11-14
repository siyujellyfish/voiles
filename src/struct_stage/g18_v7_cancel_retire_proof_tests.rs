// G18／P18.5-T4／V7-A2：真原 Checker 已啟動的
// Suspended 工作取消後，退休時須重查整份 R05/HC14/P16
// 退出證據，不能只靠 ResourceId 張數或 Cancelled 標籤。
#[cfg(test)]
mod g18_v7_cancel_retire_proof_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn pending()->int{ return 11 }\n",
            "fn wrapper()->int{\n",
            "var hold:str = \"取消退休真 UTF-8 來源🪼\"\n",
            "return await pending()\n}\n",
            "fn main()->int{ return wrapper() }\n",
        )).expect("須由唯一原 Parser／Checker 核准 async 來源")
    }

    fn suspended<'a>(
        plan: &'a PreparedPrivateScalarCall,
    ) -> (PrivateScalarCooperativeQueue<'a>, u64) {
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(queue.pending_await(id), Some("pending"));
        (queue, id)
    }

    #[test]
    fn t4_cancel_未啟動Prepared取消可交付None但須有唯一真取消紀錄() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        queue.cancel(id).unwrap();
        assert!(queue.drive_order().is_empty());
        assert_eq!(queue.cancellation_order(), &[id]);
        assert!(matches!(queue.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
        assert!(queue.jobs.is_empty());
    }

    #[test]
    fn t4_cancel_已真暫停必交付原Unwind與P16唯一ResourceId退出() {
        let plan = checked();
        let (mut queue, id) = suspended(&plan);
        queue.cancel(id).unwrap();
        let PrivateScalarRetiredJob::Cancelled(Some(cancelled)) =
            queue.retire_terminal(id).unwrap() else {
                panic!("真 Suspended 不得冒稱無事件 Prepared 取消");
            };
        assert!(matches!(cancelled.resource_events.as_slice(), [
            crate::owned_str_runtime::ResourceEvent::Created(a),
            crate::owned_str_runtime::ResourceEvent::Released(b),
        ] if a == b));
        assert_eq!(cancelled.events.iter().filter(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).count(), 1);
        assert!(!cancelled.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn t4_cancel_已開始的工作刪除取消證據不得假退成Prepared() {
        let plan = checked();
        let (mut queue, id) = suspended(&plan);
        queue.cancel(id).unwrap();
        let saved = queue.jobs.get_mut(&id).unwrap().cancellation.take().unwrap();
        assert_eq!(queue.retire_terminal(id).err(),
            Some(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.drive_order(), &[(id, PrivateScalarGraphPoll::Suspended)]);
        queue.jobs.get_mut(&id).unwrap().cancellation = Some(saved);
        assert!(matches!(queue.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_cancel_從未啟動的Prepared不能偽塞一份已執行取消證據() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        queue.cancel(id).unwrap();
        queue.jobs.get_mut(&id).unwrap().cancellation = Some(
            PrivateScalarGraphCancellation {
                events: vec![], resource_events: vec![],
            }
        );
        assert_eq!(queue.retire_terminal(id).err(),
            Some(PrivateScalarCooperativeError::Engine));
        queue.jobs.get_mut(&id).unwrap().cancellation = None;
        assert!(matches!(queue.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(None)));
    }

    #[test]
    fn t4_cancel_取消原ResourceId仍平衡但刪掉唯一Unwind不得退休() {
        let plan = checked();
        let (mut queue, id) = suspended(&plan);
        queue.cancel(id).unwrap();
        let events = &mut queue.jobs.get_mut(&id).unwrap()
            .cancellation.as_mut().unwrap().events;
        let original = events.clone();
        assert!(events.iter().any(|e| matches!(e,
            PrivateScalarCallEvent::UnwindStarted)));
        events.retain(|e| !matches!(e, PrivateScalarCallEvent::UnwindStarted));
        assert!(crate::owned_str_runtime::event_trace_balanced(
            &queue.cancellation(id).unwrap().resource_events
        ));
        assert_eq!(queue.retire_terminal(id).err(),
            Some(PrivateScalarCooperativeError::Engine));
        queue.jobs.get_mut(&id).unwrap().cancellation.as_mut().unwrap().events = original;
        assert!(matches!(queue.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_cancel_取消真FunctionExited錯置到Unwind之前即使資源張數相同亦拒絕() {
        let plan = checked();
        let (mut queue, id) = suspended(&plan);
        queue.cancel(id).unwrap();
        let events = &mut queue.jobs.get_mut(&id).unwrap()
            .cancellation.as_mut().unwrap().events;
        let original = events.clone();
        let unwind = events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::UnwindStarted
        )).unwrap();
        let exited = events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).unwrap();
        assert!(unwind < exited);
        events.swap(unwind, exited);
        assert_eq!(queue.retire_terminal(id).err(),
            Some(PrivateScalarCooperativeError::Engine));
        queue.jobs.get_mut(&id).unwrap().cancellation.as_mut().unwrap().events = original;
        assert!(matches!(queue.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_cancel_偽造Cancelled但從未實際取消且無歷史必拒絕() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let id = queue.submit(&plan).unwrap();
        queue.jobs.get_mut(&id).unwrap().continuation.status =
            PrivateDeferredAwaitStatus::Cancelled;
        queue.ready.retain(|candidate| *candidate != id);
        assert!(queue.cancellation_order().is_empty());
        assert_eq!(queue.retire_terminal(id).err(),
            Some(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.jobs.len(), 1);
    }

    #[test]
    fn t4_cancel_不同工作成功結果不得移植給未啟動的同源ID退休() {
        let plan = checked();
        let mut queue = PrivateScalarCooperativeQueue::new();
        let finished = queue.submit(&plan).unwrap();
        let unstarted = queue.submit(&plan).unwrap();
        assert_eq!(queue.drive_one().unwrap(),
            Some((finished, PrivateScalarGraphPoll::Suspended)));
        queue.wake(finished).unwrap();
        // 未啟動工作排在前方；只移走它的 ready token，
        // 不執行，也不讓其取得已完成來源的真身份。
        queue.ready.retain(|candidate| *candidate != unstarted);
        assert_eq!(queue.drive_one().unwrap(),
            Some((finished, PrivateScalarGraphPoll::Completed)));
        let result = queue.jobs.get_mut(&finished).unwrap()
            .continuation.outcome.take().unwrap();
        assert!(result.has_balanced_frame_trace());
        {
            let job = queue.jobs.get_mut(&unstarted).unwrap();
            job.continuation.status = PrivateDeferredAwaitStatus::Succeeded;
            job.continuation.outcome = Some(result);
        }
        assert_eq!(queue.retire_terminal(unstarted).err(),
            Some(PrivateScalarCooperativeError::Engine));
        assert_eq!(queue.jobs.len(), 2);
        assert_eq!(queue.drive_order(), &[
            (finished, PrivateScalarGraphPoll::Suspended),
            (finished, PrivateScalarGraphPoll::Completed),
        ]);
    }
}
