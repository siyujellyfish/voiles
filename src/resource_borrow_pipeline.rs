//! P14/P16 私有跨層安全交易：借用門檻與資源責任表共同預檢。
//! 僅演練已核准的具名整筆移動、Copyable 能力及 HC14-A 借用衝突。
//! 所有清理均為人工提供「已完成」的抽象事件，不是 voiles 析構器。
//! 不推定 P15 一般借用最後使用點、P16 析構順序／錯誤回滾或執行期值表示。
#![allow(dead_code)]

use crate::borrow_ownership_gate::{BorrowOwnershipGate, GateError};
use crate::borrow_safety::{BorrowKind, BorrowTicket};
use crate::ownership_ledger::DeclarationId;
use crate::ownership_model::{CopyPermission, ValueSlot};
use crate::resource_obligation_audit::{ResourceAuditError, ResourceObligationAudit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineError {
    Ownership(GateError),
    Resource(ResourceAuditError),
}

/// 各交易必須先以不可變參照檢查兩側規則，再一起套用成功的轉移。
/// 對外沒有取得內部兩張表的入口，防止呼叫者跳過借用或資源側的驗證。
pub(crate) struct SafetyPipeline {
    ownership: BorrowOwnershipGate,
    resources: ResourceObligationAudit,
}

impl SafetyPipeline {
    pub(crate) fn new() -> Self {
        Self {
            ownership: BorrowOwnershipGate::new(),
            resources: ResourceObligationAudit::new(),
        }
    }

    pub(crate) fn declare(&mut self, id: DeclarationId, permission: CopyPermission)
        -> Result<(), PipelineError> {
        // 經此內部封裝，兩層共享相同宣告身分，建立動作在無資源時完成。
        self.ownership.declare(id, ValueSlot::initialized(permission))
            .map_err(PipelineError::Ownership)?;
        self.resources.create_binding(id, permission)
            .map_err(PipelineError::Resource)?;
        Ok(())
    }

    pub(crate) fn register_resource(&mut self, id: DeclarationId, resource: u64)
        -> Result<(), PipelineError> {
        // 只在所有權來源可直接使用時加入抽象建立責任。
        self.ownership.read(id).map_err(PipelineError::Ownership)?;
        self.resources.register_resource(id, resource).map_err(PipelineError::Resource)
    }

    pub(crate) fn borrow(&mut self, id: DeclarationId, kind: BorrowKind)
        -> Result<BorrowTicket, PipelineError> {
        self.ownership.borrow(id, kind).map_err(PipelineError::Ownership)
    }

    pub(crate) fn finish_method(&mut self, tickets: &[BorrowTicket])
        -> Result<(), PipelineError> {
        self.ownership.finish_method_scope(tickets).map_err(PipelineError::Ownership)
    }

    pub(crate) fn move_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), PipelineError> {
        self.ownership.check_move_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.check_move_into(from, to).map_err(PipelineError::Resource)?;
        self.ownership.move_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.move_into(from, to).map_err(PipelineError::Resource)?;
        Ok(())
    }

    pub(crate) fn copy_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), PipelineError> {
        self.ownership.check_copy_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.check_copy_into(from, to).map_err(PipelineError::Resource)?;
        self.ownership.copy_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.copy_into(from, to).map_err(PipelineError::Resource)?;
        Ok(())
    }

    pub(crate) fn record_cleanup(&mut self, owner: DeclarationId)
        -> Result<usize, PipelineError> {
        // 在紀錄抽象清理前，先確認沒有活躍借用及無效來源，
        // 同時檢查 P16 責任表確實有對應所有者。
        self.ownership.read(owner).map_err(PipelineError::Ownership)?;
        self.resources.check_release_batch(&[owner]).map_err(PipelineError::Resource)?;
        self.ownership.finish_owner(owner).map_err(PipelineError::Ownership)?;
        self.resources.release_batch(&[owner]).map_err(PipelineError::Resource)
    }

    pub(crate) fn read(&self, owner: DeclarationId) -> Result<(), PipelineError> {
        self.ownership.read(owner).map_err(PipelineError::Ownership)
    }

    pub(crate) fn outstanding(&self) -> usize {
        self.resources.outstanding_count()
    }

    pub(crate) fn audit_finished(&self) -> Result<(), PipelineError> {
        self.resources.require_no_leak().map_err(PipelineError::Resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::borrow_safety::BorrowSafetyError;
    use crate::ownership_model::{CopyPermission::{Allowed, Forbidden, Undecided}, ModelError};

    #[test]
    fn 整筆非_copyable_移交後僅新持有者可以提交清理責任() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(10);
        let b = DeclarationId(11);
        p.declare(a, Forbidden).unwrap();
        p.register_resource(a, 100).unwrap();
        p.register_resource(a, 101).unwrap();
        p.move_into(a, b).unwrap();
        assert_eq!(p.read(a), Err(PipelineError::Ownership(GateError::Value(ModelError::MovedRead))));
        assert!(p.read(b).is_ok());
        assert!(p.record_cleanup(a).is_err());
        assert_eq!(p.outstanding(), 2);
        assert_eq!(p.record_cleanup(b), Ok(2));
        assert!(p.audit_finished().is_ok());
        assert!(p.record_cleanup(b).is_err());
    }

    #[test]
    fn 活躍借用同時阻止所有權移動與資源清理_失敗不提前釋放() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(20);
        let b = DeclarationId(21);
        p.declare(a, Forbidden).unwrap();
        p.register_resource(a, 200).unwrap();
        let loan = p.borrow(a, BorrowKind::SharedRead).unwrap();
        let conflict = PipelineError::Ownership(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(p.move_into(a, b), Err(conflict));
        assert_eq!(p.record_cleanup(a), Err(conflict));
        assert_eq!(p.outstanding(), 1);
        p.finish_method(&[loan]).unwrap();
        p.move_into(a, b).unwrap();
        assert_eq!(p.record_cleanup(b), Ok(1));
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 目的名稱已存在時兩層均不允許部分移交() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(30);
        let b = DeclarationId(31);
        p.declare(a, Forbidden).unwrap();
        p.declare(b, Allowed).unwrap();
        p.register_resource(a, 300).unwrap();
        assert_eq!(p.move_into(a,b),
            Err(PipelineError::Ownership(GateError::DuplicateDestination)));
        assert_eq!(p.outstanding(), 1);
        assert_eq!(p.record_cleanup(a), Ok(1));
        assert_eq!(p.record_cleanup(b), Ok(0));
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 不可複製_能力未決與抽象資源均不能繞過雙層複製驗證() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(40);
        let b = DeclarationId(41);
        let c = DeclarationId(42);
        p.declare(a, Forbidden).unwrap();
        p.declare(b, Undecided).unwrap();
        p.declare(c, Allowed).unwrap();
        p.register_resource(c, 400).unwrap();
        assert_eq!(p.copy_into(a, DeclarationId(43)),
            Err(PipelineError::Ownership(GateError::Value(ModelError::CopyForbidden))));
        assert_eq!(p.copy_into(b, DeclarationId(44)),
            Err(PipelineError::Ownership(GateError::Value(ModelError::CopyUndecided))));
        assert_eq!(p.copy_into(c, DeclarationId(45)),
            Err(PipelineError::Resource(ResourceAuditError::ResourceCopyUnverified)));
        assert_eq!(p.outstanding(), 1);
        for id in [a,b,c] { p.record_cleanup(id).unwrap(); }
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 已核准_copyable_無資源值可複製而來源仍有效() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(50);
        let b = DeclarationId(51);
        p.declare(a, Allowed).unwrap();
        p.copy_into(a,b).unwrap();
        assert!(p.read(a).is_ok());
        assert!(p.read(b).is_ok());
        assert_eq!(p.record_cleanup(a), Ok(0));
        assert_eq!(p.record_cleanup(b), Ok(0));
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 不同來源借用互不干擾_錯誤資源身分不造成重複建立() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(60);
        let b = DeclarationId(61);
        p.declare(a, Forbidden).unwrap();
        p.declare(b, Forbidden).unwrap();
        p.register_resource(a, 600).unwrap();
        assert_eq!(p.register_resource(b, 600),
            Err(PipelineError::Resource(ResourceAuditError::DuplicateResource)));
        p.register_resource(b, 601).unwrap();
        let loan = p.borrow(a, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(p.record_cleanup(a),
            Err(PipelineError::Ownership(GateError::Borrow(BorrowSafetyError::SourceIsBorrowed))));
        assert_eq!(p.record_cleanup(b), Ok(1));
        assert_eq!(p.outstanding(), 1);
        p.finish_method(&[loan]).unwrap();
        assert_eq!(p.record_cleanup(a), Ok(1));
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 未明確完成所有資源責任不得宣稱安全退出() {
        let mut p = SafetyPipeline::new();
        let a = DeclarationId(70);
        p.declare(a, Forbidden).unwrap();
        p.register_resource(a, 700).unwrap();
        assert_eq!(p.audit_finished(),
            Err(PipelineError::Resource(ResourceAuditError::OutstandingResources)));
        assert_eq!(p.outstanding(), 1);
        assert_eq!(p.record_cleanup(a), Ok(1));
        assert!(p.audit_finished().is_ok());
    }
}
