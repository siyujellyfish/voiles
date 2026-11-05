//! HC14-A：已核准的借用衝突核心模型。
//! 多個共享唯讀借用可並存；獨占可變借用與所有其他借用互斥。
//! 借用期間不得直接使用、移動或寫入來源。
//! 借用的實際來源語法、最後使用點與跨 await 有效期仍待後續階段。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::ownership_ledger::DeclarationId;

/// 每個私有借用追蹤器的身份皆不同，防止跨追蹤器票據意外相互解鎖。
static NEXT_TRACKER_ID: AtomicU64 = AtomicU64::new(1);

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
    SnapshotMismatch,
}

/// 共通不可變借用事實：來源宣告身分與 HC14-A 共享／獨占權限。
/// 由真正票據與正式私有 Checker 同時使用，不包含動態有效期判斷。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BorrowFact {
    pub(crate) source: DeclarationId,
    pub(crate) kind: BorrowKind,
}
impl BorrowFact {
    pub(crate) fn new(source: DeclarationId, kind: BorrowKind) -> Self {
        Self { source, kind }
    }
}

/// F06：CFG 分支可以安全複製與合流的靜態借用事實快照。
/// 僅儲存 DeclarationId 與權限，不簽發、持有或偽造真實 BorrowTicket。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StaticBorrowSnapshot {
    facts: Vec<BorrowFact>,
    // F06：相同靜態別名 DeclarationId 不得被兩份不同來源事實認領。
    // 只保存不可恢復的拒絕標記，不建立另一份借用活性權威。
    duplicate_alias: bool,
}
impl StaticBorrowSnapshot {
    pub(crate) fn from_active(facts: impl IntoIterator<Item = BorrowFact>) -> Self {
        Self { facts: facts.into_iter().collect(), duplicate_alias: false }
    }

    /// F06/B05：由正式 Checker 的詞彙 Binding 直接建立靜態別名身分。
    /// 同一來源可有多個不同共享別名；同一別名宣告不能重複認領。
    /// 已失效別名保留身分供一致性檢查，但不再計入活躍借用。
    /// 本入口不持有、建立或釋放任何執行期 BorrowTicket。
    pub(crate) fn from_aliases(
        aliases: impl IntoIterator<Item = (DeclarationId, BorrowFact, bool)>,
    ) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        let mut facts = Vec::new();
        let mut duplicate_alias = false;
        for (alias, fact, active) in aliases {
            if !seen.insert(alias) { duplicate_alias = true; }
            if active { facts.push(fact); }
        }
        Self { facts, duplicate_alias }
    }

    /// F08：以同一份已解析借用快照查詢來源的有效借用。
    /// 不按可變名稱推論，也不把已失效別名重新授權。
    pub(crate) fn kinds_for(&self, source: DeclarationId)
        -> impl Iterator<Item = BorrowKind> + '_ {
        self.facts.iter().filter(move |fact| fact.source == source).map(|fact| fact.kind)
    }

    pub(crate) fn has_active(&self, source: DeclarationId) -> bool {
        self.kinds_for(source).next().is_some()
    }

    pub(crate) fn validate_acquire(&self, source: DeclarationId, requested: BorrowKind)
        -> Result<(), BorrowSafetyError> {
        // 不得在全域活躍集合已自相矛盾的 CFG 快照上，憑另一個來源的
        // 局部授權繼續取得借用，否則會把錯誤狀態掩藏在後續分支。
        self.validate()?;
        BorrowSafety::validate_new_borrow(self.kinds_for(source), requested)
    }

    pub(crate) fn validate_direct(&self, source: DeclarationId)
        -> Result<(), BorrowSafetyError> {
        self.validate()?;
        BorrowSafety::validate_direct_access(self.has_active(source))
    }

    pub(crate) fn validate(&self) -> Result<(), BorrowSafetyError> {
        if self.duplicate_alias { return Err(BorrowSafetyError::SnapshotMismatch); }
        BorrowSafety::validate_active_set(
            self.facts.iter().map(|fact| (fact.source, fact.kind)),
        )
    }

    /// 只准已驗證執行路徑將快照核對真實票據；不能依快照創造票據。
    pub(crate) fn verify_tracker(&self, tracker: &BorrowSafety)
        -> Result<(), BorrowSafetyError> {
        self.validate()?;
        tracker.verify_static_snapshot(self.facts.iter().copied())
    }

    pub(crate) fn facts(&self) -> &[BorrowFact] { &self.facts }
}

/// 借用票據綁定原宣告身分與實際授權次序，避免同名遮蔽或假票據提前解除限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BorrowTicket {
    declaration: DeclarationId,
    serial: u64,
    tracker_id: u64,
    kind: BorrowKind,
}

