#[cfg(test)]
mod g18_repeated_self_call_tests {
    use super::*;

    const SOURCE: &str = r#"struct Counter{
var n:int = 7
var ready:bool = true
fn change(gate:bool)->int{
if(gate){
self.ready = false
return self.n
}
return self.n
}
fn bump(x:int)->int{
self.n = x + 1
return self.n
}
fn read()->int{
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
const first:int = owner.change(true)
return owner.change(false)
}
"#;

    fn run(source: &str) -> PrivateSelfMethodOutcome {
        let plan = PreparedPrivateSelfMethod::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        for call in out.preceding_calls.iter().chain(std::iter::once(&out)) {
            assert!(call.ticket_returned && call.has_balanced_method_frame());
        }
        out
    }

    #[test]
    fn 另一持有者溢位時已還票且其後的持有者不再執行() {
        let src = SOURCE.replace("const first:int = owner.change(true)", concat!(
            "var other:Counter = Counter()\nconst a:int = owner.bump(20)\n",
            "const b:int = other.bump(9223372036854775807)"));
        let out = run(&src);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.preceding_calls.len(), 1);
        assert_eq!(out.preceding_calls[0].result, Ok(21));
        assert_ne!(out.origin.owner, out.preceding_calls[0].origin.owner);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.has_balanced_self_borrow_trace());
        let invalid = src.replace("var other:Counter", "const other:Counter");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&invalid),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn 交替持有者共用工作圖但保有各自欄位與借票() {
        let src = SOURCE.replace("const first:int = owner.change(true)", concat!(
            "var other:Counter = Counter()\n",
            "const a:int = owner.bump(20)\nconst b:int = other.read()\n",
            "const c:int = other.bump(a + b)"))
            .replace("return owner.change(false)", "return owner.read()");
        let out = run(&src);
        let calls = &out.preceding_calls;
        assert_eq!(calls.iter().map(|c| c.result.clone()).collect::<Vec<_>>(),
            [Ok(21), Ok(7), Ok(29)]);
        assert_eq!(out.result, Ok(21));
        assert_eq!(calls[0].origin.owner, out.origin.owner);
        assert_ne!(calls[0].origin.owner, calls[1].origin.owner);
        assert_eq!(calls[1].origin.owner, calls[2].origin.owner);
        assert_eq!(calls[1].checked_loan.kind, BorrowKind::SharedRead);
        assert_eq!(calls[2].checked_loan.kind, BorrowKind::ExclusiveWrite);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn 呼叫之間保留初始化與空槽並只執行一次陳述式() {
        let src = SOURCE.replace("const first:int = owner.change(true)", concat!(
            "var x:int = 1\nvar delayed:int\nvar gate:bool = true\n",
            "const first:int = owner.bump(x)\n",
            "delayed = first + x\nowner.n = owner.n + 10\n",
            "const second:int = owner.bump(delayed + owner.n)\n",
            "x = second + x\ngate = false\n",
            "const third:int = owner.change(gate)"))
            .replace("return owner.change(false)", "return owner.bump(third + x)");
        let out = run(&src);
        assert_eq!(out.preceding_calls.iter().map(|c| c.result.clone()).collect::<Vec<_>>(),
            [Ok(2), Ok(16), Ok(16)]);
        assert_eq!(out.result, Ok(34));
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn 兩次呼叫共用布林欄位且每次使用自己的借票() {
        let src = SOURCE.replace("owner.change(false)", "owner.change(owner.ready)");
        let out = run(&src);
        assert_eq!(out.result, Ok(7));
        let [first] = out.preceding_calls.as_slice() else { panic!("缺少第一次呼叫"); };
        assert_eq!(first.branch_bool_write_observed, Some(("ready".into(), true, false)));
        assert_eq!(out.branch_checked, Some(false));
        assert_eq!(out.branch_bool_write_observed, None);
        assert_ne!(first.origin.call_span, out.origin.call_span);
        let ticket = |call: &PrivateSelfMethodOutcome| call.exit_events.iter().find_map(|ev|
            if let PrivateSharedCallExitEvent::LoanAcquired(t) = ev { Some(*t) } else { None }
        ).unwrap();
        assert_ne!(ticket(first), ticket(&out));
    }

    #[test]
    fn 連續方法回傳槽供後續實參與共享讀取() {
        let src = SOURCE.replace("owner.change(true)", "((owner.bump(10)))")
            .replace("return owner.change(false)",
                "const second:int = owner.bump(first)\nconst third:int = owner.bump(second + first)\nreturn owner.read()");
        let out = run(&src);
        assert_eq!(out.preceding_calls.iter().map(|c| c.result.clone()).collect::<Vec<_>>(),
            [Ok(11), Ok(12), Ok(24)]);
        assert_eq!(out.result, Ok(24));
        assert_eq!(out.checked_loan.kind, BorrowKind::SharedRead);
    }

    #[test]
    fn 中間方法溢位時不再簽發後續呼叫票據() {
        let src = SOURCE.replace("return owner.change(false)",
            "const second:int = owner.bump(9223372036854775807)\nreturn owner.read()");
        let out = run(&src);
        assert_eq!(out.preceding_calls.len(), 1);
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited);
        assert_eq!(out.checked_loan.kind, BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn 相同方法的兩份成功結果對調或遺失仍會拒絕() {
        let src = SOURCE.replace("return owner.change(false)",
            "const second:int = owner.change(true)\nreturn owner.change(false)");
        let mut out = run(&src);
        out.preceding_calls.swap(0, 1);
        assert!(!out.has_balanced_self_borrow_trace());
        out.preceding_calls.pop();
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn 前置呼叫順序遭變造時整批拒絕() {
        let src = SOURCE.replace("return owner.change(false)",
            "const second:int = owner.bump(first)\nreturn owner.read()");
        let mut plan = PreparedPrivateSelfMethod::prepare(&src).unwrap();
        plan.preceding_sites.swap(0, 1);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let src = SOURCE.replace("owner.change(true)", "owner.change(7)");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }
}
