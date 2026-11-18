// G18／B4/M3：原 Checker 真 main owner.bool FieldWriteEvidence，
// 原 Parser 語句順序以同一 P16 真持有者原子寫入，無假借票或資源。
#[cfg(test)]
mod g18_b4_m3_main_owner_bool_field_write_tests {
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

    fn source(fields: &str, statements: &str, call: &str) -> String {
        SOURCE.replace("var ready:bool = true\n",fields)
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(owner.ready,true,owner.n)",
                &format!("return {call}"))
    }

    fn checked(text: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            text,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原 Checker 須核准本次 bool 欄位原 FieldWriteEvidence");
        PreparedPrivateSelfMethod::prepare(text)
            .expect("同源 bool FieldWrite 不得由私有執行器錯拒")
    }

    fn run(text: &str, expected: i64, branch: bool, helper_calls: usize)
        -> PrivateSelfMethodOutcome {
        let out=checked(text).execute().expect("原 Checker 真 P16 逐步執行");
        assert_eq!(out.result,Ok(expected));
        assert_eq!(out.branch_checked,Some(branch));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),
            (helper_calls,helper_calls));
        assert!(out.method_entered && out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable bool不得虛構OwnedStr事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
        out
    }

    #[test]
    fn b4_bool_field_write_原Checker_true欄位於main真寫false後零helper() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\n","owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert_eq!(plan.main_owner_bool_field_assignments.len(),1);
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_field_write_連續寫false再true以最後動態值選入方法() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\nowner.ready = true\n",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert_eq!(plan.main_owner_bool_field_assignments.len(),2);
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_field_write_原Checker具名Copyable_bool經DeclId寫入欄位() {
        let src=source("var ready:bool = true\n",
            "const value:bool = false\nowner.ready = value\n",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert_eq!(plan.main_owner_bool_field_assignments[0].2,
            Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)));
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_field_write_原Lexer多重Group欄位自讀保留真Checker身分() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\nowner.ready = (((owner.ready)))\n",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert!(matches!(&plan.main_owner_bool_field_assignments[1].2,
            Some(PrivateSelfBoolArgumentSource::OwnerField{declaration,field})
            if *declaration==plan.owner_id && field=="ready"));
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_field_write_前序欄位false再由main局部真讀false傳方法() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\nconst gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_field_write_先複製bool真快照後修改owner不污染舊局部() {
        let src=source("var ready:bool = true\n",
            "const old:bool = owner.ready\nowner.ready = false\n",
            "owner.read(old,true,owner.n)");
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_field_write_與int欄位先後交錯真執行後方法值一致() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\nowner.n = 11\nowner.ready = true\n",
            "owner.read(owner.ready,true,owner.n)");
        run(&src,12,true,1);
    }

    #[test]
    fn b4_bool_field_write_方法注入故障仍只借還同一張self票() {
        let src=source("var ready:bool = true\n",
            "owner.ready = false\nowner.ready = true\n",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        let out=plan.execute_with_fault(true).expect("故障受控收束");
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty());
        assert_eq!(plan.execute().unwrap().result,Ok(8));
    }

    #[test]
    fn b4_bool_field_write_同型不同DeclId的bool欄位只依原Checker來源讀值() {
        let src=source("var ready:bool = true\nvar backup:bool = false\n",
            "owner.ready = owner.backup\n",
            "owner.read(owner.ready,true,owner.n)");
        let plan=checked(&src);
        assert!(matches!(&plan.main_owner_bool_field_assignments[0].2,
            Some(PrivateSelfBoolArgumentSource::OwnerField{declaration,field})
            if *declaration==plan.owner_id && field=="backup"));
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_field_write_已封存原Checker來源遭改寫必先SourceChanged() {
        let src=source("var ready:bool = true\n",
            "owner.ready = owner.ready\n",
            "owner.read(owner.ready,true,owner.n)");
        let mut plan=checked(&src);
        let Some(PrivateSelfBoolArgumentSource::OwnerField{field,..}) =
            &mut plan.main_owner_bool_field_assignments[0].2 else {
            panic!("需保留原 Checker bool 欄位RHS身分");
        };
        *field="forged".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_field_write_原Checker拒絕const欄位與錯誤型別及錯誤欄名() {
        for src in [
            source("const ready:bool = true\n",
                "owner.ready = false\n","owner.read(true,true,owner.n)"),
            source("var ready:bool = true\n",
                "owner.ready = 1\n","owner.read(true,true,owner.n)"),
            source("var ready:bool = true\n",
                "owner.missing = false\n","owner.read(true,true,owner.n)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"非法 bool FieldWrite 必由原 Checker 拒絕");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
