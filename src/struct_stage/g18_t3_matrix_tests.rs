#[cfg(test)]
mod g18_t3_matrix_tests {
    use super::*;
    use super::g18_async_method_graph_tests::SOURCE;

    #[test]
    fn t3_較早工作事件污染整批退出必須零副作用() {
        let checked=PreparedPrivateScalarCall::prepare(SOURCE).unwrap();
        let mut q=PrivateScalarCooperativeQueue::new();
        let a=q.submit(&checked).unwrap();
        let b=q.submit(&checked).unwrap();
        for id in [a,b] {
            assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            q.wake(id).unwrap();
        }
        for id in [a,b] {
            assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
            assert_eq!(q.pending_await(id),Some("add"));
        }
        let b_resources=q.retained_resources(b).unwrap().to_vec();
        let history=q.drive_order().to_vec();
        let event=q.jobs.get_mut(&a).unwrap().continuation.events.pop().unwrap();
        assert!(q.shutdown().is_err());
        assert!(q.wake(a).is_err());
        assert_eq!(q.drive_order(),history);
        assert!(q.ready_ids().is_empty());
        assert!(q.cancellation_order.is_empty());
        assert_eq!(q.retained_resources(b).unwrap(),b_resources);
        q.jobs.get_mut(&a).unwrap().continuation.events.push(event);
        assert_eq!(q.shutdown().unwrap().len(),2);
    }

    #[test]
    fn t3_合法但不同來源計畫不能冒領等待點() {
        let first=PreparedPrivateScalarCall::prepare(SOURCE).unwrap();
        let other=PreparedPrivateScalarCall::prepare(&SOURCE.replace("holder.add(3)","holder.add(4)")).unwrap();
        let mut q=PrivateScalarCooperativeQueue::new();
        let id=q.submit(&first).unwrap();
        q.drive_one().unwrap();
        q.jobs.get_mut(&id).unwrap().continuation.plan=&other;
        assert!(q.cancel(id).is_err());
        assert!(q.wake(id).is_err());
        assert!(q.ready_ids().is_empty());
        assert!(q.cancellation_order.is_empty());
        q.jobs.get_mut(&id).unwrap().continuation.plan=&first;
        q.wake(id).unwrap();
        assert_eq!(q.drive_one().unwrap(),Some((id,PrivateScalarGraphPoll::Suspended)));
    }
}
