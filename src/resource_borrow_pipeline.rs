//! P14/P16 私有跨層安全交易：借用門檻與資源責任表共同預檢。
//! 僅演練已核准的具名整筆移動、Copyable 能力及 HC14-A 借用衝突。
//! 所有清理均為人工提供「已完成」的抽象事件，不是 voiles 析構器。
//! 不推定 P15 一般借用最後使用點、P16 析構順序／錯誤回滾或執行期值表示。
#![allow(dead_code)]

use crate::borrow_ownership_gate::{BorrowOwnershipGate, GateError};
use crate::borrow_safety::{BorrowFact, BorrowKind, BorrowTicket};
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
        // 宣告同樣是雙側交易：不容許資源側重複時仍留下值側的假宣告。
        self.ownership.check_declare(id).map_err(PipelineError::Ownership)?;
        self.resources.check_create_binding(id).map_err(PipelineError::Resource)?;
        self.ownership.commit_declare_checked(id, ValueSlot::initialized(permission));
        self.resources.commit_create_binding_checked(id, permission);
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
        // 真實票據簽發前，責任帳簿必須有同一 DeclarationId；
        // 否則兩表分歧時不得簽發可被其他管線誤用的票據。
        self.resources.check_release_batch(&[id]).map_err(PipelineError::Resource)?;
        self.ownership.borrow(id, kind).map_err(PipelineError::Ownership)
    }

    /// F06：將已檢查靜態借用事實對照真正票據追蹤器；
    /// 只讀驗證，不會因不同分支的推測事實建立或釋放票據。
    pub(crate) fn verify_static_borrows(&self, facts: &[BorrowFact])
        -> Result<(), PipelineError> {
        self.ownership.verify_static_borrows(facts.iter().copied())
            .map_err(PipelineError::Ownership)
    }

    /// 有明確靜態快照時先核對，再按既有原子批次釋放操作歸還。
    /// 此入口不推定借用最後使用點，也不能接受偽造靜態證據。
    pub(crate) fn finish_method_with_snapshot(
        &mut self, tickets: &[BorrowTicket], facts: &[BorrowFact],
    ) -> Result<(), PipelineError> {
        self.verify_static_borrows(facts)?;
        self.finish_method(tickets)
    }

    pub(crate) fn finish_method(&mut self, tickets: &[BorrowTicket])
        -> Result<(), PipelineError> {
        self.ownership.finish_method_scope(tickets).map_err(PipelineError::Ownership)
    }

    pub(crate) fn move_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), PipelineError> {
        self.ownership.check_move_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.check_move_into(from, to).map_err(PipelineError::Resource)?;
        // 兩張表都通過不可變預檢後，提交期不再有任何可回傳 Err 的操作。
        // &mut self 獨占期間不存在其它觀察者可以插入狀態變化。
        self.ownership.commit_move_checked(from, to);
        self.resources.commit_move_checked(from, to);
        Ok(())
    }

    pub(crate) fn copy_into(&mut self, from: DeclarationId, to: DeclarationId)
        -> Result<(), PipelineError> {
        self.ownership.check_copy_into(from, to).map_err(PipelineError::Ownership)?;
        self.resources.check_copy_into(from, to).map_err(PipelineError::Resource)?;
        self.ownership.commit_copy_checked(from, to);
        self.resources.commit_copy_checked(to);
        Ok(())
    }

    pub(crate) fn record_cleanup(&mut self, owner: DeclarationId)
        -> Result<usize, PipelineError> {
        // 在紀錄抽象清理前，先確認沒有活躍借用及無效來源，
        // 同時檢查 P16 責任表確實有對應所有者。
        self.ownership.read(owner).map_err(PipelineError::Ownership)?;
        self.resources.check_release_batch(&[owner]).map_err(PipelineError::Resource)?;
        self.ownership.commit_finish_owner_checked(owner);
        Ok(self.resources.commit_release_batch_checked(&[owner]))
    }

    /// 真實欄位覆寫或 RHS 失敗後，只清理已由執行期確認釋放的資源。
    /// 先檢查來源可用與資源歸屬，再對兩張表維持單一宣告身分。
    pub(crate) fn record_resource_cleanup(
        &mut self, owner: DeclarationId, resource: u64,
    ) -> Result<(), PipelineError> {
        self.ownership.read(owner).map_err(PipelineError::Ownership)?;
        self.resources.check_release_resource(owner, resource)
            .map_err(PipelineError::Resource)?;
        self.resources.commit_release_resource_checked(owner, resource);
        Ok(())
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
    fn f06_票據與靜態快照不一致不得歸還或解鎖任何資源() {
        let mut pipeline = SafetyPipeline::new();
        let owner = DeclarationId(411);
        let different = DeclarationId(412);
        pipeline.declare(owner, CopyPermission::Forbidden).unwrap();
        pipeline.declare(different, CopyPermission::Forbidden).unwrap();
        pipeline.register_resource(owner, 6001).unwrap();
        let one = pipeline.borrow(owner, BorrowKind::SharedRead).unwrap();
        let two = pipeline.borrow(owner, BorrowKind::SharedRead).unwrap();
        let wrong_source = BorrowFact::new(different, BorrowKind::SharedRead);
        let mismatch = PipelineError::Ownership(
            GateError::Borrow(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(pipeline.finish_method_with_snapshot(&[one, two], &[one.fact(), wrong_source]),
            Err(mismatch));
        assert_eq!(pipeline.read(owner), Err(PipelineError::Ownership(
            GateError::Borrow(BorrowSafetyError::SourceIsBorrowed))));
        assert_eq!(pipeline.outstanding(), 1);
        assert_eq!(pipeline.verify_static_borrows(&[one.fact(), two.fact()]), Ok(()));
        pipeline.finish_method_with_snapshot(&[one, two], &[one.fact(), two.fact()])
            .unwrap();
        assert!(pipeline.read(owner).is_ok());
        assert_eq!(pipeline.record_cleanup(owner), Ok(1));
        assert_eq!(pipeline.record_cleanup(different), Ok(0));
        assert_eq!(pipeline.verify_static_borrows(&[]), Ok(()));
        pipeline.audit_finished().unwrap();
    }

    #[test]
    fn 真實借用票據須先有相同宣告來源的資源責任表() {
        let mut p = SafetyPipeline::new();
        let id = DeclarationId(8999);
        p.declare(id, Forbidden).unwrap();
        p.resources.release_batch(&[id]).unwrap();
        assert_eq!(p.borrow(id, BorrowKind::SharedRead),
            Err(PipelineError::Resource(ResourceAuditError::MissingBinding)));
        assert!(p.read(id).is_ok(), "資源帳簿分歧不能私自鎖定來源");
    }

    #[test]
    fn 單項資源清理預檢拒絕不得清除其它欄位或持有者() {
        let mut p = SafetyPipeline::new();
        let first = DeclarationId(8981);
        let second = DeclarationId(8982);
        p.declare(first, Forbidden).unwrap();
        p.declare(second, Forbidden).unwrap();
        p.register_resource(first, 881).unwrap();
        p.register_resource(first, 882).unwrap();
        p.register_resource(second, 883).unwrap();
        assert_eq!(p.record_resource_cleanup(first, 883),
            Err(PipelineError::Resource(ResourceAuditError::MissingOwnedResource)));
        assert_eq!(p.outstanding(), 3);
        assert!(p.read(first).is_ok());
        assert!(p.read(second).is_ok());
        p.record_resource_cleanup(first, 881).unwrap();
        assert_eq!(p.outstanding(), 2);
        assert_eq!(p.record_resource_cleanup(first, 881),
            Err(PipelineError::Resource(ResourceAuditError::MissingOwnedResource)));
        assert_eq!(p.record_cleanup(first), Ok(1));
        assert_eq!(p.record_cleanup(second), Ok(1));
        assert!(p.audit_finished().is_ok());
    }

    #[test]
    fn 原子宣告_責任帳簿重複身分時所有權不得提前宣告() {
        let mut p = SafetyPipeline::new();
        let source = DeclarationId(899);
        p.resources.create_binding(source, CopyPermission::Forbidden).unwrap();
        assert_eq!(p.declare(source, CopyPermission::Forbidden),
            Err(PipelineError::Resource(ResourceAuditError::DuplicateBinding)));
        assert_eq!(p.read(source),
            Err(PipelineError::Ownership(GateError::MissingSource)));
        assert_eq!(p.outstanding(), 0);
    }

    #[test]
    fn 原子移交_資源帳簿缺失時不能先移走所有權() {
        let mut p = SafetyPipeline::new();
        let source = DeclarationId(900);
        let target = DeclarationId(901);
        p.declare(source, CopyPermission::Forbidden).unwrap();
        // 故意破壞兩張表的一致性以覆蓋真正的第二層失敗分支。
        p.resources.release_batch(&[source]).unwrap();
        assert_eq!(p.move_into(source, target),
            Err(PipelineError::Resource(ResourceAuditError::MissingBinding)));
        assert!(p.read(source).is_ok(), "第二層失敗時所有權不可提前移走");
        assert_eq!(p.read(target),
            Err(PipelineError::Ownership(GateError::MissingSource)));
    }

    #[test]
    fn 原子複製_資源表目標衝突時不得產生所有權目標() {
        let mut p = SafetyPipeline::new();
        let source = DeclarationId(902);
        let target = DeclarationId(903);
        p.declare(source, CopyPermission::Allowed).unwrap();
        p.resources.create_binding(target, CopyPermission::Allowed).unwrap();
        assert_eq!(p.copy_into(source, target),
            Err(PipelineError::Resource(ResourceAuditError::DuplicateBinding)));
        assert!(p.read(source).is_ok());
        assert_eq!(p.read(target),
            Err(PipelineError::Ownership(GateError::MissingSource)));
        assert_eq!(p.outstanding(), 0);
    }

    #[test]
    fn 原子清理_資源責任缺失時不可提前結束來源所有權() {
        let mut p = SafetyPipeline::new();
        let source = DeclarationId(904);
        p.declare(source, CopyPermission::Forbidden).unwrap();
        p.resources.release_batch(&[source]).unwrap();
        assert_eq!(p.record_cleanup(source),
            Err(PipelineError::Resource(ResourceAuditError::MissingBinding)));
        assert!(p.read(source).is_ok(), "預檢拒絕清理應保留原持有者");
    }

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
