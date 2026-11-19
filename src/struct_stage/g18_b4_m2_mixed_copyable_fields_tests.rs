// G18／P18.4-B4／P18.6-M2：額外 int／bool 純值欄位依原
// Lexer／Checker 宣告次序在同一 P16 真物件中建構；不擴 self 存取。
#[cfg(test)]
mod g18_b4_m2_mixed_copyable_fields_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var count:int = 7
fn set(n:int)->int{
self.count = n
return self.count
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(9)
}
"#;

    fn source(fields: &str, main_prefix: &str) -> String {
        SOURCE.replace("var count:int = 7\n",
            &format!("var count:int = 7\n{fields}"))
            .replace("return owner.set(9)",
                &format!("{main_prefix}return owner.set(9)"))
    }

    fn checked(text: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 應核准此獨立 Copyable 欄位");
        PreparedPrivateSelfMethod::prepare(text)
            .expect("原 Checker 合法 Copyable 欄位不可被私有子集錯拒")
    }

    fn success(text: &str) -> PrivateSelfMethodOutcome {
        let out=checked(text).execute().expect("同一 P16 owner 真建構並執行");
        assert_eq!(out.result,Ok(9));
        assert!(out.method_entered && out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"int／bool 不得假造 OwnedStr ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        out
    }

    #[test]
    fn b4_mixed_copy_原Checker第二var_bool_true真建立與原欄位方法寫入() {
        let src=source("var ready:bool = true\n","");
        assert_eq!(checked(&src).additional_copy_fields,vec![
            ("ready".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),false),
        ]);
        success(&src);
    }

    #[test]
    fn b4_mixed_copy_原Checker額外const_bool_false保留immutable來源() {
        let src=source("const enabled:bool = false\n","");
        assert_eq!(checked(&src).additional_copy_fields,vec![
            ("enabled".to_owned(),PrivateSelfAdditionalCopyValue::Bool(false),true),
        ]);
        success(&src);
    }

    #[test]
    fn b4_mixed_copy_原Checker混合int_bool_int依宣告順序建立真P16() {
        let src=source(
            "var spare:int = 3\nconst ready:bool = true\nvar payload:int = 19\n","");
        assert_eq!(checked(&src).additional_copy_fields,vec![
            ("spare".to_owned(),PrivateSelfAdditionalCopyValue::Int(3),false),
            ("ready".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),true),
            ("payload".to_owned(),PrivateSelfAdditionalCopyValue::Int(19),false),
        ]);
        success(&src);
    }

    #[test]
    fn b4_mixed_copy_相等bool值不同原Checker欄位身分不得去重() {
        let src=source("var first:bool = true\nconst second:bool = true\n","");
        assert_eq!(checked(&src).additional_copy_fields,vec![
            ("first".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),false),
            ("second".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),true),
        ]);
        success(&src);
    }

    #[test]
    fn b4_mixed_copy_main先寫真int欄位再進方法仍保留bool欄位() {
        let src=source("var ready:bool = false\n",
            "owner.count = 11\n");
        assert_eq!(checked(&src).main_owner_field_assignments.len(),1);
        success(&src);
    }

    #[test]
    fn b4_mixed_copy_方法內受控故障只歸還一張self票不創造bool資源() {
        let src=source("var ready:bool = true\nconst closed:bool = false\n","");
        let plan=checked(&src);
        let failure=plan.execute_with_fault(true).expect("故障應受控返回");
        assert_eq!(failure.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failure.method_entered && !failure.method_exited);
        assert!(failure.ticket_returned && failure.has_balanced_self_borrow_trace());
        assert!(failure.events.is_empty());
        assert_eq!(plan.execute().unwrap().result,Ok(9));
    }

    #[test]
    fn b4_mixed_copy_改寫封存bool值必在任何真借票前SourceChanged() {
        let src=source("var ready:bool = true\n","");
        let mut plan=checked(&src);
        plan.additional_copy_fields[0].1=PrivateSelfAdditionalCopyValue::Bool(false);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_mixed_copy_把bool封存型別偽裝int不可建構() {
        let src=source("var ready:bool = true\n","");
        let mut plan=checked(&src);
        plan.additional_copy_fields[0].1=PrivateSelfAdditionalCopyValue::Int(1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_mixed_copy_修改封存const可寫性必SourceChanged() {
        let src=source("const ready:bool = true\n","");
        let mut plan=checked(&src);
        plan.additional_copy_fields[0].2=false;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_mixed_copy_真正str在故障歸票後釋放且不可傳入整數形參() {
        let src=source("var memo:str = \"繁體中文🪼\"\n","");
        let out=checked(&src).execute_with_fault(true).unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert_eq!(out.owned_fields.len(),1);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        let bad=src.replace("owner.set(9)", "owner.set(owner.memo)");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&bad),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn b4_mixed_copy_沒有預設值的附加欄位保守拒絕() {
        let src=source("var ready:bool\n","");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))
                | Err(PrivateSelfMethodPrepareError::Check(_))));
    }
}
