//! P17 D00／P17-A、B、E：非公開且只接受 P01 MVP-A 的真實求值切片。
//! 執行前必須取得同一來源正式 P07／P09／P10／P11／P12 的證據。
//! 本模組尚不公開 CLI、str、struct、借用或資源事件。
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};

use crate::common_ir::{
    self, FrontendMode, NodeSpan, ResolvedNames, ScalarType, VerifiedScalarHir,
};
use crate::expr::{Expr, ExprKind};
use crate::ownership_ledger::DeclarationId;
use crate::parser::{self, Binding, Block, File, Item, StmtKind};
use crate::source::Source;
use crate::{flow, functions, lexer, names, types, Diagnostic, DiagnosticKind};

/// 準備階段的錯誤一律為真正的詞彙／語法／靜態診斷；不偽造動態錯誤。
#[derive(Debug)]
pub(crate) struct PreparationFailure(pub(crate) Diagnostic);

/// 執行期問題只屬內部 evaluator，不能冒充原 P01 靜態診斷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFailure {
    pub(crate) span: Option<NodeSpan>,
    pub(crate) reason: &'static str,
}

impl RuntimeFailure {
    fn at(expr: NodeSpan, reason: &'static str) -> Self {
        Self { span: Some(expr), reason }
    }
    fn global(reason: &'static str) -> Self {
        Self { span: None, reason }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value {
    Int(i64),
    Bool(bool),
}
impl Value {
    fn matches(self, ty: ScalarType) -> bool {
        matches!((self, ty),
            (Self::Int(_), ScalarType::Int) |
            (Self::Bool(_), ScalarType::Bool))
    }
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    value: Option<Value>,
    mutable: bool,
    ty: ScalarType,
}

/// 不提供公用建構子；不是可由 check_file 的 Passed 自行捏造的執行許可。
/// 來源、AST、宣告表、具型別 HIR 與 P11 真可達事實同時保存。
pub(crate) struct PreparedProgram {
    source: Source,
    original_bytes: Box<[u8]>,
    ast: File,
    resolved: ResolvedNames,
    typed: VerifiedScalarHir,
    reachable_expressions: BTreeSet<NodeSpan>,
    reachable_assignments: BTreeSet<NodeSpan>,
}

impl PreparedProgram {
    pub(crate) fn prepare(bytes: Vec<u8>) -> Result<Self, PreparationFailure> {
        let source = Source::new(bytes);
        if let Some(problem) = source.lexical_diagnostics().into_iter().next() {
            return Err(PreparationFailure(problem));
        }
        let tokens = lexer::scan(&source);
        if let Some(problem) = tokens.diagnostics.into_iter().next() {
            return Err(PreparationFailure(problem));
        }
        let ast = parser::parse(&source, &tokens.tokens).map_err(PreparationFailure)?;
        let resolved = names::resolve_with_facts(&ast, &source)
            .map_err(PreparationFailure)?;
        let typed = types::check_with_names_and_hir(&ast, &source, &resolved)
            .map_err(PreparationFailure)?;
        let reachability = flow::check_with_typed_hir_and_reachability(
            &ast, &source, &resolved, &typed,
        ).map_err(PreparationFailure)?;
        let reachable_expressions = reachability.verified_expressions()
            .map(|fact| fact.span).collect();
        let reachable_assignments = reachability.verified_assignments()
            .map(|fact| fact.span).collect();
        functions::check(&ast, &source).map_err(PreparationFailure)?;
        let original_bytes = source.bytes.clone().into_boxed_slice();
        let plan = Self {
            source, original_bytes, ast, resolved, typed,
            reachable_expressions, reachable_assignments,
        };
        if !plan.is_same_verified_source() {
            return Err(PreparationFailure(Diagnostic {
                kind: DiagnosticKind::Static,
                message: "執行計畫與正式 Checker 的來源身分不一致".into(),
                span: None,
            }));
        }
        Ok(plan)
    }

    fn is_same_verified_source(&self) -> bool {
        if self.original_bytes.as_ref() != self.source.bytes.as_slice() {
            return false;
        }
        let Ok(text) = std::str::from_utf8(&self.source.bytes) else {
            return false;
        };
        parser::verified_public_syntax_roles(&self.ast, text)
            .is_some_and(|roles| common_ir::syntax_covers_verified_scalar_hir(
                &roles, &self.typed, FrontendMode::PublicMvpA, text,
            ))
    }

    /// 只把實際運算的值回傳給內部測試；不映射 OS exit code。
    pub(crate) fn execute(&self) -> Result<Value, RuntimeFailure> {
        if !self.is_same_verified_source() {
            return Err(RuntimeFailure::global("來源或驗證身分在準備後遭到變更"));
        }
        let mut runtime = Runtime {
            prepared: self,
            globals: HashMap::new(),
            frames: Vec::new(),
            calls: Vec::new(),
        };
        for item in &self.ast.items {
            if let Item::Binding(binding) = item {
                let slot = runtime.initialized_slot(binding)?;
                runtime.globals.insert(DeclarationId(binding.start), slot);
            }
        }
        let result = runtime.call("main", None)?;
        if result.matches(ScalarType::Int) {
            Ok(result)
        } else {
            Err(RuntimeFailure::global("入口函式未產生已驗證的 int 值"))
        }
    }

    /// 供內部回報動態錯誤來源；與 `DiagnosticKind` 靜態分類隔離。
    pub(crate) fn runtime_span(&self, problem: &RuntimeFailure)
        -> Option<crate::SourceSpan> {
        problem.span.map(|span| self.source.span(span.start, span.end))
    }
}

struct Runtime<'a> {
    prepared: &'a PreparedProgram,
    globals: HashMap<DeclarationId, Slot>,
    // 每個函式有獨立框架，區塊依來源身分建立與退出。
    frames: Vec<Vec<HashMap<DeclarationId, Slot>>>,
    // 尚未支援遞迴的內部切片，遇到循環必須安全拒絕而非爆堆疊。
    calls: Vec<String>,
}

impl Runtime<'_> {
    fn checked_declaration(&self, binding: &Binding) -> Result<Slot, RuntimeFailure> {
        let id = DeclarationId(binding.start);
        match self.prepared.resolved.declaration(id) {
            Some(fact) if fact.ty == binding.ty && fact.mutable == binding.mutable =>
                Ok(Slot { value: None, mutable: fact.mutable, ty: fact.ty }),
            _ => Err(RuntimeFailure::at(
                NodeSpan::new(binding.start, binding.end), "宣告身分與 P09 證據不一致",
            )),
        }
    }

