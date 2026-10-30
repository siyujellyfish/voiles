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
/// P16 私有持續存活的結構值模型。與單次 BuildTrace 不同，
/// 建構成功後欄位真正持有 OwnedUtf8，直到持有者移動或退出作用域。
/// str 為非 Copyable，因此不提供建立第二所有者的隱含複製 API。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StructRuntimeError {
    InvalidPlan(PlanError),
    DuplicateBinding,
    MissingBinding,
    MovedBinding,
    UninitializedField,
    UnknownField,
    ImmutableField,
    ImmutableOwner,
    NoInnerScope,
    EvaluationFailed(String),
    UnknownFailurePoint,
    NonCopyableStruct,
    Resource(StrRuntimeError),
}

#[derive(Debug)]
struct StructResource {
    name: String,
    value: crate::owned_str_runtime::OwnedUtf8,
}

#[derive(Debug)]
struct StructOwnedValue {
    // 順序是 VB16-01-A 真正完成初始化的順序，非宣告順序。
    fields: Vec<StructResource>,
    missing: Vec<String>,
    /// 以宣告欄位為準，不能依欄位目前是否已有資源推定可寫。
    field_immutable: Vec<(String,bool)>,
}

#[derive(Debug)]
struct StructOwnerSlot {
    name: String,
    mutable: bool,
    owned: Option<StructOwnedValue>,
}

#[derive(Debug)]
pub(crate) struct ReturnedStringStruct {
    value: StructOwnedValue,
}

#[derive(Debug)]
pub(crate) struct OwnedStructRuntime {
    ledger: OwnedStrRuntime,
    scopes: Vec<Vec<StructOwnerSlot>>,
}

impl OwnedStructRuntime {
    pub(crate) fn new() -> Self {
        Self { ledger: OwnedStrRuntime::new(), scopes: vec![Vec::new()] }
    }

    pub(crate) fn enter_scope(&mut self) { self.scopes.push(Vec::new()); }

    fn current_name_free(&self, name: &str) -> Result<(), StructRuntimeError> {
        if self.scopes.last().expect("根作用域存在").iter()
            .any(|slot| slot.name == name) {
            Err(StructRuntimeError::DuplicateBinding)
        } else { Ok(()) }
    }

    fn location(&self, name: &str) -> Result<(usize, usize), StructRuntimeError> {
        self.scopes.iter().enumerate().rev().find_map(|(scope_index, frame)| {
            frame.iter().position(|slot| slot.name == name)
                .map(|slot_index| (scope_index,slot_index))
        }).ok_or(StructRuntimeError::MissingBinding)
    }

    fn release_value(ledger: &mut OwnedStrRuntime, value: StructOwnedValue) {
        // 失敗、作用域退出與完整實例移動後的清理，
        // 皆遵守「此次成功取得欄位資源」的逆序。
        for field in value.fields.into_iter().rev() {
            ledger.release_detached(field.value);
        }
    }

    fn cleanup_frame(ledger: &mut OwnedStrRuntime, frame: Vec<StructOwnerSlot>) {
        // 每個持有者自末次成功綁定向前清理；已移動者不再清理。
        for slot in frame.into_iter().rev() {
            if let Some(value) = slot.owned {
                Self::release_value(ledger, value);
            }
        }
    }

    pub(crate) fn construct(&mut self, name: &str,
        fields: &[StringField<'_>], named: &[NamedString<'_>],
        fail_after_field: Option<&str>,
    ) -> Result<BuildStatus, StructRuntimeError> {
        self.construct_with_access(name, true, fields, named, fail_after_field)
    }

