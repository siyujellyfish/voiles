// G18／P18.4-B4／P18.6-M3：原 Checker 已核准的同步
// 方法 Return 算術內含純 int 普通函式，必按原 Parser
// 左到右真正呼叫／逐框架退出，不可預寫未選中葉或重發 self 票。
#[cfg(test)]
mod g18_b4_m3_method_return_helper_arithmetic_tests {
    use super::*;

    const DIRECT: &str = r#"struct Counter{
var n:int = 7
fn read()->int{
return seed()+2
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
return seed()+2
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
            .expect("須具有原 Lexer／Parser／Checker 完整純 int Return ScalarHIR");
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = checked(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(),"Copyable int 不可虛構 UTF-8 資源");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn counts(out: &PrivateSelfMethodOutcome) -> (usize, usize) {
        (out.ordinary_calls_entered,out.ordinary_calls_completed)
    }

    #[test]
    fn m3_return_arith_直接零參數seed先真返回14再外層加2等於16() {
        let plan = checked(DIRECT);
        assert!(matches!(&plan.return_calculation,
            PrivateSelfIntReturn::CheckedScalar(
                PrivateScalarReturnExpr::Addition(_, _)
            )));
        let out = run(DIRECT);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
        assert!(out.method_entered && out.method_exited);
        let loan = out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert_eq!(out.exit_events[..loan].iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).count(),1);
    }

    #[test]
    fn m3_return_arith_先求Lexer常值再進零參數helper等於16() {
        let src = DIRECT.replace("return seed()+2","return 2+seed()");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
    }

    #[test]
    fn m3_return_arith_兩個普通helper在一個方法Return左右依序退出() {
        let src = DIRECT.replace("fn read()->int{\nreturn seed()+2",
                "fn read(n:int)->int{\nreturn add(n,2)+sub(4,1)")
            .replace("fn main()->int{",
                "fn add(a:int,b:int)->int{\nreturn a+b\n}\nfn sub(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{")
            .replace("owner.read()","owner.read(5)");
        let out = run(&src);
        assert_eq!(out.result,Ok(10));
        assert_eq!(counts(&out),(2,2));
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(),1);
    }

    #[test]
    fn m3_return_arith_外層加法溢位不得變造已真完成的seed退出() {
        let src = DIRECT.replace("return seed()+2",
            "return seed()+9223372036854775807");
        let out = run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,1));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.exit_events.iter().filter(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )).count(),1);
    }

    #[test]
    fn m3_return_arith_seed本體溢位必先退出子框架才歸唯一self票() {
        let src = DIRECT.replace("return 14",
            "return 9223372036854775807+1");
        let out = run(&src);
        assert_eq!(out.result,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(counts(&out),(1,0));
        let failed = out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:false}
        )).unwrap();
        let returned = out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(failed<returned);
    }

    #[test]
    fn m3_return_arith_原if_true只在選中葉進seed返回16() {
        let out = run(BRANCH);
        assert_eq!(out.result,Ok(16));
        assert_eq!(out.branch_checked,Some(true));
        assert_eq!(counts(&out),(1,1));
        let no = run(&BRANCH.replace("owner.read(true)","owner.read(false)"));
        assert_eq!(no.result,Ok(7));
        assert_eq!(no.branch_checked,Some(false));
        assert_eq!(counts(&no),(0,0));
    }

    #[test]
    fn m3_return_arith_原else選中時才執行seed加法正常返回() {
        let src = BRANCH.replace(
            "if(flag){\nreturn seed()+2\n}\nreturn self.n",
            "if(flag){\nreturn self.n\n}else{\nreturn seed()+2\n}\nreturn self.n",
        ).replace("owner.read(true)","owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(out.branch_checked,Some(false));
        assert_eq!(counts(&out),(1,1));
        let yes = run(&src.replace("owner.read(false)","owner.read(true)"));
        assert_eq!(yes.result,Ok(7));
        assert_eq!(counts(&yes),(0,0));
    }

    #[test]
    fn m3_return_arith_巢狀bool只選中一份原seed算術工作圖() {
        let src = BRANCH.replace(
            "fn read(flag:bool)->int{\nif(flag){\nreturn seed()+2\n}\nreturn self.n",
            "fn read(outer:bool,inner:bool)->int{\nif(outer){\nif(inner){\nreturn seed()+2\n}else{\nreturn seed()-1\n}\n}\nreturn self.n+1",
        ).replace("owner.read(true)","owner.read(true,true)");
        let out = run(&src);
        assert_eq!(out.result,Ok(16));
        assert_eq!(counts(&out),(1,1));
        let no = run(&src.replace("owner.read(true,true)","owner.read(true,false)"));
        assert_eq!(no.result,Ok(13));
        assert_eq!(counts(&no),(1,1));
        let skip = run(&src.replace("owner.read(true,true)","owner.read(false,true)"));
        assert_eq!(skip.result,Ok(8));
        assert_eq!(counts(&skip),(0,0));
    }

    #[test]
    fn m3_return_arith_未選分支的seed本體溢位不得預先執行() {
        let src = BRANCH.replace("return 14",
            "return 9223372036854775807+1")
            .replace("owner.read(true)","owner.read(false)");
        let out = run(&src);
        assert_eq!(out.result,Ok(7));
        assert_eq!(counts(&out),(0,0));
        assert!(!out.exit_events.iter().any(|event|matches!(
            event,PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m3_return_arith_錯型別未知函式及封存工作圖篡改須先拒絕() {
        for src in [
            DIRECT.replace("return seed()+2","return seed()+true"),
            DIRECT.replace("return seed()+2","return missing()+2"),
            DIRECT.replace("fn seed()->int{\nreturn 14",
                "fn seed()->int{\nvar owned:str = \"不得偷進資源\"\nreturn 14"),
            DIRECT.replace("fn seed()->int","async fn seed()->int"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "未經原 Checker pure helper 證據不得執行：{src}");
        }
        let mut plan = checked(DIRECT);
        plan.return_calculation = PrivateSelfIntReturn::Constant(16);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(DIRECT);
        plan.method_helpers.get_mut("seed").unwrap().returned =
            PrivateScalarReturnExpr::Constant(17);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
