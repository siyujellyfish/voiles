// P18.5-T4／V7-E2：wake 的全域 poll 時間必須位於同一
// Checker 工作的兩筆真 poll 之間；次數正確仍不能偽造時序。
#[cfg(test)]
mod g18_t4_wake_chronology_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn leaf()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var saved:str = \"原FIFO真wake時序🪼\"\n",
            "return await leaf()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("真 Parser／Checker 必須接受此 async UTF-8 工作")
    }

    fn suspended<'a>(
        q: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall,
    ) -> u64 {
        let id = q.submit(plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Suspended));
        id
    }

    #[test]
    fn t4_chrono_首次真喚醒必在原Suspended後並於完成前() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.wake_order[0].after_drive_count, 1);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
    }

    #[test]
    fn t4_chrono_偽造時間點早於真Suspended必在drive前拒絕() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.wake_order[0].after_drive_count = 0;
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert!(q.pending_await(id).is_some());
        assert_eq!(q.drive_order().len(), 1);
        q.wake_order[0].after_drive_count = 1;
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_chrono_偽造未來時間點不得讓已暫停工作執行() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.wake_order[0].after_drive_count = 2;
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert!(q.outcome(id).is_none());
        q.wake_order[0].after_drive_count = 1;
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
    }

    #[test]
    fn t4_chrono_完成工作假喚醒晚於自身下一poll不得退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        q.wake_order[0].after_drive_count = 2;
        assert!(matches!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
        q.wake_order[0].after_drive_count = 1;
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
    }

    #[test]
    fn t4_chrono_取消工作偽造早於第一poll的wake不得退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.cancel(id).unwrap();
        q.wake_order[0].after_drive_count = 0;
        assert!(matches!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.cancellation_order(), [id]);
        q.wake_order[0].after_drive_count = 1;
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_chrono_兩工作在同次全域poll刻度真喚醒皆可依序執行() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = suspended(&mut q, &plan);
        q.wake(a).unwrap();
        q.wake(b).unwrap();
        assert_eq!(q.wake_order.iter()
            .map(|receipt| receipt.after_drive_count).collect::<Vec<_>>(),
            [2, 2]);
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(a).unwrap().result, Ok(11));
        assert_eq!(q.outcome(b).unwrap().result, Ok(11));
        q.shutdown().unwrap();
    }

    #[test]
    fn t4_chrono_兩工作跨poll真喚醒憑據保持全域非遞減時序() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = suspended(&mut q, &plan);
        q.wake(a).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((a, PrivateScalarGraphPoll::Completed)));
        q.wake(b).unwrap();
        assert_eq!(q.wake_order.iter()
            .map(|receipt| receipt.after_drive_count).collect::<Vec<_>>(),
            [2, 3]);
        assert_eq!(q.drive_one().unwrap(),
            Some((b, PrivateScalarGraphPoll::Completed)));
        assert!(q.shutdown().unwrap().is_empty());
        assert_eq!(q.outcome(a).unwrap().result, Ok(11));
        assert_eq!(q.outcome(b).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_chrono_整批關閉遇跨工作wake倒序須全員零取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let a = suspended(&mut q, &plan);
        let b = suspended(&mut q, &plan);
        q.wake(a).unwrap();
        q.drive_one().unwrap();
        q.wake(b).unwrap();
        assert_eq!(q.wake_order.iter()
            .map(|receipt| receipt.after_drive_count).collect::<Vec<_>>(),
            [2, 3]);
        // 維持每個工作的 wake 次數，僅把 b 的全域時間
        // 篡改為 a 之前，整批取消必須先拒絕且零局部作用。
        q.wake_order[1].after_drive_count = 1;
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.cancellation_order(), []);
        assert_eq!(q.status(b), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.status(a), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.ready_ids(), [b]);
        q.wake_order[1].after_drive_count = 3;
        assert_eq!(q.shutdown().unwrap(), [b]);
        assert_eq!(q.cancellation_order(), [b]);
        assert_eq!(q.status(b), Some(PrivateDeferredAwaitStatus::Cancelled));
    }
}
