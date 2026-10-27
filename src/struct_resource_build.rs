//! VB16-01-A、VA16-02-A：結構欄位的實際 UTF-8 資源取得順序與失敗回滾。
//! 先依具名引數原始次序求值，再依欄位宣告次序求值未覆蓋預設值。
//! 清理一律由明確的事件迴圈逆成功初始化次序執行，
//! 不藉 Rust 自動 Drop 次序推定 voiles 析構順序。
//! 本模組僅建構 str 最小資源子集，不能當成完整結構執行器。
#![allow(dead_code)]

use crate::struct_eval_plan::{self, FieldInfo, Origin, PlanError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildEvent {
    Initialized(String, Origin),
    Released(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    InvalidPlan(PlanError),
    MissingValue(String),
    FailedAt(String),
}

pub(crate) struct StrFieldDef<'a> {
    pub name: &'a str,
    pub default: Option<&'a str>,
    pub immutable: bool,
}

pub(crate) struct NamedStrArg<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

#[derive(Default)]
pub(crate) struct StrStructBuilder {
    /// 真正持有每個成功初始化的 UTF-8 緩衝區；最後一筆位於最上層。
    initialized: Vec<(String, String)>,
    events: Vec<BuildEvent>,
}

impl StrStructBuilder {
    pub(crate) fn new() -> Self { Self::default() }

    /// 對完整合法的建構先取得計畫，避免非法欄位名稱消耗原值。
    /// fail_at 僅是內部測試用的可重現求值錯誤注入，不是 voiles 語法。
    pub(crate) fn build(&mut self, fields: &[StrFieldDef<'_>],
        named: &[NamedStrArg<'_>], fail_at: Option<&str>) -> Result<(), BuildError> {
        if !self.initialized.is_empty() {
            return Err(BuildError::FailedAt("尚有未交接的上一筆資源".into()));
        }
        let info: Vec<_> = fields.iter().map(|f| FieldInfo {
            name:f.name, has_default:f.default.is_some(), immutable:f.immutable,
        }).collect();
        let names: Vec<_> = named.iter().map(|arg| arg.name).collect();
        let ordered = struct_eval_plan::plan(&info, &names)
            .map_err(BuildError::InvalidPlan)?;
        for step in ordered.steps {
            if fail_at == Some(step.field.as_str()) {
                self.rollback();
                return Err(BuildError::FailedAt(step.field));
            }
            let chosen = match step.origin {
                Origin::Named => named.iter()
                    .find(|arg| arg.name == step.field)
                    .map(|arg| arg.value),
                Origin::Default => fields.iter()
                    .find(|field| field.name == step.field)
                    .and_then(|field| field.default),
            };
            let Some(value) = chosen else {
                self.rollback();
                return Err(BuildError::MissingValue(step.field));
            };
            self.initialized.push((step.field.clone(), value.to_owned()));
            self.events.push(BuildEvent::Initialized(step.field, step.origin));
        }
        Ok(())
    }

    /// 模擬結構正常退出所在作用域；真正的 String 緩衝區在此釋放，
    /// 且事件順序可直接測試，與 Rust 自動釋放順序無關。
    pub(crate) fn cleanup(&mut self) {
        while let Some((field, utf8)) = self.initialized.pop() {
            self.events.push(BuildEvent::Released(field));
            drop(utf8);
        }
    }

    fn rollback(&mut self) { self.cleanup(); }

    pub(crate) fn events(&self) -> &[BuildEvent] { &self.events }
    pub(crate) fn outstanding(&self) -> usize { self.initialized.len() }
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.initialized.iter().find(|(key, _)| key == name)
            .map(|(_, text)| text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<StrFieldDef<'static>> {
        vec![
            StrFieldDef{name:"a",default:Some("old a"),immutable:false},
            StrFieldDef{name:"b",default:Some("old b"),immutable:false},
            StrFieldDef{name:"c",default:Some("old c"),immutable:true},
        ]
    }

    #[test]
    fn vb16_具名順序先於預設_覆蓋的預設不建立資源() {
        let mut builder=StrStructBuilder::new();
        builder.build(&fields(),&[
            NamedStrArg{name:"b",value:"named b"},
            NamedStrArg{name:"a",value:"named a"},
        ],None).unwrap();
        assert_eq!(builder.field("b"),Some("named b"));
        assert_eq!(builder.field("a"),Some("named a"));
        assert_eq!(builder.events(), &[
            BuildEvent::Initialized("b".into(),Origin::Named),
            BuildEvent::Initialized("a".into(),Origin::Named),
            BuildEvent::Initialized("c".into(),Origin::Default),
        ]);
        builder.cleanup();
        assert_eq!(builder.events().iter().filter(|e| matches!(e, BuildEvent::Released(_)))
            .cloned().collect::<Vec<_>>(),vec![
            BuildEvent::Released("c".into()),
            BuildEvent::Released("a".into()),
            BuildEvent::Released("b".into()),
        ]);
        assert_eq!(builder.outstanding(),0);
    }

    #[test]
    fn vb16_中途預設求值失敗只逆序清理已完成具名與預設() {
        let mut builder=StrStructBuilder::new();
        let result=builder.build(&fields(),&[
            NamedStrArg{name:"b",value:"new b"},
        ],Some("c"));
        assert_eq!(result,Err(BuildError::FailedAt("c".into())));
        assert_eq!(builder.events(),&[
            BuildEvent::Initialized("b".into(),Origin::Named),
            BuildEvent::Initialized("a".into(),Origin::Default),
            BuildEvent::Released("a".into()),
            BuildEvent::Released("b".into()),
        ]);
        assert_eq!(builder.outstanding(),0);
    }

    #[test]
    fn vb16_第一個具名求值失敗不建立也不釋放任何資源() {
        let mut builder=StrStructBuilder::new();
        let result=builder.build(&fields(),&[
            NamedStrArg{name:"b",value:"new b"},
            NamedStrArg{name:"a",value:"new a"},
        ],Some("b"));
        assert_eq!(result,Err(BuildError::FailedAt("b".into())));
        assert!(builder.events().is_empty());
        assert_eq!(builder.outstanding(),0);
    }

    #[test]
    fn vb16_建立前發現重複與未知具名欄位沒有副作用() {
        let mut builder=StrStructBuilder::new();
        let duplicate=builder.build(&fields(),&[
            NamedStrArg{name:"a",value:"first"},
            NamedStrArg{name:"a",value:"again"},
        ],None);
        assert_eq!(duplicate,Err(BuildError::InvalidPlan(
            PlanError::DuplicateField("a".into()))));
        let missing=builder.build(&fields(),&[
            NamedStrArg{name:"unknown",value:"invalid"},
        ],None);
        assert_eq!(missing,Err(BuildError::InvalidPlan(
            PlanError::UnknownField("unknown".into()))));
        assert!(builder.events().is_empty());
    }

    #[test]
    fn vb16_中文字串與未初始化_var_可安全共存並清理已取得資源() {
        let fs=[
            StrFieldDef{name:"late",default:None,immutable:false},
            StrFieldDef{name:"word",default:Some("繁體中文 🪼"),immutable:true},
        ];
        let mut builder=StrStructBuilder::new();
        builder.build(&fs,&[],None).unwrap();
        assert_eq!(builder.field("late"),None);
        assert_eq!(builder.field("word"),Some("繁體中文 🪼"));
        builder.cleanup();
        assert_eq!(builder.events(),&[
            BuildEvent::Initialized("word".into(),Origin::Default),
            BuildEvent::Released("word".into()),
        ]);
    }
}
