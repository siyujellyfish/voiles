// G18／P18.4-B4／P18.6-M3：原 main 的 Copyable int
// 具名局部可由同源 Checker 函式返回與左右加減算式建立；
// 真函式框架完成後才允許將 Copyable 值傳入同步 self 方法。
#[cfg(test)]
mod g18_b4_m3_main_local_int_call_arithmetic_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
return n
}
}
fn add(a:int,b:int)->int{
return a+b
}
fn sub(a:int,b:int)->int{
return a-b
}
fn main()->int{
var owner:Counter = Counter()
const total:int = add(9,2)+1
return owner.read(total)
}
"#;

    fn run(source:&str)->PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("必須由原 Checker 建立真 int 局部與普通函式身份");
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(),"Copyable int 不可假造 P16 資源");
        out
    }

    fn calls(out:&PrivateSelfMethodOutcome)->(usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn b4_main_math_原main局部函式結果再加一後傳入self等於12() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(12));
        assert_eq!(calls(&out),(1,1));
        let exit=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        assert!(exit<loan);
    }

    #[test]
    fn b4_main_math_兩個不同純函式分居算術左右先後真完成() {
        let src=SOURCE.replace("add(9,2)+1","add(9,2)-sub(9,4)");
        let out=run(&src);
        assert_eq!(out.result,Ok(6));
        assert_eq!(calls(&out),(2,2));
    }

    #[test]
    fn b4_main_math_內層函式完整退出後外層加法才交付main局部() {
        let src=SOURCE.replace("add(9,2)+1","add(sub(9,2),3)+1");
        let out=run(&src);
        assert_eq!(out.result,Ok(11));
        assert_eq!(calls(&out),(2,2));
    }

    #[test]
    fn b4_main_math_第二main局部以第一局部真值呼叫並交付兩值加總() {
        let src=SOURCE.replace(
            "const total:int = add(9,2)+1\nreturn owner.read(total)",
            "const first:int = add(9,2)+1\nconst second:int = sub(first,3)-2\nreturn owner.read(first+second)",
        );
        let out=run(&src);
        assert_eq!(out.result,Ok(19));
        assert_eq!(calls(&out),(2,2));
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_main_math_左函式真完成右邊溢位不得憑空借self() {
        let src=SOURCE.replace("add(9,2)+1",
            "add(9,2)+(9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(calls(&out),(1,1));
        assert!(!out.method_entered && !out.method_exited);
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )));
    }

    #[test]
    fn b4_main_math_左算術先溢位右側純函式零Enter() {
        let src=SOURCE.replace("add(9,2)+1",
            "(9223372036854775807+1)+sub(9,2)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(calls(&out),(0,0));
        assert!(!out.method_entered);
    }

    #[test]
    fn b4_main_math_原Checker零參純int函式結果參與main局部算術() {
        let src=SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b\n}",
            "fn sub(a:int,b:int)->int{\nreturn a-b\n}\nfn base()->int{\nreturn 11\n}")
            .replace("add(9,2)+1","base()+1");
        let out=run(&src);
        assert_eq!(out.result,Ok(12));
        assert_eq!(calls(&out),(1,1));
    }

    #[test]
    fn b4_main_math_未知函式資源非法型別及原計畫變造必先拒絕() {
        for src in [
            SOURCE.replace("add(9,2)+1","missing(9,2)+1"),
            SOURCE.replace("add(9,2)+1","add(9,true)+1"),
            SOURCE.replace("fn add(a:int,b:int)->int{\nreturn a+b",
                "fn add(a:int,b:int)->int{\nvar owned:str = \"非法資源\"\nreturn a+b"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "沒有原 Checker 型別／資源安全者不得接通：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.main_init_plans[0]=PrivateScalarReturnExpr::Constant(12);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
