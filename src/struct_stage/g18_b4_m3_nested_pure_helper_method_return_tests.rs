// G18／P18.4-B4／M3：同步 self 入口已取得唯一 BorrowTicket 後，
// 只有原 Checker 真選中的 Return 葉可以進入有限層純 int helper；
// 任何子框架故障必先退出，再還票／終結方法。
#[cfg(test)]
mod g18_b4_m3_nested_pure_helper_method_return_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
return add(sub(n+2,1),3)
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
return owner.read(5)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("必須真正通過原 Checker 同步 self 與純 int 函式來源");
        assert_eq!(plan.method_helpers.len(),2);
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(),"Copyable int 不得創造 P16 ResourceId");
        out
    }

    fn counts(out:&PrivateSelfMethodOutcome)->(usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn b4_return_nested_借self後內層sub先返回再進外層add最後還票() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(9));
        assert_eq!(counts(&out),(2,2));
        let acquired=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let first_helper=out.exit_events.iter().enumerate()
            .filter_map(|(i,e)|matches!(
                e,PrivateSharedCallExitEvent::FunctionEntered
            ).then_some(i)).nth(1).unwrap();
        let returned_loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(acquired<first_helper && first_helper<returned_loan);
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_return_nested_反向函式身份與回傳計畫不能互換() {
        let src=SOURCE.replace("add(sub(n+2,1),3)","sub(add(n+2,1),3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(5));
        assert_eq!(counts(&out),(2,2));
    }

    #[test]
    fn b4_return_nested_三層不同函式工作圖只取得一次self票() {
        let src=SOURCE.replace("add(sub(n+2,1),3)","add(sub(add(n,1),2),3)");
        let out=run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(counts(&out),(3,3));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_return_nested_條件true與false分別只執行選中巢狀Return() {
        let src=SOURCE.replace(
            "fn read(n:int)->int{\nreturn add(sub(n+2,1),3)",
            "fn read(flag:bool,n:int)->int{\nif(flag){\nreturn add(sub(n,2),3)\n}\nreturn sub(add(n,2),3)",
        ).replace("owner.read(5)","owner.read(true,5)");
        let yes=run(&src);
        assert_eq!(yes.result,Ok(6));
        assert_eq!(yes.branch_checked,Some(true));
        assert_eq!(counts(&yes),(2,2));
        let no=run(&src.replace("owner.read(true,5)","owner.read(false,5)"));
        assert_eq!(no.result,Ok(4));
        assert_eq!(no.branch_checked,Some(false));
        assert_eq!(counts(&no),(2,2));
    }

    #[test]
    fn b4_return_nested_內層本體溢位先Unwind內函式再歸還外方法票() {
        let src=SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
            "fn sub(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,0));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(),2);
    }

    #[test]
    fn b4_return_nested_外層本體溢位保留已完成子函式真退出() {
        let src=SOURCE.replace("fn add(a:int,b:int)->int{\nreturn a+b",
            "fn add(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(2,1));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn b4_return_nested_未選中溢位分支完全不進helper也不釋放假資源() {
        let src=SOURCE.replace(
            "fn read(n:int)->int{\nreturn add(sub(n+2,1),3)",
            "fn read(flag:bool,n:int)->int{\nif(flag){\nreturn add(sub(9223372036854775807,1),9223372036854775807)\n}\nreturn n",
        ).replace("owner.read(5)","owner.read(false,5)");
        let out=run(&src);
        assert_eq!(out.result,Ok(5));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!(counts(&out),(0,0));
        assert!(out.method_entered && out.method_exited);
    }

    #[test]
    fn b4_return_nested_未證算術包裝資源與竄改任一子計畫先拒絕() {
        for src in [
            SOURCE.replace("add(sub(n+2,1),3)","add(sub(n+2,1)+self.n,3)"),
            SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
                "fn sub(a:int,b:int)->int{\nvar s:str = \"禁止副作用\"\nreturn a-b"),
            SOURCE.replace("add(sub(n+2,1),3)","add(missing(n+2,1),3)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不能容許無原 Checker 身分／純整數框架來源：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helpers.get_mut("sub").unwrap().returned =
            PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
