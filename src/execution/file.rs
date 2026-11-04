//! P17-D：私有檔案執行入口，保留 I/O、靜態與執行期三類獨立錯誤。
//! 此介面只供本 crate 的內部測試；不改動公開 voiles 命令與退出狀態。

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::{Diagnostic, SourceSpan};
use super::{PreparedProgram, Value};

#[derive(Debug)]
pub(crate) enum FileExecutionError {
    Io { path: PathBuf, kind: ErrorKind },
    Static { path: PathBuf, diagnostic: Diagnostic },
    Runtime { path: PathBuf, span: Option<SourceSpan>, reason: &'static str },
}

/// 一次讀取原始位元組，再透過唯一真 Checker → evaluator 交接；
/// 失敗不以 `Passed` 代替型別與來源身分證明。
pub(crate) fn run_checked_file(path: &Path) -> Result<Value, FileExecutionError> {
    let bytes = std::fs::read(path).map_err(|error| FileExecutionError::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    })?;
    let verified = PreparedProgram::prepare(bytes).map_err(|error|
        FileExecutionError::Static {
            path: path.to_path_buf(),
            diagnostic: error.0,
        }
    )?;
    verified.execute().map_err(|error| FileExecutionError::Runtime {
        path: path.to_path_buf(),
        span: verified.runtime_span(&error),
        reason: error.reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticKind;
    use std::io::ErrorKind;

    /// 檔案由測試自身以真正 UTF-8 路徑／位元組建立與移除。
    fn with_file<T>(label: &str, bytes: &[u8], verify: impl FnOnce(&Path) -> T) -> T {
        let path = std::env::temp_dir().join(format!(
            "voiles_p17_{}_{}.voil", std::process::id(), label,
        ));
        std::fs::write(&path, bytes).expect("必須成功建立真正來源檔案");
        let result = verify(&path);
        std::fs::remove_file(&path).expect("必須清理真正來源檔案");
        result
    }

    #[test]
    fn p17_真實中文檔名與_utf8_來源先驗證再計算() {
        with_file("來源驗證", b"fn main()->int{\nreturn 2+3\n}\n", |path| {
            assert_eq!(run_checked_file(path).unwrap(), Value::Int(5));
        });
    }

    #[test]
    fn p17_真實檔案_io_缺檔不偽造語言診斷() {
        let missing = std::env::temp_dir().join(format!(
            "voiles_p17_missing_{}.voil", std::process::id(),
        ));
        match run_checked_file(&missing).unwrap_err() {
            FileExecutionError::Io { path, kind } => {
                assert_eq!(path, missing);
                assert_eq!(kind, ErrorKind::NotFound);
            }
            other => panic!("真正 IO 錯誤不得偽裝為靜態或動態錯誤：{other:?}"),
        }
    }

    #[test]
    fn p17_真實非法_utf8_禁止進入執行器且保留詞彙行列() {
        with_file("非法位元組", b"fn main()->int{\nreturn \xff\n}\n", |path| {
            match run_checked_file(path).unwrap_err() {
                FileExecutionError::Static { path: actual, diagnostic } => {
                    assert_eq!(actual, path);
                    assert_eq!(diagnostic.kind, DiagnosticKind::Lexical);
                    assert_eq!(diagnostic.span.unwrap().start.line, 2);
                }
                other => panic!("非法原始來源必須保留真詞彙錯誤：{other:?}"),
            }
        });
    }

    #[test]
    fn p17_真實未知名稱與未初始化屬靜態錯誤而非動態() {
        for (label, code) in [
            ("未知名稱", "fn main()->int{\nreturn absent\n}\n"),
            ("未初始化", "fn main()->int{\nvar n:int\nreturn n\n}\n"),
        ] {
            with_file(label, code.as_bytes(), |path| {
                match run_checked_file(path).unwrap_err() {
                    FileExecutionError::Static { path: actual, diagnostic } => {
                        assert_eq!(actual, path);
                        assert_eq!(diagnostic.kind, DiagnosticKind::Static);
                    }
                    other => panic!("Checker 失敗不得進到 runtime：{other:?}"),
                }
            });
        }
    }

    #[test]
    fn p17_真實運算溢位獨立回報來源行與檔案身分() {
        with_file("溢位", b"fn main()->int{\nvar n:int = 9223372036854775807\nreturn n+1\n}\n", |path| {
            match run_checked_file(path).unwrap_err() {
                FileExecutionError::Runtime { path: actual, span, reason } => {
                    assert_eq!(actual, path);
                    assert_eq!(span.unwrap().start.line, 3);
                    assert_eq!(reason, "執行時有號 64 位元整數溢位");
                }
                other => panic!("真正動態溢位不可回報為其他類別：{other:?}"),
            }
        });
    }
}
