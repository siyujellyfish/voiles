// G18／P18.5-T3／R05：真 HC14 BorrowTicket 在函式 frame
// 未銷毀前必須歸還，且已歸還票不可由下一個兄弟函式
// 再次使用同一票據身分。只驗證執行期事件，不修改 Checker。
#[cfg(test)]
mod g18_r05_t3_ticket_frame_lifetime_tests {
    use super::*;
    use super::PrivateSharedCallExitEvent as E;
    use crate::borrow_safety::{BorrowKind, BorrowSafety, BorrowTicket};

    fn signed() -> (BorrowTicket, BorrowTicket, BorrowTicket) {
        let mut safety = BorrowSafety::new();
        let a = safety.acquire(DeclarationId(97001), BorrowKind::SharedRead).unwrap();
        let b = safety.acquire(DeclarationId(97002), BorrowKind::SharedRead).unwrap();
        let c = safety.acquire(DeclarationId(97003), BorrowKind::SharedRead).unwrap();
        safety.release(c).unwrap();
        safety.release(b).unwrap();
        safety.release(a).unwrap();
        (a, b, c)
    }

    #[test]
    fn t3_frame_真兩個兄弟框架各借還獨立票再退出可平衡() {
        let (a,b,_) = signed();
        let trace = vec![
            E::FunctionEntered,
            E::FunctionEntered, E::LoanAcquired(a), E::Returned,
            E::LoanReturned(a), E::FunctionExited { returned:true },
            E::FunctionEntered, E::LoanAcquired(b), E::Returned,
            E::LoanReturned(b), E::FunctionExited { returned:true },
            E::Returned, E::FunctionExited { returned:true },
        ];
        assert!(private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_第一兄弟已退出卻讓第二兄弟代還原BorrowTicket必拒() {
        let (a,_,_) = signed();
        // 舊稽核只用當前函式深度比對 LoanReturned，導致票據
        // 隨已退出的 frame 漂移到另一個同深度兄弟再被冒還。
        let trace = vec![
            E::FunctionEntered,
            E::FunctionEntered, E::LoanAcquired(a), E::Returned,
            E::FunctionExited { returned:true },
            E::FunctionEntered, E::Returned, E::LoanReturned(a),
            E::FunctionExited { returned:true },
            E::Returned, E::FunctionExited { returned:true },
        ];
        assert!(!private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_已退票的HC14唯一身分在下一兄弟重新借入必拒() {
        let (a,_,_) = signed();
        let trace = vec![
            E::FunctionEntered,
            E::FunctionEntered, E::LoanAcquired(a), E::Returned,
            E::LoanReturned(a), E::FunctionExited { returned:true },
            E::FunctionEntered, E::LoanAcquired(a), E::Returned,
            E::LoanReturned(a), E::FunctionExited { returned:true },
            E::Returned, E::FunctionExited { returned:true },
        ];
        assert!(!private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_外層真票可跨多個已完成普通子函式再歸還() {
        let (a,_,_) = signed();
        let trace = vec![
            E::FunctionEntered, E::LoanAcquired(a),
            E::FunctionEntered, E::Returned,
            E::FunctionExited { returned:true },
            E::FunctionEntered, E::Returned,
            E::FunctionExited { returned:true },
            E::Returned, E::LoanReturned(a),
            E::FunctionExited { returned:true },
        ];
        assert!(private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_原Caller先借唯一self票由方法正常Return內還票合法() {
        let (a,_,_) = signed();
        let trace = vec![
            E::LoanAcquired(a),
            E::FunctionEntered, E::Returned,
            E::LoanReturned(a), E::FunctionExited { returned:true },
        ];
        assert!(private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_真三層失敗Unwind先內借票歸還再外借票歸還() {
        let (a,b,_) = signed();
        let trace = vec![
            E::FunctionEntered, E::LoanAcquired(a),
            E::FunctionEntered, E::LoanAcquired(b),
            E::UnwindStarted, E::LoanReturned(b),
            E::FunctionExited { returned:false },
            E::LoanReturned(a), E::FunctionExited { returned:false },
        ];
        assert!(private_shared_call_exit_balanced(&trace,false));
        let mut wrong = trace.clone();
        wrong.swap(5,7);
        assert!(!private_shared_call_exit_balanced(&wrong,false));
    }

    #[test]
    fn t3_frame_同一合法函式不同共享來源仍可不依LIFO還票() {
        let (a,b,c) = signed();
        let trace = vec![
            E::FunctionEntered,
            E::LoanAcquired(a), E::LoanAcquired(b), E::LoanAcquired(c),
            E::Returned,
            E::LoanReturned(a), E::LoanReturned(c), E::LoanReturned(b),
            E::FunctionExited { returned:true },
        ];
        assert!(private_shared_call_exit_balanced(&trace,true));
    }

    #[test]
    fn t3_frame_局部ScopeEnded已清空也不可帶未還票跨已退出frame() {
        let (a,b,_) = signed();
        let trace = vec![
            E::FunctionEntered, E::LoanAcquired(a),
            E::FunctionEntered, E::LoanAcquired(b),
            E::LocalInitialized(DeclarationId(97011)),
            E::LocalScopeEnded(DeclarationId(97011)),
            E::Returned,
            E::FunctionExited { returned:true },
            E::FunctionEntered, E::Returned, E::LoanReturned(b),
            E::FunctionExited { returned:true },
            E::Returned, E::LoanReturned(a),
            E::FunctionExited { returned:true },
        ];
        assert!(!private_shared_call_exit_balanced(&trace,true));
    }
}
