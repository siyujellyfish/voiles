// G18／P18.4-B4／P18.6-M3：原 Checker 真認證的純 int helper
// 有限巢狀直線方法局部與 var 更新，共用 self 真 BorrowTicket、
// DeclarationId、R05 函式進退及 LocalInitialized/Assigned/ScopeEnded。
#[cfg(test)]
mod g18_b4_m3_nested_pure_helper_method_local_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read(n:int)->int{
var saved:int = add(sub(n,2),3)
saved = sub(add(saved,4),2)
return saved
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
            .expect("只能使用原 Lexer／Parser／Checker 核准的純 int 同源函式");
        assert_eq!(plan.method_helpers.len(),2);
        let out=plan.execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        assert!(out.events.is_empty(),"Copyable int 不得虛構 ResourceId");
        out
    }

    fn count(out:&PrivateSelfMethodOutcome)->(usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn b4_local_nested_方法局部先內sub後外add完成才記初始化() {
        let src=SOURCE.replace("saved = sub(add(saved,4),2)\n","");
        let out=run(&src);
        assert_eq!(out.result,Ok(6));
        assert_eq!(count(&out),(2,2));
        let loan=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).unwrap();
        let init=out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LocalInitialized(_)
        )).unwrap();
        assert!(loan<init);
        assert_eq!(out.exit_events[loan..init].iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),2);
    }

    #[test]
    fn b4_local_nested_先完成局部後以兩層helper更新原var且返回8() {
        let out=run(SOURCE);
        assert_eq!(out.result,Ok(8));
        assert_eq!(count(&out),(4,4));
        assert_eq!(out.method_local_events.iter().filter(|e|matches!(
            e,PrivateSelfMethodLocalEvent::Initialized(_, _)
        )).count(),1);
        assert_eq!(out.method_local_events.iter().filter(|e|matches!(
            e,PrivateSelfMethodLocalEvent::Assigned(_, _)
        )).count(),1);
    }

    #[test]
    fn b4_local_nested_局部更新完成後Return再執行兩層仍共用唯一票據() {
        let src=SOURCE.replace("return saved","return add(sub(saved,1),2)");
        let out=run(&src);
        assert_eq!(out.result,Ok(9));
        assert_eq!(count(&out),(6,6));
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn b4_local_nested_三層局部與兩個兄弟helper保留真原參數順序() {
        let src=SOURCE.replace("add(sub(n,2),3)","add(sub(add(n,2),1),sub(n,4))")
            .replace("saved = sub(add(saved,4),2)\n","");
        let out=run(&src);
        // n=5，左=add(7-1)=6，右=1，所以 saved=7。
        assert_eq!(out.result,Ok(7));
        assert_eq!(count(&out),(4,4));
    }

    #[test]
    fn b4_local_nested_第一子helper完成後第二引數溢位不得初始化局部() {
        let src=SOURCE.replace("add(sub(n,2),3)",
            "add(sub(n,2),9223372036854775807+1)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,1));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.method_local_events.is_empty());
        assert!(!out.exit_events.iter().any(|e|matches!(
            e,PrivateSharedCallExitEvent::LocalInitialized(_)
        )));
    }

    #[test]
    fn b4_local_nested_局部內層本體溢位外helper未進並歸還self票() {
        let src=SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
            "fn sub(a:int,b:int)->int{\nreturn (a+b)+9223372036854775807");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(1,0));
        assert!(out.method_entered && !out.method_exited);
        assert!(out.ticket_returned);
        assert_eq!(out.exit_events.iter().filter(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).count(),2);
    }

    #[test]
    fn b4_local_nested_更新RHS失敗不提交LocalAssigned只逆序清理已建立局部() {
        let src=SOURCE.replace("sub(add(saved,4),2)",
            "sub(add(saved,9223372036854775807),2)");
        let out=run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out),(3,2));
        assert_eq!(out.method_local_events.iter().filter(|e|matches!(
            e,PrivateSelfMethodLocalEvent::Initialized(_, _)
        )).count(),1);
        assert_eq!(out.method_local_events.iter().filter(|e|matches!(
            e,PrivateSelfMethodLocalEvent::Assigned(_, _)
        )).count(),0);
        assert_eq!(out.method_local_events.iter().filter(|e|matches!(
            e,PrivateSelfMethodLocalEvent::ScopeEnded(_)
        )).count(),1);
        assert!(out.ticket_returned);
    }

    #[test]
    fn b4_local_nested_非純值巢狀算術包裝非法函式與竄改DeclId仍拒絕() {
        for src in [
            SOURCE.replace("add(sub(n,2),3)","add(sub(n,2)+1,3)"),
            SOURCE.replace("fn sub(a:int,b:int)->int{\nreturn a-b",
                "fn sub(a:int,b:int)->int{\nvar resource:str = \"不允許\"\nreturn a-b"),
            SOURCE.replace("sub(add(saved,4),2)","unknown(add(saved,4),2)"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "原 Checker 未核准／非純值不得接通：{src}");
        }
        let mut plan=PreparedPrivateSelfMethod::prepare(SOURCE).unwrap();
        plan.method_helpers.get_mut("sub").unwrap().parameters.swap(0,1);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
