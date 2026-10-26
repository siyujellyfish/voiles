//! HC14-A：已核准的借用衝突核心模型。
//! 多個共享唯讀借用可並存；獨占可變借用與所有其他借用互斥。
//! 借用期間不得直接使用、移動或寫入來源。
//! 借用的實際來源語法、最後使用點與跨 await 有效期仍待後續階段。
#![allow(dead_code)]

use std::collections::BTreeMap;
use crate::ownership_ledger::DeclarationId;

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
    kind: BorrowKind,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BorrowSafety {
    active: BTreeMap<DeclarationId, BTreeMap<u64, BorrowKind>>,
    next_serial: u64,
}

impl BorrowSafety {
    pub(crate) fn new() -> Self { Self::default() }

    pub(crate) fn acquire(&mut self, declaration: DeclarationId, kind: BorrowKind)
        -> Result<BorrowTicket, BorrowSafetyError> {
        let loans = self.active.get(&declaration);
        let has_any = loans.is_some_and(|existing| !existing.is_empty());
        let has_exclusive = loans.is_some_and(|existing|
            existing.values().any(|held| *held == BorrowKind::ExclusiveWrite));
        if (kind == BorrowKind::ExclusiveWrite && has_any)
            || (kind == BorrowKind::SharedRead && has_exclusive) {
            return Err(BorrowSafetyError::BorrowConflict);
        }
        let serial = self.next_serial;
        self.next_serial = self.next_serial
            .checked_add(1).ok_or(BorrowSafetyError::TooManySharedLoans)?;
        self.active.entry(declaration).or_default().insert(serial, kind);
        Ok(BorrowTicket { declaration, serial, kind })
    }

    /// 對直接讀、寫、複製或 .move() 均先執行此檢查。
    /// 借用關係本身的合法使用不等於原來源可直接被讀取。
    pub(crate) fn check_direct_access(&self, declaration: DeclarationId)
        -> Result<(), BorrowSafetyError> {
        if self.active.get(&declaration).is_some_and(|loans| !loans.is_empty()) {
            Err(BorrowSafetyError::SourceIsBorrowed)
        } else { Ok(()) }
    }

    pub(crate) fn release(&mut self, ticket: BorrowTicket)
        -> Result<(), BorrowSafetyError> {
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

    /// 方法／函式作用域出口依已核准歸還點明確釋放；不推定一般借用最後使用位置。
    pub(crate) fn finish_scope(&mut self, tickets: &[BorrowTicket])
        -> Result<(), BorrowSafetyError> {
        for ticket in tickets.iter().rev() { self.release(*ticket)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
