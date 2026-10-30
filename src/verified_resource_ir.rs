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

    /// 只接受既有已驗證具名移交路徑，與返回路徑共用同一所有權證明。
    pub(crate) fn verify_actual_events(&self, events: &[ResourceEvent])
        -> Result<u64, ResourcePlanError> {
        verify_transition(self.source, self.target, events, TransitionKind::Moved)
    }
}

/// 只代表已驗證來源的具名 `str` 透過函式返回轉交呼叫端；
/// 直接返回常值無具名宣告，不得憑空產生 DeclarationId。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NamedResourceReturn {
    pub source: DeclarationId,
    pub receiver: DeclarationId,
}

impl NamedResourceReturn {
    pub(crate) fn new(source: DeclarationId, receiver: DeclarationId)
        -> Result<Self, ResourcePlanError> {
        if source == receiver { return Err(ResourcePlanError::SameDeclaration); }
        Ok(Self { source, receiver })
    }

    pub(crate) fn verify_actual_events(&self, events: &[ResourceEvent])
        -> Result<u64, ResourcePlanError> {
        verify_transition(self.source, self.receiver, events, TransitionKind::Returned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionKind { Moved, Returned }

/// 唯一來源與目標由真實已驗證語句提供，不從模擬器自行推測來源身分。
/// 先比對不可偽造的事件形狀／資源 ID，再審核兩張安全責任表。
fn verify_transition(
    source: DeclarationId, target: DeclarationId,
    events: &[ResourceEvent], kind: TransitionKind,
) -> Result<u64, ResourcePlanError> {
    let [ResourceEvent::Created(resource), action, ResourceEvent::Released(released)] = events
        else { return Err(ResourcePlanError::EventOrder); };
    let actual = match (kind, action) {
        (TransitionKind::Moved, ResourceEvent::Moved(id))
        | (TransitionKind::Returned, ResourceEvent::Returned(id)) => id,
        _ => return Err(ResourcePlanError::EventOrder),
    };
    if resource != actual || resource != released {
        return Err(ResourcePlanError::ResourceIdentity);
    }

    let mut guard = SafetyPipeline::new();
    guard.declare(source, CopyPermission::Forbidden)?;
    guard.register_resource(source, *resource)?;
    guard.move_into(source, target)?;
    if guard.read(source).is_ok() {
        return Err(ResourcePlanError::MovedOwnerStillReadable);
    }
    if guard.read(target).is_err() {
        return Err(ResourcePlanError::TargetUnavailable);
    }
    if guard.outstanding() != 1 {
        return Err(ResourcePlanError::OutstandingResource);
    }
    let released_count = guard.record_cleanup(target)?;
    if released_count != 1 || guard.outstanding() != 0 {
        return Err(ResourcePlanError::OutstandingResource);
    }
    guard.audit_finished()?;
    Ok(*resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> NamedResourceTransfer {
        NamedResourceTransfer::new(DeclarationId(8), DeclarationId(20)).unwrap()
    }

    #[test]
    fn 真實具名字串返回擁有者須接收同一資源且只清理一次() {
        let result = NamedResourceReturn::new(DeclarationId(17), DeclarationId(83)).unwrap();
        assert_eq!(result.verify_actual_events(&[
            ResourceEvent::Created(77),
            ResourceEvent::Returned(77),
            ResourceEvent::Released(77),
        ]), Ok(77));
    }

    #[test]
    fn 返回與移動事件不互換且來源身分及清理都不可偽造() {
        let returned = NamedResourceReturn::new(DeclarationId(17), DeclarationId(83)).unwrap();
        assert_eq!(
            NamedResourceReturn::new(DeclarationId(17), DeclarationId(17)),
            Err(ResourcePlanError::SameDeclaration)
        );
        assert_eq!(returned.verify_actual_events(&[
            ResourceEvent::Created(77), ResourceEvent::Moved(77), ResourceEvent::Released(77)
        ]), Err(ResourcePlanError::EventOrder));
        assert_eq!(proof().verify_actual_events(&[
            ResourceEvent::Created(77), ResourceEvent::Returned(77), ResourceEvent::Released(77)
        ]), Err(ResourcePlanError::EventOrder));
        for invalid in [
            vec![ResourceEvent::Created(77), ResourceEvent::Returned(78), ResourceEvent::Released(77)],
            vec![ResourceEvent::Created(77), ResourceEvent::Returned(77), ResourceEvent::Released(78)],
            vec![ResourceEvent::Created(77), ResourceEvent::Returned(77)],
            vec![ResourceEvent::Created(77), ResourceEvent::Returned(77), ResourceEvent::Released(77), ResourceEvent::Released(77)],
            vec![ResourceEvent::Returned(77), ResourceEvent::Created(77), ResourceEvent::Released(77)],
        ] {
            assert!(returned.verify_actual_events(&invalid).is_err());
        }
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
