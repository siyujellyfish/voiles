//! P13：已核准的結構體整筆複製／移動及方法接收者限制。
//! 僅供內部型別安全試驗；不假設個別欄位型別 Copyable，也不擴張 MVP-A。
#![allow(dead_code)]

use crate::ownership_model::CopyPermission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructOwnershipError {
    FieldCopyForbidden,
    FieldCopyUndecided,
    PartialFieldMoveForbidden,
    ConsumingSelfForbidden,
}

/// S19：只有每一個欄位皆經型別檢查確定可複製，才允許結構實例整體複製。
/// 未決能力不得被空泛的「預設複製」轉為允許；空欄位集合僅代表所有條件已滿足。
pub(crate) fn whole_struct_copy(fields: &[CopyPermission])
    -> Result<(), StructOwnershipError> {
    if fields.iter().any(|p| *p == CopyPermission::Forbidden) {
        Err(StructOwnershipError::FieldCopyForbidden)
    } else if fields.iter().any(|p| *p == CopyPermission::Undecided) {
        Err(StructOwnershipError::FieldCopyUndecided)
    } else {
        Ok(())
    }
}

/// S21：不得單獨移動結構的某個欄位。這裡只審核移動單位，
/// 所有者是否已失效及目前借用是否合法須由前後階段另外證明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveTarget { WholeBinding, SingleField }

pub(crate) fn check_struct_move_target(target: MoveTarget)
    -> Result<(), StructOwnershipError> {
    match target {
        MoveTarget::WholeBinding => Ok(()),
        MoveTarget::SingleField => Err(StructOwnershipError::PartialFieldMoveForbidden),
    }
}

/// S12／S15：方法接收者 self 只能借用，不得消耗或移動整個 self。
/// 可變借用、同時別名及借用有效期不屬於本檢查的核准範圍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfAction { Borrow, ConsumeWhole }

pub(crate) fn check_method_self(action: SelfAction)
    -> Result<(), StructOwnershipError> {
    match action {
        SelfAction::Borrow => Ok(()),
        SelfAction::ConsumeWhole => Err(StructOwnershipError::ConsumingSelfForbidden),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CopyPermission::{Allowed, Forbidden, Undecided};

    #[test]
    fn s19_所有欄位均獲准複製才允許整體複製() {
        assert_eq!(whole_struct_copy(&[Allowed, Allowed]), Ok(()));
        assert_eq!(whole_struct_copy(&[Allowed, Forbidden]),
            Err(StructOwnershipError::FieldCopyForbidden));
        assert_eq!(whole_struct_copy(&[Allowed, Undecided]),
            Err(StructOwnershipError::FieldCopyUndecided));
        assert_eq!(whole_struct_copy(&[Undecided, Forbidden]),
            Err(StructOwnershipError::FieldCopyForbidden));
    }

    #[test]
    fn s21_禁止移動單一欄位而整筆移動仍須後續狀態查核() {
        assert_eq!(check_struct_move_target(MoveTarget::WholeBinding), Ok(()));
        assert_eq!(check_struct_move_target(MoveTarget::SingleField),
            Err(StructOwnershipError::PartialFieldMoveForbidden));
    }

    #[test]
    fn s12_s15_方法僅能借用_self_不得消耗整體() {
        assert_eq!(check_method_self(SelfAction::Borrow), Ok(()));
        assert_eq!(check_method_self(SelfAction::ConsumeWhole),
            Err(StructOwnershipError::ConsumingSelfForbidden));
    }
}
