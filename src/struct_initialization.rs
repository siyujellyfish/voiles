//! VB16-01-A 與 VA16-02-A：共用結構求值計畫與真實 UTF-8 資源。
//! 具名引數按來源順序，未覆蓋預設值按宣告順序；任何中途失敗
//! 都依完成初始化的相反順序釋放已取得 String 緩衝區。
//! 這是私有執行期建構子集，不是已完成的 P17 直譯器。
#![allow(dead_code)]
use crate::owned_str_runtime::{OwnedStrRuntime,ResourceEvent,StrRuntimeError};
use crate::struct_eval_plan::{self,EvalStep,FieldInfo,Origin,PlanError};

#[derive(Clone,Copy)]
pub(crate) struct StringField<'a> {
    pub name: &'a str,
    pub default: Option<&'a str>,
    pub immutable: bool,
}
#[derive(Clone,Copy)]
pub(crate) struct NamedString<'a> {
    pub field: &'a str,
    pub value: &'a str,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) enum BuildStatus {
    Complete,
    PartiallyInitialized(Vec<String>),
    Rejected(PlanError),
    EvaluationFailed(String),
    ResourceError(StrRuntimeError),
}
#[derive(Debug)]
pub(crate) struct BuildTrace {
    pub status: BuildStatus,
    pub evaluated: Vec<EvalStep>,
    pub lifetime_values: Vec<(String,String)>,
    pub resources: Vec<ResourceEvent>,
}

