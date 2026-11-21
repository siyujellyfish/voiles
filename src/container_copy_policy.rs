//! P13-D01-B：容器按種類獨立判定 Copyable 能力的內部安全界線。
//! P19 2A／3A：list 與 dict 各自裁決為整體非 Copyable。
//! 不得自動繼承元素或鍵值型別資格。
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

/// 保留 P13-D01-B 的按種類分派。P19 對兩種容器分別禁止整體複製。
/// 任何元素全可複製、空容器或巢狀形狀都不構成容器複製能力的授權。
pub(crate) fn approved_container_copy_permission(kind: ContainerKind)
    -> CopyPermission {
    match kind {
        ContainerKind::List | ContainerKind::Dictionary => CopyPermission::Forbidden,
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
            assert_eq!(approved_leaf_copy_permission(name), CopyPermission::Forbidden);
            assert_eq!(ValueSlot::initialized(approved_leaf_copy_permission(name)).duplicate(),
                Err(ModelError::CopyForbidden));
        }
    }

    #[test]
    fn p19_容器的禁止複製政策與元素能力獨立() {
        for name in ["list[str]", "dict{str,str}", "list[Unknown]",
            "dict{int,list[float]}"] {
            assert_eq!(approved_leaf_copy_permission(name), CopyPermission::Forbidden);
        }
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
    }

    #[test]
    fn d01_b_容器欄位不會經s19偷偷使外層結構可複製() {
        let container = approved_leaf_copy_permission("list[int]");
        assert_eq!(whole_struct_copy(&[CopyPermission::Allowed, container]),
            Err(StructOwnershipError::FieldCopyForbidden));
        let map = approved_leaf_copy_permission("dict{int,bool}");
        assert_eq!(whole_struct_copy(&[map, CopyPermission::Allowed]),
            Err(StructOwnershipError::FieldCopyForbidden));
    }

    #[test]
    fn p19_兩種容器均禁止整體複製() {
        assert_eq!(approved_container_copy_permission(ContainerKind::List),
            CopyPermission::Forbidden);
        assert_eq!(approved_container_copy_permission(ContainerKind::Dictionary),
            CopyPermission::Forbidden);
    }
    #[test]
    fn p19_兩種容器禁止複製與已核准葉型別() {
        for (ty, kind) in [
            ("list[int]", ContainerKind::List),
            ("list[str]", ContainerKind::List),
            ("list[list[float]]", ContainerKind::List),
            ("dict{int,bool}", ContainerKind::Dictionary),
            ("dict{str,int}", ContainerKind::Dictionary),
            ("dict{int,list[str]}", ContainerKind::Dictionary),
        ] {
            assert_eq!(classify_container_shape(ty), Some(kind));
            assert_eq!(approved_container_copy_permission(kind), CopyPermission::Forbidden);
            assert_eq!(approved_leaf_copy_permission(ty), CopyPermission::Forbidden);
        }
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
    }

    #[test]
    fn d01_a_a_尚未初始化時先拒絕讀取_不讓複製限制掩蓋來源狀態() {
        // 僅驗證 Rust 私有的抽象 ValueSlot；不表示合法容器 AST 或搬移已實作。
        for ty in ["list[int]", "list[str]", "dict{int,bool}", "dict{str,int}"] {
            let ability = approved_leaf_copy_permission(ty);
            assert_eq!(ability, CopyPermission::Forbidden);
            let absent = ValueSlot::uninitialized(ability);
            assert_eq!(absent.read(), Err(ModelError::UninitializedRead));
            assert_eq!(absent.duplicate(), Err(ModelError::UninitializedRead));
            let ready = ValueSlot::initialized(ability);
            assert_eq!(ready.duplicate(), Err(ModelError::CopyForbidden));
            assert!(ready.read().is_ok());
        }
    }

    #[test]
    fn d01_後續_list_a_dict_a_拒絕隱含複製但不毀損可用來源() {
        for ty in ["list[int]", "list[str]", "dict{int,float}", "dict{str,str}"] {
            let source = ValueSlot::initialized(approved_leaf_copy_permission(ty));
            assert_eq!(source.duplicate(), Err(ModelError::CopyForbidden));
            assert!(source.read().is_ok(), "複製失敗後來源仍有效");
            assert_eq!(
                whole_struct_copy(&[CopyPermission::Allowed, approved_leaf_copy_permission(ty)]),
                Err(StructOwnershipError::FieldCopyForbidden)
            );
        }
    }

    #[test]
    fn p19_容器共享借用期間禁止移動且歸還後只可整體移動() {
        use crate::borrow_ownership_gate::{BorrowOwnershipGate, GateError};
        use crate::borrow_safety::BorrowKind;
        use crate::ownership_ledger::DeclarationId;
        for ty in ["list[Optional[int,str]]", "dict{str,Optional[int,str]}"] {
            let mut gate = BorrowOwnershipGate::new();
            let owner = DeclarationId(1);
            let destination = DeclarationId(2);
            gate.declare(owner, ValueSlot::initialized(approved_leaf_copy_permission(ty))).unwrap();
            let loan = gate.borrow(owner, BorrowKind::SharedRead).unwrap();
            assert!(matches!(gate.reinitialize(owner), Err(GateError::Borrow(_))));
            assert!(matches!(gate.move_into(owner, destination), Err(GateError::Borrow(_))));
            gate.finish_method_scope(&[loan]).unwrap();
            assert_eq!(gate.copy_into(owner, destination), Err(GateError::Value(ModelError::CopyForbidden)));
            gate.move_into(owner, destination).unwrap();
            assert_eq!(gate.read(owner), Err(GateError::Value(ModelError::MovedRead)));
            assert_eq!(gate.read(destination), Ok(()));
        }
    }
}
