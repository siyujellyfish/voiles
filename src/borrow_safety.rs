//! HC14-A：已核准的借用衝突核心模型。
//! 多個共享唯讀借用可並存；獨占可變借用與所有其他借用互斥。
//! 借用期間不得直接使用、移動或寫入來源。
//! 借用的實際來源語法、最後使用點與跨 await 有效期仍待後續階段。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::ownership_ledger::DeclarationId;

/// 每個私有借用追蹤器的身份皆不同，防止跨追蹤器票據意外相互解鎖。
static NEXT_TRACKER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowKind {
    SharedRead,
    ExclusiveWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowSafetyError {
    BorrowConflict,
    SourceIsBorrowed,
    InvalidRelease,
    TooManySharedLoans,
}

/// 借用票據綁定原宣告身分與實際授權次序，避免同名遮蔽或假票據提前解除限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BorrowTicket {
    declaration: DeclarationId,
    serial: u64,
    tracker_id: u64,
    kind: BorrowKind,
}

#[derive(Debug)]
pub(crate) struct BorrowSafety {
    active: BTreeMap<DeclarationId, BTreeMap<u64, BorrowKind>>,
    next_serial: u64,
    tracker_id: u64,
}

impl Default for BorrowSafety {
    fn default() -> Self { Self::new() }
}

impl BorrowSafety {
    pub(crate) fn new() -> Self {
        // 超過身份空間時選擇明確停止，不允許回繞後重用既有追蹤器身分。
        let tracker_id = NEXT_TRACKER_ID.try_update(
            Ordering::Relaxed, Ordering::Relaxed,
            |next| next.checked_add(1),
        ).expect("借用追蹤器身分已用盡");
        Self { active: BTreeMap::new(), next_serial: 0, tracker_id }
    }

    /// 單一 HC14-A 衝突規則，供抽象借用票據與真正 .voil 來源共同使用。
    /// 無現存借用時任一權限可取得；共享／共享相容，任何獨占重疊衝突。
    pub(crate) fn validate_new_borrow(
        existing: impl IntoIterator<Item = BorrowKind>, requested: BorrowKind,
    ) -> Result<(), BorrowSafetyError> {
        // 獨占請求只要遇到任一借用即衝突；共享請求僅與獨占衝突。
        // 與既有切片版本的 HC14-A 判定等價，並支援正式 Checker 惰性查詢。
        let conflicts = existing.into_iter().any(|kind| {
            requested == BorrowKind::ExclusiveWrite
                || kind == BorrowKind::ExclusiveWrite
        });
        if conflicts { Err(BorrowSafetyError::BorrowConflict) } else { Ok(()) }
    }

    pub(crate) fn acquire(&mut self, declaration: DeclarationId, kind: BorrowKind)
        -> Result<BorrowTicket, BorrowSafetyError> {
        let existing: Vec<BorrowKind> = self.active.get(&declaration)
            .map(|loans| loans.values().copied().collect()).unwrap_or_default();
        Self::validate_new_borrow(existing.iter().copied(), kind)?;
        let serial = self.next_serial;
        self.next_serial = self.next_serial
            .checked_add(1).ok_or(BorrowSafetyError::TooManySharedLoans)?;
        self.active.entry(declaration).or_default().insert(serial, kind);
        Ok(BorrowTicket { declaration, serial, tracker_id: self.tracker_id, kind })
    }

    /// 對直接讀、寫、複製或 .move() 均先執行此檢查。
    /// 借用關係本身的合法使用不等於原來源可直接被讀取。
    /// HC14-A：由唯一規則判定原所有者能否直接讀、寫、複製或整筆移動。
    /// 私有 Checker 的借用有效期已由 P15 解析為活躍旗標，
    /// 票據追蹤器則由實際票據表得出同一布林事實；兩者不得各自改規則。
    pub(crate) fn validate_direct_access(has_active_loan: bool)
        -> Result<(), BorrowSafetyError> {
        if has_active_loan { Err(BorrowSafetyError::SourceIsBorrowed) }
        else { Ok(()) }
    }

    pub(crate) fn check_direct_access(&self, declaration: DeclarationId)
        -> Result<(), BorrowSafetyError> {
        Self::validate_direct_access(
            self.active.get(&declaration).is_some_and(|loans| !loans.is_empty())
        )
    }