    fn initialized_slot(&mut self, binding: &Binding) -> Result<Slot, RuntimeFailure> {
        let mut slot = self.checked_declaration(binding)?;
        if let Some(expr) = &binding.initializer {
            let result = self.expression(expr)?;
            if !result.matches(slot.ty) {
                return Err(RuntimeFailure::at(
                    expr.node_span(), "初始化的動態型別與已驗證型別不一致",
                ));
            }
            slot.value = Some(result);
        }
        Ok(slot)
    }

    fn find_slot(&self, id: DeclarationId) -> Option<Slot> {
        self.frames.last()
            .and_then(|scopes| scopes.iter().rev().find_map(|scope| scope.get(&id).copied()))
            .or_else(|| self.globals.get(&id).copied())
    }

    fn assign(&mut self, id: DeclarationId, value: Value, span: NodeSpan)
        -> Result<(), RuntimeFailure> {
        let local = self.frames.last_mut()
            .and_then(|scopes| scopes.iter_mut().rev().find_map(|scope| scope.get_mut(&id)));
        let slot = if let Some(slot) = local {
            slot
        } else {
            self.globals.get_mut(&id).ok_or_else(|| RuntimeFailure::at(
                span, "賦值沒有已驗證的執行期持有位置",
            ))?
        };
        if !slot.mutable || !value.matches(slot.ty) {
            return Err(RuntimeFailure::at(span, "不可修改或賦值型別不符"));
        }
        slot.value = Some(value);
        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<Value, RuntimeFailure> {
        let span = expr.node_span();
        let fact = self.prepared.typed.expression(span)
            .ok_or_else(|| RuntimeFailure::at(span, "缺少正式 P10 型別事實"))?;
        if !self.prepared.reachable_expressions.contains(&span) {
            return Err(RuntimeFailure::at(span, "來源不在 P11 已驗證的可達集合"));
        }
        let result = match &expr.kind {
            ExprKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(text)) => {
                let n = crate::syntax_shared::integer_i64(text)
                    .map_err(|_| RuntimeFailure::at(span, "執行時整數常值不合法"))?;
                Value::Int(n)
            }
            ExprKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) =>
                Value::Bool(*value),
            ExprKind::Name(_) => {
                let used = self.prepared.typed.reference(span)
                    .ok_or_else(|| RuntimeFailure::at(span, "缺少 P10 已繫結的引用"))?;
                let source = self.prepared.resolved.reference(span.start);
                if !source.is_some_and(|fact| fact == used.resolved) {
                    return Err(RuntimeFailure::at(span, "讀取身分與 P09 來源不符"));
                }
                self.find_slot(used.resolved.declaration)
                    .and_then(|slot| slot.value)
                    .ok_or_else(|| RuntimeFailure::at(span, "讀取未初始化或不可見的宣告"))?
            }
            ExprKind::Group(inner) => self.expression(inner)?,
            ExprKind::BinaryInt(binary) => {
                let proof = self.prepared.typed.arithmetic(span)
                    .ok_or_else(|| RuntimeFailure::at(span, "缺少 P10 算術來源證明"))?;
                if proof.left != binary.left.node_span()
                    || proof.right != binary.right.node_span()
                    || proof.operator != binary.operator {
                    return Err(RuntimeFailure::at(span, "算術節點與原本 Checker 證明不一致"));
                }
                let left = self.expression(&binary.left)?;
                let right = self.expression(&binary.right)?;
                let (Value::Int(a), Value::Int(b)) = (left, right) else {
                    return Err(RuntimeFailure::at(span, "已驗證加減遇到非整數"));
                };
                let checked = match binary.operator {
                    '+' => a.checked_add(b),
                    '-' => a.checked_sub(b),
                    _ => None,
                };
                Value::Int(checked.ok_or_else(|| RuntimeFailure::at(
                    span, "執行時有號 64 位元整數溢位",
                ))?)
            }
            ExprKind::Call(call) => self.call(&call.name, Some(span))?,
            _ => return Err(RuntimeFailure::at(span, "未核准語法不可進入 MVP-A 求值器")),
        };
        if result.matches(fact.ty) {
            Ok(result)
        } else {
            Err(RuntimeFailure::at(span, "執行結果違反正式 P10 型別證明"))
        }
    }

    fn block(&mut self, block: &Block) -> Result<Option<Value>, RuntimeFailure> {
        self.frames.last_mut().ok_or_else(|| RuntimeFailure::global(
            "區塊沒有函式框架",
        ))?.push(HashMap::new());
        let outcome = self.statements(block);
        // 標量切片沒有 ResourceId；pop 僅退出詞彙作用域，不產生假清理事件。
        self.frames.last_mut().expect("已建立區塊框架").pop();
        outcome
    }

    fn statements(&mut self, block: &Block) -> Result<Option<Value>, RuntimeFailure> {
        for statement in &block.statements {
            match &statement.kind {
                StmtKind::Binding(binding) => {
                    let slot = self.initialized_slot(binding)?;
                    self.frames.last_mut().and_then(|frame| frame.last_mut())
                        .expect("本區塊已建置").insert(DeclarationId(binding.start), slot);
                }
                StmtKind::Assignment(assignment) => {
                    let span = statement.node_span();
                    if !self.prepared.reachable_assignments.contains(&span) {
                        return Err(RuntimeFailure::at(span, "賦值缺少 P11 真可達證據"));
                    }
                    let used = self.prepared.typed.assignment(span)
                        .ok_or_else(|| RuntimeFailure::at(span, "賦值缺少 P10 宣告身分"))?;
                    if self.prepared.resolved.assignment(span.start)
                        != Some(used.resolved) {
                        return Err(RuntimeFailure::at(span, "賦值來源與 P09 身分不一致"));
                    }
                    let value = self.expression(&assignment.expression)?;
                    self.assign(used.resolved.declaration, value, span)?;
                }
                StmtKind::Return(ret) => return self.expression(&ret.expression).map(Some),
                StmtKind::IfChain(chain) => {
                    let mut selected = None;
                    for arm in &chain.arms {
                        let condition = self.expression(&arm.branch.condition)?;
                        match condition {
                            Value::Bool(true) => {
                                selected = Some(&arm.branch.body);
                                break;
                            }
                            Value::Bool(false) => {}
                            _ => return Err(RuntimeFailure::at(
                                arm.branch.condition.node_span(), "if 條件不是 bool",
                            )),
                        }
                    }
                    if let Some(body) = selected.or(chain.otherwise.as_ref()) {
                        if let Some(value) = self.block(body)? {
                            return Ok(Some(value));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn call(&mut self, name: &str, source_span: Option<NodeSpan>)
        -> Result<Value, RuntimeFailure> {
        if self.calls.iter().any(|active| active == name) {
            return Err(RuntimeFailure {
                span: source_span, reason: "遞迴尚未接入 P18 安全執行切片",
            });
        }
        let function = self.prepared.ast.items.iter().find_map(|item| match item {
            Item::Function(f) if f.name == name => Some(f.clone()),
            _ => None,
        }).ok_or_else(|| RuntimeFailure {
            span: source_span, reason: "不存在已驗證的無參數函式",
        })?;
        self.calls.push(name.to_string());
        self.frames.push(Vec::new());
        let result = self.block(&function.body);
        self.frames.pop();
        self.calls.pop();
        let value = result?.ok_or_else(|| RuntimeFailure {
            span: source_span, reason: "已驗證函式未產生 return",
        })?;
        if !value.matches(function.result_ty) {
            return Err(RuntimeFailure {
                span: source_span, reason: "函式返回結果與已驗證簽章不符",
            });
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Result<Value, String> {
        let plan = PreparedProgram::prepare(text.as_bytes().to_vec())
            .map_err(|error| format!("靜態拒絕：{:?} {}", error.0.kind, error.0.message))?;
        plan.execute().map_err(|error| format!("動態拒絕：{}", error.reason))
    }

    #[test]
    fn p17_原始_voil_通過所有正式_checker_後真正計算_二加三() {
        assert_eq!(run("fn main()->int{\nreturn 2+3\n}\n"), Ok(Value::Int(5)));
    }

    #[test]
    fn p17_標量讀寫_詞彙遮蔽與已驗證加減() {
        let code = "fn main()->int{\nvar n:int = 1\nif (true){\nvar n:int = 9\nn = 8\n}\nn = 4\nreturn n+2\n}\n";
        assert_eq!(run(code), Ok(Value::Int(6)));
    }

    #[test]
    fn p17_布林分支只執行被選中的控制邊() {
        let code = "fn main()->int{\nif (false){\nreturn 99\n}else if(true){\nreturn 7\n}else{\nreturn 100\n}\nreturn 0\n}\n";
        assert_eq!(run(code), Ok(Value::Int(7)));
    }

    #[test]
    fn p17_無參數前向函式回傳值可作運算元() {
        let code = "fn main()->int{\nreturn answer()+1\n}\nfn answer()->int{\nreturn 6\n}\n";
        assert_eq!(run(code), Ok(Value::Int(7)));
    }

    #[test]
    fn p17_靜態型別_未初始化_const_私有語法均在執行前拒絕() {
        for code in [
            "fn main()->int{\nreturn true\n}\n",
            "fn main()->int{\nvar n:int\nreturn n\n}\n",
            "fn main()->int{\nconst n:int = 1\nn = 2\nreturn n\n}\n",
            "fn main()->int{\nvar x:str = \"abc\"\nreturn 0\n}\n",
            "fn main()->int{\nreturn missing\n}\n",
        ] {
            assert!(PreparedProgram::prepare(code.as_bytes().to_vec()).is_err(), "{code}");
        }
    }

    #[test]
    fn p17_動態溢位與來源位置分流_不冒充靜態診斷() {
        let code = "fn main()->int{\nvar n:int = 9223372036854775807\nreturn n+1\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec())
            .expect("整數變數的運算期溢位不得由靜態 Checker 虛報");
        let error = plan.execute().expect_err("必須回報動態溢位");
        assert_eq!(error.reason, "執行時有號 64 位元整數溢位");
        let span = plan.runtime_span(&error).unwrap();
        assert_eq!(span.start.line, 3);
    }

    #[test]
    fn p17_準備後來源更換或_hir_遭偽造_不得授權執行() {
        let mut plan = PreparedProgram::prepare(b"fn main()->int{\nreturn 2\n}\n".to_vec())
            .unwrap();
        plan.source.bytes[plan.source.bytes.len() - 3] = b'3';
        assert!(plan.execute().is_err());
        let mut plan = PreparedProgram::prepare(b"fn main()->int{\nreturn 2\n}\n".to_vec())
            .unwrap();
        plan.typed = VerifiedScalarHir::default();
        assert!(plan.execute().is_err());
    }

    #[test]
    fn p18_尚未建立遞迴退出契約時安全拒絕_不爆堆疊() {
        let code = "fn again()->int{\nreturn again()\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let error = plan.execute().unwrap_err();
        assert!(error.reason.contains("遞迴"));
    }
}
