//! P16.1／P16.2／P16.6：混合 Copyable 與唯一 UTF-8 所有權的私有實際值模型。
//! bool／int 為可複製純值，不得造假資源建立或釋放事件；
//! str 是非 Copyable 的真實 String，必須整筆移交、單次清理。
//! 遵守 VB16-01-A 具名優先、未覆蓋預設次之的真正求值計畫。
//! 這是內部受限執行期，並非 P17 通用直譯器。
#![allow(dead_code)]

use crate::owned_str_runtime::{OwnedStrRuntime,OwnedUtf8,ResourceEvent,StrRuntimeError};
use crate::struct_eval_plan::{self,FieldInfo,Origin,PlanError};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum ScalarKind { Int, Bool, Str }

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum SeedValue<'a> { Int(i64), Bool(bool), Str(&'a str) }
impl SeedValue<'_> {
    fn kind(self)->ScalarKind {
        match self {
            Self::Int(_)=>ScalarKind::Int,Self::Bool(_)=>ScalarKind::Bool,
            Self::Str(_)=>ScalarKind::Str,
        }
    }
}

#[derive(Debug,Clone,Copy)]
pub(crate) struct MixedFieldSpec<'a> {
    pub name:&'a str,
    pub kind:ScalarKind,
    pub default:Option<SeedValue<'a>>,
    pub immutable:bool,
}
#[derive(Debug,Clone,Copy)]
pub(crate) struct NamedMixed<'a> {
    pub field:&'a str,
    pub value:SeedValue<'a>,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) enum MixedError {
    Plan(PlanError),
    TypeMismatch(String),
    UnknownFailurePoint,
    EvaluationFailed(String),
    DuplicateBinding,
    MissingBinding,
    MovedBinding,
    IncompleteStruct,
    NonCopyableStruct,
    MissingField,
    NoInnerScope,
    Resource(StrRuntimeError),
}

#[derive(Debug)]
enum StoredValue {
    Int(i64),
    Bool(bool),
    Str(OwnedUtf8),
}
#[derive(Debug)]
struct StoredField { name:String, value:StoredValue }
#[derive(Debug)]
struct MixedOwned {
    // 真正完成求值的順序，不能用 struct 宣告順序代替。
    fields:Vec<StoredField>,
    missing:Vec<String>,
}
#[derive(Debug)]
struct OwnedSlot { name:String, owned:Option<MixedOwned> }
#[derive(Debug)]
pub(crate) struct ReturnedMixed { value:MixedOwned }
#[derive(Debug)]
pub(crate) struct MixedStructRuntime {
    ledger:OwnedStrRuntime,
    scopes:Vec<Vec<OwnedSlot>>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum Observed<'a> { Int(i64),Bool(bool),Str(&'a str) }

impl MixedStructRuntime {
    pub(crate) fn new()->Self {
        Self{ledger:OwnedStrRuntime::new(),scopes:vec![Vec::new()]}
    }
    pub(crate) fn enter_scope(&mut self) {self.scopes.push(Vec::new());}

