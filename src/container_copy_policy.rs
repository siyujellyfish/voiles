//! P13-D01-B：容器按種類獨立判定 Copyable 能力的內部安全界線。
//! list／dict 尚無核准的具體複製規則；不得自動繼承其元素或鍵值型別的資格。
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

/// P13-D01-B 已選「分種類決定」，但尚未選每一種類的可複製與否。
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
}
