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
    InvalidFieldPlan,
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

/// 僅代表 P16 已驗證的含多個 str 欄位之結構整筆移動；
/// 實際建立／移交／逆序清理的事件全由原本執行期提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WholeStructResourcePlan {
    source: DeclarationId,
    target: DeclarationId,
}

impl WholeStructResourcePlan {
    pub(crate) fn new(source: DeclarationId, target: DeclarationId)
        -> Result<Self, ResourcePlanError> {
        if source == target { return Err(ResourcePlanError::SameDeclaration); }
        Ok(Self { source, target })
    }

    /// 已核准 P16 直線整筆移動必須是 Created×N→Moved×N→Released×N
    /// 且每一筆建立／移動順序完全一致、清理與建立嚴格逆序。
    pub(crate) fn verify_moved_events(&self, events: &[ResourceEvent])
        -> Result<usize, ResourcePlanError> {
        self.verify_transfer_events(events, TransitionKind::Moved)
    }

    /// 只對已通過靜態來源驗證的跨函式整筆結構回傳核對 Returned 事件，
    /// 不因同樣的資源身分而將 Returned 和 Moved 當成可互換操作。
    pub(crate) fn verify_returned_events(&self, events: &[ResourceEvent])
        -> Result<usize, ResourcePlanError> {
        self.verify_transfer_events(events, TransitionKind::Returned)
    }

    fn verify_transfer_events(&self, events: &[ResourceEvent], kind: TransitionKind)
        -> Result<usize, ResourcePlanError> {
        if events.is_empty() || events.len() % 3 != 0 {
            return Err(ResourcePlanError::EventOrder);
        }
        let count = events.len() / 3;
        let (created, rest) = events.split_at(count);
        let (moved, released) = rest.split_at(count);
        let mut ids = Vec::with_capacity(count);
        for event in created {
            let ResourceEvent::Created(id) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            ids.push(*id);
        }
        for (expected, event) in ids.iter().zip(moved) {
            let id = match (kind, event) {
                (TransitionKind::Moved, ResourceEvent::Moved(id))
                | (TransitionKind::Returned, ResourceEvent::Returned(id)) => id,
                _ => return Err(ResourcePlanError::EventOrder),
            };
            if expected != id { return Err(ResourcePlanError::ResourceIdentity); }
        }
        for (expected, event) in ids.iter().rev().zip(released) {
            let ResourceEvent::Released(id) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            if expected != id { return Err(ResourcePlanError::ResourceIdentity); }
        }
        let mut guard = SafetyPipeline::new();
        guard.declare(self.source, CopyPermission::Forbidden)?;
        for id in ids { guard.register_resource(self.source, id)?; }
        guard.move_into(self.source, self.target)?;
        if guard.read(self.source).is_ok() {
            return Err(ResourcePlanError::MovedOwnerStillReadable);
        }
        if guard.read(self.target).is_err() {
            return Err(ResourcePlanError::TargetUnavailable);
        }
        if guard.outstanding() != count {
            return Err(ResourcePlanError::OutstandingResource);
        }
        let cleared = guard.record_cleanup(self.target)?;
        if cleared != count || guard.outstanding() != 0 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        guard.audit_finished()?;
        Ok(count)
    }

    /// 建構途中失敗只允許已完成的 Created×N→Released×N，
    /// 不可提交任何 Moved／Returned，也不可替尚未建立的欄位製造清理。
    pub(crate) fn verify_failed_build_events(&self, events: &[ResourceEvent])
        -> Result<usize, ResourcePlanError> {
        if events.is_empty() || events.len() % 2 != 0 {
            return Err(ResourcePlanError::EventOrder);
        }
        let count = events.len() / 2;
        let (created, released) = events.split_at(count);
        let mut ids = Vec::with_capacity(count);
        for event in created {
            let ResourceEvent::Created(id) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            ids.push(*id);
        }
        for (expected, event) in ids.iter().rev().zip(released) {
            let ResourceEvent::Released(id) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            if expected != id { return Err(ResourcePlanError::ResourceIdentity); }
        }
        let mut guard = SafetyPipeline::new();
        guard.declare(self.source, CopyPermission::Forbidden)?;
        for id in ids { guard.register_resource(self.source, id)?; }
        if guard.outstanding() != count { return Err(ResourcePlanError::OutstandingResource); }
        if guard.record_cleanup(self.source)? != count || guard.outstanding() != 0 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        guard.audit_finished()?;
        Ok(count)
    }
}

