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

#[cfg(test)]
mod p16_6_r03_5_跨函式結構返回多資源責任 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::WholeStructResourcePlan;

    const SOURCE: &str =
        "struct Bundle{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn provide()->Bundle{\nvar local:Bundle = Bundle(title=\"繁體中文\")\nreturn local.move()\n}\nfn main()->int{\nvar received:Bundle = provide()\nreturn 0\n}\n";

    fn plan() -> WholeStructResourcePlan {
        let verified = verified_private_source(SOURCE).expect("先驗證真正 voil 靜態安全");
        let provider = verified.functions.iter().find(|f| f.name == "provide").unwrap();
        let caller = verified.functions.iter().find(|f| f.name == "main").unwrap();
        assert_ne!(provider.body[0].start, caller.body[0].start);
        WholeStructResourcePlan::new(
            DeclarationId(provider.body[0].start),
            DeclarationId(caller.body[0].start)).unwrap()
    }

    #[test]
    fn 真正多資源函式返回只由呼叫端逆序清理() {
        let proof = plan();
        let (value, events) = trace_persistent_struct_return_source(
            SOURCE, "provide", "received", None).unwrap();
        assert_eq!(value.as_deref(), Some("固定🪼"));
        assert_eq!(events, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Returned(1), ResourceEvent::Returned(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(proof.verify_returned_events(&events), Ok(2));
        assert!(proof.verify_moved_events(&events).is_err());
    }

    #[test]
    fn 真正返回前欄位求值失敗不得產生任何返回事件() {
        let proof = plan();
        for (field, count) in [("title", 1), ("id", 2)] {
            let (value, events) = trace_persistent_struct_return_source(
                SOURCE, "provide", "received", Some(field)).unwrap();
            assert!(value.is_none());
            assert_eq!(proof.verify_failed_build_events(&events), Ok(count));
            assert!(events.iter().all(|event| !matches!(
                event, ResourceEvent::Moved(_) | ResourceEvent::Returned(_)
            )));
        }
    }

    #[test]
    fn 禁止在跨函式返回資源橋接前隱含複製結構() {
        let illegal = SOURCE.replace("return local.move()", "return local");
        let static_error = inspect(&illegal).unwrap_err();
        let trace_error = trace_persistent_struct_return_source(
            &illegal, "provide", "received", None).unwrap_err();
        assert_eq!(static_error, trace_error);
    }
}

#[cfg(test)]
mod p16_6_r03_8_真正欄位覆寫與失敗回滾對帳 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::FieldOverwritePlan;

    const SOURCE: &str =
        "struct Holder{\nvar title:str = \"舊值\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar owner:Holder = Holder()\nowner.title = \"新值🪼\"\nreturn 0\n}\n";

    #[test]
    fn 真正_voil_覆寫後原值清理及失敗暫存回滾均核對同一宣告() {
        let verified = verified_private_source(SOURCE).unwrap();
        let main = verified.functions.iter().find(|f|f.name == "main").unwrap();
        let [declare, write, _] = main.body.as_slice() else { panic!("只接受受限三語句"); };
        assert!(matches!(write.kind, StmtKind::FieldAssign { .. }));
        let owner = DeclarationId(declare.start);
        let plan = FieldOverwritePlan::new(owner, 2, Some(0)).unwrap();
        let (updated, success) = trace_persistent_struct_field_write_source(SOURCE, false).unwrap();
        assert_eq!(updated, "新值🪼");
        assert_eq!(success, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(1),
            ResourceEvent::Released(3), ResourceEvent::Released(2),
        ]);
        assert_eq!(plan.verify_actual_events(&success,false), Ok(3));
        let (preserved, failed) = trace_persistent_struct_field_write_source(SOURCE, true).unwrap();
        assert_eq!(preserved, "舊值");
        assert_eq!(failed, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(plan.verify_actual_events(&failed,true), Ok(3));
        assert!(plan.verify_actual_events(&success,true).is_err());
        assert!(plan.verify_actual_events(&failed,false).is_err());
    }

    #[test]
    fn 未初始化的_var_str_第一次寫入不可清理不存在的舊欄位() {
        let source = SOURCE.replace("var title:str = \"舊值\"", "var title:str");
        let checked = verified_private_source(&source).unwrap();
        let main = checked.functions.iter().find(|f|f.name == "main").unwrap();
        let plan=FieldOverwritePlan::new(DeclarationId(main.body[0].start),1,None).unwrap();
        let (value,events)=trace_persistent_struct_field_write_source(&source,false).unwrap();
        assert_eq!(value,"新值🪼");
        assert_eq!(plan.verify_actual_events(&events,false),Ok(2));
    }

    #[test]
    fn 靜態禁止_const_欄位覆寫不能繞過安全_ir() {
        let invalid=SOURCE.replace("owner.title = \"新值🪼\"",
            "owner.id = \"不合法\"");
        let original=inspect(&invalid).unwrap_err();
        let runtime=trace_persistent_struct_field_write_source(&invalid,false).unwrap_err();
        assert_eq!(original,runtime);
    }
}

