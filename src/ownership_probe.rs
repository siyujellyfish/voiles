//! P13 專用的內部實際 .voil 來源靜態檢查試驗。
//! 不輸出公開 API、CLI 模式，也不修改已驗收 P01 的語法集合。
//! 僅接受 int/bool 的具名局部整筆 .move()；全域移動、
//! struct／欄位、一般 borrow／share 與執行時安全仍不支援。
#![allow(dead_code)]

use std::collections::HashMap;
use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind};
use crate::ownership_ledger::DeclarationId;
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot};
use crate::parser::{Binding, Block, File, Item, StmtKind};
use crate::source::Source;

#[derive(Clone, Copy)]
struct OwnershipBinding {
    id: DeclarationId,
    slot: ValueSlot,
}

#[derive(Clone)]
struct FlowState {
    scopes: Vec<HashMap<String, OwnershipBinding>>,
    reachable: bool,
}

struct OwnershipPass<'a> {
    source: &'a Source,
}

impl OwnershipPass<'_> {
    fn diagnostic(&self, message: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: message.into(),
            span: Some(self.source.span(start, end)),
        }
    }

    fn model_error(&self, error: ModelError, expr: &Expr) -> Diagnostic {
        let text = match error {
            ModelError::MovedRead => "來源已明確移動，不可再次使用或移動",
            ModelError::UninitializedRead => "來源尚未初始化，不能讀取或移動",
            ModelError::UncertainRead => "至少一條可達分支可能移動來源，不能安全使用",
            ModelError::CopyForbidden => "來源型別不具備合法複製能力",
            ModelError::CopyUndecided => "來源型別的複製能力尚未獲核准",
            ModelError::CapabilityConflict => "合流時的複製能力不一致",
            ModelError::NoReachableBranch => "不存在可達的分支狀態",
        };
        self.diagnostic(text, expr.start, expr.end)
    }

    fn binding<'s>(&self, state: &'s FlowState, name: &str, expr: &Expr)
        -> Result<(usize, OwnershipBinding), Diagnostic> {
        for (depth, scope) in state.scopes.iter().enumerate().rev() {
            if let Some(binding) = scope.get(name) {
                return Ok((depth, *binding));
            }
        }
        Err(self.diagnostic("找不到可以綁定的來源宣告", expr.start, expr.end))
    }

    /// 一般使用是複製；唯有明確 .move() 會使具名局部來源失效。
    /// int/bool 的複製資格只用於本 P13 試驗，並未授權其他型別。
    fn expression(&self, expr: &Expr, state: &mut FlowState) -> Result<(), Diagnostic> {
        match &expr.kind {
            ExprKind::Name(name) => {
                let (_, binding) = self.binding(state, name, expr)?;
                binding.slot.duplicate()
                    .map_err(|e| self.model_error(e, expr))?;
                Ok(())
            }
            ExprKind::ZeroArgMethod(inner, name) if name == "move" => {
                let ExprKind::Name(variable) = &inner.kind else {
                    return Err(self.diagnostic(
                        "明確移動只接受已綁定的具名來源", expr.start, expr.end
                    ));
                };
                let (depth, original) = self.binding(state, variable, inner)?;
                if depth == 0 {
                    return Err(self.diagnostic(
                        "P13 試驗尚未支援跨函式的頂層來源移動", expr.start, expr.end
                    ));
                }
                // 同名遮蔽依最內層真正宣告身分，不能因字串同名傷及外層。
                let binding = state.scopes[depth].get_mut(variable).unwrap();
                assert_eq!(binding.id, original.id);
                binding.slot.move_whole().map_err(|e| self.model_error(e, expr))?;
                Ok(())
            }
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) |
            ExprKind::ZeroArgCall(_) => Ok(()),
            ExprKind::Group(inner) => self.expression(inner, state),
            ExprKind::Add(left, right) | ExprKind::Subtract(left, right) => {
                self.expression(left, state)?;
                self.expression(right, state)
            }
            _ => Err(self.diagnostic(
                "P13 試驗不支援此類資源運算式", expr.start, expr.end
            )),
        }
    }

    fn binding_declaration(&self, binding: &Binding, state: &mut FlowState)
        -> Result<(), Diagnostic> {
        if let Some(initializer) = &binding.initializer {
            self.expression(initializer, state)?;
        }
        let scoped = OwnershipBinding {
            id: DeclarationId(binding.start),
            slot: if binding.initializer.is_some() {
                ValueSlot::initialized(CopyPermission::Allowed)
            } else {
                ValueSlot::uninitialized(CopyPermission::Allowed)
            },
        };
        state.scopes.last_mut().expect("需有作用域").insert(binding.name.clone(), scoped);
        Ok(())
    }

    fn block(&self, block: &Block, state: &mut FlowState) -> Result<(), Diagnostic> {
        state.scopes.push(HashMap::new());
        for statement in &block.statements {
            if !state.reachable { break; }
            match &statement.kind {
                StmtKind::Binding(b) => self.binding_declaration(b, state)?,
                StmtKind::Assignment { name, value } => {
                    self.expression(value, state)?;
                    // 可變權限與型別一致性已由 P10/P11 先行檢查；
                    // 只有合法 RHS 求值後才把原宣告標為重新初始化。
                    let mut found = false;
                    for scope in state.scopes.iter_mut().rev() {
                        if let Some(binding) = scope.get_mut(name) {
                            binding.slot.mark_reinitialized();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(self.diagnostic("找不到可重新初始化的宣告",
                            statement.start, statement.end));
                    }
                }
                StmtKind::Return(expr) => {
                    self.expression(expr, state)?;
                    state.reachable = false;
                }
                StmtKind::IfChain { branches, otherwise } => {
                    let entry = state.clone();
                    let mut exits = Vec::new();
                    for (condition, body) in branches {
                        let mut branch = entry.clone();
                        self.expression(condition, &mut branch)?;
                        self.block(body, &mut branch)?;
                        if branch.reachable { exits.push(branch); }
                    }
                    if let Some(other) = otherwise {
                        let mut branch = entry.clone();
                        self.block(other, &mut branch)?;
                        if branch.reachable { exits.push(branch); }
                    } else {
                        exits.push(entry);
                    }
                    if exits.is_empty() {
                        state.reachable = false;
                    } else {
                        for (level, scope) in state.scopes.iter_mut().enumerate() {
                            for (key, item) in scope.iter_mut() {
                                let candidate = exits.iter().map(|branch| {
                                    let matching = branch.scopes.get(level)
                                        .and_then(|s| s.get(key));
                                    if let Some(next) = matching {
                                        if next.id == item.id { Some(next.slot) }
                                        else { None }
                                    } else { None }
                                }).collect::<Option<Vec<_>>>()
                                    .ok_or_else(|| self.diagnostic(
                                        "分支合流無法確認原宣告的來源身分",
                                        statement.start, statement.end
                                    ))?;
                                item.slot = ValueSlot::join_reachable(
                                    &candidate.into_iter().map(Some).collect::<Vec<_>>()
                                ).map_err(|e| self.diagnostic(
                                    &format!("無法安全合流所有權狀態：{e:?}"),
                                    statement.start, statement.end
                                ))?;
                            }
                        }
                    }
                }
            }
        }
        state.scopes.pop();
        Ok(())
    }

    fn check(&self, file: &File) -> Result<(), Diagnostic> {
        let mut global = FlowState {
            scopes: vec![HashMap::new()],
            reachable: true,
        };
        // 仍沿 P09 全域宣告先後順序；禁止全域來源的移動，不模擬函式副作用。
        for item in &file.items {
            if let Item::Binding(b) = item {
                self.binding_declaration(b, &mut global)?;
            }
        }
        for item in &file.items {
            if let Item::Function(f) = item {
                let mut local = global.clone();
                self.block(&f.body, &mut local)?;
            }
        }
        Ok(())
    }
}

