//! P16 私有資源責任稽核模型（非 voiles 執行期表示或析構實作）。
//! 僅追蹤建立、單一擁有者、合法整筆所有權移交與釋放證明，
//! 不自定義何時／何順序析構、例外清理、物件佈局或隱式資源複製。
//! 不是 GC，也不因分析器物件離開 Rust 作用域就當成已清理。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use crate::ownership_ledger::DeclarationId;
use crate::ownership_model::CopyPermission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceAuditError {
    DuplicateBinding,
    MissingBinding,
    DuplicateResource,
    DuplicateCleanupTarget,
    CopyForbidden,
    CopyUndecided,
    ResourceCopyUnverified,
    OutstandingResources,
    MissingOwnedResource,
}

/// 每一個資源追蹤身分只有一個持有者。此結構只是一張稽核責任表，
/// 不是 voiles 使用者可見的資源、析構器或執行期所有權設計。
#[derive(Debug)]
struct BindingRecord {
    permission: CopyPermission,
    resources: BTreeSet<u64>,
}

#[derive(Default)]
pub(crate) struct ResourceObligationAudit {
    values: BTreeMap<DeclarationId, BindingRecord>,
    ever_created: BTreeSet<u64>,
    released: BTreeSet<u64>,
}

impl ResourceObligationAudit {
    pub(crate) fn new() -> Self { Self::default() }

    pub(crate) fn create_binding(&mut self, id: DeclarationId, permission: CopyPermission)
        -> Result<(), ResourceAuditError> {
        if self.values.contains_key(&id) { return Err(ResourceAuditError::DuplicateBinding); }
        self.values.insert(id, BindingRecord {
            permission, resources: BTreeSet::new(),
        });
        Ok(())
    }

    /// 將新資源責任綁在已建立的所有者。已釋放的 ID 也不可重複建立，
    /// 以防舊票據意外指向新資源。
    pub(crate) fn register_resource(&mut self, binding: DeclarationId, resource: u64)
        -> Result<(), ResourceAuditError> {
        if !self.values.contains_key(&binding) {
            return Err(ResourceAuditError::MissingBinding);
        }
        if !self.ever_created.insert(resource) {
            return Err(ResourceAuditError::DuplicateResource);
        }
        self.values.get_mut(&binding).expect("已存在").resources.insert(resource);
        Ok(())
    }

    /// HB13：整筆所有權責任由來源改掛目標，來源不能再當成所有者。
    /// 不在此層擅定被借用來源是否可移動，必須由 P14／P15 檢查器先授權。
    pub(crate) fn check_move_into(&self, from: DeclarationId, to: DeclarationId)
        -> Result<(), ResourceAuditError> {
        if self.values.contains_key(&to) { return Err(ResourceAuditError::DuplicateBinding); }
        if !self.values.contains_key(&from) { return Err(ResourceAuditError::MissingBinding); }
        Ok(())
    }

