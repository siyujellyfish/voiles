// G18／B4／M2：原 Checker main 已核准的 var owner.int
// 直線欄位寫入，必以唯一 FieldWriteEvidence、真 ScalarHIR
// 與 MixedStructRuntime 的同一持有者執行期物件原子提交。
#[cfg(test)]
mod g18_b4_m2_main_owner_int_field_write_tests {
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
return owner.read(false,true,5)
}
"#;

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(false,true,5)", &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(src,
            crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker須核准同一持有者var int欄位直接寫入");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("已有Checker FieldWriteEvidence不可被受限Executor錯拒")
    }

    fn run(src: &str, expected: i64, writes: usize, calls: usize,
        branch: (Option<bool>,Option<bool>)) -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        assert_eq!(plan.main_owner_field_assignments.len(),writes);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"純Copyable欄位不可虛構str ResourceId");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_owner_write_原Checker單次var_int欄位常值真寫入再由self讀取() {
        let src=source("owner.n = 11\n","owner.read(false,true,5)");
        let plan=checked(&src);
        assert_eq!(plan.main_scalar_steps,
            vec![PrivateSelfMainScalarStep::OwnerIntFieldAssign(0)]);
        run(&src,11,1,0,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_兩次主函式欄位覆寫以最後真動態值返回() {
        let src=source("owner.n = 4\nowner.n = 9\n","owner.read(false,true,5)");
        run(&src,9,2,0,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_具名int經原CheckerDeclId寫入owner欄位() {
        let src=source("const value:int = 12\nowner.n = value\n",
            "owner.read(false,true,5)");
        run(&src,12,1,0,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_原Lexer多層Group與純int加法真動態寫入() {
        let src=source(
            "const base:int = 4\nowner.n = ((base+5))\n",
            "owner.read(false,true,5)",
        );
        run(&src,9,1,0,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_普通helper須真退出後才寫欄位及借入self() {
        let src=source("owner.n = helper(9,4)\n",
            "owner.read(false,true,5)");
        let out=run(&src,5,1,1,(Some(false),None));
        let exited=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(exited<loan);
    }

    #[test]
    fn b4_owner_write_未初始化int首次賦值與owner欄位真交接() {
        let src=source(
            "var value:int\nvalue = helper(9,4)\nowner.n = value+3\n",
            "owner.read(false,true,5)",
        );
        run(&src,8,1,1,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_夾bool重賦與helper保持唯一原Parser語句順序() {
        let src=source(
            "var gate:bool = true\nowner.n = helper(10,2)\ngate = false\n",
            "owner.read(gate,true,5)",
        );
        run(&src,8,1,1,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_先寫持有者欄位後方法選中helper仍不多簽票() {
        let src=source("owner.n = 11\n","owner.read(true,false,5)");
        let out=run(&src,2,1,1,(Some(true),Some(false)));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_owner_write_同步純helper外層溢位不得寫入owner或借方法票() {
        let src=source(
            "owner.n = helper(9223372036854775807,0)+1\n",
            "owner.read(false,true,5)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty());
    }

    #[test]
    fn b4_owner_write_與Grouped方法Return保持內葉唯一真借票() {
        let src=source("owner.n = 9\n",
            "((owner.read(false,true,5)))");
        run(&src,9,1,0,(Some(false),None));
    }

    #[test]
    fn b4_owner_write_來源清冊被改或步驟重播必先SourceChanged() {
        let src=source("owner.n = 11\n","owner.read(false,true,5)");
        let mut plan=checked(&src);
        plan.main_owner_field_assignments[0].0=7;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_scalar_steps.push(PrivateSelfMainScalarStep::OwnerIntFieldAssign(0));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn b4_owner_write_原Checker拒絕const持有者不存在欄位與錯型別() {
        for src in [
            source("owner.missing = 4\n","owner.read(false,true,5)"),
            source("owner.n = true\n","owner.read(false,true,5)"),
            source("owner.n = missing\n","owner.read(false,true,5)"),
            source("owner.n = 11\n","owner.read(false,true,5)")
                .replace("var owner:Counter = Counter()",
                    "const owner:Counter = Counter()"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker不能核准未證欄位寫入：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn b4_owner_write_未接通的欄位讀取作RHS仍保守拒絕第二套ScalarHIR() {
        let src=source("owner.n = owner.n+1\n","owner.read(false,true,5)");
        verify_source_with_ability(
            &src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker已核准持有者Copyable int欄位直接讀取");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }
}