    /// const／var 結構實例可寫權限取自已確認的宣告，不由
    /// String 緩衝區或目前欄位是否已初始化推定。
    pub(crate) fn construct_with_access(&mut self, name: &str, mutable: bool,
        fields: &[StringField<'_>], named: &[NamedString<'_>],
        fail_after_field: Option<&str>,
    ) -> Result<BuildStatus, StructRuntimeError> {
        self.current_name_free(name)?;
        let schema: Vec<_> = fields.iter().map(|field| FieldInfo {
            name: field.name, has_default: field.default.is_some(),
            immutable: field.immutable,
        }).collect();
        let names: Vec<_> = named.iter().map(|arg| arg.field).collect();
        let plan = struct_eval_plan::plan(&schema, &names)
            .map_err(StructRuntimeError::InvalidPlan)?;
        if fail_after_field.is_some_and(|field|
            !plan.steps.iter().any(|step| step.field == field)) {
            return Err(StructRuntimeError::UnknownFailurePoint);
        }
        let mut pending = StructOwnedValue {
            fields: Vec::new(), missing: plan.missing_mutable_fields.clone(),
            field_immutable: fields.iter()
                .map(|field|(field.name.to_owned(),field.immutable)).collect(),
        };
        for step in &plan.steps {
            let content = match step.origin {
                Origin::Named => named.iter().find(|arg| arg.field == step.field)
                    .map(|arg| arg.value).expect("具名來源已預檢"),
                Origin::Default => fields.iter().find(|field| field.name == step.field)
                    .and_then(|field| field.default).expect("預設來源已預檢"),
            };
            // 配置本身出錯亦要清理先前成功取得的其他欄位。
            let value = match self.ledger.create_detached(content) {
                Ok(value) => value,
                Err(error) => {
                    Self::release_value(&mut self.ledger, pending);
                    return Err(StructRuntimeError::Resource(error));
                }
            };
            pending.fields.push(StructResource {name:step.field.clone(),value});
            if fail_after_field == Some(step.field.as_str()) {
                Self::release_value(&mut self.ledger, pending);
                return Err(StructRuntimeError::EvaluationFailed(step.field.clone()));
            }
        }
        let status = if pending.missing.is_empty() {
            BuildStatus::Complete
        } else {
            BuildStatus::PartiallyInitialized(pending.missing.clone())
        };
        self.scopes.last_mut().expect("根作用域存在").push(StructOwnerSlot {
            name: name.to_owned(), mutable, owned: Some(pending),
        });
        Ok(status)
    }

    pub(crate) fn read_field(&self, owner: &str, field: &str)
        -> Result<&str, StructRuntimeError> {
        let (i,j) = self.location(owner)?;
        let value = self.scopes[i][j].owned.as_ref()
            .ok_or(StructRuntimeError::MovedBinding)?;
        if value.missing.iter().any(|name| name == field) {
            return Err(StructRuntimeError::UninitializedField);
        }
        value.fields.iter().find(|resource| resource.name == field)
            .map(|resource| resource.value.text())
            .ok_or(StructRuntimeError::UninitializedField)
    }

    /// VA16-03-A：僅供已核准 str 欄位右側獨立字面值的
    /// 受限原子覆寫模型。先驗證實例與欄位可寫性、再建立
    /// 唯一 UTF-8 暫存資源；模擬 RHS 失敗只釋放暫存，
    /// 成功時提交新欄位、最後才清理舊資源。
    pub(crate) fn assign_field_checked(&mut self,
        owner: &str, field: &str, right: &str, fail_after_eval: bool,
    ) -> Result<(), StructRuntimeError> {
        let (i,j)=self.location(owner)?;
        let slot=&self.scopes[i][j];
        if !slot.mutable { return Err(StructRuntimeError::ImmutableOwner); }
        let original=slot.owned.as_ref()
            .ok_or(StructRuntimeError::MovedBinding)?;
        let immutable=original.field_immutable.iter()
            .find(|(name,_)|name==field)
            .map(|(_,immutable)|*immutable)
            .ok_or(StructRuntimeError::UnknownField)?;
        if immutable { return Err(StructRuntimeError::ImmutableField); }

        let new=self.ledger.create_detached(right)
            .map_err(StructRuntimeError::Resource)?;
        if fail_after_eval {
            self.ledger.release_detached(new);
            return Err(StructRuntimeError::EvaluationFailed(field.to_owned()));
        }
        // 值已成功建立；此後沒有可失敗操作。
        // 覆寫後欄位以新完成初始化順序清理，不把舊
        // 資源在 RHS 尚未成功時釋放。
        let value=self.scopes[i][j].owned.as_mut().expect("已預檢持有者");
        let previous=value.fields.iter().position(|item|item.name==field)
            .map(|position|value.fields.remove(position));
        value.fields.push(StructResource {
            name:field.to_owned(),value:new,
        });
        value.missing.retain(|name|name!=field);
        if let Some(old)=previous {
            self.ledger.release_detached(old.value);
        }
        Ok(())
    }

