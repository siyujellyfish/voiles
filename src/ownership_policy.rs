//! P13 已正式核准的葉型別複製政策（HA13-A、str 方案 A）。
//! int／bool 可隱含複製；str 明確不可複製；其他葉型別仍未決。
//! 這只界定 Copyable 能力，不定義 str 的儲存、釋放或借用有效期。
#![allow(dead_code)]

use crate::ownership_model::CopyPermission;

/// str 方案 A：非 Copyable。未明確核准的其他葉型別不可推定禁用。
pub(crate) fn approved_leaf_copy_permission(ty: &str) -> CopyPermission {
    match ty {
        "int" | "bool" => CopyPermission::Allowed,
        "str" => CopyPermission::Forbidden,
        _ => CopyPermission::Undecided,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::{ModelError, ValueSlot};

    #[test]
    fn ha13_核准_int_bool_與_str_禁止複製_不擴及其他型別() {
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
        for ty in ["float", "list[int]", "dict[str,int]", "Unknown", "Sample"] {
            assert_eq!(approved_leaf_copy_permission(ty), CopyPermission::Undecided);
        }
    }

    #[test]
    fn str_禁止隱含複製_但具名整筆明確移動與重新初始化可用() {
        let mut source = ValueSlot::initialized(approved_leaf_copy_permission("str"));
        assert_eq!(source.duplicate(), Err(ModelError::CopyForbidden));
        assert!(source.read().is_ok());
        let moved = source.move_whole().unwrap();
        assert!(moved.read().is_ok());
        assert_eq!(source.read(), Err(ModelError::MovedRead));
        source.mark_reinitialized();
        assert!(source.read().is_ok());
        assert_eq!(source.duplicate(), Err(ModelError::CopyForbidden));
    }

    #[test]
    fn str_已禁止與其他型別未決分別保留不同靜態錯誤() {
        assert_eq!(ValueSlot::initialized(
            approved_leaf_copy_permission("str")).duplicate(),
            Err(ModelError::CopyForbidden));
        assert_eq!(ValueSlot::initialized(
            approved_leaf_copy_permission("float")).duplicate(),
            Err(ModelError::CopyUndecided));
    }
}
