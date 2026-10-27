//! P15 私有保守作用域／借用逃逸試驗：記錄來源身分和借用票據的邊界。
//! 這不是 voiles .borrow() 語法或生命週期規格的完整實作。
//! 函式／方法之外的最後使用點、await、例外與索引逃逸未核准，均不推定可用。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use crate::borrow_safety::{
    BorrowKind, BorrowSafety, BorrowSafetyError, BorrowTicket,
};
use crate::ownership_ledger::DeclarationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifetimeError {
    UnknownOwner,
    DuplicateOwner,
    RootCannotExit,
    NoScope,
    NonLifoExit,
    Borrow(BorrowSafetyError),
    BorrowEscapeNotVerified,
}

struct ScopeFrame {
    id: u64,
    owners: BTreeSet<DeclarationId>,
    tickets: Vec<BorrowTicket>,
}

/// 只對成功記錄的借用執行作用域結束與歸還，不接受自行攜帶的
/// 票據或傳回借用。若無來源存活證據就一律拒絕，避免假裝已驗證 P15。
pub(crate) struct LifetimeGate {
    loans: BorrowSafety,
    scopes: Vec<ScopeFrame>,
    declared: BTreeMap<DeclarationId, u64>,
    next_scope: u64,
}

impl LifetimeGate {
    pub(crate) fn new() -> Self {
        Self {
            loans: BorrowSafety::new(),
            scopes: vec![ScopeFrame {id: 0, owners: BTreeSet::new(), tickets: Vec::new()}],
            declared: BTreeMap::new(),
            next_scope: 1,
        }
    }

    pub(crate) fn enter_scope(&mut self) -> Result<u64, LifetimeError> {
        let next = self.next_scope;
        self.next_scope = self.next_scope.checked_add(1)
            .ok_or(LifetimeError::NoScope)?;
        self.scopes.push(ScopeFrame {
            id: next, owners: BTreeSet::new(), tickets: Vec::new(),
        });
        Ok(next)
    }

    pub(crate) fn declare_owner(&mut self, id: DeclarationId)
        -> Result<(), LifetimeError> {
        if self.declared.contains_key(&id) {
            return Err(LifetimeError::DuplicateOwner);
        }
        let frame = self.scopes.last_mut().ok_or(LifetimeError::NoScope)?;
        frame.owners.insert(id);
        self.declared.insert(id, frame.id);
        Ok(())
    }

    pub(crate) fn borrow(&mut self, owner: DeclarationId, kind: BorrowKind)
        -> Result<(), LifetimeError> {
        if !self.declared.contains_key(&owner) {
            return Err(LifetimeError::UnknownOwner);
        }
        let ticket = self.loans.acquire(owner, kind).map_err(LifetimeError::Borrow)?;
        self.scopes.last_mut().expect("一定有根作用域").tickets.push(ticket);
        Ok(())
    }

    /// 內部借用票據視為一種無法逃逸的能力，不把跨作用域傳遞誤當
    /// 為安全引用。何時可把它寫成 voiles 的一般借用結果仍待 P15。
    pub(crate) fn return_borrowed(&self, owner: DeclarationId)
        -> Result<(), LifetimeError> {
        if !self.declared.contains_key(&owner) {
            return Err(LifetimeError::UnknownOwner);
        }
        Err(LifetimeError::BorrowEscapeNotVerified)
    }

    pub(crate) fn direct_access(&self, owner: DeclarationId)
        -> Result<(), LifetimeError> {
        if !self.declared.contains_key(&owner) {
            return Err(LifetimeError::UnknownOwner);
        }
        self.loans.check_direct_access(owner).map_err(LifetimeError::Borrow)
    }

