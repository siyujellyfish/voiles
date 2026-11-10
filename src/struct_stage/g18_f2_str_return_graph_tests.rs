// G18／F2 原 Checker -> 真 LIFO 工作圖 -> OwnedUtf8 Returned／Accepted 五項正反。
#[cfg(test)]
mod g18_f2_str_return_graph_tests {
    use super::*;
    fn run(source: &str) -> PrivateScalarCallOutcome {
        let plan = PreparedPrivateScalarCall::prepare(source).unwrap();
        let out = plan.execute().unwrap();
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.parameter_slots_after_exit, 0);
        out
    }
    #[test]
    fn f2_str_return_來源直接utf8字面真正跨函式接收與唯一釋放() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn make()->str{\nreturn \"繁體中文🪼\"\n}\nfn main()->int{\nvar result:str = make()\nreturn 0\n}\n";
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        let returned = out.events.iter().position(|e| matches!(e, PrivateScalarCallEvent::StrReturned(1))).unwrap();
        let accepted = out.events.iter().position(|e| matches!(e, PrivateScalarCallEvent::StrReturnAccepted(_, 1))).unwrap();
        let released = out.events.iter().position(|e| matches!(e, PrivateScalarCallEvent::OwnedResource(R::Released(1)))).unwrap();
        assert!(returned < accepted && accepted < released);
    }
    #[test]
    fn f2_str_return_原checker局部move後const接收無重建且同resourceid() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn provide()->str{\nvar payload:str = \"唯一🪼\"\nreturn payload.move()\n}\nfn main()->int{\nconst received:str = provide()\nreturn 0\n}\n";
        let out = run(src);
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.resource_events, [R::Created(1), R::Returned(1), R::Released(1)]);
        assert_eq!(out.events.iter().filter(|e| matches!(e, PrivateScalarCallEvent::StrReturnAccepted(_, 1))).count(), 1);
    }
    #[test]
    fn f2_str_return_同一主函式兩次真正接收必區分資源id與逆序清理() {
        use crate::owned_str_runtime::ResourceEvent as R;
        let src = "fn make()->str{\nreturn \"字串\"\n}\nfn main()->int{\nconst first:str = make()\nvar second:str = make()\nreturn 1\n}\n";
        let out = run(src);
        assert_eq!(out.result, Ok(1));
        assert_eq!(out.resource_events, [
            R::Created(1), R::Returned(1), R::Created(2), R::Returned(2),
            R::Released(2), R::Released(1),
        ]);
        assert_eq!(out.events.iter().filter_map(|e| match e {
            PrivateScalarCallEvent::StrReturnAccepted(_, id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>(), [1, 2]);
    }
    #[test]
    fn f2_str_return_原checker非法資源拷貝與借用逃逸一律拒絕() {
        for src in [
            "fn provide()->str{\nvar payload:str = \"不可拷貝\"\nreturn payload\n}\nfn main()->int{\nvar result:str = provide()\nreturn 0\n}\n",
            "fn provide()->str{\nvar payload:str = \"不可逃逸\"\nreturn payload.borrow()\n}\nfn main()->int{\nvar result:str = provide()\nreturn 0\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))));
        }
    }
    #[test]
    fn f2_str_return_竄改真正接收ResourceId即使釋放筆數平衡亦拒絕() {
        let src = "fn make()->str{\nreturn \"🪼\"\n}\nfn main()->int{\nconst received:str = make()\nreturn 0\n}\n";
        let plan = PreparedPrivateScalarCall::prepare(src).unwrap();
        let mut out = plan.execute().unwrap();
        let index = out.events.iter().position(|e| matches!(
            e, PrivateScalarCallEvent::StrReturnAccepted(_, _)
        )).unwrap();
        if let PrivateScalarCallEvent::StrReturnAccepted(id, _) = out.events[index] {
            out.events[index] = PrivateScalarCallEvent::StrReturnAccepted(id, 99999);
        }
        assert!(!out.has_balanced_frame_trace());
        assert!(matches!(plan.audit_graph_outcome(out),
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
    }
}
