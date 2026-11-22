#[cfg(test)]
mod g18_private_method_self_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        PreparedPrivateSelfMethod::prepare(src)
            .expect("已核准同步唯讀 self 方法須有正式 Checker 及 BorrowFact")
    }

    fn normal() -> &'static str {
        "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n"
    }

    #[test]
    fn g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位() {
        let plan = checked(normal());
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert!(outcome.method_entered && outcome.method_exited);
        assert!(outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_合法_const_結構接收者共享_self_不需可寫權限() {
        let src = "struct Counter{\nvar count:int = 5\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert!(outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域() {
        let result = checked(normal()).execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(result.method_entered);
        assert!(!result.method_exited);
        assert!(result.ticket_returned);
        assert!(result.events.is_empty());
    }

    #[test]
    fn g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕() {
        let mut plan = checked(normal());
        plan.loan.source = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(normal());
        plan.loan.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_移除真_checker_方法自動借用或方法證據不能執行() {
        let mut plan = checked(normal());
        plan.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(normal());
        plan.verified.plan.method_safety.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_自動_self_錯誤返回型別與消耗整個_self_由原_checker_拒絕() {
        let wrong_type = "struct Counter{\nvar count:int = 7\nfn get()->bool{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        let wrong_move = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        for source in [wrong_type, wrong_move] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(source),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_f_獨占_self_真寫入_var_int_並以同張票據返回更新後值() {
        let src = "struct Counter{\nvar count:int = 1\nfn set()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.write_value, Some(9));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(9));
        assert!(actual.method_entered && actual.method_exited);
        assert!(actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_真寫入後重跑同一已驗證來源不保留前次狀態() {
        let src = "struct Counter{\nvar count:int = 2\nfn set()->int{\nself.count = 17\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        for _ in 0..3 {
            let actual = plan.execute().unwrap();
            assert_eq!(actual.result, Ok(17));
            assert!(actual.ticket_returned && actual.events.is_empty());
        }
    }

    #[test]
    fn g18_f_獨占_self_執行中故障必歸還票據且不報成功返回() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        let failure = plan.execute_with_fault(true).unwrap();
        assert_eq!(failure.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failure.method_entered);
        assert!(!failure.method_exited);
        assert!(failure.ticket_returned);
        assert!(failure.events.is_empty());
        let ok = plan.execute().unwrap();
        assert_eq!(ok.result, Ok(8));
    }

    #[test]
    fn g18_f_const_接收者不能以獨占_self_修改原欄位() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_const_欄位即使接收者為_var_也不能由方法覆寫() {
        let src = "struct Counter{\nconst count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_篡改已驗證的獨占_self_票據或寫入值在動態配置前拒絕() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let mut changed = checked(src);
        changed.loan.kind = BorrowKind::SharedRead;
        assert!(matches!(changed.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut changed = checked(src);
        changed.write_value = Some(999);
        assert!(matches!(changed.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_獨占_self_寫入禁止非_int_值不藉執行器自行轉型() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = true\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_共享_self_真欄位加法按原_checker_hir_求值() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+3\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Add(3));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(10));
        assert!(actual.method_entered && actual.method_exited && actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_寫入後真欄位減法按最新值計算() {
        let src = "struct Counter{\nvar count:int = 2\nfn change()->int{\nself.count = 20\nreturn self.count-4\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Subtract(4));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(16));
        assert!(actual.method_entered && actual.method_exited && actual.ticket_returned);
    }

    #[test]
    fn g18_f_共享_self_真欄位加法溢位受控錯誤且歸還票據() {
        let src = "struct Counter{\nvar count:int = 9223372036854775807\nfn get()->int{\nreturn self.count+1\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(actual.method_entered && !actual.method_exited && actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_真欄位減法溢位亦不得保留借用() {
        let src = "struct Counter{\nvar count:int = 0\nfn change()->int{\nself.count = 0\nreturn self.count-9223372036854775807-2\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        // 本子集只接通 self.field ± 一個原 Checker int 常值；
        // 巢狀運算不得冒充已執行，必須在配置前拒絕。
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }

    #[test]
    fn g18_f_竄改已核准方法算術運算子或常值須在配置前拒絕() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+3\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let mut plan = checked(src);
        plan.return_calculation = PrivateSelfIntReturn::Subtract(3);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(src);
        plan.return_calculation = PrivateSelfIntReturn::Add(999);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_錯誤方法加法型別與未知欄位由原_checker_拒絕() {
        for src in [
            "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+true\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n",
            "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.missing+1\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n",
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
    fn overloaded_zero_source(reverse: bool, mutable: bool) -> String {
        let read = "fn get()->int{\nreturn self.count+3\n}\n";
        let other = "fn get(n:int)->int{\nself.count = 99\nreturn self.count\n}\n";
        let methods = if reverse { format!("{other}{read}") } else { format!("{read}{other}") };
        let owner = if mutable { "var" } else { "const" };
        format!("struct Counter{{\nvar count:int = 7\n{methods}}}\nfn main()->int{{\n{owner} value:Counter = Counter()\nreturn value.get()\n}}\n")
    }

    #[test]
    fn g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選() {
        let src = overloaded_zero_source(false, false);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Add(3));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert!(outcome.method_entered && outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行() {
        for reverse in [false, true] {
            let plan = checked(&overloaded_zero_source(reverse, true));
            assert_eq!(plan.execute().unwrap().result, Ok(10));
        }
    }

    #[test]
    fn g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選() {
        let src = "struct Counter{\nvar count:int = 7\nfn change(n:int)->int{\nreturn self.count\n}\nfn change()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.write_value, Some(9));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_零參數多載方法中途故障仍先歸還原_self_票據() {
        let plan = checked(&overloaded_zero_source(true, true));
        let outcome = plan.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕() {
        let src = overloaded_zero_source(true, true);
        let mut plan = checked(&src);
        plan.method_span = crate::common_ir::NodeSpan::new(0, 2);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.loan.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_同一_int_參數左右加法仍取同一真框架值() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\nfn get(n:int)->int{\nreturn n+n\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(3)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
    }

    fn overloaded_int_source(reverse: bool, owner_mutable: bool) -> String {
        let zero = "fn get()->int{\nself.count = 99\nreturn self.count\n}\n";
        let one = "fn get(n:int)->int{\nreturn self.count+2\n}\n";
        let methods = if reverse { format!("{one}{zero}") } else { format!("{zero}{one}") };
        let keyword = if owner_mutable { "var" } else { "const" };
        format!("struct Counter{{\nvar count:int = 7\n{methods}}}\nfn main()->int{{\n{keyword} value:Counter = Counter()\nreturn value.get(4)\n}}\n")
    }

    #[test]
    fn g18_f_同名方法單_int_實參依原_checker_型別選真共享多載() {
        let plan = checked(&overloaded_int_source(false, false));
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(4));
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.method_entered && result.method_exited && result.ticket_returned);
        assert!(result.events.is_empty());
    }

    #[test]
    fn g18_f_具_int_實參的多載宣告順序不影響真方法選擇() {
        for reverse in [false, true] {
            let plan = checked(&overloaded_int_source(reverse, true));
            assert_eq!(plan.execute().unwrap().result, Ok(9));
        }
    }

    #[test]
    fn g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nreturn self.count\n}\nfn set(n:int)->int{\nself.count = 11\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set(3)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(3));
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert!(result.ticket_returned && result.events.is_empty());
    }

    #[test]
    fn g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活() {
        let plan = checked(&overloaded_int_source(true, true));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(!failed.method_exited && failed.ticket_returned);
        assert!(failed.events.is_empty());
        assert_eq!(plan.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_f_竄改已驗證_int_方法實參值或宣告身分須在動態執行前拒絕() {
        let src = overloaded_int_source(true, true);
        let mut plan = checked(&src);
        plan.argument_binding = plan.argument_binding.map(|(id, _)| (id, 100));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_binding = Some((DeclarationId(0), 4));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.verified.plan.members.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_已核准_int_參數與_self_欄位可真混算但非_int_實參仍拒絕() {
        let other = "struct Counter{\nvar count:int = 7\nfn get(n:int)->int{\nreturn n+self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(3)\n}\n";
        let outcome = checked(other).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert!(outcome.ticket_returned);
        let bool_arg = "struct Counter{\nvar count:int = 7\nfn get(flag:bool)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(true)\n}\n";
        // 先前這個歷史案例預期尚未接通的 bool 方法為 Unsupported。
        // M2 已核准 bool 形參值槽後，合法 true 原來源必真執行，
        // 但不得轉為 int 或產生假的 UTF-8／借用票據。
        let bool_out = checked(bool_arg).execute().unwrap();
        assert_eq!(bool_out.result, Ok(7));
        assert!(bool_out.ticket_returned);
        assert!(bool_out.has_balanced_self_borrow_trace());
        assert!(bool_out.events.is_empty());
    }

    fn method_param_source(body: &str, argument: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn use()->int{{\nreturn self.count\n}}\nfn use(n:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.use({argument})\n}}\n"
        )
    }

    #[test]
    fn g18_f_方法真參數返回使用所選_DeclarationId_的框架值() {
        let src = method_param_source("return n", "13");
        let plan = checked(&src);
        let expected = plan.argument_binding.expect("真正單一 int 參數");
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Parameter(expected.0));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert!(outcome.method_entered && outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真加法在框架內求值不誤讀_self_欄位() {
        let plan = checked(&method_param_source("return n+3", "13"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(16));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真減法與重複呼叫保留同一來源參數值() {
        let plan = checked(&method_param_source("return n-4", "13"));
        for _ in 0..3 {
            let outcome = plan.execute().unwrap();
            assert_eq!(outcome.result, Ok(9));
            assert!(outcome.ticket_returned);
        }
    }

    #[test]
    fn g18_f_方法參數真加法溢位受控錯誤且先歸還_self_票據() {
        let plan = checked(&method_param_source("return n+1", "9223372036854775807"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真讀取之前故障不得報告成功返回() {
        let plan = checked(&method_param_source("return n", "13"));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failed.method_entered && !failed.method_exited && failed.ticket_returned);
        assert_eq!(plan.execute().unwrap().result, Ok(13));
    }

    #[test]
    fn g18_f_篡改已驗證的方法參數返回身分或算術方案必在配置前拒絕() {
        let src = method_param_source("return n+3", "13");
        let mut plan = checked(&src);
        plan.return_calculation = PrivateSelfIntReturn::Parameter(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        let id = plan.argument_binding.unwrap().0;
        plan.return_calculation = PrivateSelfIntReturn::ParameterSubtract(id, 3);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_方法未定義的參數來源及錯誤型別須先由原_checker_拒絕() {
        let unknown = method_param_source("return missing", "13");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let invalid = method_param_source("return n+true", "13");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&invalid),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_同步共享與獨占_self_正常及故障均歸還真票據且零假資源() {
        for source in [
            normal(),
            "struct Counter{\nvar count:int = 1\nfn set()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n",
        ] {
            let plan = checked(source);
            for fail in [false, true] {
                let out = plan.execute_with_fault(fail).unwrap();
                assert!(out.has_balanced_self_borrow_trace());
                assert!(out.ticket_returned);
                assert_eq!(out.method_exited, !fail);
                assert!(out.events.is_empty());
            }
        }
    }

    #[test]
    fn g18_r05_同步_self_偽造未歸票據或無來源資源事件必被守門拒絕() {
        let plan = checked(normal());
        let mut out = plan.execute().unwrap();
        out.ticket_returned = false;
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = plan.execute().unwrap();
        out.method_entered = false;
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = plan.execute().unwrap();
        out.events.push(crate::owned_str_runtime::ResourceEvent::Created(1));
        out.events.push(crate::owned_str_runtime::ResourceEvent::Released(1));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    fn multi_int_method_source(body: &str, a: &str, b: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calculate()->int{{\nreturn self.count\n}}\nfn calculate(n:int)->int{{\nreturn n\n}}\nfn calculate(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.calculate({a},{b})\n}}\n"
        )
    }

    #[test]
    fn g18_f_雙_int_方法真依原多載簽章選取並計算兩個獨立參數() {
        let plan = checked(&multi_int_method_source("return a+b", "11", "4"));
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(11));
        assert_eq!(plan.additional_argument_bindings.len(), 1);
        assert_eq!(plan.additional_argument_bindings[0].1, 4);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(15));
        assert!(outcome.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_f_雙_int_方法可只返回第二參數而不錯讀第一或_self_() {
        let plan = checked(&multi_int_method_source("return b", "11", "4"));
        let second = plan.additional_argument_bindings[0].0;
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Parameter(second));
        assert_eq!(plan.execute().unwrap().result, Ok(4));
    }

    #[test]
    fn g18_f_雙_int_方法依實參順序正確執行減法() {
        let plan = checked(&multi_int_method_source("return a-b", "4", "11"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(-7));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_雙_int_方法加法溢位仍歸還唯一真_self_票據() {
        let plan = checked(&multi_int_method_source(
            "return a+b", "9223372036854775807", "1",
        ));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_多_int_實參方法故障不殘留形參且重新呼叫不受污染() {
        let plan = checked(&multi_int_method_source("return b", "11", "4"));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failed.method_entered && !failed.method_exited && failed.ticket_returned);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
    }

    #[test]
    fn g18_f_竄改第二方法實參值或宣告身分均於真資源前拒絕() {
        let src = multi_int_method_source("return a+b", "11", "4");
        let mut wrong = checked(&src);
        wrong.additional_argument_bindings[0].1 = 100;
        assert!(matches!(wrong.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut wrong = checked(&src);
        wrong.additional_argument_bindings[0].0 = wrong.argument_binding.unwrap().0;
        assert!(matches!(wrong.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_三個_int_方法參數按原宣告位置獨立綁定返回第三個() {
        let src = "struct Counter{\nvar count:int = 7\nfn last(a:int,b:int,c:int)->int{\nreturn c\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.last(4,8,12)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.argument_binding.map(|entry| entry.1), Some(4));
        assert_eq!(plan.additional_argument_bindings.len(), 2);
        assert_eq!(plan.additional_argument_bindings.iter().map(|entry| entry.1).collect::<Vec<_>>(), [8, 12]);
        assert_eq!(plan.execute().unwrap().result, Ok(12));
    }

    #[test]
    fn g18_f_多_int_方法來源錯型別與不存在參數須原_checker_或安全準備拒絕() {
        let wrong_name = multi_int_method_source("return missing", "11", "4");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_name),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        // 原 Checker 已核准的 (int,bool) 方法現可真執行；
        // 舊「合法混型一律 Unsupported」反例改為真正非法的
        // int→bool 實參錯配，原型別安全拒絕不許退步。
        let mixed = "struct Counter{\nvar count:int = 7\nfn calculate(a:int,b:bool)->int{\nreturn a\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calculate(11,12)\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(mixed),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    fn mixed_self_param_source(body: &str, argument: &str, mutable: bool) -> String {
        let owner = if mutable { "var" } else { "const" };
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(n:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\n{owner} value:Counter = Counter()\nreturn value.calc({argument})\n}}\n"
        )
    }

    #[test]
    fn g18_f_共享_self_欄位加參數讀取真_runtime_與宣告身分() {
        let src = mixed_self_param_source("return self.count+n", "4", false);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.method_entered && out.method_exited && out.ticket_returned);
    }

    #[test]
    fn g18_f_共享_self_形式參數減欄位維持左減右次序() {
        let plan = checked(&mixed_self_param_source("return n-self.count", "4", false));
        assert_eq!(plan.execute().unwrap().result, Ok(-3));
        let reverse = checked(&mixed_self_param_source("return self.count-n", "4", false));
        assert_eq!(reverse.execute().unwrap().result, Ok(3));
    }

    #[test]
    fn g18_f_獨占_self_真寫入後再同方法與_int_形式參數混算() {
        let src = mixed_self_param_source("self.count = 11\nreturn self.count+n", "4", true);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(15));
        assert!(out.ticket_returned && out.events.is_empty());
    }

    #[test]
    fn g18_f_共享_self_欄位加巨大_int_實參溢位仍歸還真借用() {
        let plan = checked(&mixed_self_param_source(
            "return self.count+n", "9223372036854775807", false,
        ));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert!(out.events.is_empty());
    }

    #[test]
    fn g18_f_雙_int_方法只讀第二參數與_self_欄位不混用第一個() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(a:int,b:int)->int{\nreturn self.count+b\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(100,4)\n}\n";
        let plan = checked(src);
        let id = plan.additional_argument_bindings[0].0;
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::FieldAddParameter(id));
        assert_eq!(plan.execute().unwrap().result, Ok(11));
    }

    #[test]
    fn g18_f_篡改_self_混算方向與參數身分須在動態配置前拒絕() {
        let src = mixed_self_param_source("return self.count+n", "4", false);
        let mut plan = checked(&src);
        let id = plan.argument_binding.unwrap().0;
        plan.return_calculation = PrivateSelfIntReturn::FieldSubtractParameter(id);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.return_calculation = PrivateSelfIntReturn::FieldAddParameter(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_偽造_other_欄位與錯誤接收者不得冒充真正_self_來源() {
        let src = mixed_self_param_source("return other.count+n", "4", false);
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    fn main_named_method_source(locals: &str, actuals: &str, body: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\n{locals}\nreturn value.calc({actuals})\n}}\n"
        )
    }

    #[test]
    fn g18_r02_r04_同步方法讀取main具名_const_int_真局部框架() {
        let src = main_named_method_source(
            "const amount:int = 5", "amount,3", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 1);
        assert_eq!(plan.argument_sources[0], Some(plan.main_int_locals[0].1));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.ticket_returned && out.method_exited);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_雙具名_copyable_int_按main宣告及原方法參數次序做減法() {
        let src = main_named_method_source(
            "var left:int = 4\nconst right:int = 11", "right,left", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 2);
        assert_eq!(plan.argument_sources,
            [Some(plan.main_int_locals[1].1), Some(plan.main_int_locals[0].1)]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_main整數局部由前一已初始化名稱複製再傳入self方法() {
        let src = main_named_method_source(
            "const first:int = 12\nconst second:int = first",
            "second,4", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals[1].3, Some(plan.main_int_locals[0].1));
        assert_eq!(plan.execute().unwrap().result, Ok(8));
    }

    #[test]
    fn g18_r02_r04_同一具名_copyable_int_可作兩個方法實參不消耗來源() {
        let src = main_named_method_source(
            "const both:int = 6", "both,both", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.argument_sources, [Some(plan.main_int_locals[0].1); 2]);
        for _ in 0..3 {
            let out = plan.execute().unwrap();
            assert_eq!(out.result, Ok(12));
            assert_eq!(out.main_local_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_r02_r05_具名main實參與_self_欄位加法溢位仍清除局部及真票據() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst amount:int = 9223372036854775807\nreturn value.calc(amount)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.events.is_empty());
    }

    #[test]
    fn g18_r02_r05_同步方法具名實參故障注入仍先歸還借用後清空main局部() {
        let src = main_named_method_source(
            "const amount:int = 11", "amount,4", "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(plan.execute().unwrap().result, Ok(15));
    }

    #[test]
    fn g18_r02_r04_篡改具名來源局部值身分或方法實參來源皆須零執行拒絕() {
        let src = main_named_method_source(
            "const amount:int = 11", "amount,4", "return a+b",
        );
        let mut plan = checked(&src);
        plan.main_int_locals[0].2 = 99;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_sources[0] = Some(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_binding = plan.argument_binding.map(|(id, _)| (id, 100));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_尚未宣告或未初始化main實參不得進入同步self方法() {
        let future = main_named_method_source(
            "var later:int", "later,4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&future),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let wrong_type = main_named_method_source(
            "const amount:bool = true", "amount,4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r02_r04_同步方法實參共用原_typedhir_具名加法括號與返回() {
        let src = main_named_method_source(
            "const amount:int = 5", "(amount+3),4", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.argument_binding.map(|(_, value)| value), Some(8));
        assert_eq!(plan.argument_sources[0], None);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.ticket_returned && out.method_exited);
    }

    #[test]
    fn g18_r02_r04_巢狀括號與左右加減依原_checker_樹求值() {
        let src = main_named_method_source(
            "const amount:int = 11", "((amount-(7-3))),((2+4))",
            "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.execute().unwrap().result, Ok(1));
    }

    #[test]
    fn g18_r02_r04_已核准複合整數實參與_self_欄位回傳使用同一真框架() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst amount:int = 3\nreturn value.calc(amount+4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.ticket_returned && out.events.is_empty());
    }

    #[test]
    fn g18_r02_r05_第二個整數複合實參溢位不可提前取得_self_借用或進入方法() {
        let src = main_named_method_source(
            "const amount:int = 1",
            "3,(amount+9223372036854775807)",
            "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.ticket_returned && out.events.is_empty());
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r05_第一個複合實參溢位不得執行後續方法本體() {
        let src = main_named_method_source(
            "const amount:int = 1",
            "(amount+9223372036854775807),4",
            "return a+b",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_偽造已編譯方法實參算術樹或來源位置於執行前拒絕() {
        let src = main_named_method_source(
            "const amount:int = 5", "(amount+3),4", "return a+b",
        );
        let mut plan = checked(&src);
        plan.argument_plans[0] = PrivateScalarReturnExpr::Constant(8);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_sources[0] = Some(plan.main_int_locals[0].1);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_非法布林混用與未宣告名稱在方法複合引數前先被_checker_拒絕() {
        let wrong_type = main_named_method_source(
            "const amount:int = 5", "(amount+true),4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let unknown = main_named_method_source(
            "const amount:int = 5", "(missing+3),4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r02_r04_main整數複合初始化使用同一_typedhir_再傳同步方法() {
        let src = main_named_method_source(
            "const base:int = 5\nconst delta:int = (base+4)",
            "delta,2", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 2);
        assert_eq!(plan.main_init_plans.len(), 2);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.ticket_returned && out.method_exited);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_巢狀main局部初始化依原整數減法左右次序傳入方法() {
        let src = main_named_method_source(
            "const first:int = 12\nconst second:int = ((first-4)+2)",
            "second,3", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals[1].2, 10);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_r02_r04_已計算main局部可再被Copyable名稱引用並與_self_欄位混算() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst first:int = 3\nconst computed:int = first+4\nconst forwarded:int = computed\nreturn value.calc(forwarded)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.main_int_locals.len(), 3);
        assert_eq!(plan.main_int_locals[2].3, Some(plan.main_int_locals[1].1));
        assert_eq!(plan.execute().unwrap().result, Ok(14));
    }

    #[test]
    fn g18_r02_r05_main局部動態加法溢位在方法借用前受控失敗() {
        let src = main_named_method_source(
            "const first:int = 1\nconst overflow:int = first+9223372036854775807",
            "overflow,2", "return a+b",
        );
        let plan = checked(&src);
        for _ in 0..2 {
            let out = plan.execute().unwrap();
            assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
            assert!(!out.method_entered && !out.method_exited);
            assert!(out.ticket_returned && out.events.is_empty());
            assert_eq!(out.main_local_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_r02_r05_中途初始化溢位不可清理未進入的後續局部也不能假造返回() {
        let src = main_named_method_source(
            "const first:int = 1\nconst overflow:int = first+9223372036854775807\nconst later:int = 5",
            "later,2", "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_篡改已編譯main整數初始化工作樹必在真執行前拒絕() {
        let src = main_named_method_source(
            "const first:int = 3\nconst second:int = first+4",
            "second,2", "return a+b",
        );
        let mut plan = checked(&src);
        plan.main_init_plans[1] = PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.main_int_locals[1].2 = 99;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_main複合初始化未知或錯型別來源由原_checker_先拒絕() {
        let unknown = main_named_method_source(
            "const amount:int = missing+3", "amount,2", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let bad = main_named_method_source(
            "const amount:int = true+3", "amount,2", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&bad),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_同步self真共享借用及成功返回逐筆由共用進退帳驗證() {
        let out = checked(normal()).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let ticket = match out.exit_events.first() {
            Some(PrivateSharedCallExitEvent::LoanAcquired(ticket)) => *ticket,
            _ => panic!("必須是實際簽發的 self BorrowTicket"),
        };
        assert_eq!(ticket.fact(), out.checked_loan);
        assert_eq!(out.exit_events, [
            PrivateSharedCallExitEvent::LoanAcquired(ticket),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::Returned,
            PrivateSharedCallExitEvent::LoanReturned(ticket),
            PrivateSharedCallExitEvent::FunctionExited { returned: true },
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步self故障須真回滾後歸還同張票據再退出() {
        let out = checked(normal()).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        let ticket = match out.exit_events.first() {
            Some(PrivateSharedCallExitEvent::LoanAcquired(ticket)) => *ticket,
            _ => panic!("應有真自動 self 借用"),
        };
        assert_eq!(out.exit_events, [
            PrivateSharedCallExitEvent::LoanAcquired(ticket),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::UnwindStarted,
            PrivateSharedCallExitEvent::LoanReturned(ticket),
            PrivateSharedCallExitEvent::FunctionExited { returned: false },
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法引數尚未求值成功不得假造self借用或進入退出() {
        let src = main_named_method_source(
            "const amount:int = 1", "3,amount+9223372036854775807",
            "return a+b",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.exit_events.is_empty());
        assert!(!out.method_entered && !out.method_exited && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造同步self歸還票據缺失或重複均由共用帳簿拒絕() {
        let mut out = checked(normal()).execute().unwrap();
        out.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        ));
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(normal()).execute().unwrap();
        let returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let duplicate = out.exit_events[returned].clone();
        out.exit_events.insert(returned, duplicate);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步self刪真返回改退出及竄改票據來源均不可交付() {
        let mut out = checked(normal()).execute().unwrap();
        out.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::Returned
        ));
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(normal()).execute().unwrap();
        out.checked_loan = BorrowFact::new(DeclarationId(0), BorrowKind::SharedRead);
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_共用帳簿拒絕同步self在真返回之前提前還票() {
        let mut out = checked(normal()).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        let returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(returned, forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_共用帳簿拒絕同步self成功退出後才補還真票據() {
        let mut out = checked(normal()).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        out.exit_events.push(forged);
        assert!(!out.has_balanced_self_borrow_trace());
        let failed = checked(normal()).execute_with_fault(true).unwrap();
        assert!(failed.has_balanced_self_borrow_trace());
    }


    fn checked_method_local_source(body: &str, arguments: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.calc({arguments})\n}}\n",
        )
    }

    #[test]
    fn g18_r04_同步self方法int局部以真參數初始化後正確返回() {
        let src = checked_method_local_source(
            "const intermediate:int = a+4\nreturn intermediate", "5,2",
        );
        let plan = checked(&src);
        assert_eq!(plan.method_int_locals.len(), 1);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.has_balanced_self_borrow_trace());
        assert_eq!(result.method_local_slots_after_exit, 0);
        assert_eq!(result.method_local_events.len(), 2);
        assert_eq!(result.exit_events.iter().filter(|e|
            matches!(e, PrivateSharedCallExitEvent::Returned)
        ).count(), 1);
    }

    #[test]
    fn g18_r04_同步self連續兩個int局部依原checker宣告先後傳值及逆序離域() {
        let src = checked_method_local_source(
            "const first:int = a+b\nconst second:int = (first-2)\nreturn second+first",
            "11,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(28));
        let first = plan.method_int_locals[0].0;
        let second = plan.method_int_locals[1].0;
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(first, 15),
            PrivateSelfMethodLocalEvent::Initialized(second, 13),
            PrivateSelfMethodLocalEvent::ScopeEnded(second),
            PrivateSelfMethodLocalEvent::ScopeEnded(first),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_方法局部Copyable同一來源多次讀取不當成Move或重借() {
        let src = checked_method_local_source(
            "const initial:int = a+1\nconst copied:int = initial\nreturn initial+copied",
            "3,2",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.ticket_returned && out.events.is_empty());
        assert_eq!(out.method_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r05_方法第二int局部初始化溢位只離域已初始化第一槽() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let plan = checked(&src);
        let first = plan.method_int_locals[0].0;
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_slots_after_exit, 0);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(first, 11),
            PrivateSelfMethodLocalEvent::ScopeEnded(first),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法返回算式溢位先逆序清真int局部再歸還self票據() {
        let src = checked_method_local_source(
            "const value:int = 9223372036854775807\nreturn value+1",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_events.len(), 2);
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法存在int局部且方法故障注入前不假造初始化() {
        let src = checked_method_local_source(
            "const value:int = a+4\nreturn value", "5,2",
        );
        let plan = checked(&src);
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_local_events.is_empty());
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(plan.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_r04_篡改方法int局部編譯TypedHIR樹及真宣告身分一律執行前拒絕() {
        let src = checked_method_local_source(
            "const value:int = a+4\nreturn value", "5,2",
        );
        let mut plan = checked(&src);
        plan.method_int_locals[0].1 = PrivateScalarReturnExpr::Constant(9);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.method_int_locals[0].0 = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_方法局部未知名稱或bool混入初始化交由原Checker拒絕() {
        let unknown = checked_method_local_source(
            "const value:int = missing+4\nreturn value", "5,2",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let wrong_type = checked_method_local_source(
            "const value:int = true\nreturn value", "5,2",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r05_偽造方法局部逆序離域或重複初始化皆拒絕() {
        let src = checked_method_local_source(
            "const first:int = a\nconst second:int = b\nreturn first+second", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        out.method_local_events.swap(2, 3);
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.method_local_events.insert(1, out.method_local_events[0].clone());
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_真方法局部初始化_return_離域與還票須同一共用時間序() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_self_borrow_trace());
        let id = match &out.method_local_events[0] {
            PrivateSelfMethodLocalEvent::Initialized(id, 5) => *id,
            _ => panic!("方法局部須有真來源值"),
        };
        assert!(matches!(out.exit_events.as_slice(), [
            PrivateSharedCallExitEvent::LoanAcquired(_),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::LocalInitialized(actual),
            PrivateSharedCallExitEvent::Returned,
            PrivateSharedCallExitEvent::LocalScopeEnded(exited),
            PrivateSharedCallExitEvent::LoanReturned(_),
            PrivateSharedCallExitEvent::FunctionExited { returned: true },
        ] if *actual == id && *exited == id));
    }

    #[test]
    fn g18_r05_偽造同步方法在真正_return_之後才初始化局部必拒絕() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let first = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalInitialized(_)
        )).unwrap();
        let forged = out.exit_events.remove(first);
        let returned = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(returned + 1, forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造同步方法退出之後才清理局部雖總數相等仍拒絕() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        out.exit_events.push(forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法局部另一張事件帳漏一筆即使單帳平衡仍禁止交付() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        out.method_local_events.clear();
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.exit_events.retain(|e| !matches!(
            e, PrivateSharedCallExitEvent::LocalInitialized(_)
                | PrivateSharedCallExitEvent::LocalScopeEnded(_)
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_錯誤回滾起點前假清理方法局部不得冒充合法離域() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let cleanup = out.exit_events.remove(at);
        let unwind = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).unwrap();
        out.exit_events.insert(unwind, cleanup);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法回滾後新建int局部即使補做離域亦不可通過() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let extra = DeclarationId(0);
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).unwrap();
        out.exit_events.insert(at + 1,
            PrivateSharedCallExitEvent::LocalInitialized(extra));
        out.exit_events.insert(at + 2,
            PrivateSharedCallExitEvent::LocalScopeEnded(extra));
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_合法方法兩個真局部初始化必對應原_checker_宣告順序() {
        let src = checked_method_local_source(
            "const first:int = a+1\nconst second:int = first+b\nreturn second",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.expected_method_local_ids,
            plan.method_int_locals.iter().map(|(id, _)| *id).collect::<Vec<_>>());
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造成功方法內額外平衡局部即使兩帳一致仍須拒絕() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(at,
            PrivateSharedCallExitEvent::LocalInitialized(fake));
        out.exit_events.insert(at + 2,
            PrivateSharedCallExitEvent::LocalScopeEnded(fake));
        out.method_local_events.insert(1,
            PrivateSelfMethodLocalEvent::Initialized(fake, 7));
        out.method_local_events.insert(2,
            PrivateSelfMethodLocalEvent::ScopeEnded(fake));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步方法即使兩張事件帳同時篡改局部身份也不得冒充checker來源() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        for event in &mut out.method_local_events {
            match event {
                PrivateSelfMethodLocalEvent::Initialized(id, _)
                    | PrivateSelfMethodLocalEvent::Assigned(id, _)
                    | PrivateSelfMethodLocalEvent::ScopeEnded(id) => *id = fake,
            }
        }
        for event in &mut out.exit_events {
            match event {
                PrivateSharedCallExitEvent::LocalInitialized(id)
                    | PrivateSharedCallExitEvent::LocalScopeEnded(id) => *id = fake,
                _ => {}
            }
        }
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.expected_method_local_ids.clear();
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法框架退出後再追加真值假局部不可由main基底冒充() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        out.method_local_events.push(
            PrivateSelfMethodLocalEvent::Initialized(fake, 7));
        out.method_local_events.push(
            PrivateSelfMethodLocalEvent::ScopeEnded(fake));
        out.exit_events.push(
            PrivateSharedCallExitEvent::LocalInitialized(fake));
        out.exit_events.push(
            PrivateSharedCallExitEvent::LocalScopeEnded(fake));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法故障僅允許已進入局部對應原宣告前綴() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.expected_method_local_ids.len(), 2);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 11),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r04_同步self方法var_int_局部可按原checker賦值後真返回() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\nreturn amount", "3,4",
        );
        let plan = checked(&src);
        assert_eq!(plan.method_int_assignments.len(), 1);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_self_borrow_trace() && out.ticket_returned);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 3),
            PrivateSelfMethodLocalEvent::Assigned(plan.method_int_locals[0].0, 7),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
    }

    #[test]
    fn g18_r04_同步self方法連續兩次_var_int_更新沿用最新動態值() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\namount = (amount-2)\nreturn amount",
            "11,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(13));
        assert_eq!(out.expected_method_assignment_ids,
            [plan.method_int_locals[0].0, plan.method_int_locals[0].0]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_同步self方法第二個var局部可讀取第一個已更新值() {
        let src = checked_method_local_source(
            "var first:int = a\nvar second:int = b\nfirst = first+3\nsecond = first+second\nreturn second",
            "4,5",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(out.method_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r05_同步self第二次更新溢位須保留已提交第一次更新及逆序離域() {
        let src = checked_method_local_source(
            "var amount:int = 2\namount = amount+3\namount = amount+9223372036854775807\nreturn amount",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 2),
            PrivateSelfMethodLocalEvent::Assigned(plan.method_int_locals[0].0, 5),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_方法const_int_重新賦值或參數賦值仍原checker靜態拒絕() {
        let const_src = checked_method_local_source(
            "const amount:int = a\namount = b\nreturn amount", "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&const_src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let param_src = checked_method_local_source(
            "var amount:int = a\na = b\nreturn amount", "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&param_src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r04_方法var_int_更新後再宣告尚未證明重排必拒絕() {
        let src = checked_method_local_source(
            "var first:int = a\nfirst = first+b\nconst later:int = first\nreturn later",
            "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r05_竄改方法var_int_賦值計畫或兩張帳中的更新身分須拒絕() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\nreturn amount", "3,4",
        );
        let mut plan = checked(&src);
        plan.method_int_assignments[0].1 = PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut out = checked(&src).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        out.expected_method_assignment_ids = vec![DeclarationId(0)];
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        for event in &mut out.method_local_events {
            if let PrivateSelfMethodLocalEvent::Assigned(id, _) = event {
                *id = DeclarationId(0);
            }
        }
        assert!(!out.has_balanced_self_borrow_trace());
    }


}