    /// 一個已建構實例的所有欄位資源必須一起轉交；
    /// 所有會失敗的名稱及來源檢查都在取走任何資源之前執行。
    pub(crate) fn move_whole(&mut self, from: &str, to: &str)
        -> Result<(), StructRuntimeError> {
        self.current_name_free(to)?;
        let (i,j) = self.location(from)?;
        if self.scopes[i][j].owned.is_none() {
            return Err(StructRuntimeError::MovedBinding);
        }
        let moved = self.scopes[i][j].owned.take().expect("已預檢來源");
        for field in &moved.fields {
            self.ledger.mark_detached_move(&field.value);
        }
        self.scopes.last_mut().expect("根作用域存在").push(StructOwnerSlot {
            name: to.to_owned(), mutable: true, owned: Some(moved),
        });
        Ok(())
    }

    /// 函式返回先從原持有者取走整筆實例；每個實際存在的
    /// 欄位以相同資源身分紀錄 Returned，未初始化 var 不虛構事件。
    pub(crate) fn return_whole(&mut self, name: &str)
        -> Result<ReturnedStringStruct, StructRuntimeError> {
        let (i,j) = self.location(name)?;
        let value = self.scopes[i][j].owned.take()
            .ok_or(StructRuntimeError::MovedBinding)?;
        for field in &value.fields {
            self.ledger.mark_detached_return(&field.value);
        }
        Ok(ReturnedStringStruct { value })
    }

    /// 呼叫端接收失敗時把資源所有權退回結果，讓上層
    /// 明確選擇安全的返回處理或失敗清理，不能靠 Rust Drop。
    pub(crate) fn accept_return(&mut self, name: &str, result: ReturnedStringStruct)
        -> Result<(), (StructRuntimeError, ReturnedStringStruct)> {
        if let Err(err) = self.current_name_free(name) {
            return Err((err, result));
        }
        self.scopes.last_mut().expect("根作用域存在").push(StructOwnerSlot {
            name: name.to_owned(), mutable: true, owned: Some(result.value),
        });
        Ok(())
    }

    pub(crate) fn discard_return(&mut self, result: ReturnedStringStruct) {
        Self::release_value(&mut self.ledger, result.value);
    }

    /// str 欄位不得因 S19 而變成 Copyable；純值的整體複製
    /// 另由已核准的複製資格模型處理，不偷渡 UTF-8 隱含複製。
    pub(crate) fn copy_whole(&self, name: &str)
        -> Result<(), StructRuntimeError> {
        let (i,j) = self.location(name)?;
        if self.scopes[i][j].owned.is_none() {
            return Err(StructRuntimeError::MovedBinding);
        }
        Err(StructRuntimeError::NonCopyableStruct)
    }

    pub(crate) fn leave_scope(&mut self) -> Result<(), StructRuntimeError> {
        if self.scopes.len() == 1 {
            return Err(StructRuntimeError::NoInnerScope);
        }
        let frame = self.scopes.pop().expect("已檢查內層存在");
        Self::cleanup_frame(&mut self.ledger, frame);
        Ok(())
    }

    pub(crate) fn events(&self) -> &[ResourceEvent] { self.ledger.events() }