/// P16 已核准的具名結構 str 欄位原子覆寫／失敗回滾證據。
/// 初始欄位完成順序與覆寫欄位索引只由已完成靜態檢查的 AST 求值計畫取得。
/// RHS 暫存資源在此抽象帳簿暫歸原宣告負責：不代表 P17 的暫存值佈局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldOverwritePlan {
    owner: DeclarationId,
    initialized: usize,
    old_index: Option<usize>,
}

impl FieldOverwritePlan {
    pub(crate) fn new(
        owner: DeclarationId, initialized: usize, old_index: Option<usize>,
    ) -> Result<Self, ResourcePlanError> {
        if old_index.is_some_and(|index| index >= initialized) {
            return Err(ResourcePlanError::InvalidFieldPlan);
        }
        Ok(Self { owner, initialized, old_index })
    }

    /// 唯一合法事件序列：所有既有資源 Created、RHS Created 後才能清理。
    /// 成功：舊欄位在提交後釋放，最終以最後成功初始化順序逆序清理；
    /// 右側求值失敗：先清理新暫存，再逆序清理全部未變動的舊資源。
    pub(crate) fn verify_actual_events(
        &self, events: &[ResourceEvent], failed_rhs: bool,
    ) -> Result<usize, ResourcePlanError> {
        let total = self.initialized.checked_add(1)
            .ok_or(ResourcePlanError::InvalidFieldPlan)?;
        if events.len() != total.checked_mul(2)
            .ok_or(ResourcePlanError::InvalidFieldPlan)? {
            return Err(ResourcePlanError::EventOrder);
        }
        let (created, released) = events.split_at(total);
        let ids = created.iter().map(|event| match event {
            ResourceEvent::Created(id) => Ok(*id),
            _ => Err(ResourcePlanError::EventOrder),
        }).collect::<Result<Vec<_>,_>>()?;

        let mut expected = Vec::with_capacity(total);
        if !failed_rhs {
            if let Some(index) = self.old_index { expected.push(ids[index]); }
        }
        expected.push(ids[self.initialized]);
        for (index, id) in ids[..self.initialized].iter().enumerate().rev() {
            if failed_rhs || self.old_index != Some(index) { expected.push(*id); }
        }
        if expected.len() != total { return Err(ResourcePlanError::InvalidFieldPlan); }
        for (wanted, event) in expected.iter().zip(released) {
            let ResourceEvent::Released(actual) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            if wanted != actual { return Err(ResourcePlanError::ResourceIdentity); }
        }

        let mut ledger = SafetyPipeline::new();
        ledger.declare(self.owner, CopyPermission::Forbidden)?;
        for id in &ids { ledger.register_resource(self.owner, *id)?; }
        if ledger.outstanding() != total { return Err(ResourcePlanError::OutstandingResource); }
        for id in expected {
            ledger.record_resource_cleanup(self.owner, id)?;
        }
        if ledger.outstanding() != 0 || ledger.record_cleanup(self.owner)? != 0 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        ledger.audit_finished()?;
        Ok(total)
    }
}

/// P16 已核准的混合純值／唯一 str 結構整筆複製或移動責任。
/// Copyable int/bool 不分配 UTF-8；含 str 時只能驗證真正移動與清理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MixedResourcePlan {
    source: DeclarationId,
    target: DeclarationId,
    str_count: usize,
}

impl MixedResourcePlan {
    pub(crate) fn new(source: DeclarationId, target: DeclarationId, str_count: usize)
        -> Result<Self, ResourcePlanError> {
        if source == target { return Err(ResourcePlanError::SameDeclaration); }
        Ok(Self { source, target, str_count })
    }

    /// str 欄位全數轉交時沿用 WholeStructResourcePlan；
    /// 沒有 str 的 Copyable 結構不虛構 Created／Moved／Released 事件。
    pub(crate) fn verify_completed(
        &self, events: &[ResourceEvent], moved: bool,
    ) -> Result<usize, ResourcePlanError> {
        if self.str_count > 0 {
            if !moved { return Err(ResourcePlanError::EventOrder); }
            let count = WholeStructResourcePlan::new(self.source, self.target)?
                .verify_moved_events(events)?;
            if count != self.str_count { return Err(ResourcePlanError::ResourceIdentity); }
            return Ok(count);
        }
        if !events.is_empty() { return Err(ResourcePlanError::EventOrder); }

        let mut ledger = SafetyPipeline::new();
        ledger.declare(self.source, CopyPermission::Allowed)?;
        if moved {
            ledger.move_into(self.source, self.target)?;
            if ledger.read(self.source).is_ok() || ledger.read(self.target).is_err() {
                return Err(ResourcePlanError::MovedOwnerStillReadable);
            }
            if ledger.record_cleanup(self.target)? != 0 {
                return Err(ResourcePlanError::OutstandingResource);
            }
        } else {
            ledger.copy_into(self.source, self.target)?;
            if ledger.read(self.source).is_err() || ledger.read(self.target).is_err() {
                return Err(ResourcePlanError::TargetUnavailable);
            }
            if ledger.record_cleanup(self.target)? != 0
                || ledger.record_cleanup(self.source)? != 0 {
                return Err(ResourcePlanError::OutstandingResource);
            }
        }
        ledger.audit_finished()?;
        Ok(0)
    }

