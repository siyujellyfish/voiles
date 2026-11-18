// G18／B4/M3：只有原 Checker 核准的 main Copyable int 局部，
// 才能於原 Parser 語句位置讀取同一 P16 owner.int 的最新動態值。
#[cfg(test)]
mod g18_b4_m3_main_owner_local_int_read_tests {
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
return owner.read(true,true,7)
}
"#;

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(true,true,7)",
                &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            src, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("完整原 Checker 必須核准這個 main int 欄位讀取來源");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("原Checker已核准的 main int 局部不可錯拒")
    }

    fn success(src: &str, expected: i64, calls: usize) -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        let out=plan.execute().expect("合法原來源必真執行");
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert_eq!((out.branch_checked,out.nested_branch_checked),(Some(true),Some(true)));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable int 欄位不得虛構字串 ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_owner_local_原Checker預設欄位直接初始化const整數真值槽() {
        let src=source("const value:int = owner.n\n",
            "owner.read(true,true,value)");
        success(&src,8,1);
    }

    #[test]
    fn b4_owner_local_取值後原欄位再寫入不得污染已初始化const快照() {
        let src=source(
            "owner.n = 11\nconst value:int = owner.n\nowner.n = 3\n",
            "owner.read(true,true,value)",
        );
        success(&src,12,1);
    }

    #[test]
    fn b4_owner_local_未初始化var於原Checker首次賦值讀取欄位加法() {
        let src=source("var value:int\nvalue = owner.n+3\n",
            "owner.read(true,true,value)");
        success(&src,11,1);
    }

    #[test]
    fn b4_owner_local_前後欄位更新與var重賦必重讀同一真P16物件() {
        let src=source(
            "owner.n = 11\nvar value:int = owner.n\nowner.n = 3\nvalue = owner.n+value\n",
            "owner.read(true,true,value)",
        );
        success(&src,15,1);
    }

    #[test]
    fn b4_owner_local_原Checker欄位值供main同步helper初始化再到方法() {
        let src=source(
            "owner.n = 11\nconst value:int = helper(owner.n,2)\n",
            "owner.read(true,true,value)",
        );
        let out=success(&src,10,2);
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let helper_return=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        assert!(helper_return<loan);
    }

    #[test]
    fn b4_owner_local_欄位整數初始化溢位不得進方法或建立自借票() {
        let src=source(
            "owner.n = 9223372036854775807\nconst value:int = owner.n+1\n",
            "owner.read(true,true,value)",
        );
        let out=checked(&src).execute().expect("真執行溢位應受控退出");
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.events.is_empty() && out.has_balanced_self_borrow_trace());
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_owner_local_原Checker非法欄位不得賦給main整數局部() {
        let src=source("const value:int = owner.missing\n",
            "owner.read(true,true,value)");
        assert!(verify_source_with_ability(
            &src,crate::ownership_policy::approved_leaf_copy_permission,
        ).is_err());
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn b4_owner_local_已封存Checker欄位Initializer身分遭偷換必先SourceChanged() {
        let src=source("const value:int = owner.n\n",
            "owner.read(true,true,value)");
        let mut plan=checked(&src);
        let PrivateScalarReturnExpr::CopyStructIntField { field, .. } =
            &mut plan.main_init_plans[0] else {
            panic!("原Checker必留下 CopyStructIntField 原始身份");
        };
        *field="forged".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
