// G18／B4／M3：原 Checker main Copyable int／bool 在真正
// Parser 宣告與重賦順序執行，普通 helper 事件不可與 bool 值槽錯序。
#[cfg(test)]
mod g18_b4_m3_main_scalar_ordered_steps_tests {
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
            src, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker應已核准Copyable int與bool直線同源來源");
        PreparedPrivateSelfMethod::prepare(src)
            .expect("普通helper與bool更新應共用原Parser真執行順序")
    }

    fn run(src: &str, expected: i64, calls: usize,
        branch: (Option<bool>, Option<bool>))
        -> PrivateSelfMethodOutcome {
        let plan=checked(src);
        assert_eq!(plan.main_scalar_steps.len(),
            plan.main_bool_locals.len()+plan.main_int_locals.len()
                +plan.main_bool_assignments.len());
        let out=plan.execute().unwrap();
        assert_eq!(out.result,Ok(expected));
        assert_eq!((out.branch_checked,out.nested_branch_checked),branch);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(calls,calls));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"純Copyable不得建立OwnedStr事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn b4_ordered_main_原Checker普通helper先於bool更新真退出且方法不進分支() {
        let src=source(
            "var gate:bool = true\nconst step:int = helper(7,2)\ngate = false\n",
            "owner.read(gate,true,step)",
        );
        let out=run(&src,7,1,(Some(false),None));
        let helper=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(helper<loan);
    }

    #[test]
    fn b4_ordered_main_bool更新後宣告int再改回true選中真else() {
        let src=source(
            "var gate:bool = true\ngate = false\nconst step:int = helper(7,2)\ngate = true\n",
            "owner.read(gate,false,step)",
        );
        run(&src,2,2,(Some(true),Some(false)));
    }

    #[test]
    fn b4_ordered_main_具名bool來源更新在普通函式退出後按DeclId讀取() {
        let src=source(
            "const base:bool = false\nvar gate:bool = true\nconst step:int = helper(7,2)\ngate = base\n",
            "owner.read(gate,true,step)",
        );
        let plan=checked(&src);
        assert_eq!(plan.main_bool_assignments[0].2,Some(PrivateSelfBoolArgumentSource::MainLocal(plan.main_bool_locals[0].1)));
        run(&src,7,1,(Some(false),None));
    }

    #[test]
    fn b4_ordered_main_普通helper後宣告bool再重賦已有值槽() {
        let src=source(
            "var gate:bool = false\nconst step:int = helper(7,2)\nconst yes:bool = true\ngate = yes\n",
            "owner.read(gate,true,step)",
        );
        run(&src,6,2,(Some(true),Some(true)));
    }

    #[test]
    fn b4_ordered_main_先helper再bool參數反向調整不誤用int值槽() {
        let src=source(
            "const step:int = helper(7,2)\nconst gate:bool = true\nvar branch:bool = false\nbranch = true\n",
            "owner.read(gate,branch,step)",
        );
        run(&src,6,2,(Some(true),Some(true)));
    }

    #[test]
    fn b4_ordered_main_兩個main_helper夾bool更新與方法helper真先後順序() {
        let src=source(
            "var gate:bool = false\nconst step1:int = helper(9,4)\ngate = true\nconst step2:int = helper(step1,0)\n",
            "owner.read(gate,false,step2)",
        );
        let out=run(&src,2,3,(Some(true),Some(false)));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),2);
    }

    #[test]
    fn b4_ordered_main_前序int溢位不可執行後續bool更新與self借票() {
        let src=source(
            "var gate:bool = true\nconst overflow:int = helper(9223372036854775807,0)+1\ngate = false\n",
            "owner.read(gate,true,5)",
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
    fn b4_ordered_main_後序int溢位不會啟動本次方法或製造假Returned() {
        let src=source(
            "var gate:bool = false\ngate = true\nconst overflow:int = helper(9223372036854775807,0)+1\n",
            "owner.read(gate,false,5)",
        );
        let out=checked(&src).execute().unwrap();
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn b4_ordered_main_步驟來源交換或重播即使值一樣也必執行前拒絕() {
        let src=source(
            "const step:int = helper(7,2)\nvar gate:bool = true\ngate = true\n",
            "owner.read(gate,true,step)",
        );
        let mut plan=checked(&src);
        plan.main_scalar_steps.swap(0,1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=checked(&src);
        plan.main_scalar_steps.push(PrivateSelfMainScalarStep::BoolAssign(0));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        run(&src,6,2,(Some(true),Some(true)));
    }

    #[test]
    fn b4_ordered_main_非法const與不安全bool型別來源仍由原Checker先拒絕() {
        for src in [
            source("const gate:bool = true\ngate = false\n",
                "owner.read(gate,true,5)"),
            source("var gate:bool = true\ngate = 5\n",
                "owner.read(gate,true,5)"),
            source("var gate:bool = true\nconst n:int = gate\n",
                "owner.read(gate,true,5)"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"不得靠新步驟表放寬Checker已拒絕來源：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
}
