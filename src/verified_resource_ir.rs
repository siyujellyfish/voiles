//! P16.6-R03.2：只消耗已通過來源靜態安全驗證的具名整筆 str 移交計畫。
//! 這是 P16 已核准單一字串來源路徑的可執行安全中間證據；不是 P17 一般執行器。
//! 清理只在真實 Utf8 事件完成之後才能登記，且必須與來源 DeclarationId 對齊。

use crate::ownership_ledger::DeclarationId;
use crate::ownership_model::CopyPermission;
use crate::owned_str_runtime::ResourceEvent;
use crate::resource_borrow_pipeline::{PipelineError, SafetyPipeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NamedResourceTransfer {
    pub source: DeclarationId,
    pub target: DeclarationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourcePlanError {
    SameDeclaration,
    EventOrder,
    ResourceIdentity,
    Transfer(PipelineError),
    MovedOwnerStillReadable,
    TargetUnavailable,
    OutstandingResource,
}

impl From<PipelineError> for ResourcePlanError {
    fn from(err: PipelineError) -> Self { Self::Transfer(err) }
}

impl NamedResourceTransfer {
    pub(crate) fn new(source: DeclarationId, target: DeclarationId)
        -> Result<Self, ResourcePlanError> {
        if source == target { return Err(ResourcePlanError::SameDeclaration); }
        Ok(Self { source, target })
    }

    /// 核對實際事件及資源責任表的 *同一筆* 已驗證移動。
    /// 來源必須先由 Lexer→Parser→Checker 確認安全，不能由此代替語意驗證。
    pub(crate) fn verify_actual_events(&self, events: &[ResourceEvent])
        -> Result<u64, ResourcePlanError> {
        let [ResourceEvent::Created(resource),
             ResourceEvent::Moved(moved),
             ResourceEvent::Released(released)] = events else {
            return Err(ResourcePlanError::EventOrder);
        };
        if resource != moved || moved != released {
            return Err(ResourcePlanError::ResourceIdentity);
        }

        let mut guard = SafetyPipeline::new();
        guard.declare(self.source, CopyPermission::Forbidden)?;
        guard.register_resource(self.source, *resource)?;
        guard.move_into(self.source, self.target)?;
        if guard.read(self.source).is_ok() {
            return Err(ResourcePlanError::MovedOwnerStillReadable);
        }
        if guard.read(self.target).is_err() {
            return Err(ResourcePlanError::TargetUnavailable);
        }
        if guard.outstanding() != 1 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        let released_count = guard.record_cleanup(self.target)?;
        if released_count != 1 || guard.outstanding() != 0 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        guard.audit_finished()?;
        Ok(*resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> NamedResourceTransfer {
        NamedResourceTransfer::new(DeclarationId(8), DeclarationId(20)).unwrap()
    }

    #[test]
    fn 真正唯一資源建立移交釋放序列同時通過兩張責任表() {
        assert_eq!(proof().verify_actual_events(&[
            ResourceEvent::Created(99),
            ResourceEvent::Moved(99),
            ResourceEvent::Released(99),
        ]), Ok(99));
    }

    #[test]
    fn 預檢拒絕同一宣告身分且不建立任何擁有者() {
        assert_eq!(
            NamedResourceTransfer::new(DeclarationId(8), DeclarationId(8)),
            Err(ResourcePlanError::SameDeclaration));
    }

    #[test]
    fn 不同資源身分與重複或缺失清理都必須保守拒絕() {
        for invalid in [
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(2), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(1)],
            vec![ResourceEvent::Moved(1), ResourceEvent::Created(1), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(1), ResourceEvent::Released(1), ResourceEvent::Released(1)],
        ] {
            assert!(proof().verify_actual_events(&invalid).is_err());
        }
    }
}
