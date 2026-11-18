// G18／B4/M3：只有原Checker核准的 owner.bool Copyable 值葉，
// 才可在原 Parser main bool 宣告／賦值時讀同一 P16 真持有者。
#[cfg(test)]
mod g18_b4_m3_main_owner_bool_local_read_tests {
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
return owner.read(true,true,owner.n)
}
"#;

    fn source(fields: &str, locals: &str, call: &str) -> String {
        SOURCE.replace("var ready:bool = true\n",fields)
            .replace("var owner:Counter = Counter()\n",
                &format!("var owner:Counter = Counter()\n{locals}"))
            .replace("return owner.read(true,true,owner.n)",
                &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            src, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker須核准main bool局部讀原Copyable結構欄位");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("同源Checker bool欄位局部不得錯拒")
    }

    fn run(src: &str, expected: i64, gate: bool, calls: usize)
        -> PrivateSelfMethodOutcome {
        let out=checked(src).execute().expect("原Parser逐語句真P16值槽");
        assert_eq!(out.result,Ok(expected));
        assert_eq!(out.branch_checked,Some(gate));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert!(out.method_entered && out.method_exited);
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable bool不得虛構OwnedStr資源事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_bool_local_原Checker_true欄位初始化const真局部再作方法bool參() {
        let src=source("var ready:bool = true\n",
            "const gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        let plan=checked(&src);
        assert!(matches!(&plan.main_bool_sources[0],
            Some(PrivateSelfBoolArgumentSource::OwnerField{declaration,field})
            if *declaration==plan.owner_id && field=="ready"));
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_local_原Checker_false欄位在main局部傳遞後零helper() {
        let src=source("var ready:bool = false\n",
            "const gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        let out=run(&src,7,false,0);
        assert_eq!(out.nested_branch_checked,None);
    }

    #[test]
    fn b4_bool_local_原Lexer多重Group包覆欄位bool初始化真值槽() {
        let src=source("const ready:bool = true\n",
            "const gate:bool = (((owner.ready)))\n",
            "owner.read(gate,true,owner.n)");
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_local_原Checker未初始化var首次賦值讀真owner_bool() {
        let src=source("var ready:bool = true\n",
            "var gate:bool\ngate = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_local_var重賦必讀原P16值而不是宣告常值() {
        let src=source("var ready:bool = true\n",
            "var gate:bool = false\ngate = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_local_已複製bool局部再賦false不得污染原owner欄位() {
        let src=source("var ready:bool = true\n",
            "var gate:bool = owner.ready\ngate = false\n",
            "owner.read(gate,true,owner.n)");
        run(&src,7,false,0);
    }

    #[test]
    fn b4_bool_local_欄位真值先成main別名再傳方法仍唯一DeclId() {
        let src=source("var ready:bool = true\n",
            "const gate:bool = owner.ready\nconst alias:bool = gate\n",
            "owner.read(alias,true,owner.n)");
        let plan=checked(&src);
        assert_ne!(plan.main_bool_locals[0].1,plan.main_bool_locals[1].1);
        run(&src,8,true,1);
    }

    #[test]
    fn b4_bool_local_先更新int後main從同一P16讀bool再用最新int實參() {
        let src=source("var ready:bool = true\n",
            "owner.n = 11\nconst gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        run(&src,12,true,1);
    }

    #[test]
    fn b4_bool_local_真bool來源方法中注入故障必唯一歸票() {
        let src=source("var ready:bool = true\n",
            "const gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        let plan=checked(&src);
        let fail=plan.execute_with_fault(true).expect("方法故障應受控退出");
        assert_eq!(fail.result,Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(fail.method_entered && !fail.method_exited);
        assert!(fail.ticket_returned && fail.has_balanced_self_borrow_trace());
        assert!(fail.events.is_empty());
        assert_eq!(plan.execute().unwrap().result,Ok(8));
    }

    #[test]
    fn b4_bool_local_封存Initializer欄位身分被改必SourceChanged() {
        let src=source("var ready:bool = true\n",
            "const gate:bool = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        let mut plan=checked(&src);
        let Some(PrivateSelfBoolArgumentSource::OwnerField{field,..}) =
            &mut plan.main_bool_sources[0] else {
            panic!("原Checker應保留bool欄位來源身分");
        };
        *field="forged".to_owned();
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_local_封存賦值欄位DeclId被改必SourceChanged() {
        let src=source("var ready:bool = true\n",
            "var gate:bool\ngate = owner.ready\n",
            "owner.read(gate,true,owner.n)");
        let mut plan=checked(&src);
        let Some(PrivateSelfBoolArgumentSource::OwnerField{declaration,..}) =
            &mut plan.main_bool_assignments[0].2 else {
            panic!("原Checker應保留賦值bool欄位DeclId");
        };
        *declaration=DeclarationId(0);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_bool_local_原Checker拒絕int欄位冒充bool與未知欄位() {
        for src in [
            source("var ready:bool = true\n",
                "const gate:bool = owner.n\n",
                "owner.read(gate,true,owner.n)"),
            source("var ready:bool = true\n",
                "const gate:bool = owner.missing\n",
                "owner.read(gate,true,owner.n)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err());
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