impl BorrowTicket {
    pub(crate) fn fact(self) -> BorrowFact {
        BorrowFact::new(self.declaration, self.kind)
    }
}

#[derive(Debug)]
pub(crate) struct BorrowSafety {
    active: BTreeMap<DeclarationId, BTreeMap<u64, BorrowKind>>,
    next_serial: u64,
    tracker_id: u64,
}

impl Default for BorrowSafety {
    fn default() -> Self { Self::new() }
}

impl BorrowSafety {
    pub(crate) fn new() -> Self {
        // 超過身份空間時選擇明確停止，不允許回繞後重用既有追蹤器身分。
        let tracker_id = NEXT_TRACKER_ID.try_update(
            Ordering::Relaxed, Ordering::Relaxed,
            |next| next.checked_add(1),
        ).expect("借用追蹤器身分已用盡");
        Self { active: BTreeMap::new(), next_serial: 0, tracker_id }
    }

    /// 單一 HC14-A 衝突規則，供抽象借用票據與真正 .voil 來源共同使用。
    /// 無現存借用時任一權限可取得；共享／共享相容，任何獨占重疊衝突。
    pub(crate) fn validate_new_borrow(
        existing: impl IntoIterator<Item = BorrowKind>, requested: BorrowKind,
    ) -> Result<(), BorrowSafetyError> {
        // 獨占請求只要遇到任一借用即衝突；共享請求僅與獨占衝突。
        // 與既有切片版本的 HC14-A 判定等價，並支援正式 Checker 惰性查詢。
        let conflicts = existing.into_iter().any(|kind| {
            requested == BorrowKind::ExclusiveWrite
                || kind == BorrowKind::ExclusiveWrite
        });
        if conflicts { Err(BorrowSafetyError::BorrowConflict) } else { Ok(()) }
    }

    /// 驗證已建立的活躍借用快照；適用於分支合流的保守聯集。
    /// 不重新簽發 BorrowTicket，也不以同名文字合併不同來源。
    pub(crate) fn validate_active_set(
        loans: impl IntoIterator<Item = (DeclarationId, BorrowKind)>,
    ) -> Result<(), BorrowSafetyError> {
        let mut active = BTreeMap::<DeclarationId, (bool, bool)>::new();
        for (source, kind) in loans {
            let state = active.entry(source).or_default();
            let existing = state.0.then_some(BorrowKind::SharedRead).into_iter()
                .chain(state.1.then_some(BorrowKind::ExclusiveWrite));
            Self::validate_new_borrow(existing, kind)?;
            match kind {
                BorrowKind::SharedRead => state.0 = true,
                BorrowKind::ExclusiveWrite => state.1 = true,
            }
        }
        Ok(())
    }

    /// F06：嚴格核對靜態借用快照與已實際簽發的票據集合。
    /// 以宣告身分、借用種類及重數比較，不重建或簽發票據；
    /// 分支推測產生的借用事實不得在此冒充實際有效的序號。
    pub(crate) fn verify_static_snapshot(
        &self, facts: impl IntoIterator<Item = BorrowFact>,
    ) -> Result<(), BorrowSafetyError> {
        let mut expected = BTreeMap::<DeclarationId, (usize, usize)>::new();
        for fact in facts {
            let entry = expected.entry(fact.source).or_default();
            let count = match fact.kind {
                BorrowKind::SharedRead => &mut entry.0,
                BorrowKind::ExclusiveWrite => &mut entry.1,
            };
            *count = count.checked_add(1).ok_or(BorrowSafetyError::TooManySharedLoans)?;
        }
        let mut actual = BTreeMap::<DeclarationId, (usize, usize)>::new();
        for (&source, loans) in &self.active {
            let mut counts = (0_usize, 0_usize);
            for kind in loans.values() {
                match kind {
                    BorrowKind::SharedRead => counts.0 += 1,
                    BorrowKind::ExclusiveWrite => counts.1 += 1,
                }
            }
            actual.insert(source, counts);
        }
        if expected == actual { Ok(()) }
        else { Err(BorrowSafetyError::SnapshotMismatch) }
    }

    pub(crate) fn acquire(&mut self, declaration: DeclarationId, kind: BorrowKind)
        -> Result<BorrowTicket, BorrowSafetyError> {
        // 直接對票據表惰性遍歷，不因每次借用而配置新的 Vec。
        let existing = self.active.get(&declaration).into_iter()
            .flat_map(|loans| loans.values().copied());
        Self::validate_new_borrow(existing, kind)?;
        let serial = self.next_serial;
        self.next_serial = self.next_serial
            .checked_add(1).ok_or(BorrowSafetyError::TooManySharedLoans)?;
        self.active.entry(declaration).or_default().insert(serial, kind);
        Ok(BorrowTicket { declaration, serial, tracker_id: self.tracker_id, kind })
    }

