// P17-F：同一正式私有 Checker 核准的 struct 建立式 → 真混合資源求值。
// 僅一個 struct、一個 main、一次完整建構與 int 常值 return；其他保守拒絕。
// 包含真私有來源 AST、token、宣告／型別 HIR；絕不建立另一套 Parser。

#[derive(Debug)]
pub(crate) enum ConstructorExecutionError {
    Static(Diagnostic),
    Unsupported(&'static str),
    ChangedSource,
    Runtime(crate::mixed_struct_runtime::MixedError),
}

#[derive(Debug, Clone)]
enum CheckedScalar {
    Int(i64),
    Bool(bool),
    Str(String),
}
impl CheckedScalar {
    fn borrowed(&self) -> crate::mixed_struct_runtime::SeedValue<'_> {
        use crate::mixed_struct_runtime::SeedValue;
        match self {
            Self::Int(value) => SeedValue::Int(*value),
            Self::Bool(value) => SeedValue::Bool(*value),
            Self::Str(value) => SeedValue::Str(value),
        }
    }
}

#[derive(Debug)]
struct CheckedFieldSeed {
    name: String,
    kind: crate::mixed_struct_runtime::ScalarKind,
    default: Option<CheckedScalar>,
    immutable: bool,
}
#[derive(Debug)]
struct CheckedNamedSeed {
    field: String,
    value: CheckedScalar,
}

struct PreparedPrivateConstructor {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    instance: String,
    fields: Vec<CheckedFieldSeed>,
    named: Vec<CheckedNamedSeed>,
    return_value: i64,
}

#[derive(Debug)]
struct PrivateConstructorOutcome {
    result: Result<i64, crate::mixed_struct_runtime::MixedError>,
    observed: Vec<(String, String)>,
    actual_events: Vec<crate::owned_str_runtime::ResourceEvent>,
}

include!("execute_constructor_preflight.rs");

impl PreparedPrivateConstructor {
    fn execute(&self) -> Result<PrivateConstructorOutcome, ConstructorExecutionError> {
        self.execute_with_fault(None)
    }

    /// 錯誤注入只屬內部測試；不是 voiles 可用的例外語法。
    fn execute_with_fault(&self, fail_after: Option<&str>)
        -> Result<PrivateConstructorOutcome, ConstructorExecutionError> {
        use crate::mixed_struct_runtime::{
            MixedFieldSpec, MixedStructRuntime, NamedMixed, Observed,
        };
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(ConstructorExecutionError::ChangedSource);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) {
            return Err(ConstructorExecutionError::ChangedSource);
        }
        let specs: Vec<MixedFieldSpec<'_>> = self.fields.iter().map(|field|
            MixedFieldSpec {
                name: &field.name,
                kind: field.kind,
                default: field.default.as_ref().map(CheckedScalar::borrowed),
                immutable: field.immutable,
            }
        ).collect();
        let named: Vec<NamedMixed<'_>> = self.named.iter().map(|arg| NamedMixed {
            field: &arg.field,
            value: arg.value.borrowed(),
        }).collect();
        let mut runtime = MixedStructRuntime::new();
        let built = runtime.construct(&self.instance, &specs, &named, fail_after);
        let mut observed = Vec::new();
        let result = match built {
            Ok(()) => {
                for field in &self.fields {
                    let visible = match runtime.field(&self.instance, &field.name)
                        .map_err(ConstructorExecutionError::Runtime)? {
                        Observed::Int(value) => format!("int:{value}"),
                        Observed::Bool(value) => format!("bool:{value}"),
                        Observed::Str(value) => format!("str:{value}"),
                    };
                    observed.push((field.name.clone(), visible));
                }
                Ok(self.return_value)
            }
            Err(error) => Err(error),
        };
        // 原 P16 Runtime 釋放真正持有的所有資源。失敗時建構期間
        // 未提交的資源也須由原 Runtime 自己逆序清理後才能取得事件。
        let actual_events = runtime.finish().map_err(ConstructorExecutionError::Runtime)?;
        if !crate::owned_str_runtime::event_trace_balanced(&actual_events) {
            return Err(ConstructorExecutionError::Unsupported("真結構資源事件未單次平衡"));
        }
        Ok(PrivateConstructorOutcome { result, observed, actual_events })
    }
}

#[cfg(test)]
include!("execute_constructor_tests.rs");
