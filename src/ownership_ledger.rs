//! P13 多筆來源身分的內部所有權證據追蹤。
//! 尚無具體 Copyable 型別表、借用衝突規則或任何 voiles AST 連線。
//! 所有轉移操作只供內部測試，不能當成真實語言安全保證。
#![allow(dead_code)]

use std::collections::BTreeMap;
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot};

/// 僅在單一來源檔的已解析宣告中用原始位元組起點作身分。
/// 這不是字串名稱；不同作用域的同名宣告不可混在一起。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DeclarationId(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerError {
    MissingDeclaration,
    DuplicateDeclaration,
    IncompatibleBranchBindings,
    NoReachableBranch,
    Value(ModelError),
}

impl From<ModelError> for LedgerError {
    fn from(error: ModelError) -> Self { Self::Value(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnershipLedger {
    values: BTreeMap<DeclarationId, ValueSlot>,
}

impl OwnershipLedger {
    pub(crate) fn new() -> Self { Self { values: BTreeMap::new() } }

    pub(crate) fn declare(&mut self, id: DeclarationId, permission: CopyPermission,
        initialized: bool) -> Result<(), LedgerError> {
        if self.values.contains_key(&id) { return Err(LedgerError::DuplicateDeclaration); }
        let value = if initialized {
            ValueSlot::initialized(permission)
        } else {
            ValueSlot::uninitialized(permission)
        };
        self.values.insert(id, value);
        Ok(())
    }

    pub(crate) fn read(&self, id: DeclarationId) -> Result<(), LedgerError> {
        self.values.get(&id).ok_or(LedgerError::MissingDeclaration)?.read()?;
        Ok(())
    }

    pub(crate) fn duplicate_to(&mut self, source: DeclarationId, target: DeclarationId)
        -> Result<(), LedgerError> {
        // 在任何來源狀態改變之前，先確定目標沒有重複身分。
        if self.values.contains_key(&target) { return Err(LedgerError::DuplicateDeclaration); }
        let value = self.values.get(&source).ok_or(LedgerError::MissingDeclaration)?.duplicate()?;
        self.values.insert(target, value);
        Ok(())
    }

    pub(crate) fn move_to(&mut self, source: DeclarationId, target: DeclarationId)
        -> Result<(), LedgerError> {
        if self.values.contains_key(&target) { return Err(LedgerError::DuplicateDeclaration); }
        let value = self.values.get_mut(&source).ok_or(LedgerError::MissingDeclaration)?.move_whole()?;
        self.values.insert(target, value);
        Ok(())
    }

    /// 僅供外部已檢查可變性、借用與合法賦值時更新模型狀態。
    /// 本 API 不會自行證明該賦值符合完整語言規格。
    pub(crate) fn reinitialize_after_validated_write(&mut self, id: DeclarationId)
        -> Result<(), LedgerError> {
        self.values.get_mut(&id).ok_or(LedgerError::MissingDeclaration)?
            .mark_reinitialized();
        Ok(())
    }

    /// 每個仍可達分支的原有宣告身分必須完全相同。
    /// 不允許拿另一個作用域新建的同名宣告覆蓋原有來源證明。
    pub(crate) fn join_reachable(branches: &[Option<Self>]) -> Result<Self, LedgerError> {
        let mut reachable = branches.iter().flatten();
        let mut joined = reachable.next().cloned().ok_or(LedgerError::NoReachableBranch)?;
        for branch in reachable {
            if joined.values.len() != branch.values.len() ||
                joined.values.keys().ne(branch.values.keys()) {
                return Err(LedgerError::IncompatibleBranchBindings);
            }
            for (id, source) in &branch.values {
                let current = joined.values.get_mut(id).expect("已比較來源身分");
                *current = ValueSlot::join_reachable(&[Some(*current), Some(*source)])?;
            }
        }
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::{CopyPermission as Cap, ValueState};

    fn id(at: usize) -> DeclarationId { DeclarationId(at) }

    #[test]
    fn 同名字串也必須依各自宣告來源身分隔離移動狀態() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(5), Cap::Undecided, true).unwrap();
        ledger.declare(id(25), Cap::Undecided, true).unwrap();
        ledger.move_to(id(25), id(30)).unwrap();
        assert!(ledger.read(id(5)).is_ok(), "內層遮蔽不得毀損外層所有者");
        assert_eq!(ledger.read(id(25)), Err(LedgerError::Value(ModelError::MovedRead)));
        assert!(ledger.read(id(30)).is_ok());
    }

    #[test]
    fn 沒有型別能力證據時複製一律保守拒絕且不新增目標() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(1), Cap::Undecided, true).unwrap();
        assert_eq!(ledger.duplicate_to(id(1), id(2)),
            Err(LedgerError::Value(ModelError::CopyUndecided)));
        assert_eq!(ledger.read(id(2)), Err(LedgerError::MissingDeclaration));
        assert!(ledger.read(id(1)).is_ok());
    }

