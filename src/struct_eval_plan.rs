//! VB16-01-A：結構建立求值順序的獨立、確定性計畫。
//! 先依來源書寫順序求值具名引數，之後才按欄位宣告順序
//! 求值未被覆蓋的預設值；D08 不求值已覆蓋預設值。
//! 計畫階段不消耗持有值，不假設執行期析構順序。
#![allow(dead_code)]

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldInfo<'a> {
    pub name: &'a str,
    pub has_default: bool,
    pub immutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Named,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvalStep {
    pub field: String,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvalPlan {
    pub steps: Vec<EvalStep>,
    pub missing_mutable_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlanError {
    DuplicateField(String),
    UnknownField(String),
    MissingConstDefault(String),
    DuplicateDefinition(String),
}

pub(crate) fn plan(fields: &[FieldInfo<'_>], named: &[&str])
    -> Result<EvalPlan, PlanError> {
    let mut declarations = HashSet::new();
    for field in fields {
        if !declarations.insert(field.name) {
            return Err(PlanError::DuplicateDefinition(field.name.to_owned()));
        }
        if field.immutable && !field.has_default {
            return Err(PlanError::MissingConstDefault(field.name.to_owned()));
        }
    }
    let mut supplied = HashSet::new();
    let mut steps = Vec::new();
    // 先對全部具名欄位預檢，不得在未知或重複欄位上消耗資源。
    for name in named {
        if !declarations.contains(name) {
            return Err(PlanError::UnknownField((*name).to_owned()));
        }
        if !supplied.insert(*name) {
            return Err(PlanError::DuplicateField((*name).to_owned()));
        }
        steps.push(EvalStep {field: (*name).to_owned(), origin: Origin::Named});
    }
    let mut missing_mutable_fields = Vec::new();
    for field in fields {
        if supplied.contains(field.name) { continue; }
        if field.has_default {
            steps.push(EvalStep {field: field.name.to_owned(), origin: Origin::Default});
        } else {
            missing_mutable_fields.push(field.name.to_owned());
        }
    }
    Ok(EvalPlan {steps, missing_mutable_fields})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vb16_先依來源具名參數順序再依欄位宣告順序評估未覆蓋預設() {
        let fields=[
            FieldInfo{name:"a",has_default:true,immutable:false},
            FieldInfo{name:"b",has_default:true,immutable:false},
            FieldInfo{name:"c",has_default:true,immutable:false},
            FieldInfo{name:"d",has_default:true,immutable:true},
        ];
        let p=plan(&fields,&["c","a"]).unwrap();
        assert_eq!(p.steps, vec![
            EvalStep{field:"c".into(),origin:Origin::Named},
            EvalStep{field:"a".into(),origin:Origin::Named},
            EvalStep{field:"b".into(),origin:Origin::Default},
            EvalStep{field:"d".into(),origin:Origin::Default},
        ]);
    }

    #[test]
    fn vb16_已具名覆寫欄位不評估其預設() {
        let fields=[FieldInfo{name:"value",has_default:true,immutable:false}];
        assert_eq!(plan(&fields,&["value"]).unwrap().steps,
            vec![EvalStep{field:"value".into(),origin:Origin::Named}]);
    }

    #[test]
    fn vb16_缺失的_var_欄位留下未初始化證明不偽造資源() {
        let fields=[
            FieldInfo{name:"v",has_default:false,immutable:false},
            FieldInfo{name:"id",has_default:true,immutable:true},
        ];
        let p=plan(&fields,&[]).unwrap();
        assert_eq!(p.missing_mutable_fields,vec!["v"]);
        assert_eq!(p.steps,vec![EvalStep{field:"id".into(),origin:Origin::Default}]);
    }

    #[test]
    fn vb16_錯誤具名引數在任何求值開始前拒絕() {
        let fields=[FieldInfo{name:"name",has_default:true,immutable:false}];
        assert_eq!(plan(&fields,&["name","name"]),Err(PlanError::DuplicateField("name".into())));
        assert_eq!(plan(&fields,&["missing"]),Err(PlanError::UnknownField("missing".into())));
    }

    #[test]
    fn vb16_const_欄位未有宣告時預設違反_f01() {
        let fields=[FieldInfo{name:"id",has_default:false,immutable:true}];
        assert_eq!(plan(&fields,&["id"]),Err(PlanError::MissingConstDefault("id".into())));
    }

    #[test]
    fn vb16_所有具名欄位排列均維持來源先行與預設宣告順序() {
        let fields=[
            FieldInfo{name:"a",has_default:true,immutable:false},
            FieldInfo{name:"b",has_default:true,immutable:false},
            FieldInfo{name:"c",has_default:true,immutable:false},
        ];
        for supplied in [
            ["a","b"],["b","a"],["a","c"],["c","a"],["b","c"],["c","b"]
        ] {
            let result=plan(&fields,&supplied).unwrap();
            assert_eq!(result.steps[0].field,supplied[0]);
            assert_eq!(result.steps[1].field,supplied[1]);
            assert_eq!(result.steps[0].origin,Origin::Named);
            assert_eq!(result.steps[1].origin,Origin::Named);
            let omitted=fields.iter().find(|f| !supplied.contains(&f.name)).unwrap();
            assert_eq!(result.steps[2],EvalStep{
                field:omitted.name.into(),origin:Origin::Default
            });
        }
    }

    #[test]
    fn vb16_重複宣告欄位與缺少_const_預設不產生求值計畫() {
        let duplicate=[
            FieldInfo{name:"a",has_default:true,immutable:false},
            FieldInfo{name:"a",has_default:true,immutable:false},
        ];
        assert_eq!(plan(&duplicate,&[]),Err(PlanError::DuplicateDefinition("a".into())));
        let missing=[
            FieldInfo{name:"id",has_default:false,immutable:true},
            FieldInfo{name:"value",has_default:true,immutable:false},
        ];
        assert_eq!(plan(&missing,&["value"]),Err(PlanError::MissingConstDefault("id".into())));
    }
}
