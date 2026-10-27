//! P13-D01-B：容器按種類獨立判定 Copyable 能力的內部安全界線。
//! 使用者已選 list A／dict A：本階段兩類維持 Undecided，最終規則延後裁決。
//! 不得自動繼承元素或鍵值型別資格，也不可把未決誤記為永久禁止。
//! 此模組不解析完整容器型別，也不定義容器複製、借用或執行期表示。
#![allow(dead_code)]

use crate::ownership_model::CopyPermission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    List,
    Dictionary,
}

/// 僅識別已知容器型別形狀供保守能力分派；文法是否合法由正式語法分析器驗證。
pub(crate) fn classify_container_shape(ty: &str) -> Option<ContainerKind> {
    if ty.starts_with("list[") && ty.ends_with(']') {
        Some(ContainerKind::List)
    } else if ty.starts_with("dict{") && ty.ends_with('}') {
        Some(ContainerKind::Dictionary)
    } else {
        None
    }
}

/// D01-B 依種類分派；後續 list A／dict A 已各自選擇本階段保持未決。
/// 任何元素全可複製、空容器或巢狀形狀都不構成容器複製能力的授權。
pub(crate) fn approved_container_copy_permission(kind: ContainerKind)
    -> CopyPermission {
    match kind {
        ContainerKind::List => CopyPermission::Undecided,
        ContainerKind::Dictionary => CopyPermission::Undecided,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::{ModelError, ValueSlot};
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::struct_ownership::{whole_struct_copy, StructOwnershipError};

    #[test]
    fn d01_b_容器類型各自分類_不將非容器或不完整形狀假認為合法容器() {
        assert_eq!(classify_container_shape("list[int]"), Some(ContainerKind::List));
        assert_eq!(classify_container_shape("list[list[bool]]"), Some(ContainerKind::List));
        assert_eq!(classify_container_shape("dict{str,int}"), Some(ContainerKind::Dictionary));
        assert_eq!(classify_container_shape("list"), None);
        assert_eq!(classify_container_shape("dict"), None);
        assert_eq!(classify_container_shape("str"), None);
        assert_eq!(classify_container_shape("Optional[int]"), None);
    }

    #[test]
    fn d01_b_即使元素或鍵值全部copyable也不得自行核准容器copyable() {
        for name in ["list[int]", "list[float]", "list[bool]",
            "dict{int,float}", "dict{bool,int}", "list[list[int]]"] {
            assert_eq!(approved_leaf_copy_permission(name), CopyPermission::Undecided);
            assert_eq!(ValueSlot::initialized(approved_leaf_copy_permission(name)).duplicate(),
                Err(ModelError::CopyUndecided));
        }
    }

    #[test]
    fn d01_b_元素有str或其他未決能力也不擅自推定整個容器確定禁止() {
        for name in ["list[str]", "dict{str,str}", "list[Unknown]",
            "dict{int,list[float]}"] {
            assert_eq!(approved_leaf_copy_permission(name), CopyPermission::Undecided);
        }
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
    }

    #[test]
    fn d01_b_容器欄位不會經s19偷偷使外層結構可複製() {
        let container = approved_leaf_copy_permission("list[int]");
        assert_eq!(whole_struct_copy(&[CopyPermission::Allowed, container]),
            Err(StructOwnershipError::FieldCopyUndecided));
        let map = approved_leaf_copy_permission("dict{int,bool}");
        assert_eq!(whole_struct_copy(&[map, CopyPermission::Allowed]),
            Err(StructOwnershipError::FieldCopyUndecided));
    }

    #[test]
    fn d01_b_各容器即使無元素複製需求_仍須等待具體規格才可轉為allowed() {
        assert_eq!(approved_container_copy_permission(ContainerKind::List),
            CopyPermission::Undecided);
        assert_eq!(approved_container_copy_permission(ContainerKind::Dictionary),
            CopyPermission::Undecided);
    }
    #[test]
    fn d01_後續_list_a_dict_a_保留兩種類獨立未決與已核准葉型別() {
        for (ty, kind) in [
            ("list[int]", ContainerKind::List),
            ("list[str]", ContainerKind::List),
            ("list[list[float]]", ContainerKind::List),
            ("dict{int,bool}", ContainerKind::Dictionary),
            ("dict{str,int}", ContainerKind::Dictionary),
            ("dict{int,list[str]}", ContainerKind::Dictionary),
        ] {
            assert_eq!(classify_container_shape(ty), Some(kind));
            assert_eq!(approved_container_copy_permission(kind), CopyPermission::Undecided);
            assert_eq!(approved_leaf_copy_permission(ty), CopyPermission::Undecided);
        }
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
    }

    #[test]
    fn d01_a_a_尚未初始化時先拒絕讀取_不讓容器未決能力掩蓋來源狀態() {
        // 僅驗證 Rust 私有的抽象 ValueSlot；不表示合法容器 AST 或搬移已實作。
        for ty in ["list[int]", "list[str]", "dict{int,bool}", "dict{str,int}"] {
            let ability = approved_leaf_copy_permission(ty);
            assert_eq!(ability, CopyPermission::Undecided);
            let absent = ValueSlot::uninitialized(ability);
            assert_eq!(absent.read(), Err(ModelError::UninitializedRead));
            assert_eq!(absent.duplicate(), Err(ModelError::UninitializedRead));
            let ready = ValueSlot::initialized(ability);
            assert_eq!(ready.duplicate(), Err(ModelError::CopyUndecided));
            assert!(ready.read().is_ok());
        }
    }

    #[test]
    fn d01_後續_list_a_dict_a_拒絕隱含複製但不毀損可用來源() {
        for ty in ["list[int]", "list[str]", "dict{int,float}", "dict{str,str}"] {
            let source = ValueSlot::initialized(approved_leaf_copy_permission(ty));
            assert_eq!(source.duplicate(), Err(ModelError::CopyUndecided));
            assert!(source.read().is_ok(), "Copyable 未決不等於來源已移動");
            assert_eq!(
                whole_struct_copy(&[CopyPermission::Allowed, approved_leaf_copy_permission(ty)]),
                Err(StructOwnershipError::FieldCopyUndecided)
            );
        }
    }

}