    #[test]
    fn 已授權複製後來源與目標身分獨立_但不能推定具體型別資格() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(1), Cap::Allowed, true).unwrap();
        ledger.duplicate_to(id(1), id(2)).unwrap();
        ledger.move_to(id(1), id(3)).unwrap();
        assert_eq!(ledger.read(id(1)), Err(LedgerError::Value(ModelError::MovedRead)));
        assert!(ledger.read(id(2)).is_ok());
        assert!(ledger.read(id(3)).is_ok());
    }

    #[test]
    fn 重複目標或不存在的來源不得造成不完整的移動() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(1), Cap::Forbidden, true).unwrap();
        ledger.declare(id(2), Cap::Forbidden, true).unwrap();
        assert_eq!(ledger.move_to(id(1), id(2)), Err(LedgerError::DuplicateDeclaration));
        assert_eq!(ledger.move_to(id(3), id(4)), Err(LedgerError::MissingDeclaration));
        assert!(ledger.read(id(1)).is_ok());
        assert_eq!(ledger.read(id(4)), Err(LedgerError::MissingDeclaration));
    }

    #[test]
    fn 移動來源禁止讀取_合法重新初始化狀態才可重新讀取() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(7), Cap::Forbidden, true).unwrap();
        ledger.move_to(id(7), id(8)).unwrap();
        assert_eq!(ledger.read(id(7)), Err(LedgerError::Value(ModelError::MovedRead)));
        ledger.reinitialize_after_validated_write(id(7)).unwrap();
        assert!(ledger.read(id(7)).is_ok());
        assert!(ledger.read(id(8)).is_ok());
    }

    #[test]
    fn 分支一邊移動另一邊未移動_合流後保守拒絕讀取() {
        let mut original = OwnershipLedger::new();
        original.declare(id(1), Cap::Undecided, true).unwrap();
        // 僅保留共同於分支前建立的可見名稱：移動目標尚非共同宣告。
        let mut moved_early = OwnershipLedger::new();
        moved_early.declare(id(1), Cap::Undecided, true).unwrap();
        moved_early.move_to(id(1), id(9)).unwrap();
        // 不可把條件分支限定的目的宣告偷偷外洩到外層。
        assert_eq!(
            OwnershipLedger::join_reachable(&[Some(original.clone()), Some(moved_early)]),
            Err(LedgerError::IncompatibleBranchBindings)
        );
        let mut one = original.clone();
        one.values.get_mut(&id(1)).unwrap().move_whole().unwrap();
        let joined = OwnershipLedger::join_reachable(&[Some(one), Some(original)]).unwrap();
        assert_eq!(joined.values.get(&id(1)).unwrap().state(), ValueState::MayBeUnavailable);
        assert_eq!(joined.read(id(1)), Err(LedgerError::Value(ModelError::UncertainRead)));
    }

    #[test]
    fn 分支歧異宣告來源不能因同名就合流視為相同資源() {
        let mut a = OwnershipLedger::new();
        a.declare(id(10), Cap::Undecided, true).unwrap();
        let mut b = OwnershipLedger::new();
        b.declare(id(20), Cap::Undecided, true).unwrap();
        assert_eq!(
            OwnershipLedger::join_reachable(&[Some(a), Some(b)]),
            Err(LedgerError::IncompatibleBranchBindings)
        );
    }

    #[test]
    fn 已提前返回的分支不應使可達分支失去資源狀態() {
        let mut live = OwnershipLedger::new();
        live.declare(id(1), Cap::Forbidden, true).unwrap();
        assert_eq!(
            OwnershipLedger::join_reachable(&[None, Some(live.clone())]),
            Ok(live)
        );
        assert_eq!(OwnershipLedger::join_reachable(&[None, None]),
            Err(LedgerError::NoReachableBranch));
    }

    #[test]
    fn 尚未初始化的來源無法複製或移動() {
        let mut ledger = OwnershipLedger::new();
        ledger.declare(id(1), Cap::Allowed, false).unwrap();
        assert_eq!(ledger.duplicate_to(id(1), id(2)),
            Err(LedgerError::Value(ModelError::UninitializedRead)));
        assert_eq!(ledger.move_to(id(1), id(2)),
            Err(LedgerError::Value(ModelError::UninitializedRead)));
        assert_eq!(ledger.read(id(2)), Err(LedgerError::MissingDeclaration));
    }
}