    pub(crate) fn move_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), ResourceAuditError> {
        self.check_move_into(from, to)?;
        let record = self.values.remove(&from).expect("已驗證來源");
        self.values.insert(to, record);
        Ok(())
    }

    /// Copyable 值若含抽象資源責任，仍不做任何自選深／淺複製；
    /// 無資源的已核准 Copyable 葉值可建立獨立的空責任紀錄。
    pub(crate) fn check_copy_into(&self, from: DeclarationId, to: DeclarationId)
        -> Result<(), ResourceAuditError> {
        if self.values.contains_key(&to) { return Err(ResourceAuditError::DuplicateBinding); }
        let source = self.values.get(&from).ok_or(ResourceAuditError::MissingBinding)?;
        match source.permission {
            CopyPermission::Forbidden => return Err(ResourceAuditError::CopyForbidden),
            CopyPermission::Undecided => return Err(ResourceAuditError::CopyUndecided),
            CopyPermission::Allowed => {},
        }
        if !source.resources.is_empty() {
            return Err(ResourceAuditError::ResourceCopyUnverified);
        }
        Ok(())
    }

    pub(crate) fn copy_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), ResourceAuditError> {
        self.check_copy_into(from, to)?;
        self.values.insert(to, BindingRecord {
            permission: CopyPermission::Allowed,
            resources: BTreeSet::new(),
        });
        Ok(())
    }

    /// 只記錄由已授權清理事件證明完成的責任，不執行真實析構。
    /// 先驗證整批所有者互異且皆有效，避免錯誤清理中途釋放部分責任。
    /// 輸入順序不代表 voiles 正式清理順序。
    pub(crate) fn check_release_batch(&self, ids: &[DeclarationId])
        -> Result<(), ResourceAuditError> {
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(*id) { return Err(ResourceAuditError::DuplicateCleanupTarget); }
            if !self.values.contains_key(id) { return Err(ResourceAuditError::MissingBinding); }
        }
        Ok(())
    }

    pub(crate) fn release_batch(&mut self, ids: &[DeclarationId])
        -> Result<usize, ResourceAuditError> {
        self.check_release_batch(ids)?;
        let mut count = 0;
        for id in ids {
            let record = self.values.remove(id).expect("已預檢成功");
            for resource in record.resources {
                // 建立時只有一個持有者；不應出現第二次移交或重複釋放。
                debug_assert!(!self.released.contains(&resource));
                self.released.insert(resource);
                count += 1;
            }
        }
        Ok(count)
    }

    /// 只釋放仍屬於指定宣告的單一 UTF-8 資源。
    /// 結構欄位覆寫的舊資源、或 RHS 求值失敗的暫存資源
    /// 不可提前清理其它欄位；預檢不改動任何帳簿。
    pub(crate) fn check_release_resource(
        &self, owner: DeclarationId, resource: u64,
    ) -> Result<(), ResourceAuditError> {
        let binding = self.values.get(&owner)
            .ok_or(ResourceAuditError::MissingBinding)?;
        if !binding.resources.contains(&resource) {
            return Err(ResourceAuditError::MissingOwnedResource);
        }
        Ok(())
    }

    pub(crate) fn release_resource(
        &mut self, owner: DeclarationId, resource: u64,
    ) -> Result<(), ResourceAuditError> {
        self.check_release_resource(owner, resource)?;
        let record = self.values.get_mut(&owner).expect("已預檢持有者");
        let removed = record.resources.remove(&resource);
        debug_assert!(removed);
        let first = self.released.insert(resource);
        debug_assert!(first);
        Ok(())
    }

    pub(crate) fn outstanding_count(&self) -> usize {
        self.ever_created.len() - self.released.len()
    }

    /// 明確要求全部資源責任已完成；不依靠 Drop 或垃圾回收
    /// 掩蓋不匹配的資源建立與釋放。
    pub(crate) fn require_no_leak(&self) -> Result<(), ResourceAuditError> {
        if self.outstanding_count() == 0 { Ok(()) }
        else { Err(ResourceAuditError::OutstandingResources) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CopyPermission::{Allowed, Forbidden, Undecided};

    #[test]
    fn 單一資源清理不得誤清其它欄位或提前結束所有者責任() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(11);
        let b = DeclarationId(22);
        audit.create_binding(a, Forbidden).unwrap();
        audit.register_resource(a, 101).unwrap();
        audit.register_resource(a, 102).unwrap();
        assert_eq!(audit.check_release_resource(b, 101),
            Err(ResourceAuditError::MissingBinding));
        assert_eq!(audit.release_resource(a, 103),
            Err(ResourceAuditError::MissingOwnedResource));
        assert_eq!(audit.outstanding_count(), 2);
        audit.release_resource(a, 101).unwrap();
        assert_eq!(audit.release_resource(a, 101),
            Err(ResourceAuditError::MissingOwnedResource));
        assert_eq!(audit.outstanding_count(), 1);
        assert_eq!(audit.release_batch(&[a]), Ok(1));
        audit.require_no_leak().unwrap();
    }

    #[test]
    fn p16_資源整筆移交不複製責任且目標一次釋放後無洩漏() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(10);
        let b = DeclarationId(11);
        audit.create_binding(a, Forbidden).unwrap();
        audit.register_resource(a, 100).unwrap();
        audit.register_resource(a, 101).unwrap();
        audit.move_into(a, b).unwrap();
        assert_eq!(audit.release_batch(&[a]), Err(ResourceAuditError::MissingBinding));
        assert_eq!(audit.outstanding_count(), 2);
        assert_eq!(audit.release_batch(&[b]), Ok(2));
        assert_eq!(audit.release_batch(&[b]), Err(ResourceAuditError::MissingBinding));
        assert_eq!(audit.require_no_leak(), Ok(()));
    }

    #[test]
    fn p16_重複資源身分_甚至釋放後也不得重新分配() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(20);
        let b = DeclarationId(21);
        audit.create_binding(a, Forbidden).unwrap();
        audit.create_binding(b, Forbidden).unwrap();
        audit.register_resource(a, 200).unwrap();
        assert_eq!(audit.register_resource(b, 200), Err(ResourceAuditError::DuplicateResource));
        audit.release_batch(&[a]).unwrap();
        assert_eq!(audit.register_resource(b, 200), Err(ResourceAuditError::DuplicateResource));
        assert_eq!(audit.release_batch(&[b]), Ok(0));
        assert_eq!(audit.require_no_leak(), Ok(()));
    }

    #[test]
    fn p16_已核准無資源_leaf_准複製_未知及非_copyable_仍拒絕() {
        let mut audit = ResourceObligationAudit::new();
        for (id, cap) in [(30, Allowed), (40, Forbidden), (50, Undecided)] {
            audit.create_binding(DeclarationId(id), cap).unwrap();
        }
        audit.copy_into(DeclarationId(30), DeclarationId(31)).unwrap();
        assert_eq!(audit.copy_into(DeclarationId(40), DeclarationId(41)),
            Err(ResourceAuditError::CopyForbidden));
        assert_eq!(audit.copy_into(DeclarationId(50), DeclarationId(51)),
            Err(ResourceAuditError::CopyUndecided));
        assert_eq!(audit.release_batch(&[DeclarationId(41)]),
            Err(ResourceAuditError::MissingBinding));
        audit.release_batch(&[DeclarationId(30),DeclarationId(31),
            DeclarationId(40),DeclarationId(50)]).unwrap();
        assert!(audit.require_no_leak().is_ok());
    }

    #[test]
    fn p16_就算_allowed_若有資源責任仍不自選深淺複製語意() {
        let mut audit = ResourceObligationAudit::new();
        let source = DeclarationId(60);
        audit.create_binding(source, Allowed).unwrap();
        audit.register_resource(source, 600).unwrap();
        assert_eq!(audit.copy_into(source, DeclarationId(61)),
            Err(ResourceAuditError::ResourceCopyUnverified));
        assert_eq!(audit.outstanding_count(), 1);
        audit.release_batch(&[source]).unwrap();
        assert!(audit.require_no_leak().is_ok());
    }

    #[test]
    fn p16_重複目標與不存在目標的批次清理必須原子拒絕() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(70);
        let b = DeclarationId(71);
        audit.create_binding(a, Forbidden).unwrap();
        audit.create_binding(b, Forbidden).unwrap();
        audit.register_resource(a, 700).unwrap();
        audit.register_resource(b, 701).unwrap();
        assert_eq!(audit.release_batch(&[a, a]),
            Err(ResourceAuditError::DuplicateCleanupTarget));
        assert_eq!(audit.release_batch(&[a, DeclarationId(72)]),
            Err(ResourceAuditError::MissingBinding));
        assert_eq!(audit.outstanding_count(), 2);
        assert_eq!(audit.release_batch(&[a,b]), Ok(2));
        assert!(audit.require_no_leak().is_ok());
    }

    #[test]
    fn p16_搬移到既有目標需在更動前拒絕而原所有者仍有效() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(80);
        let b = DeclarationId(81);
        audit.create_binding(a, Forbidden).unwrap();
        audit.create_binding(b, Allowed).unwrap();
        audit.register_resource(a, 800).unwrap();
        assert_eq!(audit.move_into(a, b), Err(ResourceAuditError::DuplicateBinding));
        assert_eq!(audit.release_batch(&[a]), Ok(1));
        assert_eq!(audit.release_batch(&[b]), Ok(0));
        assert!(audit.require_no_leak().is_ok());
    }

    #[test]
    fn p16_未清理責任須明確偵測不可倚賴_rust_drop_或_gc() {
        let mut audit = ResourceObligationAudit::new();
        let a = DeclarationId(90);
        audit.create_binding(a, Forbidden).unwrap();
        audit.register_resource(a, 900).unwrap();
        assert_eq!(audit.require_no_leak(), Err(ResourceAuditError::OutstandingResources));
        assert_eq!(audit.outstanding_count(), 1);
        audit.release_batch(&[a]).unwrap();
        assert_eq!(audit.require_no_leak(), Ok(()));
    }

    #[test]
    fn p16_不存在持有者不可建立資源_失敗不佔用資源識別() {
        let mut audit = ResourceObligationAudit::new();
        let id = DeclarationId(100);
        assert_eq!(audit.register_resource(id, 1000), Err(ResourceAuditError::MissingBinding));
        audit.create_binding(id, Forbidden).unwrap();
        audit.register_resource(id, 1000).unwrap();
        assert_eq!(audit.create_binding(id, Forbidden), Err(ResourceAuditError::DuplicateBinding));
        audit.release_batch(&[id]).unwrap();
        assert!(audit.require_no_leak().is_ok());
    }
}