    pub(crate) fn leave_scope(&mut self, expected: u64)
        -> Result<(), LifetimeError> {
        if self.scopes.len() <= 1 { return Err(LifetimeError::RootCannotExit); }
        let frame = self.scopes.last().expect("非根作用域");
        if frame.id != expected { return Err(LifetimeError::NonLifoExit); }
        // 只有全部票據能合法歸還才移除範圍／來源身分；
        // 此為分析紀錄的清除，不聲稱 P16 資源清理次序。
        self.loans.finish_scope(&frame.tickets).map_err(LifetimeError::Borrow)?;
        let finished = self.scopes.pop().expect("確認可退出");
        for id in finished.owners {
            self.declared.remove(&id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p15_區塊內建立的來源離開後不可重新借用或直接存取() {
        let mut gate = LifetimeGate::new();
        let scope = gate.enter_scope().unwrap();
        let id = DeclarationId(10);
        gate.declare_owner(id).unwrap();
        gate.borrow(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(gate.direct_access(id),
            Err(LifetimeError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        gate.leave_scope(scope).unwrap();
        assert_eq!(gate.direct_access(id), Err(LifetimeError::UnknownOwner));
        assert_eq!(gate.borrow(id, BorrowKind::SharedRead), Err(LifetimeError::UnknownOwner));
    }

    #[test]
    fn p15_外層來源在內層借用歸還後仍可安全使用() {
        let mut gate = LifetimeGate::new();
        let outer = DeclarationId(12);
        gate.declare_owner(outer).unwrap();
        let inner = gate.enter_scope().unwrap();
        gate.borrow(outer, BorrowKind::ExclusiveWrite).unwrap();
        assert!(gate.direct_access(outer).is_err());
        gate.leave_scope(inner).unwrap();
        assert!(gate.direct_access(outer).is_ok());
        let again = gate.enter_scope().unwrap();
        gate.borrow(outer, BorrowKind::SharedRead).unwrap();
        gate.leave_scope(again).unwrap();
    }

    #[test]
    fn p15_內部借用回傳一律保守拒絕_直到逃逸生命週期獲准() {
        let mut gate = LifetimeGate::new();
        let owner = DeclarationId(20);
        gate.declare_owner(owner).unwrap();
        let scope = gate.enter_scope().unwrap();
        gate.borrow(owner, BorrowKind::SharedRead).unwrap();
        assert_eq!(gate.return_borrowed(owner),
            Err(LifetimeError::BorrowEscapeNotVerified));
        gate.leave_scope(scope).unwrap();
        assert_eq!(gate.return_borrowed(owner),
            Err(LifetimeError::BorrowEscapeNotVerified));
    }

    #[test]
    fn p15_巢狀作用域必須按反向巢狀關係結束_失敗不損毀票據() {
        let mut gate = LifetimeGate::new();
        let owner = DeclarationId(30);
        gate.declare_owner(owner).unwrap();
        let first = gate.enter_scope().unwrap();
        gate.borrow(owner, BorrowKind::SharedRead).unwrap();
        let second = gate.enter_scope().unwrap();
        gate.borrow(owner, BorrowKind::SharedRead).unwrap();
        assert_eq!(gate.leave_scope(first), Err(LifetimeError::NonLifoExit));
        assert!(gate.direct_access(owner).is_err());
        gate.leave_scope(second).unwrap();
        assert!(gate.direct_access(owner).is_err());
        gate.leave_scope(first).unwrap();
        assert!(gate.direct_access(owner).is_ok());
    }

    #[test]
    fn p15_不同詞彙作用域同名來源採宣告身分隔離() {
        let mut gate = LifetimeGate::new();
        let outer = DeclarationId(42);
        let inner = DeclarationId(52);
        gate.declare_owner(outer).unwrap();
        let scope = gate.enter_scope().unwrap();
        gate.declare_owner(inner).unwrap();
        gate.borrow(inner, BorrowKind::ExclusiveWrite).unwrap();
        assert!(gate.direct_access(outer).is_ok());
        assert!(gate.direct_access(inner).is_err());
        gate.leave_scope(scope).unwrap();
        assert_eq!(gate.direct_access(inner), Err(LifetimeError::UnknownOwner));
        assert!(gate.direct_access(outer).is_ok());
    }

    #[test]
    fn p15_重複身分和無主借用遭拒_根作用域不可錯誤退出() {
        let mut gate = LifetimeGate::new();
        let owner = DeclarationId(64);
        gate.declare_owner(owner).unwrap();
        assert_eq!(gate.declare_owner(owner), Err(LifetimeError::DuplicateOwner));
        assert_eq!(gate.borrow(DeclarationId(65), BorrowKind::SharedRead),
            Err(LifetimeError::UnknownOwner));
        assert_eq!(gate.leave_scope(0), Err(LifetimeError::RootCannotExit));
        assert!(gate.direct_access(owner).is_ok());
    }
}
