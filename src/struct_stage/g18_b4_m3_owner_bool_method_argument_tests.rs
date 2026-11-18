// G18／P18.4-B4／P18.6-M3：原Checker CopyStructBoolField 真值作
// 同一 P16 owner 同步方法 bool 實參，必先求值才取得 self BorrowTicket。
#[cfg(test)]
mod g18_b4_m3_owner_bool_method_argument_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
var ready:bool = true
fn read(gate:bool,branch:bool,step:int)->int{
if(gate){
if(branch){
return helper(step+3,2)
}else{
return helper(step+2,5)
}
}
return self.n
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(owner.ready,true,owner.n)
}
"#;

    fn source(fields: &str, prefix: &str, call: &str) -> String {
        SOURCE.replace("var ready:bool = true\n",
            &format!("{fields}"))
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{prefix}"))
            .replace("return owner.read(owner.ready,true,owner.n)",
                &format!("return {call}"))
    }

    fn checked(text: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker須核准Copyable bool欄位作方法bool實參");
        PreparedPrivateSelfMethod::prepare(text)
            .expect("原Checker合法bool欄位實參不得由私有Executor錯拒")
    }

    fn run(text: &str, expected: i64, branch: bool,
        ordinary_calls: usize) -> PrivateSelfMethodOutcome {
        let plan=checked(text);
        let out=plan.execute().expect("真P16欄位值與HC14方法均需受控執行");
        assert_eq!(out.result,Ok(expected));
        assert_eq!(out.branch_checked,Some(branch));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),
            (ordinary_calls,ordinary_calls));
        assert!(out.method_entered && out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable bool 不得建立 OwnedStr 事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        out
    }

    #[test]
    fn b4_bool_method_arg_原Checker_true欄位直接作首個方法bool實參() {
        let out=run(SOURCE,8,true,1);
        assert_eq!(out.nested_branch_checked,Some(true));
    }

    #[test]
    fn b4_bool_method_arg_原Checker_false欄位令未選helper零執行() {
        let src=source("var ready:bool = false\n","",
            "owner.read(owner.ready,true,owner.n)");
        let out=run(&src,7,false,0);
        assert_eq!(out.nested_branch_checked,None);
    }

    #[test]
    fn b4_bool_method_arg_原Checker_const_bool欄位真預設且不得混成int() {
        let src=source("const ready:bool = true\n","",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert_eq!(plan.additional_copy_fields,vec![
            ("ready".to_owned(),PrivateSelfAdditionalCopyValue::Bool(true),true)
        ]);
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_method_arg_原Lexer_Group包裹bool欄位仍是同一Checker值葉() {
        let src=source("var ready:bool = true\n","",
            "owner.read(((owner.ready)),true,owner.n)");
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_method_arg_欄位bool在第二bool實參依原位置進巢狀if() {
        let src=source("var ready:bool = true\n","",
            "owner.read(true,owner.ready,owner.n)");
        let out=run(&src,8,true,1);
        assert_eq!(out.nested_branch_checked,Some(true));
    }

    #[test]
    fn b4_bool_method_arg_先更新int欄位後同物件bool實參與int實參皆真讀() {
        let src=source("var ready:bool = true\n","owner.n = 11\n",
            "owner.read(owner.ready,true,owner.n)");
        run(&src,12,true,1);
    }

    #[test]
    fn b4_bool_method_arg_原P16布林實參方法入口故障仍唯一還票() {
        let plan=checked(SOURCE);
        let failure=plan.execute_with_fault(true).expect("真方法故障受控返回");
        assert_eq!(failure.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failure.method_entered && !failure.method_exited);
        assert!(failure.ticket_returned && failure.has_balanced_self_borrow_trace());
        assert!(failure.events.is_empty());
        assert_eq!(plan.execute().unwrap().result,Ok(8));
    }

    #[test]
    fn b4_bool_method_arg_偽造封存bool欄位名稱須在BorrowTicket前SourceChanged() {
        let mut plan=checked(SOURCE);
        let Some(PrivateSelfBoolArgumentSource::OwnerField{field,..}) =
            &mut plan.method_bool_sources[0] else {
            panic!("必保留原Checker bool欄位來源");
        };
        *field="forged".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_method_arg_偽造封存bool欄位DeclId不得借入self() {
        let mut plan=checked(SOURCE);
        let Some(PrivateSelfBoolArgumentSource::OwnerField{declaration,..}) =
            &mut plan.method_bool_sources[0] else {
            panic!("必保留原Checker bool owner DeclId");
        };
        *declaration=DeclarationId(0);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_method_arg_原Checker拒絕int欄位冒充bool與不存在欄位() {
        for src in [
            source("var ready:bool = true\n","",
                "owner.read(owner.n,true,owner.n)"),
            source("var ready:bool = true\n","",
                "owner.read(owner.missing,true,owner.n)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker未授權的bool欄位不應進入方法實參");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
