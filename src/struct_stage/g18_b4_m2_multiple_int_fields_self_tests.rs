// G18／B4/M2：原 Checker 合法多個純 Copyable int 預設欄位，
// 同一 P16 主持有者構造後才入 self；不接通未授權的資源欄位。
#[cfg(test)]
mod g18_b4_m2_multiple_int_fields_self_tests {
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

    fn extra(definitions: &str) -> String {
        SOURCE.replace("var count:int = 7\n",
            &format!("var count:int = 7\n{definitions}"))
    }

    fn plan(source: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 應核准純整數多欄位結構");
        PreparedPrivateSelfMethod::prepare(source)
            .expect("原Checker合法的額外int預設欄位不應錯拒")
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out=plan(source).execute().expect("P16真構造與方法借票須成功");
        assert_eq!(out.result,Ok(9));
        assert!(out.method_entered && out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"所有int欄位不可憑空造出字串資源事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        out
    }

    #[test]
    fn b4_multi_fields_兩個var_int預設依原Checker順序同物件建構() {
        let src=extra("var spare:int = 3\n");
        let p=plan(&src);
        assert_eq!(p.additional_copy_fields,vec![
            ("spare".to_owned(),PrivateSelfAdditionalCopyValue::Int(3),false)
        ]);
        run(&src);
    }

    #[test]
    fn b4_multi_fields_保留原Checker_const_int額外欄位不可當可寫() {
        let src=extra("const spare:int = 13\n");
        let p=plan(&src);
        assert_eq!(p.additional_copy_fields,vec![
            ("spare".to_owned(),PrivateSelfAdditionalCopyValue::Int(13),true)
        ]);
        run(&src);
    }

    #[test]
    fn b4_multi_fields_三個int欄位依原Lexer宣告次序分別真預設() {
        let src=extra("var spare:int = 3\nconst payload:int = 17\n");
        let p=plan(&src);
        assert_eq!(p.additional_copy_fields,vec![
            ("spare".to_owned(),PrivateSelfAdditionalCopyValue::Int(3),false),
            ("payload".to_owned(),PrivateSelfAdditionalCopyValue::Int(17),true),
        ]);
        run(&src);
    }

    #[test]
    fn b4_multi_fields_main真欄位更新後方法獨占寫入仍只更新指定count() {
        let src=extra("var spare:int = 3\n")
            .replace("return owner.set(9)",
                "owner.count = 11\nreturn owner.set(9)");
        let p=plan(&src);
        assert_eq!(p.main_owner_field_assignments.len(),1);
        let out=run(&src);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_multi_fields_方法入口故障仍唯一還self票並逐作用域離開() {
        let src=extra("var spare:int = 3\nconst payload:int = 17\n");
        let p=plan(&src);
        let fail=p.execute_with_fault(true).expect("受控故障");
        assert_eq!(fail.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(fail.method_entered && !fail.method_exited);
        assert!(fail.ticket_returned && fail.has_balanced_self_borrow_trace());
        assert!(fail.events.is_empty());
        assert_eq!(p.execute().unwrap().result,Ok(9));
    }

    #[test]
    fn b4_multi_fields_額外真Checker欄位封存預設遭改寫須先SourceChanged() {
        let src=extra("var spare:int = 3\n");
        let mut p=plan(&src);
        p.additional_copy_fields[0].1=PrivateSelfAdditionalCopyValue::Int(99);
        assert!(matches!(p.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_multi_fields_未開放額外str資源欄位不得偽裝Copyable_int() {
        let src=extra("var memo:str = \"測試\"\n");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))
                | Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn b4_multi_fields_額外bool已由原Checker核准時必真建構且不當作int() {
        let src=extra("var gate:bool = true\n");
        let p=plan(&src);
        assert_eq!(p.additional_copy_fields,vec![
            ("gate".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),false),
        ]);
        run(&src);
    }
}
