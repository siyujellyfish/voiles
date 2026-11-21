use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use voiles_core::EntryProgram;

struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

pub(super) fn build(input: &Path, output: &Path, standalone: bool) -> Result<(), String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")) {
        return Err("原生建置目前支援 Linux x86-64 GNU".into());
    }
    if input.extension().and_then(|s| s.to_str()) != Some("voil") {
        return Err("入口必須使用 .voil 副檔名".into());
    }
    let rust = EntryProgram::prepare(input)?.native_source()?;
    let parent = output.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".voiles-build-{}", std::process::id()));
    fs::create_dir(&temp).map_err(|e| format!("無法建立建置目錄：{e}"))?;
    let stage = Staging(temp);
    let source = stage.0.join("program.rs");
    let binary = stage.0.join("program");
    fs::write(&source, rust.as_bytes()).map_err(|e| e.to_string())?;
    let mut compiler = Command::new("rustc");
    compiler.args(["--edition=2021", "--target", "x86_64-unknown-linux-gnu"]);
    if standalone {
        compiler.args(["-C", "target-feature=+crt-static", "-C", "strip=symbols"]);
    }
    let status = compiler.arg(&source).arg("-o").arg(&binary).status().map_err(|e| format!("無法啟動 rustc：{e}"))?;
    if !status.success() { return Err(format!("原生建置失敗（{status}）")); }
    fs::hard_link(&binary, output).map_err(|e| format!("無法建立輸出 {}：{e}", output.display()))
}