    pub(crate) fn release(&mut self, ticket: BorrowTicket)
        -> Result<(), BorrowSafetyError> {
        if ticket.tracker_id != self.tracker_id {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        let Some(loans) = self.active.get_mut(&ticket.declaration) else {
            return Err(BorrowSafetyError::InvalidRelease);
        };
        if loans.get(&ticket.serial) != Some(&ticket.kind) {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        loans.remove(&ticket.serial);
        if loans.is_empty() { self.active.remove(&ticket.declaration); }
        Ok(())
    }

    /// 方法／函式作用域出口依已核准歸還點明確釋放。
    /// 必須先檢查整批票據均合法，避免錯誤路徑中只釋放部分借用。
    /// 不在這裡推定一般借用最後使用位置。
    pub(crate) fn finish_scope(&mut self, tickets: &[BorrowTicket])
        -> Result<(), BorrowSafetyError> {
        let mut seen = std::collections::BTreeSet::new();
        for ticket in tickets {
            if ticket.tracker_id != self.tracker_id {
                return Err(BorrowSafetyError::InvalidRelease);
            }
            if !seen.insert((ticket.declaration, ticket.serial)) {
                return Err(BorrowSafetyError::InvalidRelease);
            }
            let actual = self.active.get(&ticket.declaration)
                .and_then(|loans| loans.get(&ticket.serial));
            if actual != Some(&ticket.kind) {
                return Err(BorrowSafetyError::InvalidRelease);
            }
        }
        for ticket in tickets.iter().rev() {
            // 上方預檢完成後，同一批次不會重複票據，故以下不應失敗。
            self.release(*ticket)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 靜態來源活躍旗標與真正票據的直接存取規則必須相同() {
        let owner=DeclarationId(48);
        let mut tickets=BorrowSafety::new();
        assert_eq!(BorrowSafety::validate_direct_access(false),
            tickets.check_direct_access(owner));
        let borrow=tickets.acquire(owner,BorrowKind::SharedRead).unwrap();
        assert_eq!(BorrowSafety::validate_direct_access(true),
            tickets.check_direct_access(owner));
        tickets.release(borrow).unwrap();
        assert_eq!(BorrowSafety::validate_direct_access(false),
            tickets.check_direct_access(owner));
    }

    #[test]
    fn hc14_同來源兩個共享唯讀借用可並存_但來源不得直接使用() {
        let mut safety = BorrowSafety::new();
        let id = DeclarationId(10);
        let a = safety.acquire(id, BorrowKind::SharedRead).unwrap();
        let b = safety.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(safety.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.acquire(id, BorrowKind::ExclusiveWrite),
            Err(BorrowSafetyError::BorrowConflict));
        safety.release(a).unwrap();
        assert_eq!(safety.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        safety.release(b).unwrap();
        assert!(safety.check_direct_access(id).is_ok());
    }

    #[test]
    fn hc14_可變借用互斥_不可同時讀讀寫或寫寫() {
        let mut safety = BorrowSafety::new();
        let id = DeclarationId(20);
        let write = safety.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.acquire(id, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(safety.acquire(id, BorrowKind::ExclusiveWrite),
            Err(BorrowSafetyError::BorrowConflict));
        safety.release(write).unwrap();
        assert!(safety.acquire(id, BorrowKind::SharedRead).is_ok());
    }

    #[test]
    fn hc14_同名字串不同宣告來源不會產生假的借用衝突() {
        let mut safety = BorrowSafety::new();
        let outer = DeclarationId(30);
        let inner = DeclarationId(50);
        let shared = safety.acquire(outer, BorrowKind::SharedRead).unwrap();
        assert!(safety.acquire(inner, BorrowKind::ExclusiveWrite).is_ok());
        assert!(safety.check_direct_access(inner).is_err());
        assert!(safety.check_direct_access(outer).is_err());
        safety.release(shared).unwrap();
        assert!(safety.check_direct_access(outer).is_ok());
    }

    #[test]
    fn hc14_錯誤或重複票據不可提前解除真實借用限制() {
        let mut safety = BorrowSafety::new();
        let original = safety.acquire(DeclarationId(10), BorrowKind::ExclusiveWrite).unwrap();
        let invalid = BorrowTicket { declaration: DeclarationId(11), ..original };
        assert_eq!(safety.release(invalid), Err(BorrowSafetyError::InvalidRelease));
        assert!(safety.check_direct_access(DeclarationId(10)).is_err());
        safety.release(original).unwrap();
        assert_eq!(safety.release(original), Err(BorrowSafetyError::InvalidRelease));
    }


    #[test]
    fn hc14_批次歸還發現無效票據時不得提前解除其他來源() {
        let mut safety = BorrowSafety::new();
        let a = DeclarationId(17);
        let b = DeclarationId(19);
        let first = safety.acquire(a, BorrowKind::SharedRead).unwrap();
        let second = safety.acquire(b, BorrowKind::ExclusiveWrite).unwrap();
        let fake = BorrowTicket { declaration: DeclarationId(99), ..first };
        assert_eq!(safety.finish_scope(&[fake, second]),
            Err(BorrowSafetyError::InvalidRelease));
        // 舊版在反向歸還時先釋放 second，再碰到 fake 才報錯。
        assert_eq!(safety.check_direct_access(a), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.check_direct_access(b), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.acquire(b, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        safety.finish_scope(&[first, second]).unwrap();
        assert!(safety.check_direct_access(a).is_ok());
        assert!(safety.check_direct_access(b).is_ok());
    }

    #[test]
    fn hc14_重複票據與錯誤種類不會在批次結束途中解鎖() {
        let mut safety = BorrowSafety::new();
        let id = DeclarationId(27);
        let loan = safety.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.finish_scope(&[loan, loan]),
            Err(BorrowSafetyError::InvalidRelease));
        assert!(safety.check_direct_access(id).is_err());
        let wrong_kind = BorrowTicket { kind: BorrowKind::SharedRead, ..loan };
        assert_eq!(safety.finish_scope(&[loan, wrong_kind]),
            Err(BorrowSafetyError::InvalidRelease));
        assert!(safety.check_direct_access(id).is_err());
        safety.finish_scope(&[loan]).unwrap();
        assert_eq!(safety.finish_scope(&[loan]),
            Err(BorrowSafetyError::InvalidRelease));
    }

    #[test]
    fn hc14_不同追蹤器的借用票據即使宣告位置與序號相同也不能互換() {
        let id = DeclarationId(31);
        let mut first = BorrowSafety::new();
        let mut second = BorrowSafety::new();
        let first_ticket = first.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        let second_ticket = second.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(first_ticket.serial, second_ticket.serial);
        assert_ne!(first_ticket.tracker_id, second_ticket.tracker_id);
        assert_eq!(second.release(first_ticket), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(second.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(first.release(second_ticket), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(first.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        first.finish_scope(&[first_ticket]).unwrap();
        second.finish_scope(&[second_ticket]).unwrap();
    }

    #[test]
    fn hc14_批次中混入跨追蹤器票據不得部分解除其餘借用() {
        let id = DeclarationId(40);
        let mut first = BorrowSafety::new();
        let mut second = BorrowSafety::new();
        let first_ticket = first.acquire(id, BorrowKind::SharedRead).unwrap();
        let second_ticket = second.acquire(id, BorrowKind::SharedRead).unwrap();
        let third_ticket = second.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(second.finish_scope(&[first_ticket, third_ticket]),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(second.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        second.finish_scope(&[second_ticket, third_ticket]).unwrap();
        assert!(second.check_direct_access(id).is_ok());
        first.release(first_ticket).unwrap();
    }

    #[test]
    fn hc14_函式或方法離開時歸還_但不臆定一般生命週期() {
        let mut safety = BorrowSafety::new();
        let source = DeclarationId(91);
        let shared1 = safety.acquire(source, BorrowKind::SharedRead).unwrap();
        let shared2 = safety.acquire(source, BorrowKind::SharedRead).unwrap();
        safety.finish_scope(&[shared1, shared2]).unwrap();
        assert!(safety.check_direct_access(source).is_ok());
        assert!(safety.acquire(source, BorrowKind::ExclusiveWrite).is_ok());
    }
}

#[cfg(test)]
mod p14_單一借用衝突規則 {
    use super::*;

    #[test]
    fn 抽象票據與來源衝突規則維持相同共享獨占矩陣() {
        assert_eq!(BorrowSafety::validate_new_borrow(&[],BorrowKind::SharedRead),Ok(()));
        assert_eq!(BorrowSafety::validate_new_borrow(&[],BorrowKind::ExclusiveWrite),Ok(()));
        assert_eq!(BorrowSafety::validate_new_borrow(
            &[BorrowKind::SharedRead,BorrowKind::SharedRead],BorrowKind::SharedRead),Ok(()));
        for (existing, requested) in [
            (vec![BorrowKind::SharedRead],BorrowKind::ExclusiveWrite),
            (vec![BorrowKind::ExclusiveWrite],BorrowKind::SharedRead),
            (vec![BorrowKind::ExclusiveWrite],BorrowKind::ExclusiveWrite),
        ] {
            assert_eq!(BorrowSafety::validate_new_borrow(&existing,requested),
                Err(BorrowSafetyError::BorrowConflict));
        }
    }
}

#[cfg(test)]
mod p15_6_一般最後使用點與方法作用域歸還區分 {
    use super::*;
    #[test]
    fn 一般別名最後使用點可單獨歸還而方法_self_須等作用域結束() {
        let mut safety = BorrowSafety::new();
        let ordinary = DeclarationId(201);
        let method_self = DeclarationId(202);
        let ordinary_ticket = safety.acquire(ordinary, BorrowKind::SharedRead).unwrap();
        let self_ticket = safety.acquire(method_self, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.check_direct_access(ordinary), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.check_direct_access(method_self), Err(BorrowSafetyError::SourceIsBorrowed));
        // 一般借用已達最後合法使用點，只解除自己的票據。
        safety.release(ordinary_ticket).unwrap();
        assert!(safety.check_direct_access(ordinary).is_ok());
        assert_eq!(safety.check_direct_access(method_self), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.acquire(method_self, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        // 函式／方法接收者借用須在其作用域出口整批歸還。
        safety.finish_scope(&[self_ticket]).unwrap();
        assert!(safety.check_direct_access(method_self).is_ok());
    }
    #[test]
    fn 方法作用域退出失敗不可提前解鎖接收者() {
        let mut safety = BorrowSafety::new();
        let receiver = DeclarationId(203);
        let ticket = safety.acquire(receiver, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.finish_scope(&[ticket, ticket]), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(safety.check_direct_access(receiver), Err(BorrowSafetyError::SourceIsBorrowed));
        safety.finish_scope(&[ticket]).unwrap();
        assert!(safety.check_direct_access(receiver).is_ok());
    }
}
