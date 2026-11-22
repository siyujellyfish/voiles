//! 公開命令共用 P21 已檢查的模組與執行計畫。
use crate::execution::registry::RegistryFailure;
use crate::struct_stage::modules::{LinkedProgram, ModuleError};
use crate::Diagnostic;
use crate::owned_str_runtime::StrRuntimeError;
use std::path::Path;

pub struct EntryProgram(LinkedProgram);

impl EntryProgram {
    pub fn prepare(path: &Path) -> Result<Self, String> {
        LinkedProgram::prepare(path).map(Self).map_err(module_error)
    }

    pub fn bootstrap_digits(&self) -> Result<String, String> {
        self.0.bootstrap_digits().map_err(module_error)
    }

    pub fn native_source(&self) -> Result<String, String> {
        self.0.native_source().map_err(module_error)
    }

    pub fn execute(&self) -> Result<i64, String> {
        let (result, _) = self.0.execute().map_err(module_error)?;
        result.map_err(module_error)
    }
}

fn source_error(path: &Path, diagnostic: Diagnostic) -> String {
    let position = diagnostic.span.map(|span| format!(":{}:{}", span.start.line, span.start.column))
        .unwrap_or_default();
    format!("{}{position}: {}: {}", path.display(), diagnostic.kind, diagnostic.message)
}

fn file_error(error: RegistryFailure) -> String {
    match error {
        RegistryFailure::Io { path, kind } => format!("{}: 檔案讀取錯誤（{kind:?}）", path.display()),
        RegistryFailure::DuplicateSource { path, canonical } =>
            format!("{}: 重複來源（{}）", path.display(), canonical.display()),
        RegistryFailure::Static { path, diagnostic } => source_error(&path, diagnostic),
        RegistryFailure::SourceChanged { path } => format!("{}: 來源在檢查後已變更", path.display()),
    }
}

fn module_error(error: ModuleError) -> String {
    match error {
        ModuleError::Runtime(error) => {
            format!("執行期錯誤：{}", runtime_message(error))
        }
        ModuleError::RuntimeAt { path, span, error } => format!("{}:{}:{}: 執行期錯誤：{}",
            path.display(), span.start.line, span.start.column, runtime_message(error)),
        ModuleError::File(error) => file_error(error),
        ModuleError::Source { path, diagnostic } => source_error(&path, diagnostic),
        ModuleError::Import { path, span, cause } => format!("{}:{}:{}: 匯入失敗：{}",
            path.display(), span.start.line, span.start.column, file_error(cause)),
        ModuleError::Cycle { path, span, chain } => {
            let chain = chain.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" → ");
            format!("{}:{}:{}: 循環匯入：{chain}", path.display(), span.start.line, span.start.column)
        }
    }
}

fn runtime_message(error: StrRuntimeError) -> &'static str {
    match error {
        StrRuntimeError::MissingOwner => "找不到資源持有者",
        StrRuntimeError::DuplicateBinding => "重複綁定",
        StrRuntimeError::ImmutableBinding => "不可修改唯讀綁定",
        StrRuntimeError::MovedSource => "來源已移動",
        StrRuntimeError::NoInnerScope => "缺少內層作用域",
        StrRuntimeError::IncompleteReturn => "函式未完成回傳",
        StrRuntimeError::EvaluationFailed => "運算失敗",
        StrRuntimeError::ExecutionBudgetExhausted => "執行步數已耗盡",
        StrRuntimeError::ExhaustedIdentity => "資源識別碼已耗盡",
        StrRuntimeError::OutstandingResource => "仍有資源尚待釋放",
    }
}