#[cfg(test)]
mod p16_6_r03_9_混合純值與資源責任帳簿實際來源 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::MixedResourcePlan;

    const MIXED: &str =
        "struct Mixed{\nvar num:int = 3\nvar text:str = \"舊值\"\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Mixed = Mixed(text=\"繁體中文🪼\",num=7)\nvar target:Mixed = original.move()\nreturn 0\n}\n";
    const PURE: &str =
        "struct Plain{\nvar num:int = 3\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Plain = Plain(num=9)\nvar target:Plain = original\nreturn 0\n}\n";

    fn proven(src: &str, str_count: usize) -> MixedResourcePlan {
        let checked=verified_private_source(src).expect("真實來源須先經私有安全檢查");
        let main=checked.functions.iter().find(|f|f.name=="main").unwrap();
        let [declare,assign,_]=main.body.as_slice() else { panic!("限制三語句"); };
        MixedResourcePlan::new(
            DeclarationId(declare.start),DeclarationId(assign.start),str_count).unwrap()
    }

    #[test]
    fn 真正混合資源整筆移動只登記_str_並精確清理() {
        let proof=proven(MIXED,1);
        let (value,events)=trace_mixed_struct_source(MIXED,None).unwrap();
        assert_eq!(value,Some(7));
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Moved(1),ResourceEvent::Released(1),
        ]);
        assert_eq!(proof.verify_completed(&events,true),Ok(1));
        assert!(proof.verify_completed(&events,false).is_err());
    }

    #[test]
    fn 真正全_copyable_結構複製沒有_任何_資源事件() {
        let proof=proven(PURE,0);
        let (value,events)=trace_mixed_struct_source(PURE,None).unwrap();
        assert_eq!(value,Some(9));
        assert!(events.is_empty());
        assert_eq!(proof.verify_completed(&events,false),Ok(0));
    }

    #[test]
    fn 求值至_int_或_bool_才失敗只回滾先前真的建立之_str() {
        let proof=proven(MIXED,1);
        for failure in ["num","flag"] {
            let (value,events)=trace_mixed_struct_source(MIXED,Some(failure)).unwrap();
            assert!(value.is_none());
            assert_eq!(events,vec![
                ResourceEvent::Created(1),ResourceEvent::Released(1),
            ]);
            assert_eq!(proof.verify_failed_build(&events,1),Ok(1));
        }
    }

    #[test]
    fn 混合結構不能在進入真實事件模擬前隱含複製_str() {
        let illegal=MIXED.replace("original.move()","original");
        let static_error=inspect(&illegal).unwrap_err();
        let runtime_error=trace_mixed_struct_source(&illegal,None).unwrap_err();
        assert_eq!(static_error,runtime_error);
    }
}

#[cfg(test)]
mod p16_6_r03_10_單層布林分支與作用域資源清理 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as E;
    use crate::verified_resource_ir::ScopedResourcePlan;

    const SRC: &str =
        "struct Holder{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar outer:Holder = Holder(text=\"外層\")\nif(true){\nvar inner:Holder = Holder(text=\"內層\")\n}\nreturn 0\n}\n";

    fn proof(src:&str, with_inner:bool)->ScopedResourcePlan {
        let verified=verified_private_source(src).unwrap();
        let main=verified.functions.iter().find(|f|f.name=="main").unwrap();
        let [outside,branch,_]=main.body.as_slice() else {panic!("須為三語句");};
        let selected=if with_inner {
            let StmtKind::If {yes,..}=&branch.kind else {panic!("須為 if");};
            Some((DeclarationId(yes[0].start),1))
        } else {None};
        ScopedResourcePlan::new(DeclarationId(outside.start),1,selected).unwrap()
    }

    #[test]
    fn 真正_if_true_內層釋放先於外層_資源事件和宣告身分一致() {
        let events=trace_scope_struct_source(SRC,None).unwrap();
        assert_eq!(events,vec![
            E::Created(1),E::Created(2),E::Released(2),E::Released(1),
        ]);
        assert_eq!(proof(SRC,true).verify_actual_events(&events),Ok(2));
    }

    #[test]
    fn 真正_if_false_沒有_else_只由外層負責清理() {
        let src=SRC.replace("if(true)","if(false)");
        let events=trace_scope_struct_source(&src,None).unwrap();
        assert_eq!(events,vec![E::Created(1),E::Released(1)]);
        assert_eq!(proof(&src,false).verify_actual_events(&events),Ok(1));
        assert!(proof(&src,true).verify_actual_events(&events).is_err());
    }

    #[test]
    fn 真正分支內提前_return_及欄位建立失敗依然先清理內層() {
        let early=SRC.replace(
            "var inner:Holder = Holder(text=\"內層\")\n}",
            "var inner:Holder = Holder(text=\"內層\")\nreturn 0\n}");
        for (source,failure) in [(SRC,None),(SRC,Some("text")),(early.as_str(),None)] {
            let events=trace_scope_struct_source(source,failure).unwrap();
            assert_eq!(proof(source,true).verify_actual_events(&events),Ok(2));
        }
    }

    #[test]
    fn 未知條件不能假裝已執行內層資源路徑() {
        let illegal=SRC.replace("if(true)","if(1==1)");
        assert!(trace_scope_struct_source(&illegal,None).is_err());
    }
}
