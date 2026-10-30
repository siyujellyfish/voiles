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

#[cfg(test)]
mod p16_6_r03_3_函式返回跨來源宣告身分稽核 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::NamedResourceReturn;

    #[test]
    fn 真實具名函式返回依來源及接收宣告身分核對同一資源() {
        let src = "fn create()->str{\nvar payload:str = \"繁體中文🪼\"\nreturn payload.move()\n}\nfn main()->int{\nvar received:str = create()\nreturn 0\n}\n";
        let verified = verified_private_source(src).unwrap();
        let provider = verified.functions.iter().find(|f| f.name == "create").unwrap();
        let main = verified.functions.iter().find(|f| f.name == "main").unwrap();
        let source_id = DeclarationId(provider.body[0].start);
        let target_id = DeclarationId(main.body[0].start);
        assert_ne!(source_id, target_id);
        assert_eq!(&src[source_id.0..source_id.0 + 3], "var");
        assert_eq!(&src[target_id.0..target_id.0 + 3], "var");

        let (value, events) = trace_string_return_source(src, "create", "received").unwrap();
        assert_eq!(value, "繁體中文🪼");
        assert_eq!(events, vec![
            ResourceEvent::Created(1),
            ResourceEvent::Returned(1),
            ResourceEvent::Released(1),
        ]);
        let proof = NamedResourceReturn::new(source_id, target_id).unwrap();
        assert_eq!(proof.verify_actual_events(&events), Ok(1));
    }

    #[test]
    fn 不合法隱含複製在函式返回資源橋接之前就被靜態拒絕() {
        let illegal = "fn create()->str{\nvar payload:str = \"來源\"\nreturn payload\n}\nfn main()->int{\nvar received:str = create()\nreturn 0\n}\n";
        let static_error = inspect(illegal).unwrap_err();
        let trace_error = trace_string_return_source(illegal, "create", "received").unwrap_err();
        assert_eq!(static_error, trace_error, "IR 不可取代或跳過真正靜態安全預檢");
    }
}

#[cfg(test)]
mod p16_6_r03_4_結構整筆移交與失敗回滾資源責任 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::WholeStructResourcePlan;

    const SOURCE: &str =
        "struct Message{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar original:Message = Message(title=\"繁體中文\")\nvar transferred:Message = original.move()\nreturn 0\n}\n";

    #[test]
    fn 真實來源多個欄位轉交的宣告身分與雙資源帳簿相符() {
        let verified = verified_private_source(SOURCE).expect("來源先完成安全檢查");
        let main = verified.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [source, target, _] = main.body.as_slice() else { panic!("只接受三語句"); };
        let plan = WholeStructResourcePlan::new(
            DeclarationId(source.start), DeclarationId(target.start)).unwrap();
        let (retained, events) = trace_persistent_struct_move_source(SOURCE, None).unwrap();
        assert_eq!(retained.as_deref(), Some("固定🪼"));
        assert_eq!(events, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Moved(1), ResourceEvent::Moved(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(plan.verify_moved_events(&events), Ok(2));
    }

    #[test]
    fn 兩種已核准失敗切點只能逆序釋放已建立資源() {
        let verified = verified_private_source(SOURCE).unwrap();
        let main = verified.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [source, target, _] = main.body.as_slice() else { panic!("只接受三語句"); };
        let plan = WholeStructResourcePlan::new(
            DeclarationId(source.start), DeclarationId(target.start)).unwrap();
        for (field, count) in [("title", 1), ("id", 2)] {
            let (value, events) = trace_persistent_struct_move_source(SOURCE, Some(field)).unwrap();
            assert_eq!(value, None);
            assert_eq!(plan.verify_failed_build_events(&events), Ok(count));
            assert!(events.iter().all(|e| !matches!(
                e, ResourceEvent::Moved(_) | ResourceEvent::Returned(_)
            )));
        }
    }

    #[test]
    fn 含有非_copyable_欄位的結構不能偷換成隱含複製後交給資源_ir() {
        let illegal = SOURCE.replace("original.move()", "original");
        let static_error = inspect(&illegal).unwrap_err();
        let trace_error = trace_persistent_struct_move_source(&illegal, None).unwrap_err();
        assert_eq!(static_error, trace_error);
    }
}
