// G18／P18.6-M2／P18.4-B4：原 Parser 未決的 seed() 零參數來源
// 必須由原 Checker 唯一符號決議及正式 ScalarHIR 封存，再由
// 同步 self 執行器實際進入純 int helper；不可猜測為建構式。
#[cfg(test)]
mod g18_m2_zero_arg_helper_self_write_tests {
    use super::*;

    const DIRECT: &str = r#"struct Counter{
var n:int = 7
fn set()->int{
self.n = seed()
return self.n
}
}
fn seed()->int{
return 14
}
fn main()->int{
var owner:Counter = Counter()
return owner.set()
}
"#;

    const BRANCH: &str = r#"struct Counter{
var n:int = 7
fn set(flag:bool)->int{
if(flag){
self.n = seed()
return self.n
}else{
return 22
}
return self.n
}
}
fn seed()->int{
return 14
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(true)
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        let plan = PreparedPrivateSelfMethod::prepare(source)
            .expect("必須經原 Checker 唯一零參數分類及 ScalarHIR／真 self 借票");
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        plan
    }

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let out = checked(source).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.ticket_returned);
        assert!(out.events.is_empty(), "Copyable int 不可虛構 P16 UTF-8 資源");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    fn count(out: &PrivateSelfMethodOutcome) -> (usize, usize) {
        (out.ordinary_calls_entered, out.ordinary_calls_completed)
    }

    #[test]
    fn m2_zero_arg_直接self寫入真seed零參數回傳14並唯一歸票() {
        let plan = checked(DIRECT);
        assert!(matches!(plan.write_checked_int,
            Some(PrivateScalarReturnExpr::Call { ref name, ref arguments, awaited: false })
                if name == "seed" && arguments.is_empty()
        ));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(count(&out), (1,1));
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanAcquired(_)
        )).count(), 1);
    }

    #[test]
    fn m2_zero_arg_直接零參數helper與加法真ScalarHIR組合寫入19() {
        let source = DIRECT.replace("self.n = seed()", "self.n = seed()+5");
        let out = run(&source);
        assert_eq!(out.result, Ok(19));
        assert_eq!(count(&out), (1,1));
    }

    #[test]
    fn m2_zero_arg_零參數helper作另一helper實參先內後外真退出() {
        let source = DIRECT.replace("self.n = seed()", "self.n = add(seed(),4)")
            .replace("fn main()->int{",
                "fn add(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{");
        let out = run(&source);
        assert_eq!(out.result, Ok(18));
        assert_eq!(count(&out), (2,2));
        let loan_return = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let last_helper_exit = out.exit_events.iter().rposition(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: true }
        )).unwrap();
        assert!(last_helper_exit < loan_return);
    }

    #[test]
    fn m2_zero_arg_真if選葉啟動seed寫入14再歸唯一票() {
        let out = run(BRANCH);
        assert_eq!(out.result, Ok(14));
        assert_eq!(out.branch_checked, Some(true));
        assert!(out.branch_write_applied);
        assert_eq!(count(&out), (1,1));
    }

    #[test]
    fn m2_zero_arg_未選if_true不可以提前進入會溢位的seed() {
        let source = BRANCH.replace("return 14",
            "return 9223372036854775807+1")
            .replace("owner.set(true)", "owner.set(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(22));
        assert_eq!(count(&out), (0,0));
        assert!(!out.branch_write_applied);
        assert!(!out.exit_events.iter().any(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )));
    }

    #[test]
    fn m2_zero_arg_原if_else零參數寫入只在false葉執行() {
        let source = BRANCH.replace(
            "self.n = seed()\nreturn self.n\n}else{\nreturn 22",
            "return self.n\n}else{\nself.n = seed()\nreturn self.n",
        ).replace("owner.set(true)", "owner.set(false)");
        let out = run(&source);
        assert_eq!(out.result, Ok(14));
        assert_eq!(count(&out), (1,1));
        assert!(out.branch_else_write_applied);
        assert!(!out.branch_write_applied);
    }

    #[test]
    fn m2_zero_arg_直接seed本體溢位先Unwind子框架再歸self票() {
        let source = DIRECT.replace("return 14",
            "return 9223372036854775807+1");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(count(&out), (1,0));
        assert!(out.method_entered && !out.method_exited);
        let failed = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::FunctionExited { returned: false }
        )).unwrap();
        let loan = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(failed < loan);
        assert_eq!(out.exit_events.iter().filter(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).count(), 1);
    }

    #[test]
    fn m2_zero_arg_選葉seed本體溢位不可提交欄位且須真歸票() {
        let source = BRANCH.replace("return 14",
            "return 9223372036854775807+1");
        let out = run(&source);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.ticket_returned);
        assert!(!out.branch_write_applied);
        assert_eq!(count(&out), (1,0));
    }

    #[test]
    fn m2_zero_arg_不合法零引數型別async或資源helper皆不得準備() {
        for source in [
            DIRECT.replace("return 14", "return true"),
            DIRECT.replace("fn seed()->int", "async fn seed()->int"),
            DIRECT.replace("fn seed()->int{\nreturn 14",
                "fn seed()->int{\nvar s:str = \"不能偷用資源\"\nreturn 14"),
            DIRECT.replace("seed()", "missing()"),
            DIRECT.replace("var owner:Counter", "const owner:Counter"),
            DIRECT.replace("fn seed()->int", "fn seed(value:int)->int"),
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&source),
                Err(PrivateSelfMethodPrepareError::Check(_))
                    | Err(PrivateSelfMethodPrepareError::Unsupported(_))),
                "不能繞過原 Checker 的唯一零參數函式／型別／借票決議：{source}");
        }
    }

    #[test]
    fn m2_zero_arg_已封存真Call與原Checker工作圖變造先拒絕() {
        let mut plan = checked(DIRECT);
        plan.write_checked_int = Some(PrivateScalarReturnExpr::Constant(14));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(DIRECT);
        plan.method_helpers.get_mut("seed").unwrap().returned =
            PrivateScalarReturnExpr::Constant(15);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(BRANCH);
        let (write, _) = plan.method_branch_write.unwrap();
        let PrivateSelfApprovedBranchWrite::CheckedIntAt(span) = write else {
            panic!("原 Checker 應簽發真選葉零參數 RHS Span");
        };
        plan.method_branch_int_plans.insert(span,
            PrivateScalarReturnExpr::Constant(14));
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
