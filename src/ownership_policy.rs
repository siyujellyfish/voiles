//! P13 已正式核准的具體複製能力政策（HA13-A）。
//! 只授權 int／bool；其餘葉型別未決，不把未決誤當永久禁止。
//! struct 的整體複製由 S19 各欄位能力遞迴合取處理。
#![allow(dead_code)]

use crate::ownership_model::CopyPermission;

/// HA13-A：已核准型別才可隱含複製；未知能力不許推定。
pub(crate) fn approved_leaf_copy_permission(ty: &str) -> CopyPermission {
    match ty {
        "int" | "bool" => CopyPermission::Allowed,
        _ => CopyPermission::Undecided,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ha13_僅批准_int_bool_葉型別預設複製() {
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
        for ty in ["str", "float", "list[int]", "dict[str,int]", "Unknown", "Sample"] {
            assert_eq!(approved_leaf_copy_permission(ty), CopyPermission::Undecided);
        }
    }

    #[test]
    fn ha13_未決不能被當成可複製或已確認不可複製() {
        assert!(crate::ownership_model::ValueSlot::initialized(
            approved_leaf_copy_permission("str")).duplicate().is_err());
    }
}
