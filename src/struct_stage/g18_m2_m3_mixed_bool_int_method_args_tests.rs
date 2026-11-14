// G18／P18.6-M2／P18.4-B4-M3：原 Checker 同步 self
// bool 條件與 int 方法形參並存，真 Parser 位置分離 bool/int
// 值槽，不互換 DeclarationId，並與普通 helper 共享退出責任。
#[cfg(test)]
mod g18_m2_m3_mixed_bool_int_method_args_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool,n:int)->int{
if(flag){
return helper(n+3,2)
}else{
return helper(n,3)
}
return self.n
}
}
fn helper(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true,5)
}
"#;

    fn run(source:&str) -> PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source).unwrap();
        assert!(!plan.method_bool_arguments.is_empty(),
            "bool 必須由原 Lexer／Parser 宣告身分提供，不能當成 i64");
        assert!(plan.argument_binding.is_some(),
            "int 必須由獨立原 Checker int DeclarationId 真框架提供");
        assert_ne!(plan.method_bool_arguments[0].0,plan.argument_binding.unwrap().0);
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace() && out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(),"Copyable bool/int 不得偽造 str ResourceId");
        out
    }

    #[test]
    fn m2_mix_bool在前int在後_真flag_true調用純helper結果6() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(6));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn m2_mix_動態false只走原elsehelper且使用真int形參結果2() {
        let out=run(&SOURCE.replace("owner.read(true,5)","owner.read(false,5)"));
        assert_eq!(out.result,Ok(2));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionEntered
        )).count(),2);
    }

    #[test]
    fn m2_mix_方法bool置後仍依原Parser型別位置綁定而非按名稱猜測() {
        let src=SOURCE.replace("fn read(flag:bool,n:int)->int",
            "fn read(n:int,flag:bool)->int")
            .replace("owner.read(true,5)","owner.read(5,true)");
        let out=run(&src);
        assert_eq!(out.result,Ok(6));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn m2_mix_多個int夾bool且動態選else取尾int真DeclarationId() {
        let src=SOURCE.replace("fn read(flag:bool,n:int)->int",
            "fn read(left:int,flag:bool,n:int)->int")
            .replace("helper(n+3,2)","helper(left+3,2)")
            .replace("owner.read(true,5)","owner.read(3,false,8)");
        let out=run(&src);
        assert_eq!(out.result,Ok(5));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn m2_mix_巢狀if真bool形參與int參數同時保留唯一身份() {
        let src=SOURCE.replace(
            "if(flag){\nreturn helper(n+3,2)\n}else{\nreturn helper(n,3)\n}",
            "if(true){\nif(flag){\nreturn helper(n+3,2)\n}else{\nreturn helper(n,3)\n}\n}",
        );
        let true_out=run(&src);
        assert_eq!(true_out.result,Ok(6));
        assert_eq!((true_out.branch_checked,true_out.nested_branch_checked),
            (Some(true),Some(true)));
        let false_out=run(&src.replace("owner.read(true,5)","owner.read(false,5)"));
        assert_eq!(false_out.result,Ok(2));
        assert_eq!((false_out.branch_checked,false_out.nested_branch_checked),
            (Some(true),Some(false)));
        assert_eq!((false_out.ordinary_calls_entered,false_out.ordinary_calls_completed),(1,1));
    }

    #[test]
    fn m2_mix_未選false條件之helper實參溢位不得提前求值或假Enter() {
        let src=SOURCE.replace("owner.read(true,5)","owner.read(false,5)")
            .replace(
                "if(flag){\nreturn helper(n+3,2)\n}else{\nreturn helper(n,3)\n}",
                "if(flag){\nreturn helper(n+9223372036854775807,2)\n}",
            );
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(0,0));
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_mix_真選中helper本體溢位先內層失敗再歸還self唯一票() {
        let src=SOURCE.replace("return a-b",
            "return (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,0));
        let inner=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited { returned:false }
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(inner<loan);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn m2_mix_錯型別多bool未知形參與竄改不同型別DeclId都被拒絕() {
        for src in [
            SOURCE.replace("owner.read(true,5)","owner.read(5,true)"),
            SOURCE.replace("owner.read(true,5)","owner.read(true)"),
            SOURCE.replace("helper(n+3,2)","helper(flag+3,2)"),
            SOURCE.replace("fn read(flag:bool,n:int)->int",
                "fn read(flag:bool,n:bool)->int")
                .replace("owner.read(true,5)","owner.read(true,false)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_)) |
                Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不容許 bool/int 自動混型或不符 Parser 形參：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let int_id=plan.argument_binding.unwrap().0;
        plan.method_bool_arguments[0]=(int_id,true);
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        let bool_id=plan.method_bool_arguments[0].0;
        plan.argument_binding=Some((bool_id,5));
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