    /// 對直接讀、寫、複製或 .move() 均先執行此檢查。
    /// 借用關係本身的合法使用不等於原來源可直接被讀取。
    /// HC14-A：由唯一規則判定原所有者能否直接讀、寫、複製或整筆移動。
    /// 私有 Checker 的借用有效期已由 P15 解析為活躍旗標，
    /// 票據追蹤器則由實際票據表得出同一布林事實；兩者不得各自改規則。
    pub(crate) fn validate_direct_access(has_active_loan: bool)
        -> Result<(), BorrowSafetyError> {
        if has_active_loan { Err(BorrowSafetyError::SourceIsBorrowed) }
        else { Ok(()) }
    }

    pub(crate) fn check_direct_access(&self, declaration: DeclarationId)
        -> Result<(), BorrowSafetyError> {
        Self::validate_direct_access(
            self.active.get(&declaration).is_some_and(|loans| !loans.is_empty())
        )
    }

    /// G18／P18.4：函式參數或方法 self 的真正讀寫只能
    /// 憑仍存在的 BorrowTicket 授權。追蹤器、來源、序號和
    /// 借用種類必須同時與本次動態追蹤表完全吻合；共享票據
    /// 只能唯讀，獨占票據可讀寫。這不是原所有者的直接存取。
    pub(crate) fn check_ticket_access(
        &self, ticket: BorrowTicket, requested: BorrowKind,
    ) -> Result<DeclarationId, BorrowSafetyError> {
        if ticket.tracker_id != self.tracker_id {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        let active = self.active.get(&ticket.declaration)
            .and_then(|loans| loans.get(&ticket.serial));
        if active != Some(&ticket.kind) {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        if requested == BorrowKind::ExclusiveWrite
            && ticket.kind != BorrowKind::ExclusiveWrite {
            return Err(BorrowSafetyError::BorrowConflict);
        }
        Ok(ticket.declaration)
    }

    pub(crate) fn release(&mut self, ticket: BorrowTicket)
        -> Result<(), BorrowSafetyError> {
        if ticket.tracker_id != self.tracker_id {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        let Some(loans) = self.active.get_mut(&ticket.declaration) else {
            return Err(BorrowSafetyError::InvalidRelease);
        };
        if loans.get(&ticket.serial) != Some(&ticket.kind) {
            return Err(BorrowSafetyError::InvalidRelease);
        }
        self.commit_release_checked(ticket);
        Ok(())
    }

    /// 一張票據通過追蹤器、來源、序號及借用種類預檢後的不可失敗提交。
    /// 僅供 BorrowSafety 已驗證的單張釋放及整批釋放使用。
    fn commit_release_checked(&mut self, ticket: BorrowTicket) {
        let loans = self.active.get_mut(&ticket.declaration)
            .expect("已驗證借用來源仍存在");
        debug_assert_eq!(loans.get(&ticket.serial), Some(&ticket.kind));
        let previous = loans.remove(&ticket.serial);
        debug_assert_eq!(previous, Some(ticket.kind));
        if loans.is_empty() { self.active.remove(&ticket.declaration); }
    }

    /// 方法／函式作用域出口依已核准歸還點明確釋放。
    /// 必須先檢查整批票據均合法，避免錯誤路徑中只釋放部分借用。
    /// 不在這裡推定一般借用最後使用位置。
    pub(crate) fn finish_scope(&mut self, tickets: &[BorrowTicket])
        -> Result<(), BorrowSafetyError> {
        let mut seen = std::collections::BTreeSet::new();
        for ticket in tickets {
            if ticket.tracker_id != self.tracker_id {
                return Err(BorrowSafetyError::InvalidRelease);
            }
            if !seen.insert((ticket.declaration, ticket.serial)) {
                return Err(BorrowSafetyError::InvalidRelease);
            }
            let actual = self.active.get(&ticket.declaration)
                .and_then(|loans| loans.get(&ticket.serial));
            if actual != Some(&ticket.kind) {
                return Err(BorrowSafetyError::InvalidRelease);
            }
        }
        for ticket in tickets.iter().rev() {
            // 上方預檢完成後，同一批次不會重複票據，故以下不應失敗。
            self.commit_release_checked(*ticket);
        }
        Ok(())
    }

    /// F06／F09：正式 Checker 先依已解析詞彙 DeclarationId 建立靜態
    /// 借用事實，與本次呼叫真正簽出的票據逐筆核對後才整批歸還。
    /// 此處不從 BorrowTicket.fact() 倒推靜態證據；任何快照差異都
    /// 必須在釋放前拒絕，不得產生半批次解鎖。
    pub(crate) fn finish_scope_verified(
        &mut self, tickets: &[BorrowTicket], checked: &[BorrowFact],
    ) -> Result<(), BorrowSafetyError> {
        self.verify_static_snapshot(checked.iter().copied())?;
        // 即使靜態快照與 tracker 完全吻合，呼叫者仍可能只傳入
        // 一部分真正票據；禁止在「已核對快照」的包裝介面回傳
        // 假完成。票據完整性必須在任何解鎖前以唯讀條件證明。
        // finish_scope 已原子預檢每張票據的 tracker／序號／來源、
        // 權限及不可重複；數量相同便代表恰好覆蓋全部活躍票據。
        if tickets.len() != checked.len() {
            return Err(BorrowSafetyError::SnapshotMismatch);
        }
        self.finish_scope(tickets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f08_失效合流快照不可對無關來源發放新借用或直接操作授權() {
        let a = DeclarationId(4201);
        let unrelated = DeclarationId(4202);
        let invalid = StaticBorrowSnapshot::from_active([
            BorrowFact::new(a, BorrowKind::SharedRead),
            BorrowFact::new(a, BorrowKind::ExclusiveWrite),
        ]);
        assert_eq!(invalid.validate(), Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(invalid.validate_acquire(unrelated, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(invalid.validate_direct(unrelated),
            Err(BorrowSafetyError::BorrowConflict));
        assert!(!invalid.has_active(unrelated));
        let clean = StaticBorrowSnapshot::default();
        assert_eq!(clean.validate_acquire(unrelated, BorrowKind::ExclusiveWrite), Ok(()));
        assert_eq!(clean.validate_direct(unrelated), Ok(()));
    }

    #[test]
    fn f08_靜態合流與新借用及直接讀寫都引用同一宣告身分快照() {
        let outer = DeclarationId(401);
        let shadow = DeclarationId(407);
        let facts = StaticBorrowSnapshot::from_active([
            BorrowFact::new(outer, BorrowKind::SharedRead),
            BorrowFact::new(outer, BorrowKind::SharedRead),
            BorrowFact::new(shadow, BorrowKind::ExclusiveWrite),
        ]);
        assert!(facts.has_active(outer));
        assert!(facts.has_active(shadow));
        assert!(!facts.has_active(DeclarationId(408)));
        assert_eq!(facts.kinds_for(outer).count(), 2);
        assert_eq!(facts.validate_acquire(outer, BorrowKind::SharedRead), Ok(()));
        assert_eq!(facts.validate_acquire(outer, BorrowKind::ExclusiveWrite),
            Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(facts.validate_acquire(shadow, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(facts.validate_direct(outer), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(facts.validate_direct(shadow), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(facts.validate_direct(DeclarationId(408)), Ok(()));
        assert_eq!(facts.validate_acquire(DeclarationId(408), BorrowKind::ExclusiveWrite),
            Ok(()));
    }

    #[test]
    fn f06_正式靜態快照合流與真實票據來源權限一致且不可造假() {
        let source = DeclarationId(77);
        let sibling = DeclarationId(79);
        let mut tracker = BorrowSafety::new();
        let first = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let second = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let actual = StaticBorrowSnapshot::from_active([first.fact(), second.fact()]);
        assert_eq!(actual.validate(), Ok(()));
        assert_eq!(actual.verify_tracker(&tracker), Ok(()));
        let missing = StaticBorrowSnapshot::from_active([first.fact()]);
        assert_eq!(missing.validate(), Ok(()));
        assert_eq!(missing.verify_tracker(&tracker),
            Err(BorrowSafetyError::SnapshotMismatch));
        let wrong_source = StaticBorrowSnapshot::from_active([
            first.fact(), BorrowFact::new(sibling, BorrowKind::SharedRead),
        ]);
        assert_eq!(wrong_source.verify_tracker(&tracker),
            Err(BorrowSafetyError::SnapshotMismatch));
        let conflicting = StaticBorrowSnapshot::from_active([
            first.fact(), BorrowFact::new(source, BorrowKind::ExclusiveWrite),
        ]);
        assert_eq!(conflicting.validate(), Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(conflicting.verify_tracker(&tracker),
            Err(BorrowSafetyError::BorrowConflict));
        // 快照檢查失敗仍不得對實際追蹤器產生副作用。
        assert_eq!(tracker.check_direct_access(source),
            Err(BorrowSafetyError::SourceIsBorrowed));
        tracker.finish_scope(&[first, second]).unwrap();
        assert_eq!(actual.verify_tracker(&tracker),
            Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(StaticBorrowSnapshot::default().verify_tracker(&tracker), Ok(()));
    }

    #[test]
    fn f06_f09_已解析快照與真實票據不符時批次歸還無副作用() {
        let source = DeclarationId(2001);
        let shadow = DeclarationId(2002);
        let mut tracker = BorrowSafety::new();
        let a = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let b = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let valid = [
            BorrowFact::new(source, BorrowKind::SharedRead),
            BorrowFact::new(source, BorrowKind::SharedRead),
        ];
        let invalid = [
            BorrowFact::new(source, BorrowKind::SharedRead),
            BorrowFact::new(shadow, BorrowKind::SharedRead),
        ];
        assert_eq!(tracker.finish_scope_verified(&[a, b], &invalid),
            Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(tracker.finish_scope_verified(&[a], &valid),
            Err(BorrowSafetyError::SnapshotMismatch),
            "來源／權限快照即使正確，也不得只歸還部分真實票據");
        assert_eq!(tracker.finish_scope_verified(&[], &valid),
            Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(tracker.finish_scope_verified(&[a, a], &valid),
            Err(BorrowSafetyError::InvalidRelease),
            "不允許重複票據冒充完整共享借用重數");
        assert_eq!(tracker.check_direct_access(source),
            Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(tracker.finish_scope_verified(&[a, b], &valid), Ok(()));
        assert_eq!(tracker.check_direct_access(source), Ok(()));
        assert_eq!(tracker.finish_scope_verified(&[a, b], &valid),
            Err(BorrowSafetyError::SnapshotMismatch),
            "已歸還票據與過期靜態快照都不能再次解鎖");
    }

    #[test]
    fn f06_靜態快照須與真實票據的來源權限及共享重數完全相等() {
        let mut tracker = BorrowSafety::new();
        let outer = DeclarationId(17);
        let shadow = DeclarationId(31);
        assert_eq!(tracker.verify_static_snapshot([]), Ok(()));
        let one = tracker.acquire(outer, BorrowKind::SharedRead).unwrap();
        let two = tracker.acquire(outer, BorrowKind::SharedRead).unwrap();
        let three = tracker.acquire(shadow, BorrowKind::ExclusiveWrite).unwrap();
        let accurate = [one.fact(), two.fact(), three.fact()];
        assert_eq!(tracker.verify_static_snapshot(accurate), Ok(()));
        for incorrect in [
            vec![one.fact(), three.fact()],
            vec![one.fact(), one.fact(), one.fact(), three.fact()],
            vec![one.fact(), two.fact(), BorrowFact::new(outer, BorrowKind::ExclusiveWrite)],
            vec![one.fact(), two.fact(), BorrowFact::new(DeclarationId(30), BorrowKind::ExclusiveWrite)],
        ] {
            assert_eq!(tracker.verify_static_snapshot(incorrect),
                Err(BorrowSafetyError::SnapshotMismatch));
            assert_eq!(tracker.check_direct_access(outer),
                Err(BorrowSafetyError::SourceIsBorrowed));
            assert_eq!(tracker.check_direct_access(shadow),
                Err(BorrowSafetyError::SourceIsBorrowed));
        }
        tracker.finish_scope(&[one, two, three]).unwrap();
        assert_eq!(tracker.verify_static_snapshot([]), Ok(()));
        assert_eq!(tracker.verify_static_snapshot([one.fact()]),
            Err(BorrowSafetyError::SnapshotMismatch));
    }

    #[test]
    fn 真實票據與靜態借用事實共用同一來源身分及權限型別() {
        let source = DeclarationId(217);
        let mut tracker = BorrowSafety::new();
        let ticket = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(ticket.fact(), BorrowFact::new(source, BorrowKind::SharedRead));
        tracker.release(ticket).unwrap();
        assert_eq!(tracker.check_direct_access(source), Ok(()));
    }

    #[test]
    fn 分支合流活躍借用快照以來源身分檢查共享及獨占一致性() {
        let a = DeclarationId(90);
        let b = DeclarationId(91);
        for allowed in [
            vec![(a, BorrowKind::SharedRead), (a, BorrowKind::SharedRead)],
            vec![(a, BorrowKind::ExclusiveWrite), (b, BorrowKind::SharedRead)],
            vec![(a, BorrowKind::SharedRead), (b, BorrowKind::ExclusiveWrite)],
        ] {
            assert_eq!(BorrowSafety::validate_active_set(allowed), Ok(()));
        }
        for forbidden in [
            vec![(a, BorrowKind::SharedRead), (a, BorrowKind::ExclusiveWrite)],
            vec![(a, BorrowKind::ExclusiveWrite), (a, BorrowKind::SharedRead)],
            vec![(a, BorrowKind::ExclusiveWrite), (a, BorrowKind::ExclusiveWrite)],
        ] {
            assert_eq!(BorrowSafety::validate_active_set(forbidden),
                Err(BorrowSafetyError::BorrowConflict));
        }
    }

    #[test]
    fn 靜態來源活躍旗標與真正票據的直接存取規則必須相同() {
        let owner=DeclarationId(48);
        let mut tickets=BorrowSafety::new();
        assert_eq!(BorrowSafety::validate_direct_access(false),
            tickets.check_direct_access(owner));
        let borrow=tickets.acquire(owner,BorrowKind::SharedRead).unwrap();
        assert_eq!(BorrowSafety::validate_direct_access(true),
            tickets.check_direct_access(owner));
        tickets.release(borrow).unwrap();
        assert_eq!(BorrowSafety::validate_direct_access(false),
            tickets.check_direct_access(owner));
    }

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
    fn hc14_批次歸還發現無效票據時不得提前解除其他來源() {
        let mut safety = BorrowSafety::new();
        let a = DeclarationId(17);
        let b = DeclarationId(19);
        let first = safety.acquire(a, BorrowKind::SharedRead).unwrap();
        let second = safety.acquire(b, BorrowKind::ExclusiveWrite).unwrap();
        let fake = BorrowTicket { declaration: DeclarationId(99), ..first };
        assert_eq!(safety.finish_scope(&[fake, second]),
            Err(BorrowSafetyError::InvalidRelease));
        // 舊版在反向歸還時先釋放 second，再碰到 fake 才報錯。
        assert_eq!(safety.check_direct_access(a), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.check_direct_access(b), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.acquire(b, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        safety.finish_scope(&[first, second]).unwrap();
        assert!(safety.check_direct_access(a).is_ok());
        assert!(safety.check_direct_access(b).is_ok());
    }

    #[test]
    fn hc14_重複票據與錯誤種類不會在批次結束途中解鎖() {
        let mut safety = BorrowSafety::new();
        let id = DeclarationId(27);
        let loan = safety.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.finish_scope(&[loan, loan]),
            Err(BorrowSafetyError::InvalidRelease));
        assert!(safety.check_direct_access(id).is_err());
        let wrong_kind = BorrowTicket { kind: BorrowKind::SharedRead, ..loan };
        assert_eq!(safety.finish_scope(&[loan, wrong_kind]),
            Err(BorrowSafetyError::InvalidRelease));
        assert!(safety.check_direct_access(id).is_err());
        safety.finish_scope(&[loan]).unwrap();
        assert_eq!(safety.finish_scope(&[loan]),
            Err(BorrowSafetyError::InvalidRelease));
    }

    #[test]
    fn hc14_不同追蹤器的借用票據即使宣告位置與序號相同也不能互換() {
        let id = DeclarationId(31);
        let mut first = BorrowSafety::new();
        let mut second = BorrowSafety::new();
        let first_ticket = first.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        let second_ticket = second.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(first_ticket.serial, second_ticket.serial);
        assert_ne!(first_ticket.tracker_id, second_ticket.tracker_id);
        assert_eq!(second.release(first_ticket), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(second.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(first.release(second_ticket), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(first.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        first.finish_scope(&[first_ticket]).unwrap();
        second.finish_scope(&[second_ticket]).unwrap();
    }

    #[test]
    fn hc14_批次中混入跨追蹤器票據不得部分解除其餘借用() {
        let id = DeclarationId(40);
        let mut first = BorrowSafety::new();
        let mut second = BorrowSafety::new();
        let first_ticket = first.acquire(id, BorrowKind::SharedRead).unwrap();
        let second_ticket = second.acquire(id, BorrowKind::SharedRead).unwrap();
        let third_ticket = second.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(second.finish_scope(&[first_ticket, third_ticket]),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(second.check_direct_access(id), Err(BorrowSafetyError::SourceIsBorrowed));
        second.finish_scope(&[second_ticket, third_ticket]).unwrap();
        assert!(second.check_direct_access(id).is_ok());
        first.release(first_ticket).unwrap();
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

#[cfg(test)]
mod r03_f06_b05_別名身分獨立證據 {
    use super::*;

    #[test]
    fn f06_b05_不同別名共享同來源保留重數並對帳真票據() {
        let source = DeclarationId(900);
        let mut tracker = BorrowSafety::new();
        let one = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let two = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let snapshot = StaticBorrowSnapshot::from_aliases([
            (DeclarationId(901), one.fact(), true),
            (DeclarationId(902), two.fact(), true),
        ]);
        assert_eq!(snapshot.validate(), Ok(()));
        assert_eq!(snapshot.kinds_for(source).count(), 2);
        assert_eq!(snapshot.verify_tracker(&tracker), Ok(()));
        tracker.finish_scope_verified(&[one, two], snapshot.facts()).unwrap();
        assert_eq!(snapshot.verify_tracker(&tracker), Err(BorrowSafetyError::SnapshotMismatch));
    }

    #[test]
    fn f06_b05_重複靜態別名不允許冒充兩張合法共享票據() {
        let source = DeclarationId(920);
        let mut tracker = BorrowSafety::new();
        let one = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let two = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        let alias = DeclarationId(921);
        let fake = StaticBorrowSnapshot::from_aliases([
            (alias, one.fact(), true), (alias, two.fact(), true),
        ]);
        assert_eq!(fake.validate(), Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(fake.validate_acquire(DeclarationId(922), BorrowKind::SharedRead),
            Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(fake.validate_direct(DeclarationId(922)),
            Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(fake.verify_tracker(&tracker), Err(BorrowSafetyError::SnapshotMismatch));
        // 假快照不能影響實際票據來源，也不能半批次歸還。
        assert_eq!(tracker.check_direct_access(source), Err(BorrowSafetyError::SourceIsBorrowed));
        tracker.finish_scope(&[one, two]).unwrap();
    }

    #[test]
    fn f06_b05_失效別名不被算作活躍借用或虛構票據() {
        let source = DeclarationId(930);
        let expired = StaticBorrowSnapshot::from_aliases([
            (DeclarationId(931), BorrowFact::new(source, BorrowKind::SharedRead), false),
        ]);
        assert_eq!(expired.validate(), Ok(()));
        assert_eq!(expired.validate_direct(source), Ok(()));
        assert_eq!(expired.kinds_for(source).count(), 0);
        let mut tracker = BorrowSafety::new();
        assert_eq!(expired.verify_tracker(&tracker), Ok(()));
        let ticket = tracker.acquire(source, BorrowKind::SharedRead).unwrap();
        assert_eq!(expired.verify_tracker(&tracker), Err(BorrowSafetyError::SnapshotMismatch));
        assert_eq!(tracker.check_direct_access(source), Err(BorrowSafetyError::SourceIsBorrowed));
        tracker.release(ticket).unwrap();
    }

    #[test]
    fn f06_b05_遮蔽別名與參數自身借用使用真宣告身分() {
        let outer = DeclarationId(940);
        let shadow = DeclarationId(941);
        let parameter = DeclarationId(942);
        let snapshot = StaticBorrowSnapshot::from_aliases([
            (outer, BorrowFact::new(parameter, BorrowKind::SharedRead), true),
            (shadow, BorrowFact::new(parameter, BorrowKind::SharedRead), false),
            (parameter, BorrowFact::new(parameter, BorrowKind::SharedRead), true),
        ]);
        assert_eq!(snapshot.validate(), Ok(()));
        assert_eq!(snapshot.kinds_for(parameter).count(), 2);
        assert_eq!(snapshot.validate_direct(parameter), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(snapshot.validate_direct(shadow), Ok(()));
        assert_eq!(snapshot.validate_acquire(parameter, BorrowKind::ExclusiveWrite),
            Err(BorrowSafetyError::BorrowConflict));
    }


}

#[cfg(test)]
mod p14_單一借用衝突規則 {
    use super::*;

    #[test]
    fn 抽象票據與來源衝突規則維持相同共享獨占矩陣() {
        assert_eq!(BorrowSafety::validate_new_borrow([].into_iter(),BorrowKind::SharedRead),Ok(()));
        assert_eq!(BorrowSafety::validate_new_borrow([].into_iter(),BorrowKind::ExclusiveWrite),Ok(()));
        assert_eq!(BorrowSafety::validate_new_borrow(
            [BorrowKind::SharedRead,BorrowKind::SharedRead].into_iter(),BorrowKind::SharedRead),Ok(()));
        for (existing, requested) in [
            (vec![BorrowKind::SharedRead],BorrowKind::ExclusiveWrite),
            (vec![BorrowKind::ExclusiveWrite],BorrowKind::SharedRead),
            (vec![BorrowKind::ExclusiveWrite],BorrowKind::ExclusiveWrite),
        ] {
            assert_eq!(BorrowSafety::validate_new_borrow(existing.iter().copied(),requested),
                Err(BorrowSafetyError::BorrowConflict));
        }
    }
}

#[cfg(test)]
mod p15_6_一般最後使用點與方法作用域歸還區分 {
    use super::*;
    #[test]
    fn 一般別名最後使用點可單獨歸還而方法_self_須等作用域結束() {
        let mut safety = BorrowSafety::new();
        let ordinary = DeclarationId(201);
        let method_self = DeclarationId(202);
        let ordinary_ticket = safety.acquire(ordinary, BorrowKind::SharedRead).unwrap();
        let self_ticket = safety.acquire(method_self, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.check_direct_access(ordinary), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.check_direct_access(method_self), Err(BorrowSafetyError::SourceIsBorrowed));
        // 一般借用已達最後合法使用點，只解除自己的票據。
        safety.release(ordinary_ticket).unwrap();
        assert!(safety.check_direct_access(ordinary).is_ok());
        assert_eq!(safety.check_direct_access(method_self), Err(BorrowSafetyError::SourceIsBorrowed));
        assert_eq!(safety.acquire(method_self, BorrowKind::SharedRead),
            Err(BorrowSafetyError::BorrowConflict));
        // 函式／方法接收者借用須在其作用域出口整批歸還。
        safety.finish_scope(&[self_ticket]).unwrap();
        assert!(safety.check_direct_access(method_self).is_ok());
    }
    #[test]
    fn 方法作用域退出失敗不可提前解鎖接收者() {
        let mut safety = BorrowSafety::new();
        let receiver = DeclarationId(203);
        let ticket = safety.acquire(receiver, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(safety.finish_scope(&[ticket, ticket]), Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(safety.check_direct_access(receiver), Err(BorrowSafetyError::SourceIsBorrowed));
        safety.finish_scope(&[ticket]).unwrap();
        assert!(safety.check_direct_access(receiver).is_ok());
    }

    #[test]
    fn g18_d_真共享票據只准借用唯讀而不授權原持有者存取() {
        let id = DeclarationId(8401);
        let mut tracker = BorrowSafety::new();
        let ticket = tracker.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::SharedRead), Ok(id));
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::ExclusiveWrite),
            Err(BorrowSafetyError::BorrowConflict));
        assert_eq!(tracker.check_direct_access(id),
            Err(BorrowSafetyError::SourceIsBorrowed));
        tracker.release(ticket).unwrap();
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::SharedRead),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(tracker.check_direct_access(id), Ok(()));
    }

    #[test]
    fn g18_d_獨占票據確實能授權借用讀寫但離域後失效() {
        let id = DeclarationId(8402);
        let mut tracker = BorrowSafety::new();
        let ticket = tracker.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::SharedRead), Ok(id));
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::ExclusiveWrite), Ok(id));
        tracker.finish_scope_verified(&[ticket],
            &[BorrowFact::new(id, BorrowKind::ExclusiveWrite)]).unwrap();
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::SharedRead),
            Err(BorrowSafetyError::InvalidRelease));
    }

    #[test]
    fn g18_d_不同追蹤器票據不能作另一次函式的參數存取能力() {
        let id = DeclarationId(8403);
        let mut first = BorrowSafety::new();
        let mut second = BorrowSafety::new();
        let original = first.acquire(id, BorrowKind::SharedRead).unwrap();
        let other = second.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(second.check_ticket_access(original, BorrowKind::SharedRead),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(first.check_ticket_access(other, BorrowKind::SharedRead),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(first.check_ticket_access(original, BorrowKind::SharedRead), Ok(id));
    }

    #[test]
    fn g18_d_多個獨立共享票據其中一個歸還後不得憑過期序號讀取() {
        let id = DeclarationId(8404);
        let mut tracker = BorrowSafety::new();
        let a = tracker.acquire(id, BorrowKind::SharedRead).unwrap();
        let b = tracker.acquire(id, BorrowKind::SharedRead).unwrap();
        tracker.release(a).unwrap();
        assert_eq!(tracker.check_ticket_access(a, BorrowKind::SharedRead),
            Err(BorrowSafetyError::InvalidRelease));
        assert_eq!(tracker.check_ticket_access(b, BorrowKind::SharedRead), Ok(id));
        assert_eq!(tracker.check_direct_access(id),
            Err(BorrowSafetyError::SourceIsBorrowed));
        tracker.release(b).unwrap();
        assert_eq!(tracker.check_direct_access(id), Ok(()));
    }
}