    fn current_free(&self,name:&str)->Result<(),MixedError> {
        if self.scopes.last().expect("根作用域必須存在").iter()
            .any(|slot|slot.name==name) {
            Err(MixedError::DuplicateBinding)
        } else {Ok(())}
    }
    fn locate(&self,name:&str)->Result<(usize,usize),MixedError> {
        self.scopes.iter().enumerate().rev().find_map(|(i,scope)|
            scope.iter().position(|slot|slot.name==name).map(|j|(i,j))
        ).ok_or(MixedError::MissingBinding)
    }
    fn release_value(ledger:&mut OwnedStrRuntime,value:MixedOwned) {
        for field in value.fields.into_iter().rev() {
            if let StoredValue::Str(resource)=field.value {
                ledger.release_detached(resource);
            }
        }
    }
    fn cleanup(ledger:&mut OwnedStrRuntime,frame:Vec<OwnedSlot>) {
        for slot in frame.into_iter().rev() {
            if let Some(value)=slot.owned {Self::release_value(ledger,value);}
        }
    }
    pub(crate) fn construct(&mut self,name:&str,
        fields:&[MixedFieldSpec<'_>],named:&[NamedMixed<'_>],
        fail_after:Option<&str>,
    )->Result<(),MixedError> {
        self.current_free(name)?;
        let schema:Vec<_>=fields.iter().map(|field|FieldInfo{
            name:field.name,has_default:field.default.is_some(),
            immutable:field.immutable,
        }).collect();
        let names:Vec<_>=named.iter().map(|arg|arg.field).collect();
        let plan=struct_eval_plan::plan(&schema,&names).map_err(MixedError::Plan)?;
        // 全部型別與失敗點先核對，不能在第一個具名欄位已分配
        // 後才發現第二個引數不合法，留下無法回滾的資源。
        for field in fields {
            if field.default.is_some_and(|value|value.kind()!=field.kind) {
                return Err(MixedError::TypeMismatch(field.name.to_owned()));
            }
        }
        for arg in named {
            let field=fields.iter().find(|field|field.name==arg.field)
                .expect("求值計畫已驗證具名欄位存在");
            if arg.value.kind()!=field.kind {
                return Err(MixedError::TypeMismatch(arg.field.to_owned()));
            }
        }
        if fail_after.is_some_and(|name|!plan.steps.iter().any(|step|step.field==name)) {
            return Err(MixedError::UnknownFailurePoint);
        }
        let mut pending=MixedOwned {
            fields:Vec::new(),missing:plan.missing_mutable_fields,
        };
        for step in plan.steps {
            let seed=match step.origin {
                Origin::Named=>named.iter().find(|arg|arg.field==step.field)
                    .expect("已預檢具名引數").value,
                Origin::Default=>fields.iter().find(|field|field.name==step.field)
                    .expect("已預檢預設欄位").default.expect("預設已存在"),
            };
            let value=match seed {
                SeedValue::Int(value)=>StoredValue::Int(value),
                SeedValue::Bool(value)=>StoredValue::Bool(value),
                SeedValue::Str(text)=>match self.ledger.create_detached(text) {
                    Ok(resource)=>StoredValue::Str(resource),
                    Err(err)=>{
                        Self::release_value(&mut self.ledger,pending);
                        return Err(MixedError::Resource(err));
                    }
                },
            };
            pending.fields.push(StoredField {name:step.field.clone(),value});
            if fail_after==Some(step.field.as_str()) {
                Self::release_value(&mut self.ledger,pending);
                return Err(MixedError::EvaluationFailed(step.field));
            }
        }
        self.scopes.last_mut().expect("根作用域存在").push(OwnedSlot{
            name:name.to_owned(),owned:Some(pending),
        });
        Ok(())
    }
    pub(crate) fn field(&self,owner:&str,field:&str)->Result<Observed<'_>,MixedError> {
        let (i,j)=self.locate(owner)?;
        let value=self.scopes[i][j].owned.as_ref()
            .ok_or(MixedError::MovedBinding)?;
        let member=value.fields.iter().find(|member|member.name==field)
            .ok_or(MixedError::MissingField)?;
        Ok(match &member.value {
            StoredValue::Int(n)=>Observed::Int(*n),
            StoredValue::Bool(b)=>Observed::Bool(*b),
            StoredValue::Str(resource)=>Observed::Str(resource.text()),
        })
    }
    fn valid_whole(&self,name:&str)->Result<(usize,usize),MixedError> {
        let (i,j)=self.locate(name)?;
        let value=self.scopes[i][j].owned.as_ref()
            .ok_or(MixedError::MovedBinding)?;
        if !value.missing.is_empty() {return Err(MixedError::IncompleteStruct);}
        Ok((i,j))
    }
    pub(crate) fn move_whole(&mut self,source:&str,target:&str)
        ->Result<(),MixedError> {
        self.current_free(target)?;
        let (i,j)=self.valid_whole(source)?;
        let value=self.scopes[i][j].owned.take().expect("已預檢持有者");
        for field in &value.fields {
            if let StoredValue::Str(resource)=&field.value {
                self.ledger.mark_detached_move(resource);
            }
        }
        self.scopes.last_mut().expect("根作用域存在").push(OwnedSlot {
            name:target.to_owned(),owned:Some(value),
        });
        Ok(())
    }
    /// 只允許已完成的全純值結構獨立複本；
    /// 發現任何 str 必須在插入目標前完整拒絕，不能偷複製 String。
    pub(crate) fn copy_whole(&mut self,source:&str,target:&str)
        ->Result<(),MixedError> {
        self.current_free(target)?;
        let (i,j)=self.valid_whole(source)?;
        let original=self.scopes[i][j].owned.as_ref().expect("已預檢");
        if original.fields.iter().any(|field|matches!(&field.value,StoredValue::Str(_))) {
            return Err(MixedError::NonCopyableStruct);
        }
        let fields=original.fields.iter().map(|field|StoredField {
            name:field.name.clone(),
            value:match &field.value {
                StoredValue::Int(n)=>StoredValue::Int(*n),
                StoredValue::Bool(b)=>StoredValue::Bool(*b),
                StoredValue::Str(_)=>unreachable!("含 str 已整批預檢拒絕"),
            },
        }).collect();
        self.scopes.last_mut().expect("根作用域存在").push(OwnedSlot{
            name:target.to_owned(),owned:Some(MixedOwned {fields,missing:Vec::new()}),
        });
        Ok(())
    }
    pub(crate) fn return_whole(&mut self,name:&str)
        ->Result<ReturnedMixed,MixedError> {
        let (i,j)=self.valid_whole(name)?;
        let value=self.scopes[i][j].owned.take().expect("已預檢");
        for field in &value.fields {
            if let StoredValue::Str(resource)=&field.value {
                self.ledger.mark_detached_return(resource);
            }
        }
        Ok(ReturnedMixed{value})
    }
    pub(crate) fn accept_return(&mut self,name:&str,result:ReturnedMixed)
        ->Result<(),(MixedError,ReturnedMixed)> {
        if let Err(err)=self.current_free(name) {return Err((err,result));}
        self.scopes.last_mut().expect("根作用域存在").push(OwnedSlot{
            name:name.to_owned(),owned:Some(result.value),
        });
        Ok(())
    }
    pub(crate) fn discard_return(&mut self,value:ReturnedMixed) {
        Self::release_value(&mut self.ledger,value.value);
    }
    pub(crate) fn leave_scope(&mut self)->Result<(),MixedError> {
        if self.scopes.len()==1 {return Err(MixedError::NoInnerScope);}
        let frame=self.scopes.pop().expect("已預檢作用域");
        Self::cleanup(&mut self.ledger,frame);
        Ok(())
    }
    pub(crate) fn events(&self)->&[ResourceEvent] {self.ledger.events()}
    pub(crate) fn finish(mut self)->Result<Vec<ResourceEvent>,MixedError> {
        if self.scopes.len()!=1 {return Err(MixedError::NoInnerScope);}
        let root=self.scopes.pop().expect("根作用域存在");
        Self::cleanup(&mut self.ledger,root);
        self.ledger.finish().map_err(MixedError::Resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn all<'a>()->[MixedFieldSpec<'a>;3] {
        [MixedFieldSpec{name:"num",kind:ScalarKind::Int,
            default:Some(SeedValue::Int(42)),immutable:false},
         MixedFieldSpec{name:"text",kind:ScalarKind::Str,
            default:Some(SeedValue::Str("繁體中文🪼")),immutable:false},
         MixedFieldSpec{name:"flag",kind:ScalarKind::Bool,
            default:Some(SeedValue::Bool(true)),immutable:true}]
    }
    fn released(events:&[ResourceEvent])->Vec<u64> {
        events.iter().filter_map(|event|match event {
            ResourceEvent::Released(id)=>Some(*id),_=>None,
        }).collect()
    }
    #[test]
    fn 混合型別具名與預設求值只對_str_建立資源() {
        let mut rt=MixedStructRuntime::new();
        rt.construct("a",&all(),&[
            NamedMixed{field:"flag",value:SeedValue::Bool(false)},
            NamedMixed{field:"text",value:SeedValue::Str("新值")},
        ],None).unwrap();
        assert_eq!(rt.field("a","flag"),Ok(Observed::Bool(false)));
        assert_eq!(rt.field("a","num"),Ok(Observed::Int(42)));
        assert_eq!(rt.field("a","text"),Ok(Observed::Str("新值")));
        assert_eq!(rt.finish().unwrap(),vec![
            ResourceEvent::Created(1),ResourceEvent::Released(1),
        ]);
    }
    #[test]
    fn 混合型別整筆移動不複製_str_緩衝區也不替純值造事件() {
        let mut rt=MixedStructRuntime::new();
        rt.construct("src",&all(),&[],None).unwrap();
        let ptr=match rt.field("src","text").unwrap() {
            Observed::Str(v)=>v.as_ptr(),_=>unreachable!(),
        };
        rt.move_whole("src","dst").unwrap();
        assert_eq!(rt.field("src","text"),Err(MixedError::MovedBinding));
        assert_eq!(rt.field("dst","num"),Ok(Observed::Int(42)));
        assert_eq!(match rt.field("dst","text").unwrap() {
            Observed::Str(v)=>v.as_ptr(),_=>unreachable!(),
        },ptr);
        assert_eq!(rt.finish().unwrap(),vec![
            ResourceEvent::Created(1),ResourceEvent::Moved(1),
            ResourceEvent::Released(1),
        ]);
    }
    #[test]
    fn 含非_copyable_str_的整體複製先拒絕_來源保持原狀() {
        let mut rt=MixedStructRuntime::new();
        rt.construct("src",&all(),&[],None).unwrap();
        let before=rt.events().len();
        assert_eq!(rt.copy_whole("src","dst"),Err(MixedError::NonCopyableStruct));
        assert_eq!(rt.events().len(),before);
        assert_eq!(rt.field("src","text"),Ok(Observed::Str("繁體中文🪼")));
        assert_eq!(rt.field("dst","text"),Err(MixedError::MissingBinding));
        assert_eq!(released(&rt.finish().unwrap()),vec![1]);
    }
    #[test]
    fn 全純值結構可複製成兩個獨立值_沒有虛構資源釋放() {
        let schema=[
            MixedFieldSpec{name:"num",kind:ScalarKind::Int,
                default:Some(SeedValue::Int(7)),immutable:false},
            MixedFieldSpec{name:"flag",kind:ScalarKind::Bool,
                default:Some(SeedValue::Bool(false)),immutable:true},
        ];
        let mut rt=MixedStructRuntime::new();
        rt.construct("src",&schema,&[],None).unwrap();
        rt.copy_whole("src","dst").unwrap();
        rt.move_whole("src","third").unwrap();
        assert_eq!(rt.field("src","num"),Err(MixedError::MovedBinding));
        assert_eq!(rt.field("dst","num"),Ok(Observed::Int(7)));
        assert_eq!(rt.field("third","flag"),Ok(Observed::Bool(false)));
        assert!(rt.finish().unwrap().is_empty());
    }
    #[test]
    fn 混合具名與預設失敗只清理真正已取得_str_資源() {
        let mut rt=MixedStructRuntime::new();
        assert_eq!(rt.construct("a",&all(),&[
            NamedMixed{field:"text",value:SeedValue::Str("失敗的資源")},
            NamedMixed{field:"num",value:SeedValue::Int(11)},
        ],Some("num")),Err(MixedError::EvaluationFailed("num".into())));
        assert_eq!(rt.finish().unwrap(),vec![
            ResourceEvent::Created(1),ResourceEvent::Released(1),
        ]);
    }
    #[test]
    fn 混合型別錯誤或非法具名在任何資源配置前拒絕() {
        let mut rt=MixedStructRuntime::new();
        assert_eq!(rt.construct("a",&all(),&[
            NamedMixed{field:"text",value:SeedValue::Str("資源")},
            NamedMixed{field:"num",value:SeedValue::Bool(true)},
        ],None),Err(MixedError::TypeMismatch("num".into())));
        assert!(rt.events().is_empty());
        assert!(matches!(rt.construct("b",&all(),&[
            NamedMixed{field:"text",value:SeedValue::Str("甲")},
            NamedMixed{field:"text",value:SeedValue::Str("乙")},
        ],None),Err(MixedError::Plan(PlanError::DuplicateField(_)))));
        assert!(rt.finish().unwrap().is_empty());
    }
    #[test]
    fn 混合結構返回原作用域不釋放_由呼叫端最終清理() {
        let mut rt=MixedStructRuntime::new();
        rt.enter_scope();
        rt.construct("local",&all(),&[],None).unwrap();
        let returned=rt.return_whole("local").unwrap();
        rt.leave_scope().unwrap();
        assert!(released(rt.events()).is_empty());
        rt.accept_return("receiver",returned).unwrap();
        assert_eq!(rt.field("receiver","text"),Ok(Observed::Str("繁體中文🪼")));
        assert_eq!(rt.finish().unwrap(),vec![
            ResourceEvent::Created(1),ResourceEvent::Returned(1),
            ResourceEvent::Released(1),
        ]);
    }
    #[test]
    fn 返回接收衝突由上層顯式釋放不能依賴_rust_drop() {
        let mut rt=MixedStructRuntime::new();
        rt.construct("receiver",&all(),&[],None).unwrap();
        rt.enter_scope();
        rt.construct("local",&all(),&[],None).unwrap();
        let ret=rt.return_whole("local").unwrap();
        rt.leave_scope().unwrap();
        let (err,value)=rt.accept_return("receiver",ret).unwrap_err();
        assert_eq!(err,MixedError::DuplicateBinding);
        assert_eq!(released(rt.events()),Vec::<u64>::new());
        rt.discard_return(value);
        assert_eq!(released(&rt.finish().unwrap()),vec![2,1]);
    }
    #[test]
    fn 缺失_var_不能整體移動_亦不能冒充完整複製() {
        let schema=[
            MixedFieldSpec{name:"missing",kind:ScalarKind::Str,
                default:None,immutable:false},
            MixedFieldSpec{name:"flag",kind:ScalarKind::Bool,
                default:Some(SeedValue::Bool(true)),immutable:true},
        ];
        let mut rt=MixedStructRuntime::new();
        rt.construct("partial",&schema,&[],None).unwrap();
        assert_eq!(rt.move_whole("partial","dst"),Err(MixedError::IncompleteStruct));
        assert_eq!(rt.copy_whole("partial","dst"),Err(MixedError::IncompleteStruct));
        assert_eq!(rt.return_whole("partial").unwrap_err(),MixedError::IncompleteStruct);
        assert!(rt.finish().unwrap().is_empty());
    }
    #[test]
    fn 內層混合實例先清理_外層可複製純值仍有效() {
        let mut rt=MixedStructRuntime::new();
        rt.construct("outer",&all(),&[],None).unwrap();
        rt.enter_scope();
        rt.construct("inner",&all(),&[],None).unwrap();
        rt.leave_scope().unwrap();
        assert_eq!(released(rt.events()),vec![2]);
        assert_eq!(rt.field("outer","text"),Ok(Observed::Str("繁體中文🪼")));
        assert_eq!(released(&rt.finish().unwrap()),vec![2,1]);
    }
}