    pub(crate) fn finish(mut self) -> Result<Vec<ResourceEvent>, StructRuntimeError> {
        if self.scopes.len() != 1 {
            return Err(StructRuntimeError::NoInnerScope);
        }
        let root = self.scopes.pop().expect("根作用域存在");
        Self::cleanup_frame(&mut self.ledger, root);
        self.ledger.finish().map_err(StructRuntimeError::Resource)
    }
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

    #[test]
    fn vb16_所有成功初始化步驟的失敗切點皆恰好逆序釋放一次() {
        let fields=fields();
        let named=[
            NamedString{field:"c",value:"丙"},
            NamedString{field:"a",value:"甲"},
        ];
        for (index,failed_field) in ["c","a","b","id"].iter().enumerate() {
            let trace=build_string_struct(&fields,&named,Some(failed_field));
            assert_eq!(trace.status,BuildStatus::EvaluationFailed((*failed_field).into()));
            let expected:Vec<u64>=(1..=(index+1) as u64).collect();
            let mut reversed=expected.clone();
            reversed.reverse();
            assert_eq!(created(&trace.resources),expected,"失敗欄位：{failed_field}");
            assert_eq!(released(&trace.resources),reversed,"失敗欄位：{failed_field}");
            assert_eq!(trace.evaluated.len(),index+1);
        }
    }
}

#[cfg(test)]
mod p16_真實持續存活結構值資源責任 {
    use super::*;
    fn fields() -> [StringField<'static>;3] {
        [
            StringField{name:"a",default:Some("甲"),immutable:false},
            StringField{name:"b",default:Some("乙"),immutable:false},
            StringField{name:"fixed",default:Some("固定🪼"),immutable:true},
        ]
    }
    fn ids(events: &[ResourceEvent], created: bool) -> Vec<u64> {
        events.iter().filter_map(|event| match event {
            ResourceEvent::Created(id) if created => Some(*id),
            ResourceEvent::Released(id) if !created => Some(*id),
            _ => None,
        }).collect()
    }

