// G18／P18.6-M3／P18.4-B4：原 Checker 真 Group
// 及巢狀括號包覆同步純 int helper Return。只在實際
// 選中方法退出葉求值，外層 BorrowTicket 依 R05 真歸還。
#[cfg(test)]
mod g18_b4_m3_grouped_helper_method_return_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return (seed()+2)
}
}
fn seed()->int{
return 14
}
fn main()->int{
var owner:Counter = Counter()
return owner.read()
}
"#;

    const BRANCH: &str = r#"struct Counter{
var n:int = 7
fn read(flag:bool)->int{
if(flag){
return (seed()+2)
}
return self.n
}
}
fn seed()->int{
return 14
}
fn main()->int{
var owner:Counter = Counter()
return owner.read(true)
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("只接受原 Checker 完整 Group/ScalarHIR 的純 helper Return");
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = checked(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "純 Copyable int 不可偽造 UTF-8 資源");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn counts(out: &PrivateSelfMethodOutcome) -> (usize,usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn m3_grouped_真Group包覆seed加2回傳16並只進一次helper() {
        let plan = checked(SOURCE);
        assert!(matches!(&plan.return_calculation,
            PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Addition(_, _)
            )));
        let out = run(SOURCE);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).count(),1);
    }

    #[test]
    fn m3_grouped_真雙層括號同一TypedHIR不重複執行函式() {
        let src = SOURCE.replace("return (seed()+2)", "return ((seed()+2))");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
    }

    #[test]
    fn m3_grouped_真純helper用Group先整數常值後呼叫返回16() {
        let src = SOURCE.replace("return (seed()+2)", "return (2+seed())");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
    }

    #[test]
    fn m3_grouped_if_true真執行而false不啟動封存helper() {
        let yes = run(BRANCH);
        assert_eq!(yes.result,Ok(16));
        assert_eq!(counts(&yes),(1,1));
        let no = run(&BRANCH.replace("owner.read(true)","owner.read(false)"));
        assert_eq!(no.result,Ok(7));
        assert_eq!(counts(&no),(0,0));
        assert_eq!(no.branch_checked,Some(false));
    }

    #[test]
    fn m3_grouped_原else包覆純helper只在false葉真呼叫() {
        let src = BRANCH.replace(
            "if(flag){\nreturn (seed()+2)\n}\nreturn self.n",
            "if(flag){\nreturn self.n\n}else{\nreturn (seed()+2)\n}\nreturn self.n",
        ).replace("owner.read(true)","owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
        let skip = run(&src.replace("owner.read(false)","owner.read(true)"));
        assert_eq!(skip.result,Ok(7));
        assert_eq!(counts(&skip),(0,0));
    }

    #[test]
    fn m3_grouped_巢狀bool兩Group選葉只能真執行一次() {
        let src = BRANCH.replace(
            "fn read(flag:bool)->int{\nif(flag){\nreturn (seed()+2)\n}\nreturn self.n",
            "fn read(outer:bool,inner:bool)->int{\nif(outer){\nif(inner){\nreturn (seed()+2)\n}else{\nreturn (seed()-1)\n}\n}\nreturn self.n+1",
        ).replace("owner.read(true)","owner.read(true,true)");
        let yes = run(&src);
        assert_eq!(yes.result,Ok(16));
        assert_eq!(counts(&yes),(1,1));
        let no = run(&src.replace("owner.read(true,true)","owner.read(true,false)"));
        assert_eq!(no.result,Ok(13));
        assert_eq!(counts(&no),(1,1));
        let skip = run(&src.replace("owner.read(true,true)","owner.read(false,true)"));
        assert_eq!(skip.result,Ok(8));
        assert_eq!(counts(&skip),(0,0));
    }

    #[test]
    fn m3_grouped_真Helper本體溢位須先退出子框架後歸票() {
        let src = SOURCE.replace("return 14","return 9223372036854775807+1");
        let out = run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,0));
        let child_exit = out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).unwrap();
        let loan = out.exit_events.iter().position(|e|matches!(
            e,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(child_exit<loan);
    }

    #[test]
    fn m3_grouped_不核准混合self欄位錯型別與篡改原圖須拒絕() {
        for src in [
            SOURCE.replace("return (seed()+2)","return (seed()+self.n)"),
            SOURCE.replace("return (seed()+2)","return (seed()+true)"),
            SOURCE.replace("return (seed()+2)","return (missing()+2)"),
            SOURCE.replace("fn seed()->int{\nreturn 14",
                "fn seed()->int{\nvar text:str = \"不得偷用資源\"\nreturn 14"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "原 Checker 未封存的 Group/Call 不准執行：{src}");
        }
        let mut plan = checked(SOURCE);
        plan.return_calculation = PrivateSelfIntReturn::Constant(16);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
