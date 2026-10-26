//! P13 所有權狀態模型（尚未接入完整語言）。
//! 能力由已核准的葉型別政策或已驗證 S19 結構能力提供；str 已明確非 Copyable。
//! 不處理借用、執行時資源清理、欄位存取或生命週期。
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyPermission { Allowed, Forbidden, Undecided }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueState {
    Available, Uninitialized, Moved, MayBeUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelError {
    CopyForbidden, CopyUndecided, UninitializedRead, MovedRead,
    UncertainRead, CapabilityConflict, NoReachableBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueSlot {
    state: ValueState,
    copy_permission: CopyPermission,
}

impl ValueSlot {
    pub(crate) fn initialized(permission: CopyPermission) -> Self {
        Self { state: ValueState::Available, copy_permission: permission }
    }

    pub(crate) fn uninitialized(permission: CopyPermission) -> Self {
        Self { state: ValueState::Uninitialized, copy_permission: permission }
    }

    pub(crate) fn state(self) -> ValueState { self.state }

    pub(crate) fn read(self) -> Result<(), ModelError> {
        match self.state {
            ValueState::Available => Ok(()),
            ValueState::Uninitialized => Err(ModelError::UninitializedRead),
            ValueState::Moved => Err(ModelError::MovedRead),
            ValueState::MayBeUnavailable => Err(ModelError::UncertainRead),
        }
    }

    /// 沒有核准複製能力時，不得因既有「一般使用預設複製」就放行。
    pub(crate) fn duplicate(self) -> Result<Self, ModelError> {
        self.read()?;
        match self.copy_permission {
            CopyPermission::Allowed => Ok(self),
            CopyPermission::Forbidden => Err(ModelError::CopyForbidden),
            CopyPermission::Undecided => Err(ModelError::CopyUndecided),
        }
    }

    /// 模擬整筆值明確轉移；非語言 AST 中的 move 表達式。
    pub(crate) fn move_whole(&mut self) -> Result<Self, ModelError> {
        self.read()?;
        let target = Self::initialized(self.copy_permission);
        self.state = ValueState::Moved;
        Ok(target)
    }

    /// 此動作必須由將來確定「賦值合法／無衝突借用」的分析器呼叫。
    pub(crate) fn mark_reinitialized(&mut self) {
        self.state = ValueState::Available;
    }

    /// 呼叫端供給各「仍可達」分支；None 表示已 return 的路徑。
    pub(crate) fn join_reachable(branches: &[Option<Self>]) -> Result<Self, ModelError> {
        let mut iter = branches.iter().flatten().copied();
        let mut result = iter.next().ok_or(ModelError::NoReachableBranch)?;
        for next in iter {
            if result.copy_permission != next.copy_permission {
                return Err(ModelError::CapabilityConflict);
            }
            if result.state != next.state {
                result.state = ValueState::MayBeUnavailable;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未決能力與禁止複製的資源都不可偷複製() {
        let unknown = ValueSlot::initialized(CopyPermission::Undecided);
        assert_eq!(unknown.duplicate(), Err(ModelError::CopyUndecided));
        assert_eq!(unknown.read(), Ok(()));
        assert_eq!(
            ValueSlot::initialized(CopyPermission::Forbidden).duplicate(),
            Err(ModelError::CopyForbidden)
        );
    }

    #[test]
    fn 只有外部允許複製才能模擬兩個獨立有效值() {
        let mut original = ValueSlot::initialized(CopyPermission::Allowed);
        let copy = original.duplicate().unwrap();
        let new_owner = original.move_whole().unwrap();
        assert_eq!(original.read(), Err(ModelError::MovedRead));
        assert!(copy.read().is_ok());
        assert!(new_owner.read().is_ok());
    }

    #[test]
    fn 明確移動後不可再讀或再移動_重新初始化才恢復() {
        let mut source = ValueSlot::initialized(CopyPermission::Forbidden);
        assert!(source.move_whole().unwrap().read().is_ok());
        assert_eq!(source.read(), Err(ModelError::MovedRead));
        assert_eq!(source.move_whole(), Err(ModelError::MovedRead));
        source.mark_reinitialized();
        assert!(source.read().is_ok());
    }

    #[test]
    fn 尚未初始化時不得讀取或移動() {
        let mut source = ValueSlot::uninitialized(CopyPermission::Forbidden);
        assert_eq!(source.read(), Err(ModelError::UninitializedRead));
        assert_eq!(source.move_whole(), Err(ModelError::UninitializedRead));
    }

    #[test]
    fn 分支合流有一條移動就不可宣稱來源仍可讀() {
        let mut moved = ValueSlot::initialized(CopyPermission::Undecided);
        moved.move_whole().unwrap();
        let ready = ValueSlot::initialized(CopyPermission::Undecided);
        let merged = ValueSlot::join_reachable(&[Some(moved), Some(ready)]).unwrap();
        assert_eq!(merged.state(), ValueState::MayBeUnavailable);
        assert_eq!(merged.read(), Err(ModelError::UncertainRead));
        assert!(ValueSlot::join_reachable(&[Some(ready), Some(ready)]).unwrap().read().is_ok());
    }

    #[test]
    fn 不可達路徑不納入合流而沒有可達後繼時回報模型錯誤() {
        let ready = ValueSlot::initialized(CopyPermission::Undecided);
        assert_eq!(ValueSlot::join_reachable(&[None, Some(ready)]), Ok(ready));
        assert_eq!(ValueSlot::join_reachable(&[None, None]), Err(ModelError::NoReachableBranch));
    }

    #[test]
    fn 不同能力不能在合流時猜測其複製許可() {
        let a = ValueSlot::initialized(CopyPermission::Allowed);
        let b = ValueSlot::initialized(CopyPermission::Undecided);
        assert_eq!(ValueSlot::join_reachable(&[Some(a), Some(b)]),
            Err(ModelError::CapabilityConflict));
    }

    #[test]
    fn 不把完整語言_move_borrow_偷偷開放到_mvp_a() {
        use crate::{lexer, parser, source};
        for method in ["move", "borrow"] {
            let code = format!(
                "fn main()->int{{\nvar n:int = 1\nreturn n.{method}()\n}}\n"
            );
            let source = source::Source::new(code.into_bytes());
            let tokens = lexer::scan(&source);
            assert!(tokens.diagnostics.is_empty());
            assert!(parser::parse(&source, &tokens.tokens).is_err());
        }
    }
}
