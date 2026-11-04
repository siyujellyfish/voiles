// P17-C：同一份真私有來源及真 Rust String 執行事件的正反測試。

#[cfg(test)]
mod p17_checked_private_runtime_tests {
    use super::*;
    const INPUT: &str = "fn main()->int{\nvar owner:str = \"繁體中文🪼\"\nvar target:str = owner.move()\nreturn 0\n}\n";

    #[test]
    fn p17_c_真私有_checker_來源的字串實際建立移動逆向清理() {
        let result = execute_checked_single_str_move(INPUT).unwrap();
        use crate::owned_str_runtime::ResourceEvent::*;
        assert_eq!(result.result, 0);
        assert_eq!(result.actual_events, vec![Created(1), Moved(1), Released(1)]);
    }

    #[test]
    fn p17_c_真實字串跳脫只採用既有_lexer_已解碼結果() {
        let code = INPUT.replace("繁體中文🪼", "繁體\\n中文");
        let result = execute_checked_single_str_move(&code).unwrap();
        assert!(crate::owned_str_runtime::event_trace_balanced(&result.actual_events));
    }

    #[test]
    fn p17_c_非_copyable_來源的隱含複製被正式_checker_拒絕() {
        let code = INPUT.replace("owner.move()", "owner");
        assert!(matches!(execute_checked_single_str_move(&code),
            Err(PrivateStrExecutionError::Static(_))));
    }

    #[test]
    fn p17_c_超出已證明範圍的合法語法在資源建立前保守阻擋() {
        assert!(matches!(execute_checked_single_str_move("fn main()->int{\nreturn 0\n}\n"),
            Err(PrivateStrExecutionError::Unsupported(_))));
    }

    #[test]
    fn p17_c_竄改真_checker_具名移動證據不得執行() {
        let mut checked = verify_source_with_ability(
            INPUT, crate::ownership_policy::approved_leaf_copy_permission,
        ).unwrap();
        checked.plan.declarations.direct_moves.clear();
        assert!(matches!(checked_single_move(&checked),
            Err(PrivateStrExecutionError::Unsupported(_))));
    }
}
