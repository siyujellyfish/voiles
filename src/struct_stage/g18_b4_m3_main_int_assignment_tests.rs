// G18／B4/M3：main var int 真Checker已核准的直線重新賦值。
// 來源與計算只透過原 ScalarHIR、原 Parser 語句順序及 SharedCallExit。
#[cfg(test)]
mod g18_b4_m3_main_int_assignment_tests {
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
return owner.read(true,true,5)
}
"#;

    fn source(statements: &str, call: &str) -> String {
        SOURCE.replace("var owner:Counter = Counter()\n",
            &format!("var owner:Counter = Counter()\n{statements}"))
            .replace("return owner.read(true,true,5)", &format!("return {call}"))
    }

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            src,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker應核准已初始化var int的合法重新賦值");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("已核准原ScalarHIR的main int重賦不得被Executor錯拒")
    }

    fn run(src: &str, expected: i64, main_assignments: usize,
        calls: usize, branch: (Option<bool>, Option<bool>))
        -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        assert_eq!(plan.main_int_assignments.len(),main_assignments);
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),
            (calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"Copyable int不建立P16 str資源");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_int_assign_原Checker_var_int常值覆寫後方法使用最後值() {
        let src=source("var step:int = 3\nstep = 5\n",
            "owner.read(true,false,step)");
        run(&src,2,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_int_assign_原Checker_具名int_declid覆寫不依相同數值猜身分() {
        let src=source("const original:int = 5\nvar step:int = 3\nstep = original\n",
            "owner.read(true,false,step)");
        let plan=checked(&src);
        assert_eq!(plan.main_int_assignments[0].0,plan.main_int_locals[1].1);
        run(&src,2,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_int_assign_原Checker普通helper作RHS真執行先於self借票() {
        let src=source("var step:int = 3\nstep = helper(9,4)\n",
            "owner.read(true,false,step)");
        let out=run(&src,2,1,2,(Some(true),Some(false)));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn b4_int_assign_原Checker_多層Group加減自來源真求值() {
        let src=source("var step:int = 3\nstep = ((step+5))-2\n",
            "owner.read(true,true,step)");
        run(&src,7,1,1,(Some(true),Some(true)));
    }

    #[test]
    fn b4_int_assign_與bool變更交錯仍嚴格按原Parser結果選葉() {
        let src=source(
            "var gate:bool = true\nvar step:int = 3\nstep = helper(9,4)\ngate = false\nstep = step+2\n",
            "owner.read(gate,true,step)",
        );
        run(&src,7,2,1,(Some(false),None));
    }

    #[test]
    fn b4_int_assign_先普通helper再bool宣告仍有唯一self真票() {
        let src=source(
            "var step:int = 3\nstep = helper(9,4)\nconst gate:bool = true\n",
            "owner.read(gate,true,step)",
        );
        let out=run(&src,6,1,2,(Some(true),Some(true)));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_int_assign_真動態溢位停止後續bool與方法不假進入() {
        let src=source(
            "var gate:bool = true\nvar step:int = 5\nstep = step+9223372036854775807\ngate = false\n",
            "owner.read(gate,true,step)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty());
    }

    #[test]
    fn b4_int_assign_真helper成功而外部加法溢位不啟動self() {
        let src=source(
            "var gate:bool = true\nvar step:int = 5\nstep = helper(9223372036854775807,0)+1\ngate = false\n",
            "owner.read(gate,true,step)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_int_assign_準備後變造int賦值目標或重播步驟SourceChanged() {
        let src=source(
            "var first:int = 5\nvar step:int = 3\nstep = first\n",
            "owner.read(true,false,step)",
        );
        let mut plan=checked(&src);
        plan.main_int_assignments[0].0=plan.main_int_locals[0].1;
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_scalar_steps.push(PrivateSelfMainScalarStep::IntAssign(0));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        run(&src,2,1,1,(Some(true),Some(false)));
    }

    #[test]
    fn b4_int_assign_原Checker_const未知與bool錯型別賦值仍安全拒絕() {
        for src in [
            source("const step:int = 3\nstep = 5\n","owner.read(true,true,step)"),
            source("var step:int = 3\nstep = unknown\n","owner.read(true,true,step)"),
            source("var step:int = 3\nstep = true\n","owner.read(true,true,step)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission
            ).is_err(),"原Checker不能核准非法main int重新賦值：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