    /// 僅審核已核准求值計畫在失敗切點前真正完成的 str 建立：
    /// 失敗之前的 int/bool 不產生任何資源身分。
    pub(crate) fn verify_failed_build(
        &self, events: &[ResourceEvent], str_before_failure: usize,
    ) -> Result<usize, ResourcePlanError> {
        if str_before_failure > self.str_count { return Err(ResourcePlanError::InvalidFieldPlan); }
        if str_before_failure == 0 {
            if !events.is_empty() { return Err(ResourcePlanError::EventOrder); }
            return Ok(0);
        }
        let count = WholeStructResourcePlan::new(self.source, self.target)?
            .verify_failed_build_events(events)?;
        if count != str_before_failure { return Err(ResourcePlanError::ResourceIdentity); }
        Ok(count)
    }
}

/// 僅限已通過 P16 私有安全驗證且條件為已知 bool 的單層 if；
/// 最多只有外層持有者與真正執行分支中的一個內層持有者。
/// 早退或失敗回滾均仍需內層先釋放，再清理外層。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScopedResourcePlan {
    outer: DeclarationId,
    outer_count: usize,
    executed_inner: Option<(DeclarationId, usize)>,
}

impl ScopedResourcePlan {
    pub(crate) fn new(
        outer: DeclarationId, outer_count: usize,
        executed_inner: Option<(DeclarationId, usize)>,
    ) -> Result<Self, ResourcePlanError> {
        if executed_inner.is_some_and(|(id, _)| id == outer) {
            return Err(ResourcePlanError::SameDeclaration);
        }
        Ok(Self { outer, outer_count, executed_inner })
    }

    pub(crate) fn verify_actual_events(&self, events: &[ResourceEvent])
        -> Result<usize, ResourcePlanError> {
        let inner_count = self.executed_inner.map_or(0, |(_, n)| n);
        let total = self.outer_count.checked_add(inner_count)
            .ok_or(ResourcePlanError::InvalidFieldPlan)?;
        if events.len() != total.checked_mul(2)
            .ok_or(ResourcePlanError::InvalidFieldPlan)? {
            return Err(ResourcePlanError::EventOrder);
        }
        let (created, released) = events.split_at(total);
        let ids = created.iter().map(|e|match e {
            ResourceEvent::Created(id) => Ok(*id),
            _ => Err(ResourcePlanError::EventOrder),
        }).collect::<Result<Vec<_>,_>>()?;
        let mut expected = Vec::with_capacity(total);
        expected.extend(ids[self.outer_count..].iter().rev().copied());
        expected.extend(ids[..self.outer_count].iter().rev().copied());
        for (wanted,event) in expected.iter().zip(released) {
            let ResourceEvent::Released(id) = event else {
                return Err(ResourcePlanError::EventOrder);
            };
            if wanted != id { return Err(ResourcePlanError::ResourceIdentity); }
        }

        let mut ledger = SafetyPipeline::new();
        ledger.declare(self.outer, CopyPermission::Forbidden)?;
        for id in &ids[..self.outer_count] { ledger.register_resource(self.outer, *id)?; }
        if let Some((inner, _)) = self.executed_inner {
            ledger.declare(inner, CopyPermission::Forbidden)?;
            for id in &ids[self.outer_count..] {
                ledger.register_resource(inner, *id)?;
            }
            if ledger.outstanding() != total {
                return Err(ResourcePlanError::OutstandingResource);
            }
            if ledger.record_cleanup(inner)? != inner_count {
                return Err(ResourcePlanError::OutstandingResource);
            }
        }
        if ledger.record_cleanup(self.outer)? != self.outer_count
            || ledger.outstanding() != 0 {
            return Err(ResourcePlanError::OutstandingResource);
        }
        ledger.audit_finished()?;
        Ok(total)
    }
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
    fn 單層已知布林分支內層必須先於外層逆序清理() {
        let plan = ScopedResourcePlan::new(
            DeclarationId(10), 2, Some((DeclarationId(44), 2))).unwrap();
        let events = [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Created(4),
            ResourceEvent::Released(4), ResourceEvent::Released(3),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ];
        assert_eq!(plan.verify_actual_events(&events), Ok(4));
    }

