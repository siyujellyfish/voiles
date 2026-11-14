// G18／P18.5-T4／V7-E2：每份重新排入 ready 的 Suspended
// 工作都必須有一次經原 Checker／E5 驗證的真 wake()；
// 僅有正確 status、checkpoint 和 poll 歷史仍不足以取得 token。
#[cfg(test)]
mod g18_t4_wake_token_ledger_tests {
    use super::*;

    fn checked() -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(concat!(
            "async fn leaf()->int{ return 11 }\n",
            "fn caller()->int{\n",
            "var saved:str = \"真正的 wake 授權🪼\"\n",
            "return await leaf()\n}\n",
            "fn main()->int{ return caller() }\n",
        )).expect("必須經原 Lexer／Parser／Checker 接受合法 async 來源")
    }

    fn suspended<'a>(
        q: &mut PrivateScalarCooperativeQueue<'a>,
        plan: &'a PreparedPrivateScalarCall,
    ) -> u64 {
        let id = q.submit(plan).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.wake_order(), []);
        id
    }

    #[test]
    fn t4_wake_token_Prepared憑空新增wake歷史不得開始執行() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = q.submit(&plan).unwrap();
        q.wake_order.push(PrivateScalarWakeReceipt {
            id, after_drive_count: q.drive_order.len(),
        });
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.drive_order(), []);
        q.wake_order.pop();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Suspended)));
    }

    #[test]
    fn t4_wake_token_真Suspended被偷塞ready不可跳過wake守門() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.ready.push_back(id);
        assert_eq!(q.drive_one(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), [id]);
        assert_eq!(q.drive_order(), &[(id, PrivateScalarGraphPoll::Suspended)]);
        assert!(q.pending_await(id).is_some());
        q.ready.pop_front();
        q.wake(id).unwrap();
        assert_eq!(q.wake_order(), [id]);
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
    }

    #[test]
    fn t4_wake_token_偽wake歷史沒有ready時不可再簽第二張() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake_order.push(PrivateScalarWakeReceipt {
            id, after_drive_count: q.drive_order.len(),
        });
        assert_eq!(q.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.wake_order(), [id]);
        assert!(q.pending_await(id).is_some());
        q.wake_order.pop();
        q.wake(id).unwrap();
        assert_eq!(q.ready_ids(), [id]);
    }

    #[test]
    fn t4_wake_token_重複偽wake紀錄不得授予新執行權() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        let stamp = q.drive_order.len();
        q.wake_order.extend([
            PrivateScalarWakeReceipt { id, after_drive_count: stamp },
            PrivateScalarWakeReceipt { id, after_drive_count: stamp },
        ]);
        assert_eq!(q.wake(id), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.drive_order().len(), 1);
        q.wake_order.clear();
        q.wake(id).unwrap();
        assert_eq!(q.wake_order(), [id]);
    }

    #[test]
    fn t4_wake_token_完成後刪除真wake歷史不得安全退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),
            Some((id, PrivateScalarGraphPoll::Completed)));
        assert_eq!(q.outcome(id).unwrap().result, Ok(11));
        assert_eq!(q.wake_order(), [id]);
        q.wake_order.pop();
        assert!(matches!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Succeeded));
        assert_eq!(q.drive_order().len(), 2);
        q.wake_order.push(PrivateScalarWakeReceipt {
            id, after_drive_count: q.drive_order.len(),
        });
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Completed(_)));
    }

    #[test]
    fn t4_wake_token_真wake後尚未poll合法取消並完整歸還資源() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.cancel(id).unwrap();
        assert_eq!(q.ready_ids(), []);
        assert_eq!(q.wake_order(), [id]);
        assert_eq!(q.cancellation_order(), [id]);
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_wake_token_已取消工作多出一筆偽wake不得退休() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let id = suspended(&mut q, &plan);
        q.wake(id).unwrap();
        q.cancel(id).unwrap();
        q.wake_order.push(PrivateScalarWakeReceipt {
            id, after_drive_count: q.drive_order.len(),
        });
        assert!(matches!(q.retire_terminal(id),
            Err(PrivateScalarCooperativeError::Engine)));
        assert_eq!(q.status(id), Some(PrivateDeferredAwaitStatus::Cancelled));
        assert_eq!(q.cancellation_order(), [id]);
        q.wake_order.pop();
        assert!(matches!(q.retire_terminal(id).unwrap(),
            PrivateScalarRetiredJob::Cancelled(Some(_))));
    }

    #[test]
    fn t4_wake_token_整批關閉遇假wake須全部保留後續正常逆序取消() {
        let plan = checked();
        let mut q = PrivateScalarCooperativeQueue::new();
        let corrupted = suspended(&mut q, &plan);
        let clean = suspended(&mut q, &plan);
        q.wake_order.push(PrivateScalarWakeReceipt {
            id: corrupted, after_drive_count: q.drive_order.len(),
        });
        assert_eq!(q.shutdown(), Err(PrivateScalarCooperativeError::Engine));
        assert_eq!(q.status(clean), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.status(corrupted), Some(PrivateDeferredAwaitStatus::Suspended));
        assert_eq!(q.cancellation_order(), []);
        assert_eq!(q.drive_order().len(), 2);
        q.wake_order.pop();
        assert_eq!(q.shutdown().unwrap(), [clean, corrupted]);
        assert_eq!(q.cancellation_order(), [clean, corrupted]);
    }
}
