// P16.6-R03：以單一已驗證來源，核對真實 UTF-8 事件與抽象所有權／資源帳簿。
// 此處僅覆蓋已核准的直線 str 建立、具名整筆移交與最終清理，非一般 P17 執行器。

#[cfg(test)]
mod p16_6_r03_來源身分與資源帳簿交叉驗證 {
    use super::*;
    use crate::resource_borrow_pipeline::SafetyPipeline;
    use crate::owned_str_runtime::ResourceEvent;

    #[test]
    fn 來源起點與真實緩衝區事件都對應唯一資源持有者() {
        let code = "fn main()->int{\nvar owner:str = \"繁體中文 🌊\"\nvar target:str = owner.move()\nreturn 0\n}\n";
        let checked = verified_private_source(code).expect("先經真正來源安全驗證");
        let main = checked.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [first, second, _return] = main.body.as_slice() else {
            panic!("只接受已驗證三個語句的直線結構");
        };
        let (source_name, target_name) = match (&first.kind, &second.kind) {
            (
                StmtKind::Declare { name: source_name, .. },
                StmtKind::Declare { name: target_name, init: Some(Term {
                    kind: TermKind::Move(moved), ..
                }), .. },
            ) if source_name == moved => (source_name, target_name),
            _ => panic!("未取得真正來源中的具名整筆資源移交"),
        };
        assert_eq!(source_name, "owner");
        assert_eq!(target_name, "target");
        assert_ne!(first.start, second.start);
        let source_id = DeclarationId(first.start);
        let target_id = DeclarationId(second.start);
        let events = trace_named_string_move_source(code)
            .expect("UTF-8 字串實際生命週期證據");
        let [ResourceEvent::Created(resource), ResourceEvent::Moved(moved),
             ResourceEvent::Released(released)] = events.as_slice() else {
            panic!("只容許一次建立、整筆移動、最終一次釋放");
        };
        assert_eq!(resource, moved, "移動不能重新建立另一份資源");
        assert_eq!(resource, released, "必須釋放唯一已移交持有者");

        let mut pipeline = SafetyPipeline::new();
        pipeline.declare(source_id, CopyPermission::Forbidden).unwrap();
        pipeline.register_resource(source_id, *resource).unwrap();
        pipeline.move_into(source_id, target_id).unwrap();
        assert!(pipeline.read(source_id).is_err(), "移動後原來源已失效");
        assert!(pipeline.read(target_id).is_ok());
        assert_eq!(pipeline.outstanding(), 1);
        assert_eq!(pipeline.record_cleanup(target_id).unwrap(), 1);
        assert_eq!(pipeline.outstanding(), 0);
        pipeline.audit_finished().expect("雙模型均無資源漏放");
        assert!(pipeline.record_cleanup(target_id).is_err(),
            "同一持有者不能清理兩次");
    }

    #[test]
    fn 不合法隱含複製不能在資源模擬器中繞過已驗證門檻() {
        let illegal = "fn main()->int{\nvar owner:str = \"中文\"\nvar target:str = owner\nreturn 0\n}\n";
        let original = inspect(illegal).unwrap_err();
        let private = verified_private_source(illegal).err().unwrap();
        let runtime = trace_named_string_move_source(illegal).unwrap_err();
        assert_eq!(original, private);
        assert_eq!(private, runtime);
        assert_eq!(runtime.kind, DiagnosticKind::Static);
    }
}
