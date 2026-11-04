//! P21-A：多檔來源的非公開身分與驗證預檢。
 //! 本階段沒有匯入／模組求值語法；每個檔案仍各自經唯一 P01 Checker。
 //! 不把登錄成功當成可跨檔呼叫、共享資源或執行權限。

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::PreparedProgram;
use crate::Diagnostic;

#[derive(Debug)]
pub(crate) enum RegistryFailure {
    Io { path: PathBuf, kind: ErrorKind },
    DuplicateSource { path: PathBuf, canonical: PathBuf },
    Static { path: PathBuf, diagnostic: Diagnostic },
    SourceChanged { path: PathBuf },
}

pub(crate) struct VerifiedSourceFile {
    /// 呼叫者提供的原始檔案路徑，讓診斷不丟失 Unicode 或路徑別名。
    pub(crate) requested_path: PathBuf,
    /// 作業系統解出的同一真實檔案身分，不能以名稱字串比較代替。
    pub(crate) canonical_path: PathBuf,
    /// Unix 裝置與 inode：硬連結也必須識別為同一實體來源。
    /// 非 Unix 平台只維持原 canonical PathBuf 的保守策略。
    physical_identity: Option<(u64, u64)>,
    /// 每檔只持有其自己經原 Checker 核准的來源與 HIR，不能跨檔合併。
    pub(crate) prepared: PreparedProgram,
}

pub(crate) struct VerifiedSourceRegistry {
    pub(crate) files: Vec<VerifiedSourceFile>,
}

fn file_io(path: &Path, error: std::io::Error) -> RegistryFailure {
    RegistryFailure::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

/// OS 實際檔案身分與文字路徑分開；Linux/Unix 硬連結會共享 inode。
/// 不把此資訊解讀為 voiles 模組宣告名稱、載入順序或所有權。
fn physical_identity(path: &Path) -> Result<Option<(u64, u64)>, RegistryFailure> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path).map_err(|error| file_io(path, error))?;
        Ok(Some((meta.dev(), meta.ino())))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(None)
    }
}