/// 內部試驗的真實 .voil 來源驗證；不傳出公共語法或改變 check_file。
pub(crate) fn inspect_source(text: &str) -> Result<(), Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(e) = source.lexical_diagnostics().into_iter().next() { return Err(e); }
    let scanned = crate::lexer::scan(&source);
    if let Some(e) = scanned.diagnostics.into_iter().next() { return Err(e); }
    let file = crate::parser::parse_move_probe(&source, &scanned.tokens)?;
    crate::names::resolve(&file, &source)?;
    crate::types::check_move_probe(&file, &source)?;
    crate::flow::check(&file, &source)?;
    crate::functions::check(&file, &source)?;
    OwnershipPass { source: &source }.check(&file)
}

#[cfg(test)]
mod tests {
    use super::inspect_source;
    use crate::{CheckStatus, DiagnosticKind};
    use std::path::Path;

    fn valid(text: &str) {
        assert!(inspect_source(text).is_ok(), "應完成 P13 試驗靜態證明：{text}");
    }

    fn unsafe_source(text: &str, contains: &str) {
        let err = inspect_source(text).expect_err("P13 試驗應靜態拒絕");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.span.is_some());
        assert!(err.message.contains(contains), "錯誤診斷：{}", err.message);
    }

    #[test]
    fn 真正_voil_具名整體_move_來源在後續被禁止使用() {
        unsafe_source(
            "fn main()->int{\nvar a:int = 3\nvar b:int = a.move()\nreturn a\n}\n",
            "已明確移動"
        );
    }

    #[test]
    fn 真正_voil_移動目的有效而變數可合法重初始化() {
        valid("fn main()->int{\nvar a:int = 3\nvar b:int = a.move()\na = 5\nreturn a + b\n}\n");
    }

    #[test]
    fn 一般名稱存取複製後再移動_原複本仍有效() {
        valid("fn main()->int{\nvar a:int = 3\nvar b:int = a\nvar c:int = a.move()\nreturn b + c\n}\n");
    }

    #[test]
    fn 同名遮蔽移動不會讓原外層來源失效() {
        valid("fn main()->int{\nvar a:int = 3\nif (true){\nvar a:int = 4\nvar b:int = a.move()\n}\nreturn a\n}\n");
    }

    #[test]
    fn 分支有一路徑移動_後面不可再讀() {
        unsafe_source(
            "fn main()->int{\nvar a:int = 3\nif (true){\nvar b:int = a.move()\n}\nreturn a\n}\n",
            "可達分支"
        );
    }

    #[test]
    fn 分支提前返回不污染仍可達的來源() {
        valid("fn main()->int{\nvar a:int = 3\nif (true){\nvar b:int = a.move()\nreturn b\n}else{\nvar c:int = a\n}\nreturn a\n}\n");
    }

    #[test]
    fn 再次移動同一來源必須被拒絕() {
        unsafe_source(
            "fn main()->int{\nvar a:int = 3\nvar b:int = a.move()\nvar c:int = a.move()\nreturn b\n}\n",
            "已明確移動"
        );
    }

    #[test]
    fn 移動來源不可是未初始化或全域變數() {
        unsafe_source(
            "fn main()->int{\nvar a:int\nvar b:int = a.move()\nreturn 0\n}\n",
            "初始化"
        );
        unsafe_source(
            "var a:int = 3\nfn main()->int{\nreturn a.move()\n}\n",
            "頂層來源"
        );
    }

    #[test]
    fn 完整語言其他未核准來源形式仍必須拒絕() {
        for text in [
            "fn main()->int{\nvar a:int = 3\nreturn (a).move()\n}\n",
            "fn main()->int{\nvar a:int = 3\nreturn a.borrow()\n}\n",
            "fn main()->int{\nvar a:int = 3\nreturn a.unknown()\n}\n",
            "fn main()->int{\nreturn helper().move()\n}\nfn helper()->int{\nreturn 3\n}\n",
        ] {
            assert!(inspect_source(text).is_err(), "不應擴大試驗來源形式：{text}");
        }
    }

    #[test]
    fn 獨立_voil_素材必須由真正試驗前端解析及靜態證明() {
        const GOOD: &[&str] = &[
            include_str!("../tests/fixtures/p13_probe/整體移動及保留複本.voil"),
            include_str!("../tests/fixtures/p13_probe/重新初始化.voil"),
        ];
        const BAD: &[&str] = &[
            include_str!("../tests/fixtures/p13_probe/移動後讀取錯誤.voil"),
            include_str!("../tests/fixtures/p13_probe/分支移動錯誤.voil"),
        ];
        for source in GOOD { valid(source); }
        for source in BAD {
            assert_eq!(
                inspect_source(source).expect_err("P13 負例應拒絕").kind,
                DiagnosticKind::Static
            );
        }
    }


    #[test]
    fn 分支兩側均重新初始化_合流後允許安全讀取() {
        valid("fn main()->int{\nvar a:int = 3\nif (true){\nvar b:int = a.move()\na = 4\n}else{\nvar c:int = a.move()\na = 5\n}\nreturn a\n}\n");
    }

    #[test]
    fn 分支只有一側重新初始化_合流後仍拒絕來源讀取() {
        unsafe_source(
            "fn main()->int{\nvar a:int = 3\nif (true){\nvar b:int = a.move()\na = 4\n}else{\nvar c:int = a.move()\n}\nreturn a\n}\n",
            "可達分支"
        );
    }

    #[test]
    fn 遮蔽區域內移動後讀取必須拒絕_不借用外層同名來源() {
        unsafe_source(
            "fn main()->int{\nvar a:int = 3\nif (true){\nvar a:int = 4\nvar b:int = a.move()\nvar c:int = a\n}\nreturn a\n}\n",
            "已明確移動"
        );
    }

    #[test]
    fn 不變更對外_mvp_a_的_passed_與_move_拒絕() {
        use crate::check_file;
        let good = check_file(Path::new("tests/fixtures/voiles/P01-正01.voil"));
        assert_eq!(good.status, CheckStatus::Passed);
        let code = "fn main()->int{\nvar n:int = 3\nreturn n.move()\n}\n";
        assert!(inspect_source(code).is_ok());
        let source = crate::source::Source::new(code.as_bytes().to_vec());
        let lexed = crate::lexer::scan(&source);
        assert!(crate::parser::parse(&source, &lexed.tokens).is_err());
    }
}