    #[test]
    fn 成功建立後資源保持存活_直到持有者作用域退出才反序釋放() {
        let mut rt=OwnedStructRuntime::new();
        let args=[
            NamedString{field:"b",value:"具名乙"},
            NamedString{field:"a",value:"具名甲"},
        ];
        assert_eq!(rt.construct("owner",&fields(),&args,None),Ok(BuildStatus::Complete));
        assert_eq!(rt.read_field("owner","a"),Ok("具名甲"));
        assert_eq!(rt.read_field("owner","fixed"),Ok("固定🪼"));
        assert_eq!(ids(rt.events(),true),vec![1,2,3]);
        assert!(ids(rt.events(),false).is_empty(),
            "成功建構不能在傳回前立即析構");
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![3,2,1]);
    }

    #[test]
    fn 整筆結構轉移不分割欄位_各資源身分和_utf8_緩衝區皆保留() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("from",&fields(),&[],None).unwrap();
        let before=rt.read_field("from","fixed").unwrap().as_ptr();
        rt.move_whole("from","to").unwrap();
        assert_eq!(rt.read_field("from","a"),Err(StructRuntimeError::MovedBinding));
        assert_eq!(rt.read_field("to","fixed"),Ok("固定🪼"));
        assert_eq!(rt.read_field("to","fixed").unwrap().as_ptr(),before);
        assert!(ids(rt.events(),false).is_empty());
        let events=rt.finish().unwrap();
        assert_eq!(ids(&events,true),vec![1,2,3]);
        assert_eq!(ids(&events,false),vec![3,2,1]);
        assert_eq!(events.iter().filter(|e| matches!(e,ResourceEvent::Moved(_))).count(),3);
    }

    #[test]
    fn 具名和預設順序中的失敗必須只回滾本次成功取得的欄位() {
        for (failure,all_ids) in [("b",vec![4]),("a",vec![4,5]),("fixed",vec![4,5,6])] {
            let mut rt=OwnedStructRuntime::new();
            rt.construct("stable",&fields(),&[],None).unwrap();
            let args=[NamedString{field:"b",value:"乙"},NamedString{field:"a",value:"甲"}];
            assert_eq!(rt.construct("failed",&fields(),&args,Some(failure)),
                Err(StructRuntimeError::EvaluationFailed(failure.into())));
            assert_eq!(rt.read_field("stable","fixed"),Ok("固定🪼"));
            assert_eq!(rt.read_field("failed","a"),
                Err(StructRuntimeError::MissingBinding));
            // stable 先建立 3 個 ID；失敗工程只產生新欄位事件。
            let actual=ids(rt.events(),false);
            let mut reversed=all_ids.clone();
            reversed.reverse();
            assert_eq!(actual,reversed);
            let all=rt.finish().unwrap();
            assert_eq!(ids(&all,false).len(),3+all_ids.len());
        }
    }

    #[test]
    fn 重名及錯誤求值計畫須在任何配置與移動前拒絕() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("original",&fields(),&[],None).unwrap();
        let before=rt.events().len();
        assert_eq!(rt.construct("original",&fields(),&[],None),
            Err(StructRuntimeError::DuplicateBinding));
        assert_eq!(rt.construct("other",&fields(),&[
            NamedString{field:"a",value:"A"},
            NamedString{field:"a",value:"A2"},
        ],None),Err(StructRuntimeError::InvalidPlan(
            PlanError::DuplicateField("a".into()))));
        assert_eq!(rt.events().len(),before);
        assert_eq!(rt.read_field("original","a"),Ok("甲"));
        rt.finish().unwrap();
    }

    #[test]
    fn 沒有真實求值步驟的失敗切點拒絕且不會配置資源() {
        let mut rt=OwnedStructRuntime::new();
        let schema=[
            StringField{name:"pending",default:None,immutable:false},
            StringField{name:"fixed",default:Some("甲"),immutable:true},
        ];
        assert_eq!(rt.construct("bad",&schema,&[],Some("pending")),
            Err(StructRuntimeError::UnknownFailurePoint));
        assert!(rt.events().is_empty());
        assert_eq!(rt.construct("partial",&schema,&[],None),
            Ok(BuildStatus::PartiallyInitialized(vec!["pending".into()])));
        assert_eq!(rt.read_field("partial","pending"),
            Err(StructRuntimeError::UninitializedField));
        assert_eq!(rt.read_field("partial","fixed"),Ok("甲"));
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![1]);
    }

    #[test]
    fn 內層結構先釋放_外層結構持有者依然有效() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("outer",&fields(),&[],None).unwrap();
        rt.enter_scope();
        rt.construct("inside",&fields(),&[],None).unwrap();
        assert!(ids(rt.events(),false).is_empty());
        rt.leave_scope().unwrap();
        assert_eq!(ids(rt.events(),false),vec![6,5,4]);
        assert_eq!(rt.read_field("outer","fixed"),Ok("固定🪼"));
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![6,5,4,3,2,1]);
    }

    #[test]
    fn 結構整筆返回後原函式退出不能釋放_由呼叫端負責清理() {
        let mut rt=OwnedStructRuntime::new();
        rt.enter_scope();
        rt.construct("local",&fields(),&[],None).unwrap();
        let returned=rt.return_whole("local").unwrap();
        assert_eq!(rt.read_field("local","a"),Err(StructRuntimeError::MovedBinding));
        rt.leave_scope().unwrap();
        assert!(ids(rt.events(),false).is_empty());
        rt.accept_return("received",returned).unwrap();
        assert_eq!(rt.read_field("received","fixed"),Ok("固定🪼"));
        let events=rt.finish().unwrap();
        assert_eq!(ids(&events,false),vec![3,2,1]);
        assert_eq!(events.iter().filter(|e|matches!(e,ResourceEvent::Returned(_))).count(),3);
    }

    #[test]
    fn 接收返回值遭遇同名衝突_不得丟失原值或返回資源責任() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("occupied",&fields(),&[],None).unwrap();
        rt.enter_scope();
        rt.construct("local",&fields(),&[],None).unwrap();
        let returned=rt.return_whole("local").unwrap();
        rt.leave_scope().unwrap();
        let (err,returned)=rt.accept_return("occupied",returned)
            .expect_err("不得在目的已存在時隱含析構被返回資源");
        assert_eq!(err,StructRuntimeError::DuplicateBinding);
        assert_eq!(rt.read_field("occupied","fixed"),Ok("固定🪼"));
        assert!(ids(rt.events(),false).is_empty());
        rt.discard_return(returned);
        assert_eq!(ids(rt.events(),false),vec![6,5,4]);
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![6,5,4,3,2,1]);
    }

    #[test]
    fn 非_copyable_字串結構不得隱含整體複製() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        assert_eq!(rt.copy_whole("owner"),Err(StructRuntimeError::NonCopyableStruct));
        assert_eq!(rt.read_field("owner","a"),Ok("甲"));
        rt.move_whole("owner","moved").unwrap();
        assert_eq!(rt.copy_whole("owner"),Err(StructRuntimeError::MovedBinding));
        assert_eq!(rt.read_field("moved","b"),Ok("乙"));
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![3,2,1]);
    }

    #[test]
    fn 已移動整體來源重移動或撞到目的名稱都不能破壞持有者() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("a",&fields(),&[],None).unwrap();
        rt.construct("b",&fields(),&[],None).unwrap();
        assert_eq!(rt.move_whole("a","b"),Err(StructRuntimeError::DuplicateBinding));
        assert_eq!(rt.read_field("a","a"),Ok("甲"));
        rt.move_whole("a","moved").unwrap();
        assert_eq!(rt.move_whole("a","again"),Err(StructRuntimeError::MovedBinding));
        assert_eq!(rt.read_field("moved","a"),Ok("甲"));
        assert_eq!(ids(&rt.finish().unwrap(),false),vec![3,2,1,6,5,4]);
    }
}

