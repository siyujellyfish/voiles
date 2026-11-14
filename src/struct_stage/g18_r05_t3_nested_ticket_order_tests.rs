// G18／P18.5-T3／R05：以原 HC14 BorrowSafety 真簽發的票據驗證
// 跨函式內外歸還因果次序，不能只靠張數與結尾空集合冒充通過。
#[cfg(test)]
mod g18_r05_t3_nested_ticket_order_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;
    use crate::borrow_safety::{BorrowSafety, BorrowTicket, BorrowKind};

    fn signed_pair() -> (BorrowTicket, BorrowTicket) {
        let mut borrow = BorrowSafety::new();
        let outer = borrow.acquire(DeclarationId(8001), BorrowKind::SharedRead).unwrap();
        let inner = borrow.acquire(DeclarationId(8002), BorrowKind::SharedRead).unwrap();
        assert_eq!(
            borrow.check_ticket_access(outer, BorrowKind::SharedRead),
            Ok(DeclarationId(8001)),
        );
        assert_eq!(
            borrow.check_ticket_access(inner, BorrowKind::SharedRead),
            Ok(DeclarationId(8002)),
        );
        borrow.release(inner).unwrap();
        borrow.release(outer).unwrap();
        (outer, inner)
    }

    fn success(outer: BorrowTicket, inner: BorrowTicket) -> Vec<E> {
        vec![
            E::FunctionEntered,
            E::LoanAcquired(outer),
            E::FunctionEntered,
            E::LoanAcquired(inner),
            E::FunctionEntered,
            E::Returned,
            E::FunctionExited { returned: true },
            E::Returned,
            E::LoanReturned(inner),
            E::FunctionExited { returned: true },
            E::Returned,
            E::LoanReturned(outer),
            E::FunctionExited { returned: true },
        ]
    }

    fn failed(outer: BorrowTicket, inner: BorrowTicket) -> Vec<E> {
        vec![
            E::FunctionEntered,
            E::LoanAcquired(outer),
            E::FunctionEntered,
            E::LoanAcquired(inner),
            E::FunctionEntered,
            E::UnwindStarted,
            E::FunctionExited { returned: false },
            E::LoanReturned(inner),
            E::FunctionExited { returned: false },
            E::LoanReturned(outer),
            E::FunctionExited { returned: false },
        ]
    }

    #[test]
    fn t3_ticket_真兩層成功返回必先內票後外票() {
        let (outer, inner) = signed_pair();
        let events = success(outer, inner);
        assert!(private_shared_call_exit_balanced(&events, true));
        assert!(!private_shared_call_exit_balanced(&events, false));
    }

    #[test]
    fn t3_ticket_成功時交換內外票還序即使張數相同仍必拒絕() {
        let (outer, inner) = signed_pair();
        let mut events = success(outer, inner);
        let original = events.len();
        events.swap(8, 11);
        assert_eq!(events.len(), original);
        assert!(matches!(events[8], E::LoanReturned(ticket) if ticket == outer));
        assert!(!private_shared_call_exit_balanced(&events, true));
    }

    #[test]
    fn t3_ticket_原HC14真兩層故障唯一Unwind必內後外還票() {
        let (outer, inner) = signed_pair();
        let events = failed(outer, inner);
        assert!(private_shared_call_exit_balanced(&events, false));
        assert_eq!(events.iter().filter(|e|
            matches!(e, E::UnwindStarted)
        ).count(), 1);
    }

    #[test]
    fn t3_ticket_故障中交換內外票只改順序仍不得偽造平衡() {
        let (outer, inner) = signed_pair();
        let mut events = failed(outer, inner);
        let original = events.len();
        events.swap(7, 9);
        assert_eq!(events.len(), original);
        assert!(matches!(events[7], E::LoanReturned(ticket) if ticket == outer));
        assert!(!private_shared_call_exit_balanced(&events, false));
    }

    #[test]
    fn t3_ticket_同一詞彙框架兩本共享借用可依原最後使用點正序歸還() {
        let (first, second) = signed_pair();
        // 這兩本票是在同一真 frame 中建立的獨立來源；原 P15
        // 最後使用點可能與取得序不同，不能錯誤要求全域 LIFO。
        let events = vec![
            E::FunctionEntered,
            E::LoanAcquired(first),
            E::LoanAcquired(second),
            E::Returned,
            E::LoanReturned(first),
            E::LoanReturned(second),
            E::FunctionExited { returned: true },
        ];
        assert!(private_shared_call_exit_balanced(&events, true));
    }

    #[test]
    fn t3_ticket_不同真Tracker序號不能冒充同一票據歸還() {
        let (outer, _) = signed_pair();
        let mut another = BorrowSafety::new();
        let foreign = another.acquire(DeclarationId(8001), BorrowKind::SharedRead).unwrap();
        assert_ne!(outer, foreign);
        let invalid = vec![
            E::FunctionEntered,
            E::LoanAcquired(outer),
            E::Returned,
            E::LoanReturned(foreign),
            E::FunctionExited { returned: true },
        ];
        assert!(!private_shared_call_exit_balanced(&invalid, true));
        another.release(foreign).unwrap();
    }

    #[test]
    fn t3_ticket_重複歸還或未歸還真票都不能假成功() {
        let (outer, _) = signed_pair();
        let duplicate = vec![
            E::FunctionEntered,
            E::LoanAcquired(outer),
            E::Returned,
            E::LoanReturned(outer),
            E::LoanReturned(outer),
            E::FunctionExited { returned: true },
        ];
        let missing = vec![
            E::FunctionEntered,
            E::LoanAcquired(outer),
            E::Returned,
            E::FunctionExited { returned: true },
        ];
        assert!(!private_shared_call_exit_balanced(&duplicate, true));
        assert!(!private_shared_call_exit_balanced(&missing, true));
    }

    #[test]
    fn t3_ticket_三層真借用與局部逆序清理成功但錯序竄改必拒絕() {
        let mut borrow = BorrowSafety::new();
        let p = borrow.acquire(DeclarationId(9101), BorrowKind::SharedRead).unwrap();
        let q = borrow.acquire(DeclarationId(9102), BorrowKind::SharedRead).unwrap();
        let r = borrow.acquire(DeclarationId(9103), BorrowKind::SharedRead).unwrap();
        borrow.release(r).unwrap();
        borrow.release(q).unwrap();
        borrow.release(p).unwrap();
        let trace = vec![
            E::FunctionEntered,
            E::LocalInitialized(DeclarationId(101)),
            E::LoanAcquired(p),
            E::FunctionEntered,
            E::LocalInitialized(DeclarationId(102)),
            E::LoanAcquired(q),
            E::FunctionEntered,
            E::LocalInitialized(DeclarationId(103)),
            E::LoanAcquired(r),
            E::UnwindStarted,
            E::LoanReturned(r),
            E::LocalScopeEnded(DeclarationId(103)),
            E::FunctionExited { returned: false },
            E::LoanReturned(q),
            E::LocalScopeEnded(DeclarationId(102)),
            E::FunctionExited { returned: false },
            E::LoanReturned(p),
            E::LocalScopeEnded(DeclarationId(101)),
            E::FunctionExited { returned: false },
        ];
        assert!(private_shared_call_exit_balanced(&trace, false));
        // 兩次竄改都維持同樣事件總數及票據集合，僅破壞動態先後。
        let mut wrong_ticket = trace.clone();
        wrong_ticket.swap(10, 16);
        assert!(!private_shared_call_exit_balanced(&wrong_ticket, false));
        let mut wrong_scope = trace;
        wrong_scope.swap(11, 14);
        assert!(!private_shared_call_exit_balanced(&wrong_scope, false));
    }
}
