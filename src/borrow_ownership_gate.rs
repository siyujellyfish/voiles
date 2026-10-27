//! P14 受限私有借用／所有權整合：HC14-A 借用期間拒絕直接操作來源。
//! 僅測試已核准來源身分、借用衝突與 P13 轉移，不新增 voiles 借用文法。
//! 不處理 P15 跨作用域生命週期或 P16 資源清理；不供公開檢查器使用。
#![allow(dead_code)]

use std::collections::BTreeMap;
use crate::borrow_safety::{
    BorrowKind, BorrowSafety, BorrowSafetyError, BorrowTicket,
};
use crate::ownership_ledger::DeclarationId;
use crate::ownership_model::{ModelError, ValueSlot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateError {
    MissingSource,
    DuplicateDestination,
    Value(ModelError),
    Borrow(BorrowSafetyError),
}

/// 實際讀取、複製、移動和重新賦值共用同一來源身分門檻；不允許
/// 僅在借用方法中檢查衝突、卻讓另一條所有權操作路徑繞過它。
pub(crate) struct BorrowOwnershipGate {
    values: BTreeMap<DeclarationId, ValueSlot>,
    loans: BorrowSafety,
}

impl BorrowOwnershipGate {
    pub(crate) fn new() -> Self {
        Self { values: BTreeMap::new(), loans: BorrowSafety::new() }
    }

    pub(crate) fn declare(&mut self, id: DeclarationId, value: ValueSlot)
        -> Result<(), GateError> {
        if self.values.contains_key(&id) { return Err(GateError::DuplicateDestination); }
        self.values.insert(id, value);
        Ok(())
    }

    pub(crate) fn borrow(&mut self, id: DeclarationId, kind: BorrowKind)
        -> Result<BorrowTicket, GateError> {
        self.values.get(&id).ok_or(GateError::MissingSource)?
            .read().map_err(GateError::Value)?;
        self.loans.acquire(id, kind).map_err(GateError::Borrow)
    }

    fn direct(&self, id: DeclarationId) -> Result<ValueSlot, GateError> {
        self.loans.check_direct_access(id).map_err(GateError::Borrow)?;
        let slot = self.values.get(&id).ok_or(GateError::MissingSource)?;
        slot.read().map_err(GateError::Value)?;
        Ok(*slot)
    }

    pub(crate) fn read(&self, id: DeclarationId) -> Result<(), GateError> {
        self.direct(id).map(|_| ())
    }

    pub(crate) fn copy_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), GateError> {
        if self.values.contains_key(&to) { return Err(GateError::DuplicateDestination); }
        let copied = self.direct(from)?.duplicate().map_err(GateError::Value)?;
        self.values.insert(to, copied);
        Ok(())
    }

    pub(crate) fn move_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), GateError> {
        if self.values.contains_key(&to) { return Err(GateError::DuplicateDestination); }
        self.direct(from)?;
        let moved = self.values.get_mut(&from).expect("已確認合法來源")
            .move_whole().map_err(GateError::Value)?;
        self.values.insert(to, moved);
        Ok(())
    }

    pub(crate) fn reinitialize(&mut self, id: DeclarationId) -> Result<(), GateError> {
        self.loans.check_direct_access(id).map_err(GateError::Borrow)?;
        self.values.get_mut(&id).ok_or(GateError::MissingSource)?
            .mark_reinitialized();
        Ok(())
    }

    pub(crate) fn finish_method_scope(&mut self, tickets: &[BorrowTicket])
        -> Result<(), GateError> {
        self.loans.finish_scope(tickets).map_err(GateError::Borrow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::CopyPermission::{Allowed, Forbidden, Undecided};

    #[test]
    fn p14_共享借用可並存_原來源直接讀取與複製仍封鎖() {
        let mut gate = BorrowOwnershipGate::new();
        let source = DeclarationId(11);
        gate.declare(source, ValueSlot::initialized(Allowed)).unwrap();
        let a = gate.borrow(source, BorrowKind::SharedRead).unwrap();
        let b = gate.borrow(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(gate.read(source), Err(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        assert_eq!(gate.copy_into(source, DeclarationId(12)),
            Err(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        assert_eq!(gate.borrow(source, BorrowKind::ExclusiveWrite),
            Err(GateError::Borrow(BorrowSafetyError::BorrowConflict)));
        gate.finish_method_scope(&[a,b]).unwrap();
        gate.copy_into(source, DeclarationId(12)).unwrap();
        assert!(gate.read(source).is_ok());
        assert!(gate.read(DeclarationId(12)).is_ok());
    }

    #[test]
    fn p14_獨占借用期間不可再借_移動_重新初始化() {
        let mut gate = BorrowOwnershipGate::new();
        let source = DeclarationId(21);
        gate.declare(source, ValueSlot::initialized(Forbidden)).unwrap();
        let loan = gate.borrow(source, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(gate.borrow(source, BorrowKind::SharedRead),
            Err(GateError::Borrow(BorrowSafetyError::BorrowConflict)));
        assert_eq!(gate.move_into(source, DeclarationId(22)),
            Err(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        assert_eq!(gate.reinitialize(source),
            Err(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        assert_eq!(gate.read(DeclarationId(22)), Err(GateError::MissingSource));
        gate.finish_method_scope(&[loan]).unwrap();
        gate.move_into(source, DeclarationId(22)).unwrap();
        assert_eq!(gate.read(source), Err(GateError::Value(ModelError::MovedRead)));
        assert!(gate.read(DeclarationId(22)).is_ok());
    }

    #[test]
    fn p14_錯誤目標與未決複製不造成半完成移動() {
        let mut gate = BorrowOwnershipGate::new();
        let source = DeclarationId(31);
        let destination = DeclarationId(32);
        gate.declare(source, ValueSlot::initialized(Undecided)).unwrap();
        gate.declare(destination, ValueSlot::initialized(Allowed)).unwrap();
        assert_eq!(gate.move_into(source, destination), Err(GateError::DuplicateDestination));
        assert!(gate.read(source).is_ok());
        assert_eq!(gate.copy_into(source, DeclarationId(33)),
            Err(GateError::Value(ModelError::CopyUndecided)));
        assert_eq!(gate.read(DeclarationId(33)), Err(GateError::MissingSource));
        assert!(gate.read(source).is_ok());
    }

    #[test]
    fn p14_借用無效來源與重複來源宣告均不得取得票據() {
        let mut gate = BorrowOwnershipGate::new();
        let source = DeclarationId(41);
        assert_eq!(gate.borrow(source, BorrowKind::SharedRead), Err(GateError::MissingSource));
        gate.declare(source, ValueSlot::uninitialized(Forbidden)).unwrap();
        assert_eq!(gate.borrow(source, BorrowKind::SharedRead),
            Err(GateError::Value(ModelError::UninitializedRead)));
        assert_eq!(gate.declare(source, ValueSlot::initialized(Allowed)),
            Err(GateError::DuplicateDestination));
        gate.reinitialize(source).unwrap();
        let loan = gate.borrow(source, BorrowKind::SharedRead).unwrap();
        gate.finish_method_scope(&[loan]).unwrap();
        gate.move_into(source, DeclarationId(42)).unwrap();
        assert_eq!(gate.borrow(source, BorrowKind::SharedRead),
            Err(GateError::Value(ModelError::MovedRead)));
    }

    #[test]
    fn p14_相同名稱但不同宣告來源身分不互相封鎖() {
        let mut gate = BorrowOwnershipGate::new();
        let outer = DeclarationId(51);
        let inner = DeclarationId(61);
        gate.declare(outer, ValueSlot::initialized(Allowed)).unwrap();
        gate.declare(inner, ValueSlot::initialized(Allowed)).unwrap();
        let borrowed = gate.borrow(inner, BorrowKind::ExclusiveWrite).unwrap();
        assert!(gate.read(outer).is_ok());
        gate.copy_into(outer, DeclarationId(71)).unwrap();
        assert_eq!(gate.read(inner), Err(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed)));
        gate.finish_method_scope(&[borrowed]).unwrap();
        assert!(gate.read(inner).is_ok());
    }

    #[test]
    fn p14_借用歸還後仍須遵循原有_copyable_與已移動狀態() {
        let mut gate = BorrowOwnershipGate::new();
        let source = DeclarationId(81);
        gate.declare(source, ValueSlot::initialized(Forbidden)).unwrap();
        let loan = gate.borrow(source, BorrowKind::SharedRead).unwrap();
        gate.finish_method_scope(&[loan]).unwrap();
        assert_eq!(gate.copy_into(source, DeclarationId(82)),
            Err(GateError::Value(ModelError::CopyForbidden)));
        gate.move_into(source, DeclarationId(82)).unwrap();
        assert_eq!(gate.move_into(source, DeclarationId(83)),
            Err(GateError::Value(ModelError::MovedRead)));
        gate.reinitialize(source).unwrap();
        assert!(gate.read(source).is_ok());
    }
}
