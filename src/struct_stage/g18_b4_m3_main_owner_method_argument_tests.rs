// G18／B4／M3：main 真持有者欄位 Copyable int 作同步方法實參，
// 原 Checker 簽發的 TypedHIR／DeclarationId 必先真動態求值再借 self。
#[cfg(test)]
mod g18_b4_m3_main_owner_method_argument_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
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
return owner.read(true,true,owner.n)
}
"#;

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(true,true,owner.n)",
                &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker須核准Copyable int欄位作方法實參");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("同源Checker欄位方法實參不得錯拒")
    }

    fn success(src: &str, expected: i64, ordinary: usize) -> PrivateSelfMethodOutcome {
        let plan = checked(src);
        let out = plan.execute().expect("合法來源必能真執行");
        assert_eq!(out.result, Ok(expected));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),
            (ordinary,ordinary));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!(out.nested_branch_checked,Some(true));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"純int欄位不產生假P16 str事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        out
    }

    #[test]
    fn b4_owner_method_arg_預設真int欄位直接作step並於自借前求值() {
        success(SOURCE,8,1);
    }

    #[test]
    fn b4_owner_method_arg_先寫入十一後以同一真持有者欄位做實參() {
        let src=source("owner.n = 11\n","owner.read(true,true,owner.n)");
        success(&src,12,1);
    }

    #[test]
    fn b4_owner_method_arg_原Lexer雙層Group與欄位算術作實參() {
        let src=source("owner.n = 9\n",
            "owner.read(true,true,((owner.n+2)))");
        success(&src,12,1);
    }

    #[test]
    fn b4_owner_method_arg_原P13禁止巢狀欄位helper作方法引數須靜態拒絕() {
        let src=source("","owner.read(true,true,helper(owner.n,2))");
        // 原 P13 方法引數不接受此巢狀 Call：不能為讓私有
        // executor 測試成功而新增未核准的表達式語意。
        assert!(verify_source_with_ability(&src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).is_err(),"原Checker不得接受未核准的巢狀方法引數");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn b4_owner_method_arg_前序helper寫入後欄位值再計算方法step() {
        let src=source("owner.n = helper(10,2)\n",
            "owner.read(true,true,owner.n+3)");
        success(&src,12,2);
    }

    #[test]
    fn b4_owner_method_arg_實參欄位加法溢位不可進方法或簽self票() {
        let src=source("owner.n = 9223372036854775807\n",
            "owner.read(true,true,owner.n+1)");
        let out=checked(&src).execute().expect("溢位應受控回傳而非panic");
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.events.is_empty());
        assert!(out.has_balanced_self_borrow_trace());
        assert!(!out.exit_events.iter().any(|event| matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_owner_method_arg_封存Checker欄位引數遭改綁必拒絕副作用前() {
        let mut plan=checked(SOURCE);
        let PrivateScalarReturnExpr::CopyStructIntField { field, .. } =
            &mut plan.argument_plans[0] else {
            panic!("必有原Checker欄位實參身分");
        };
        *field="forged".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_owner_method_arg_不存在欄位不能冒充原Checker已核准int實參() {
        let src=source("","owner.read(true,true,owner.missing)");
        assert!(verify_source_with_ability(&src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).is_err());
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }
}