impl VerifiedSourceRegistry {
    /// 從呼叫者明示的真路徑建立來源登錄，不載入未知 import，不執行初始化。
    /// 任一來源有 I/O、重複身分或 Checker 錯誤時，不回傳部分已驗證清單。
    pub(crate) fn prepare_paths(paths: &[PathBuf]) -> Result<Self, RegistryFailure> {
        let mut seen = BTreeSet::<PathBuf>::new();
        let mut seen_physical = BTreeSet::<(u64, u64)>::new();
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            let canonical = std::fs::canonicalize(path)
                .map_err(|error| file_io(path, error))?;
            let physical = physical_identity(path)?;
            if !seen.insert(canonical.clone())
                || physical.is_some_and(|identity| !seen_physical.insert(identity)) {
                return Err(RegistryFailure::DuplicateSource {
                    path: path.clone(),
                    canonical,
                });
            }
            let original_bytes = std::fs::read(path)
                .map_err(|error| file_io(path, error))?;
            let prepared = PreparedProgram::prepare(original_bytes)
                .map_err(|error| RegistryFailure::Static {
                    path: path.clone(),
                    diagnostic: error.0,
                })?;
            files.push(VerifiedSourceFile {
                requested_path: path.clone(), canonical_path: canonical,
                physical_identity: physical, prepared,
            });
        }
        let result = Self { files };
        // 完成所有 Checker 後，再查一次內容及物理身分；
        // 避免前一檔在讀取後續檔案期間已遭替換。
        result.check_unchanged()?;
        Ok(result)
    }

    /// 後續需要真正模組解析之前的來源快照防偽：同一路徑別名不得
    /// 改指另一個檔案，即使內容相同；檔案位元組變更亦不沿用舊 HIR。
    /// 此方法不執行任何 voiles 程式。
    pub(crate) fn check_unchanged(&self) -> Result<(), RegistryFailure> {
        for entry in &self.files {
            let path = &entry.requested_path;
            let canonical = std::fs::canonicalize(path)
                .map_err(|error| file_io(path, error))?;
            if canonical != entry.canonical_path
                || physical_identity(path)? != entry.physical_identity {
                return Err(RegistryFailure::SourceChanged { path: path.clone() });
            }
            let bytes = std::fs::read(path)
                .map_err(|error| file_io(path, error))?;
            if bytes.as_slice() != entry.prepared.original_bytes.as_ref() {
                return Err(RegistryFailure::SourceChanged { path: path.clone() });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticKind, execution::Value};

    fn with_directory<T>(label: &str, verify: impl FnOnce(&Path) -> T) -> T {
        let root = std::env::temp_dir().join(format!(
            "voiles_p21_{}_{}", std::process::id(), label,
        ));
        std::fs::create_dir(&root).expect("必須建立本次獨立真實測試目錄");
        let result = verify(&root);
        std::fs::remove_dir_all(&root).expect("必須清理測試目錄");
        result
    }

    fn write(path: &Path, content: &[u8]) {
        std::fs::write(path, content).expect("真實檔案寫入成功");
    }

    #[test]
    fn p21_a_兩份相同來源內容仍有各自真路徑與獨立_checker_身分() {
        with_directory("two_independent", |dir| {
            let a = dir.join("甲.voil");
            let b = dir.join("乙.voil");
            let code = b"fn main()->int{\nreturn 2+3\n}\n";
            write(&a, code);
            write(&b, code);
            let registry = VerifiedSourceRegistry::prepare_paths(&[a.clone(), b.clone()]).unwrap();
            assert_eq!(registry.files.len(), 2);
            assert_ne!(registry.files[0].canonical_path, registry.files[1].canonical_path);
            assert_eq!(registry.files[0].requested_path, a);
            assert_eq!(registry.files[1].requested_path, b);
            registry.check_unchanged().unwrap();
            // 這是兩份單檔計畫，各自獨立求值；不是跨模組呼叫。
            for entry in &registry.files {
                assert_eq!(entry.prepared.execute(), Ok(Value::Int(5)));
            }
        });
    }

    #[test]
    fn p21_a_同檔路徑別名不得登錄兩份相互冒充的來源權威() {
        with_directory("duplicate_alias", |dir| {
            let actual = dir.join("原始.voil");
            write(&actual, b"fn main()->int{\nreturn 1\n}\n");
            let alias = dir.join(".").join("原始.voil");
            let error = VerifiedSourceRegistry::prepare_paths(
                &[actual.clone(), alias.clone()],
            ).err().expect("應拒絕重複來源");
            match error {
                RegistryFailure::DuplicateSource { path, canonical } => {
                    assert_eq!(path, alias);
                    assert_eq!(canonical, std::fs::canonicalize(actual).unwrap());
                }
                other => panic!("必須保持重複來源的獨立錯誤分類：{other:?}"),
            }
        });
    }

    #[test]
    fn p21_a_第二檔不存在時不得回傳已驗證第一檔的部分登錄() {
        with_directory("missing_second", |dir| {
            let first = dir.join("首檔.voil");
            let missing = dir.join("未找到.voil");
            write(&first, b"fn main()->int{\nreturn 1\n}\n");
            let error = VerifiedSourceRegistry::prepare_paths(
                &[first, missing.clone()],
            ).err().expect("缺檔不應建立完整登錄");
            assert!(matches!(error, RegistryFailure::Io {
                path, kind: ErrorKind::NotFound,
            } if path == missing));
        });
    }

    #[test]
    fn p21_a_第二檔非法_utf8_保留原請求路徑與詞彙診斷() {
        with_directory("invalid_utf8", |dir| {
            let first = dir.join("首檔.voil");
            let invalid = dir.join("壞掉🪼.voil");
            write(&first, b"fn main()->int{\nreturn 1\n}\n");
            write(&invalid, b"fn main()->int{\nreturn \xff\n}\n");
            let error = VerifiedSourceRegistry::prepare_paths(
                &[first, invalid.clone()],
            ).err().expect("非法 UTF-8 必須拒絕整份登錄");
            match error {
                RegistryFailure::Static { path, diagnostic } => {
                    assert_eq!(path, invalid);
                    assert_eq!(diagnostic.kind, DiagnosticKind::Lexical);
                    assert_eq!(diagnostic.span.unwrap().start.line, 2);
                }
                other => panic!("不得把原始位元組錯誤偽裝為其他類別：{other:?}"),
            }
        });
    }

    #[test]
    fn p21_a_來源檢查通過後真檔案更換位元組不得沿用已驗證計畫() {
        with_directory("file_changed", |dir| {
            let file = dir.join("版本.voil");
            write(&file, b"fn main()->int{\nreturn 1\n}\n");
            let registry = VerifiedSourceRegistry::prepare_paths(&[file.clone()]).unwrap();
            registry.check_unchanged().unwrap();
            write(&file, b"fn main()->int{\nreturn 9\n}\n");
            assert!(matches!(registry.check_unchanged(),
                Err(RegistryFailure::SourceChanged { path }) if path == file));
        });
    }

    #[test]
    fn p21_a_未知跨檔_import_語法必須保留原單檔_checker_拒絕() {
        with_directory("no_import_semantics", |dir| {
            let file = dir.join("不核准匯入.voil");
            write(&file, b"import hidden\nfn main()->int{\nreturn 0\n}\n");
            assert!(matches!(
                VerifiedSourceRegistry::prepare_paths(&[file.clone()]),
                Err(RegistryFailure::Static { path, .. }) if path == file,
            ));
        });
    }


    #[cfg(unix)]
    #[test]
    fn p21_a_unix_硬連結不同正規路徑仍須判定同一實體來源() {
        with_directory("hardlink_duplicate", |dir| {
            let primary = dir.join("原檔.voil");
            let hardlink = dir.join("硬連結.voil");
            write(&primary, b"fn main()->int{\nreturn 2\n}\n");
            std::fs::hard_link(&primary, &hardlink).unwrap();
            assert_ne!(
                std::fs::canonicalize(&primary).unwrap(),
                std::fs::canonicalize(&hardlink).unwrap(),
            );
            let error = VerifiedSourceRegistry::prepare_paths(
                &[primary, hardlink.clone()],
            ).err().expect("兩個路徑指向同一 inode 必須拒絕");
            assert!(matches!(
                error, RegistryFailure::DuplicateSource { path, .. } if path == hardlink,
            ));
        });
    }

    #[cfg(unix)]
    #[test]
    fn p21_a_unix_檔案被相同位元組的另一_inode_取代仍拒絕舊證據() {
        with_directory("replaced_inode", |dir| {
            let original = dir.join("實際.voil");
            let replacement = dir.join("候補.voil");
            let bytes = b"fn main()->int{\nreturn 2\n}\n";
            write(&original, bytes);
            let registry = VerifiedSourceRegistry::prepare_paths(&[original.clone()]).unwrap();
            write(&replacement, bytes);
            // Unix rename 覆寫舊路徑，內容相等但物理檔案身分不同。
            std::fs::rename(&replacement, &original).unwrap();
            assert_eq!(std::fs::read(&original).unwrap(), bytes);
            assert!(matches!(registry.check_unchanged(),
                Err(RegistryFailure::SourceChanged { path }) if path == original));
        });
    }

    #[cfg(unix)]
    #[test]
    fn p21_a_符號連結改指其他同內容來源仍不可沿用原檔案身分() {
        use std::os::unix::fs::symlink;
        with_directory("retarget_alias", |dir| {
            let a = dir.join("原本.voil");
            let b = dir.join("其他.voil");
            let alias = dir.join("捷徑.voil");
            let code = b"fn main()->int{\nreturn 1\n}\n";
            write(&a, code);
            write(&b, code);
            symlink(&a, &alias).unwrap();
            let registry = VerifiedSourceRegistry::prepare_paths(&[alias.clone()]).unwrap();
            std::fs::remove_file(&alias).unwrap();
            symlink(&b, &alias).unwrap();
            assert!(matches!(registry.check_unchanged(),
                Err(RegistryFailure::SourceChanged { path }) if path == alias));
        });
    }
}