    #[test]
    fn 未執行的分支不得創造內層所有者或資源事件() {
        let plan = ScopedResourcePlan::new(DeclarationId(10), 1, None).unwrap();
        assert_eq!(plan.verify_actual_events(&[
            ResourceEvent::Created(1),ResourceEvent::Released(1),
        ]), Ok(1));
        assert_eq!(ScopedResourcePlan::new(
            DeclarationId(10),1,Some((DeclarationId(10),1))),
            Err(ResourcePlanError::SameDeclaration));
        assert!(plan.verify_actual_events(&[
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]).is_err());
    }

    #[test]
    fn 作用域釋放順序錯置_移動事件_雙重資源身分都必須拒絕() {
        let plan = ScopedResourcePlan::new(
            DeclarationId(10),1,Some((DeclarationId(44),1))).unwrap();
        for events in [
            vec![ResourceEvent::Created(1),ResourceEvent::Created(2),
                 ResourceEvent::Released(1),ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1),ResourceEvent::Created(2),
                 ResourceEvent::Moved(2),ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1),ResourceEvent::Created(1),
                 ResourceEvent::Released(1),ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1),ResourceEvent::Created(2),
                 ResourceEvent::Released(2)],
        ] {
            assert!(plan.verify_actual_events(&events).is_err());
        }
    }

    #[test]
    fn 混合結構僅對真正_str_建立移交釋放_而純值複製不造假事件() {
        let mixed = MixedResourcePlan::new(DeclarationId(10), DeclarationId(40), 1).unwrap();
        assert_eq!(mixed.verify_completed(&[
            ResourceEvent::Created(1),
            ResourceEvent::Moved(1),
            ResourceEvent::Released(1),
        ], true), Ok(1));
        assert!(mixed.verify_completed(&[], false).is_err());
        assert!(mixed.verify_completed(&[], true).is_err());
        let pure = MixedResourcePlan::new(DeclarationId(10), DeclarationId(40), 0).unwrap();
        assert_eq!(pure.verify_completed(&[], false), Ok(0));
        assert_eq!(pure.verify_completed(&[], true), Ok(0));
        assert!(pure.verify_completed(&[ResourceEvent::Created(1)], false).is_err());
    }

    #[test]
    fn 混合欄位失敗只清理當時已完成之_str_不把純值當成資源() {
        let plan = MixedResourcePlan::new(DeclarationId(10), DeclarationId(40), 2).unwrap();
        assert_eq!(plan.verify_failed_build(&[], 0), Ok(0));
        let first = [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ];
        assert_eq!(plan.verify_failed_build(&first, 1), Ok(1));
        assert!(plan.verify_failed_build(&first, 2).is_err());
        assert!(plan.verify_failed_build(&first, 0).is_err());
        assert!(plan.verify_failed_build(&first, 3).is_err());
        assert!(plan.verify_failed_build(&[
            ResourceEvent::Created(1), ResourceEvent::Moved(1),
        ], 1).is_err());
        assert_eq!(MixedResourcePlan::new(DeclarationId(10),DeclarationId(10),0),
            Err(ResourcePlanError::SameDeclaration));
    }

    #[test]
    fn 欄位原子覆寫舊資源先釋放而新資源由持有者退出清理() {
        let plan = FieldOverwritePlan::new(DeclarationId(12), 2, Some(0)).unwrap();
        assert_eq!(plan.verify_actual_events(&[
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(1),
            ResourceEvent::Released(3), ResourceEvent::Released(2),
        ], false), Ok(3));
        assert_eq!(plan.verify_actual_events(&[
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ], true), Ok(3));
    }

    #[test]
    fn 未初始化欄位第一次覆寫不得清理不存在的舊值() {
        let plan = FieldOverwritePlan::new(DeclarationId(12), 1, None).unwrap();
        let e = [ResourceEvent::Created(5), ResourceEvent::Created(6),
                 ResourceEvent::Released(6), ResourceEvent::Released(5)];
        assert_eq!(plan.verify_actual_events(&e, false), Ok(2));
        assert_eq!(plan.verify_actual_events(&e, true), Ok(2));
        assert_eq!(FieldOverwritePlan::new(DeclarationId(12), 1, Some(1)),
            Err(ResourcePlanError::InvalidFieldPlan));
    }

    #[test]
    fn 欄位覆寫拒絕先釋放舊值_雙重清理_混入移動及相同資源身分() {
        let plan = FieldOverwritePlan::new(DeclarationId(12), 2, Some(0)).unwrap();
        for e in [
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Released(1), ResourceEvent::Created(3),
                 ResourceEvent::Released(3), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Created(3), ResourceEvent::Released(1),
                 ResourceEvent::Released(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(1),
                 ResourceEvent::Created(3), ResourceEvent::Released(1),
                 ResourceEvent::Released(3), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Created(3), ResourceEvent::Moved(1),
                 ResourceEvent::Released(3), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Created(3), ResourceEvent::Released(3),
                 ResourceEvent::Released(1), ResourceEvent::Released(2)],
        ] {
            assert!(plan.verify_actual_events(&e, false).is_err());
        }
    }

    #[test]
    fn 多欄位整筆移交遵守建立順序與嚴格逆序清理() {
        let p = WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(19)).unwrap();
        assert_eq!(p.verify_moved_events(&[
            ResourceEvent::Created(11), ResourceEvent::Created(12),
            ResourceEvent::Moved(11), ResourceEvent::Moved(12),
            ResourceEvent::Released(12), ResourceEvent::Released(11),
        ]), Ok(2));
    }

    #[test]
    fn 多資源跨函式返回以相同宣告身分帳簿審核並逆序清理() {
        let p = WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(19)).unwrap();
        let returned = [
            ResourceEvent::Created(11), ResourceEvent::Created(12),
            ResourceEvent::Returned(11), ResourceEvent::Returned(12),
            ResourceEvent::Released(12), ResourceEvent::Released(11),
        ];
        assert_eq!(p.verify_returned_events(&returned), Ok(2));
        assert_eq!(p.verify_moved_events(&returned), Err(ResourcePlanError::EventOrder));
    }

    #[test]
    fn 結構返回不得混入移動_交錯事件_偽造身分或錯誤清理順序() {
        let p = WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(19)).unwrap();
        for bad in [
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Returned(1), ResourceEvent::Moved(2),
                 ResourceEvent::Released(2), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Returned(2), ResourceEvent::Returned(1),
                 ResourceEvent::Released(2), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Returned(1), ResourceEvent::Returned(2),
                 ResourceEvent::Released(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Returned(1), ResourceEvent::Returned(2),
                 ResourceEvent::Released(2), ResourceEvent::Released(3)],
        ] {
            assert!(p.verify_returned_events(&bad).is_err());
        }
    }

    #[test]
    fn 建構失敗只能逆序清理已建立資源且不移交() {
        let p = WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(19)).unwrap();
        assert_eq!(p.verify_failed_build_events(&[
            ResourceEvent::Created(11), ResourceEvent::Released(11)
        ]), Ok(1));
        assert_eq!(p.verify_failed_build_events(&[
            ResourceEvent::Created(11), ResourceEvent::Created(12),
            ResourceEvent::Released(12), ResourceEvent::Released(11)
        ]), Ok(2));
    }

    #[test]
    fn 多資源失序_重複身分_造假轉交或清理不得被帳簿接受() {
        let p = WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(19)).unwrap();
        assert_eq!(WholeStructResourcePlan::new(DeclarationId(7), DeclarationId(7)),
            Err(ResourcePlanError::SameDeclaration));
        for bad in [
            vec![ResourceEvent::Created(1), ResourceEvent::Created(1),
                 ResourceEvent::Moved(1), ResourceEvent::Moved(1),
                 ResourceEvent::Released(1), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Moved(2), ResourceEvent::Moved(1),
                 ResourceEvent::Released(2), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Moved(1), ResourceEvent::Moved(2),
                 ResourceEvent::Released(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(1),
                 ResourceEvent::Released(1), ResourceEvent::Released(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Returned(1),
                 ResourceEvent::Released(1)],
        ] {
            assert!(p.verify_moved_events(&bad).is_err());
        }
        for bad in [
            vec![ResourceEvent::Created(1), ResourceEvent::Created(2),
                 ResourceEvent::Released(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Moved(1)],
            vec![ResourceEvent::Created(1), ResourceEvent::Released(2)],
            vec![ResourceEvent::Created(1), ResourceEvent::Created(1),
                 ResourceEvent::Released(1), ResourceEvent::Released(1)],
        ] {
            assert!(p.verify_failed_build_events(&bad).is_err());
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