/// 失敗點是用於驗證可重現清理證據的模擬錯誤，而非新語言語法。
pub(crate) fn build_string_struct(
    fields: &[StringField<'_>],
    named: &[NamedString<'_>],
    fail_after_field: Option<&str>,
) -> BuildTrace {
    let schema: Vec<_> = fields.iter().map(|f| FieldInfo {
        name:f.name, has_default:f.default.is_some(), immutable:f.immutable
    }).collect();
    let names: Vec<_> = named.iter().map(|a|a.field).collect();
    let plan = match struct_eval_plan::plan(&schema,&names) {
        Ok(p)=>p,
        Err(error)=>return BuildTrace {
            status:BuildStatus::Rejected(error),
            evaluated:vec![],lifetime_values:vec![],resources:vec![],
        },
    };
    let mut rt=OwnedStrRuntime::new();
    rt.enter_scope();
    let mut evaluated=Vec::new();
    let mut status=if plan.missing_mutable_fields.is_empty() {
        BuildStatus::Complete
    } else {
        BuildStatus::PartiallyInitialized(plan.missing_mutable_fields.clone())
    };
    for step in &plan.steps {
        let value=match step.origin {
            Origin::Named=>named.iter().find(|n|n.field==step.field)
                .map(|n|n.value).expect("具名欄位已預檢"),
            Origin::Default=>fields.iter().find(|f|f.name==step.field)
                .and_then(|f|f.default).expect("未覆蓋預設已預檢"),
        };
        match rt.declare(&step.field,true,value) {
            Ok(())=>evaluated.push(step.clone()),
            Err(err)=>{status=BuildStatus::ResourceError(err);break;}
        }
        if fail_after_field==Some(step.field.as_str()) {
            status=BuildStatus::EvaluationFailed(step.field.clone());
            break;
        }
    }
    let lifetime_values=evaluated.iter().map(|step|{
        (step.field.clone(),rt.read(&step.field).expect("資源應存活").to_owned())
    }).collect();
    rt.leave_scope().expect("私有建構作用域應可離開");
    let resources=rt.finish().expect("所有已成功取得資源須已反序清理");
    BuildTrace {status,evaluated,lifetime_values,resources}
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::struct_eval_plan::Origin::{Named,Default};
    fn created(events:&[ResourceEvent])->Vec<u64>{
        events.iter().filter_map(|e|match e{ResourceEvent::Created(n)=>Some(*n),_=>None}).collect()
    }
    fn released(events:&[ResourceEvent])->Vec<u64>{
        events.iter().filter_map(|e|match e{ResourceEvent::Released(n)=>Some(*n),_=>None}).collect()
    }
    fn fields()->[StringField<'static>;4]{
        [
            StringField{name:"a",default:Some("甲預設"),immutable:false},
            StringField{name:"b",default:Some("乙預設"),immutable:false},
            StringField{name:"c",default:Some("丙預設"),immutable:false},
            StringField{name:"id",default:Some("固定"),immutable:true},
        ]
    }
    #[test]
    fn va16_結構_var_str_欄位獨立常值覆寫先釋放舊值() {
        let mut rt=OwnedStrRuntime::new();
        rt.enter_scope();
        rt.declare("text",true,"old").unwrap();
        rt.assign("text","new").unwrap();
        assert_eq!(rt.read("text"),Ok("new"));
        assert_eq!(released(rt.events()),vec![1]);
        rt.leave_scope().unwrap();
        let events=rt.finish().unwrap();
        assert_eq!(created(&events),vec![1,2]);
        assert_eq!(released(&events),vec![1,2]);
    }

    #[test]
    fn va16_結構欄位求值失敗時舊值可用且暫存新資源被清理() {
        let mut rt=OwnedStrRuntime::new();
        rt.enter_scope();
        rt.declare("text",true,"old").unwrap();
        assert_eq!(rt.assign_checked("text","not committed",true),
            Err(StrRuntimeError::EvaluationFailed));
        assert_eq!(rt.read("text"),Ok("old"));
        assert_eq!(released(rt.events()),vec![2]);
        rt.leave_scope().unwrap();
        let events=rt.finish().unwrap();
        assert_eq!(created(&events),vec![1,2]);
        assert_eq!(released(&events),vec![2,1]);
    }

    #[test]
    fn vb16_具名與預設欄位真實資源依核准順序求值及反序清理(){
        let out=build_string_struct(&fields(),&[
            NamedString{field:"c",value:"丙"},
            NamedString{field:"a",value:"甲"},
        ],None);
        assert_eq!(out.status,BuildStatus::Complete);
        assert_eq!(out.evaluated,vec![
            EvalStep{field:"c".into(),origin:Named},
            EvalStep{field:"a".into(),origin:Named},
            EvalStep{field:"b".into(),origin:Default},
            EvalStep{field:"id".into(),origin:Default},
        ]);
        assert_eq!(out.lifetime_values,vec![
            ("c".into(),"丙".into()),("a".into(),"甲".into()),
            ("b".into(),"乙預設".into()),("id".into(),"固定".into())]);
        assert_eq!(created(&out.resources),vec![1,2,3,4]);
        assert_eq!(released(&out.resources),vec![4,3,2,1]);
    }
    #[test]
    fn vb16_被具名覆蓋的預設值不執行亦不產生資源(){
        let f=[StringField{name:"x",default:Some("不應執行"),immutable:false}];
        let o=build_string_struct(&f,&[NamedString{field:"x",value:"實際"}],None);
        assert_eq!(o.status,BuildStatus::Complete);
        assert_eq!(o.lifetime_values,vec![("x".into(),"實際".into())]);
        assert_eq!(created(&o.resources),vec![1]);
        assert_eq!(released(&o.resources),vec![1]);
    }
    #[test]
    fn vb16_具名引數完成後失敗僅清理成功取得的資源(){
        let o=build_string_struct(&fields(),&[
            NamedString{field:"c",value:"丙"},NamedString{field:"a",value:"甲"}
        ],Some("a"));
        assert_eq!(o.status,BuildStatus::EvaluationFailed("a".into()));
        assert_eq!(created(&o.resources),vec![1,2]);
        assert_eq!(released(&o.resources),vec![2,1]);
    }
    #[test]
    fn vb16_預設求值中途失敗連先前具名資源一併逆序回收(){
        let o=build_string_struct(&fields(),&[
            NamedString{field:"c",value:"丙"}
        ],Some("b"));
        assert_eq!(o.status,BuildStatus::EvaluationFailed("b".into()));
        assert_eq!(o.evaluated.iter().map(|e|e.field.as_str()).collect::<Vec<_>>(),
            vec!["c","a","b"]);
        assert_eq!(created(&o.resources),vec![1,2,3]);
        assert_eq!(released(&o.resources),vec![3,2,1]);
    }
    #[test]
    fn vb16_未初始化_var_不造假資源_const_預設仍正常(){
        let f=[
            StringField{name:"pending",default:None,immutable:false},
            StringField{name:"id",default:Some("已完成"),immutable:true},
        ];
        let o=build_string_struct(&f,&[],None);
        assert_eq!(o.status,BuildStatus::PartiallyInitialized(vec!["pending".into()]));
        assert_eq!(created(&o.resources),vec![1]);
        assert_eq!(released(&o.resources),vec![1]);
    }
    #[test]
    fn vb16_非法具名引數於配置任何資源前拒絕(){
        let f=fields();
        let o=build_string_struct(&f,&[
            NamedString{field:"a",value:"甲"},NamedString{field:"a",value:"乙"},
        ],None);
        assert!(matches!(o.status,BuildStatus::Rejected(PlanError::DuplicateField(_))));
        assert!(o.resources.is_empty());
        let o=build_string_struct(&f,&[NamedString{field:"no",value:"錯誤"}],None);
        assert!(matches!(o.status,BuildStatus::Rejected(PlanError::UnknownField(_))));
        assert!(o.resources.is_empty());
    }
    #[test]
    fn vb16_缺失_const_預設即使提供具名值亦依_f01_拒絕(){
        let f=[StringField{name:"id",default:None,immutable:true}];
        let o=build_string_struct(&f,&[NamedString{field:"id",value:"來源"}],None);
        assert_eq!(o.status,BuildStatus::Rejected(
            PlanError::MissingConstDefault("id".into())));
        assert!(o.resources.is_empty());
    }
}
