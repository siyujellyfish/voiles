// P17-F 真 Checker／真欄位配置／覆蓋預設與 rollback 正反測試。
#[cfg(test)]
mod p17_verified_constructor_tests {
    use super::*;
    use crate::mixed_struct_runtime::MixedError;
    use crate::owned_str_runtime::ResourceEvent::*;

    const NAMED: &str = "struct Bundle{\nvar first:str = \"第一預設\"\nvar middle:int = 3\nconst flag:bool = true\nvar last:str = \"最後預設\"\n}\nfn main()->int{\nvar instance:Bundle = Bundle(last=\"具名最後\",first=\"具名第一\",middle=7)\nreturn 0\n}\n";

    #[test]
    fn p17_f_真私有_checker_具名來源次序先於未覆蓋的預設() {
        let checked = PreparedPrivateConstructor::prepare(NAMED).unwrap();
        let actual = checked.execute().unwrap();
        assert_eq!(actual.result, Ok(0));
        assert_eq!(actual.actual_events, [Created(1), Created(2), Released(2), Released(1)]);
        assert_eq!(actual.observed, [
            ("first".into(), "str:具名第一".into()),
            ("middle".into(), "int:7".into()),
            ("flag".into(), "bool:true".into()),
            ("last".into(), "str:具名最後".into()),
        ]);
    }

    #[test]
    fn p17_f_未覆蓋預設按原始欄位宣告順序建立且逆序清理() {
        let code = "struct Bundle{\nvar a:str = \"預設甲\"\nvar b:str = \"預設乙\"\n}\nfn main()->int{\nvar item:Bundle = Bundle(b=\"具名乙\")\nreturn 5\n}\n";
        let actual = PreparedPrivateConstructor::prepare(code).unwrap().execute().unwrap();
        assert_eq!(actual.result, Ok(5));
        assert_eq!(actual.observed, [
            ("a".into(), "str:預設甲".into()),
            ("b".into(), "str:具名乙".into()),
        ]);
        assert_eq!(actual.actual_events, [Created(1), Created(2), Released(2), Released(1)]);
    }

    #[test]
    fn p17_f_被具名覆蓋的字串預設不得配置額外資源() {
        let code = "struct Sample{\nvar title:str = \"不能求值的預設\"\n}\nfn main()->int{\nvar item:Sample = Sample(title=\"真正具名值\")\nreturn 0\n}\n";
        let actual = PreparedPrivateConstructor::prepare(code).unwrap().execute().unwrap();
        assert_eq!(actual.observed, [("title".into(), "str:真正具名值".into())]);
        assert_eq!(actual.actual_events, [Created(1), Released(1)]);
    }

    #[test]
    fn p17_f_部分求值失敗時所有真實資源依逆序清理且不提交() {
        let code = "struct Pair{\nvar a:str = \"甲\"\nvar b:str = \"乙\"\n}\nfn main()->int{\nvar item:Pair = Pair()\nreturn 0\n}\n";
        let plan = PreparedPrivateConstructor::prepare(code).unwrap();
        let actual = plan.execute_with_fault(Some("b")).unwrap();
        assert_eq!(actual.result, Err(MixedError::EvaluationFailed("b".into())));
        assert!(actual.observed.is_empty());
        assert_eq!(actual.actual_events, [Created(1), Created(2), Released(2), Released(1)]);
    }

    #[test]
    fn p17_f_重複與未知具名欄位在真_checker_階段拒絕() {
        for code in [
            NAMED.replace("middle=7", "middle=7,middle=8"),
            NAMED.replace("middle=7", "unknown=7"),
        ] {
            assert!(matches!(PreparedPrivateConstructor::prepare(&code),
                Err(ConstructorExecutionError::Static(_))));
        }
    }

    #[test]
    fn p17_f_改寫已驗證原始位元組不能產生任何動態資源事件() {
        let mut plan = PreparedPrivateConstructor::prepare(NAMED).unwrap();
        plan.verified.source.bytes[0] = b'x';
        assert!(matches!(plan.execute(), Err(ConstructorExecutionError::ChangedSource)));
    }

    #[test]
    fn p17_f_已通過_checker_但未接通的結構方法仍保守拒絕() {
        let code = "struct Counter{\nvar n:int = 1\nfn get()->int{\nreturn self.n\n}\n}\nfn main()->int{\nvar v:Counter = Counter()\nreturn 0\n}\n";
        assert!(matches!(PreparedPrivateConstructor::prepare(code),
            Err(ConstructorExecutionError::Unsupported(_))));
    }
}
