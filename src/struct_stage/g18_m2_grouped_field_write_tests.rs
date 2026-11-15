// G18／P18.6-M2：原Checker真Group欄位RHS由同一
// Parser/Lexer值葉而非第二套解析取得，BorrowTicket與
// 選中分支的共享退出事件完全遵照既有HC14/R05。
#[cfg(test)]
mod g18_m2_grouped_field_write_tests {
    use super::*;

    const DIRECT: &str = r#"struct Counter{
var n:int = 7
fn set(p:int)->int{
self.n=p+2
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(12)
}
"#;

    const BRANCH: &str = r#"struct Counter{
var n:int = 7
fn set(flag:bool,p:int)->int{
if(flag){
self.n=p+2
return self.n
}else{
self.n=7
return self.n
}
return self.n
}
}
fn main()->int{
var owner:Counter = Counter()
return owner.set(true,12)
}
"#;

    fn checked(source: &str) -> PreparedPrivateSelfMethod {
        verify_source_with_ability(
            source,crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("原Checker必先核准方法Group欄位RHS及獨占借用");
        let plan=PreparedPrivateSelfMethod::prepare(source)
            .expect("已核准原Checker純int欄位Group不應由執行器錯拒");
        assert_eq!(plan.loan.kind,BorrowKind::ExclusiveWrite);
        plan
    }

    fn run(source: &str, expected: Result<i64, PrivateSelfMethodRuntimeError>)
        -> PrivateSelfMethodOutcome {
        let out=checked(source).execute().unwrap();
        assert_eq!(out.result,expected);
        assert!(out.method_entered && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        assert!(out.events.is_empty(),"純int不得假造OwnedStr資源事件");
        assert_eq!((out.main_local_slots_after_exit,out.method_local_slots_after_exit),(0,0));
        out
    }

    #[test]
    fn m2_write_group_直接括號包覆原Checker方法int參數真寫欄位() {
        run(&DIRECT.replace("self.n=p+2","self.n=(p)"),Ok(12));
    }

    #[test]
    fn m2_write_group_直接多層括號整數常值只讀真Lexer值葉() {
        run(&DIRECT.replace("self.n=p+2","self.n=((12))"),Ok(12));
    }

    #[test]
    fn m2_write_group_直接多層括號包覆原Checker方法加減算式() {
        run(&DIRECT.replace("self.n=p+2","self.n=((p+2))"),Ok(14));
    }

    #[test]
    fn m2_write_group_直接各自括號包覆加減左右方法純值() {
        run(&DIRECT.replace("self.n=p+2","self.n=(p)+(2)"),Ok(14));
    }

    #[test]
    fn m2_write_group_直接單層合法整數零值保留真自借票() {
        run(&DIRECT.replace("self.n=p+2","self.n=(0)"),Ok(0));
    }

    #[test]
    fn m2_write_group_原Checker同步純helper在括號內仍用真CallExit() {
        let src=DIRECT.replace("self.n=p+2","self.n=(seed()+2)")
            .replace("fn main()->int{","fn seed()->int{\nreturn 10\n}\nfn main()->int{");
        let out=run(&src,Ok(12));
        assert_eq!((out.ordinary_calls_entered,out.ordinary_calls_completed),(1,1));
        let child=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::FunctionExited{returned:true}
        )).unwrap();
        let loan=out.exit_events.iter().position(|event|matches!(
            event,PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        assert!(child<loan);
    }

    #[test]
    fn m2_write_group_真if分支括號參數算術寫入只在選中true發生() {
        let src=BRANCH.replace("self.n=p+2","self.n=(p+2)");
        let out=run(&src,Ok(14));
        assert_eq!(out.branch_checked,Some(true));
        assert!(out.branch_write_applied);
        assert!(!out.branch_else_write_applied);
    }

    #[test]
    fn m2_write_group_真else分支多層括號常值寫入不執行未選true() {
        let src=BRANCH.replace("self.n=7","self.n=((9))")
            .replace("owner.set(true,12)","owner.set(false,12)");
        let out=run(&src,Ok(9));
        assert_eq!(out.branch_checked,Some(false));
        assert!(!out.branch_write_applied);
        assert!(out.branch_else_write_applied);
    }

    #[test]
    fn m2_write_group_true與else皆使用合法Group時兩條真路徑不混寫() {
        let src=BRANCH.replace("self.n=p+2","self.n=((p+2))")
            .replace("self.n=7","self.n=(3)");
        let yes=run(&src,Ok(14));
        assert!(yes.branch_write_applied && !yes.branch_else_write_applied);
        let no=run(&src.replace("owner.set(true,12)","owner.set(false,12)"),Ok(3));
        assert!(!no.branch_write_applied && no.branch_else_write_applied);
    }

    #[test]
    fn m2_write_group_直接非同步方法直接await也必取得唯一真self獨占票() {
        let src=DIRECT.replace("fn set(p:int)","async fn set(p:int)")
            .replace("self.n=p+2","self.n=((p+2))")
            .replace("return owner.set(12)","return await owner.set(12)");
        let mut q=PrivateCheckedDeferredSelfAwaitQueue::new();
        let id=q.prepare(&src).expect("原Checker核准async self直接await");
        q.suspend(id).unwrap();
        let (_,out)=q.drive_next().unwrap().unwrap();
        assert_eq!(out.result,Ok(14));
        assert_eq!(out.checked_loan.kind,BorrowKind::ExclusiveWrite);
        assert!(out.awaited_method && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
        q.verify_ready_history().unwrap();
    }

    #[test]
    fn m2_write_group_直接括號算術溢位只報Runtime錯誤並歸唯一self票() {
        let src=DIRECT.replace("self.n=p+2",
            "self.n=(p+9223372036854775807)");
        let out=run(&src,Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited);
        let unwind=out.exit_events.iter().position(|event|
            matches!(event,PrivateSharedCallExitEvent::UnwindStarted)).unwrap();
        let returned=out.exit_events.iter().position(|event|
            matches!(event,PrivateSharedCallExitEvent::LoanReturned(_))).unwrap();
        assert!(unwind<returned);
    }

    #[test]
    fn m2_write_group_靜態錯型別或消耗self不得因括號被放行() {
        for src in [
            DIRECT.replace("self.n=p+2","self.n=(true)"),
            DIRECT.replace("self.n=p+2","self.n=(self.move())"),
            DIRECT.replace("self.n=p+2","self.n=((self.n.borrow()))"),
        ] {
            assert!(verify_source_with_ability(
                &src,crate::ownership_policy::approved_leaf_copy_permission,
            ).is_err(),"原Checker不得接受不安全Group寫入：{src}");
            assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn m2_write_group_已驗證直接欄位括號Token被竄改執行前必拒絕() {
        let src=DIRECT.replace("self.n=p+2","self.n=((p+2))");
        let mut plan=checked(&src);
        let index=plan.verified.tokens.iter().position(|tok|
            tok.start>plan.method_span.start && tok.end<plan.method_span.end
                && matches!(tok.kind,TokenKind::Symbol('('))
        ).unwrap();
        plan.verified.tokens[index].kind=TokenKind::Symbol('[');
        assert!(matches!(plan.execute(),Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }
}