#[cfg(test)]
mod p16_持續結構欄位原子覆寫安全 {
    use super::*;

    fn fields()->[StringField<'static>;3] {
        [
            StringField{name:"a",default:Some("舊甲"),immutable:false},
            StringField{name:"b",default:Some("原乙"),immutable:false},
            StringField{name:"fixed",default:Some("固定🪼"),immutable:true},
        ]
    }
    fn released(events:&[ResourceEvent])->Vec<u64> {
        events.iter().filter_map(|item|
            if let ResourceEvent::Released(id)=item {Some(*id)}else{None}
        ).collect()
    }

    #[test]
    fn 覆寫先建立新字串後立即釋放舊欄位_最後按更新初始化次序清理() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        rt.assign_field_checked("owner","a","新甲🪼",false).unwrap();
        assert_eq!(rt.read_field("owner","a"),Ok("新甲🪼"));
        assert_eq!(rt.read_field("owner","b"),Ok("原乙"));
        assert_eq!(released(rt.events()),vec![1]);
        assert_eq!(rt.finish().unwrap(),vec![
            ResourceEvent::Created(1),ResourceEvent::Created(2),
            ResourceEvent::Created(3),ResourceEvent::Created(4),
            ResourceEvent::Released(1),
            ResourceEvent::Released(4),ResourceEvent::Released(3),
            ResourceEvent::Released(2),
        ]);
    }

    #[test]
    fn 右側求值失敗只釋放暫存不改原欄位() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        assert_eq!(rt.assign_field_checked("owner","a","不應提交",true),
            Err(StructRuntimeError::EvaluationFailed("a".into())));
        assert_eq!(rt.read_field("owner","a"),Ok("舊甲"));
        assert_eq!(released(rt.events()),vec![4]);
        assert_eq!(released(&rt.finish().unwrap()),vec![4,3,2,1]);
    }

    #[test]
    fn const_欄位拒絕覆寫且無任何新資源建立() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        let before=rt.events().len();
        assert_eq!(rt.assign_field_checked("owner","fixed","非法",false),
            Err(StructRuntimeError::ImmutableField));
        assert_eq!(rt.events().len(),before);
        assert_eq!(rt.read_field("owner","fixed"),Ok("固定🪼"));
        assert_eq!(released(&rt.finish().unwrap()),vec![3,2,1]);
    }

    #[test]
    fn const_結構實例不能因_var_欄位可寫就繞過_s18() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct_with_access("owner",false,&fields(),&[],None).unwrap();
        let before=rt.events().len();
        assert_eq!(rt.assign_field_checked("owner","a","非法",false),
            Err(StructRuntimeError::ImmutableOwner));
        assert_eq!(rt.events().len(),before);
        assert_eq!(rt.read_field("owner","a"),Ok("舊甲"));
        assert_eq!(released(&rt.finish().unwrap()),vec![3,2,1]);
    }

    #[test]
    fn 無預設_var_欄位首次完整初始化不需釋放不存在舊資源() {
        let schema=[
            StringField{name:"pending",default:None,immutable:false},
            StringField{name:"fixed",default:Some("固定"),immutable:true},
        ];
        let mut rt=OwnedStructRuntime::new();
        assert_eq!(rt.construct("owner",&schema,&[],None),
            Ok(BuildStatus::PartiallyInitialized(vec!["pending".into()])));
        assert_eq!(rt.read_field("owner","pending"),
            Err(StructRuntimeError::UninitializedField));
        rt.assign_field_checked("owner","pending","首次🪼",false).unwrap();
        assert_eq!(rt.read_field("owner","pending"),Ok("首次🪼"));
        assert_eq!(rt.events(),&[
            ResourceEvent::Created(1),ResourceEvent::Created(2)
        ]);
        assert_eq!(released(&rt.finish().unwrap()),vec![2,1]);
    }

    #[test]
    fn 不存在欄位不得先建立新資源() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        let before=rt.events().len();
        assert_eq!(rt.assign_field_checked("owner","missing","不應存在",false),
            Err(StructRuntimeError::UnknownField));
        assert_eq!(rt.events().len(),before);
        assert_eq!(released(&rt.finish().unwrap()),vec![3,2,1]);
    }

    #[test]
    fn 整筆轉移後舊持有者不可覆寫_新持有者仍可安全原子覆寫() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        rt.move_whole("owner","moved").unwrap();
        assert_eq!(rt.assign_field_checked("owner","a","非法",false),
            Err(StructRuntimeError::MovedBinding));
        rt.assign_field_checked("moved","a","新持有者",false).unwrap();
        assert_eq!(rt.read_field("moved","a"),Ok("新持有者"));
        assert_eq!(released(rt.events()),vec![1]);
        assert_eq!(released(&rt.finish().unwrap()),vec![1,4,3,2]);
    }

    #[test]
    fn 先失敗再成功覆寫仍不提前釋放原值或重複釋放() {
        let mut rt=OwnedStructRuntime::new();
        rt.construct("owner",&fields(),&[],None).unwrap();
        assert_eq!(rt.assign_field_checked("owner","b","先失敗",true),
            Err(StructRuntimeError::EvaluationFailed("b".into())));
        assert_eq!(rt.read_field("owner","b"),Ok("原乙"));
        rt.assign_field_checked("owner","b","後成功",false).unwrap();
        assert_eq!(rt.read_field("owner","b"),Ok("後成功"));
        // 失敗暫存 id4，成功欄位 id5，舊欄位 id2 僅成功後釋放。
        assert_eq!(released(rt.events()),vec![4,2]);
        assert_eq!(released(&rt.finish().unwrap()),vec![4,2,5,3,1]);
    }
}
