//! P17 D00／P17-A、B、E：非公開且只接受 P01 MVP-A 的真實求值切片。
//! 執行前必須取得同一來源正式 P07／P09／P10／P11／P12 的證據。
//! 本模組尚不公開 CLI、str、struct、借用或資源事件。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

mod file;
mod registry;
mod trace;
mod source_audit;

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

/// P18 真正走到的控制轉移；僅記錄 Runtime 已執行的路徑。
/// 與靜態 CFG、BorrowFact、ResourceEvent 各自獨立。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlEvent {
    FunctionEntered { name: String, span: NodeSpan },
    FunctionExited { name: String, span: NodeSpan, returned: bool },
    ConditionChecked { span: NodeSpan, selected: bool },
    ElseSelected { span: NodeSpan },
    Returned { span: NodeSpan },
}

/// 評估與真控制邊同時回傳；遇動態錯誤仍保留已執行路徑，
#[derive(Debug)]
pub(crate) struct ExecutionOutcome {
    pub(crate) result: Result<Value, RuntimeFailure>,
    pub(crate) events: Vec<ControlEvent>,
}

/// D18-01-A～D：安全預算是**執行環境配置**，不是 voiles 語言
/// 固定的最大遞迴深度；只有內部 Rust 驗證可配置，未增設 CLI。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExecutionBudget {
    pub(crate) max_call_frames: usize,
    pub(crate) max_steps: usize,
}
impl Default for ExecutionBudget {
    fn default() -> Self {
        Self { max_call_frames: 96, max_steps: 100_000 }
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
            && source_audit::atoms_match_source(&self.ast, text)
    }

    /// P17-A：準備後的 P09 宣告與名稱、P10 具型別 HIR、P11 真可達集合
    /// 必須仍與**同一原始來源重新執行既有 Checker** 的唯一結果一致。
    /// 不是新 Parser、也不以 Passed 單一布林值授權任意被修改的證據。
    fn has_original_checker_facts(&self) -> bool {
        let Ok(fresh) = Self::prepare(self.original_bytes.to_vec()) else {
            return false;
        };
        self.resolved == fresh.resolved
            && self.typed == fresh.typed
            && self.reachable_expressions == fresh.reachable_expressions
            && self.reachable_assignments == fresh.reachable_assignments
    }

    /// 只把實際運算的值回傳給內部測試；不映射 OS exit code。
    pub(crate) fn execute(&self) -> Result<Value, RuntimeFailure> {
        self.execute_with_trace().result
    }

    /// P18-E：由實際 Runtime 產生執行紀錄；來源或 HIR 無效時絕不開始執行。
    pub(crate) fn execute_with_trace(&self) -> ExecutionOutcome {
        self.execute_with_trace_budget(ExecutionBudget::default())
    }

    /// D18-01：只允許此已核准非公開 evaluator 更換資源預算。
    /// 不把 Rust 預設數值宣布為 voiles 語言限制，也不靠宿主爆堆疊。
    pub(crate) fn execute_with_trace_budget(&self, budget: ExecutionBudget)
        -> ExecutionOutcome {
        if budget.max_call_frames == 0 || budget.max_steps == 0 {
            return ExecutionOutcome {
                result: Err(RuntimeFailure::global("執行環境資源預算不可為零")),
                events: Vec::new(),
            };
        }
        if !self.is_same_verified_source()
            || !self.has_original_checker_facts() {
            return ExecutionOutcome {
                result: Err(RuntimeFailure::global("來源或驗證身分在準備後遭到變更")),
                events: Vec::new(),
            };
        }
        let mut runtime = Runtime {
            prepared: self,
            globals: BTreeMap::new(),
            frames: Vec::new(),
            trace: Vec::new(),
            budget,
            remaining_steps: budget.max_steps,
        };
        let result = (|| {
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
        })();
        // P18-E：執行器本身所產生的控制邊若破壞堆疊／返回不變量，
        // 即使計算有值也不得把它作為成功結果交付。
        if trace::audit_control_events_against_source(
            &runtime.trace, &self.ast,
            std::str::from_utf8(&self.source.bytes).unwrap_or(""),
        ).is_err() {
            return ExecutionOutcome {
                result: Err(RuntimeFailure::global("控制事件堆疊或來源驗證失敗")),
                events: runtime.trace,
            };
        }
        ExecutionOutcome { result, events: runtime.trace }
    }

    /// 供內部回報動態錯誤來源；與 `DiagnosticKind` 靜態分類隔離。
    pub(crate) fn runtime_span(&self, problem: &RuntimeFailure)
        -> Option<crate::SourceSpan> {
        problem.span.map(|span| self.source.span(span.start, span.end))
    }
}

struct Runtime<'a> {
    prepared: &'a PreparedProgram,
    globals: BTreeMap<DeclarationId, Slot>,
    // 每個函式有獨立框架，區塊依來源身分建立與退出。
    frames: Vec<Vec<BTreeMap<DeclarationId, Slot>>>,
    trace: Vec<ControlEvent>,
    budget: ExecutionBudget,
    remaining_steps: usize,
}

impl Runtime<'_> {
    /// 真正每次執行到語句或運算式才消耗步數，不提前計入未走過
    /// 的 AST／CFG 控制邊。預算耗盡時交給當前函式逐層退出。
    fn consume_step(&mut self, span: NodeSpan) -> Result<(), RuntimeFailure> {
        if self.remaining_steps == 0 {
            return Err(RuntimeFailure::at(span, "執行步數預算耗盡"));
        }
        self.remaining_steps -= 1;
        Ok(())
    }

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
        self.consume_step(span)?;
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
        ))?.push(BTreeMap::new());
        let outcome = self.statements(block);
        // 標量切片沒有 ResourceId；pop 僅退出詞彙作用域，不產生假清理事件。
        self.frames.last_mut().expect("已建立區塊框架").pop();
        outcome
    }

    fn statements(&mut self, block: &Block) -> Result<Option<Value>, RuntimeFailure> {
        for statement in &block.statements {
            self.consume_step(statement.node_span())?;
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
                StmtKind::Return(ret) => {
                    let value = self.expression(&ret.expression)?;
                    self.trace.push(ControlEvent::Returned { span: statement.node_span() });
                    return Ok(Some(value));
                },
                StmtKind::IfChain(chain) => {
                    let mut selected = None;
                    for arm in &chain.arms {
                        let condition = self.expression(&arm.branch.condition)?;
                        if let Value::Bool(selected) = condition {
                            self.trace.push(ControlEvent::ConditionChecked {
                                span: arm.branch.condition.node_span(),
                                selected,
                            });
                        }
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
                    if selected.is_none() {
                        if let Some(otherwise) = chain.otherwise.as_ref() {
                            self.trace.push(ControlEvent::ElseSelected {
                                span: NodeSpan::new(otherwise.start, otherwise.end),
                            });
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
        // 既有 AW06-A 直接／相互遞迴均屬合法函式呼叫。
        // 只限制**本次執行環境**能承擔的動態真框架數；
        // 必須在進入下一層前檢查，讓已進入的所有框架都能
        // 在錯誤冒泡時照真來源逐層退出，從不依賴 Rust stack overflow。
        if self.frames.len() >= self.budget.max_call_frames {
            return Err(RuntimeFailure {
                span: source_span, reason: "遞迴呼叫框架預算耗盡",
            });
        }
        let function = self.prepared.ast.items.iter().find_map(|item| match item {
            Item::Function(f) if f.name == name => Some(f.clone()),
            _ => None,
        }).ok_or_else(|| RuntimeFailure {
            span: source_span, reason: "不存在已驗證的無參數函式",
        })?;
        let function_span = NodeSpan::new(function.start, function.end);
        self.trace.push(ControlEvent::FunctionEntered {
            name: name.to_string(), span: function_span,
        });
        self.frames.push(Vec::new());
        let result = self.block(&function.body);
        self.frames.pop();
        self.trace.push(ControlEvent::FunctionExited {
            name: name.to_string(), span: function_span,
            returned: matches!(&result, Ok(Some(value)) if value.matches(function.result_ty)),
        });
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
    fn p17_a_同源正式_hir_與可達證據完整時可重複真求值() {
        let text = "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nif(true){\nreturn helper()+3\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        for _ in 0..2 {
            let outcome = plan.execute_with_trace();
            assert_eq!(outcome.result, Ok(Value::Int(5)));
            assert!(matches!(outcome.events.first(), Some(ControlEvent::FunctionEntered { .. })));
        }
    }

    #[test]
    fn p17_a_已解析同型別名稱引用被轉接至其他宣告必須零事件拒絕() {
        let text = "fn main()->int{\nvar a:int = 2\nvar b:int = 9\nreturn a\n}\n";
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        let names: Vec<_> = plan.resolved.declarations.keys().copied().collect();
        assert_eq!(names.len(), 2);
        let key = *plan.resolved.references.keys().next().unwrap();
        let original = plan.resolved.references[&key];
        let forged = *names.iter().find(|name| **name != original).unwrap();
        plan.resolved.references.insert(key, forged);
        let result = plan.execute_with_trace();
        assert!(result.result.is_err());
        assert!(result.events.is_empty());
    }

    #[test]
    fn p17_a_真賦值身分偷換同型別變數不能開始執行() {
        let text = "fn main()->int{\nvar a:int = 2\nvar b:int = 9\na = 5\nreturn a\n}\n";
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        let target_key = *plan.resolved.assignments.keys().next().unwrap();
        let original = plan.resolved.assignments[&target_key];
        let wrong = *plan.resolved.declarations.keys()
            .find(|id| **id != original).unwrap();
        plan.resolved.assignments.insert(target_key, wrong);
        let result = plan.execute_with_trace();
        assert!(result.result.is_err());
        assert!(result.events.is_empty());
    }

    #[test]
    fn p17_a_正式宣告可變性或型別事實被換掉不得沿用原授權() {
        let text = "fn main()->int{\nvar a:int = 1\nreturn a\n}\n";
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        let key = *plan.resolved.declarations.keys().next().unwrap();
        plan.resolved.declarations.get_mut(&key).unwrap().mutable = false;
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_a_刪除已驗證運算式可達證據須在入口前安全拒絕() {
        let text = "fn main()->int{\nreturn 2+3\n}\n";
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        assert!(!plan.reachable_expressions.is_empty());
        let removed = *plan.reachable_expressions.iter().next().unwrap();
        plan.reachable_expressions.remove(&removed);
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p17_a_捏造不存在的可達表達式或賦值邊不得增加執行許可() {
        let text = "fn main()->int{\nvar n:int = 1\nn = 3\nreturn n\n}\n";
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        let artificial = NodeSpan::new(0, 1);
        assert!(plan.reachable_expressions.insert(artificial));
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
        let mut plan = PreparedProgram::prepare(text.as_bytes().to_vec()).unwrap();
        assert!(!plan.reachable_assignments.is_empty());
        plan.reachable_assignments.clear();
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
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
        let position = plan.source.bytes.len() - 3;
        plan.source.bytes[position] = b'3';
        assert!(plan.execute().is_err());
        let mut plan = PreparedProgram::prepare(b"fn main()->int{\nreturn 2\n}\n".to_vec())
            .unwrap();
        plan.typed = VerifiedScalarHir::default();
        assert!(plan.execute().is_err());
    }

    #[test]
    fn p18_無限遞迴耗盡預算安全拒絕_不爆堆疊() {
        let code = "fn again()->int{\nreturn again()\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 4, max_steps: 100,
        });
        assert_eq!(outcome.result.unwrap_err().reason, "遞迴呼叫框架預算耗盡");
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ControlEvent::FunctionEntered { .. })).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ControlEvent::FunctionExited { returned: false, .. })).count(), 4);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, ControlEvent::Returned { .. })));
        assert_eq!(trace::audit_control_events_against_source(
            &outcome.events, &plan.ast, code), Ok(()));
    }
    #[test]
    fn p18_多層非遞迴函式框架按實際呼叫返回() {
        let code = "fn main()->int{\nreturn middle()+1\n}\nfn middle()->int{\nreturn leaf()+1\n}\nfn leaf()->int{\nreturn 2\n}\n";
        assert_eq!(run(code), Ok(Value::Int(4)));
    }

    #[test]
    fn p18_提前回傳阻止同函式剩餘可執行路徑的溢位() {
        let code = "fn main()->int{\nvar n:int = 9223372036854775807\nif (true){\nreturn 1\n}\nreturn n+1\n}\n";
        assert_eq!(run(code), Ok(Value::Int(1)));
    }

    #[test]
    fn p18_函式框架的遮蔽不影響被呼叫函式的頂層宣告() {
        let code = "var n:int = 5\nfn read()->int{\nreturn n\n}\nfn main()->int{\nvar n:int = 1\nreturn read()+n\n}\n";
        assert_eq!(run(code), Ok(Value::Int(6)));
    }

    #[test]
    fn p18_同一已驗證函式可重複呼叫且每次只執行實際路徑() {
        let code = "fn pick()->int{\nif (true){\nreturn 3\n}\nreturn 9\n}\nfn main()->int{\nreturn pick()+pick()\n}\n";
        assert_eq!(run(code), Ok(Value::Int(6)));
    }

    #[test]
    fn p18_真控制邊紀錄只包含實際檢查的條件與返回() {
        let code = "fn main()->int{\nif (false){\nreturn 99\n}else if(true){\nreturn 7\n}else{\nreturn 100\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(7)));
        let selected: Vec<_> = outcome.events.iter().filter_map(|event| match event {
            ControlEvent::ConditionChecked { selected, .. } => Some(*selected),
            _ => None,
        }).collect();
        assert_eq!(selected, [false, true]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ControlEvent::Returned { .. })).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, ControlEvent::ElseSelected { .. })));
    }

    #[test]
    fn p18_無任何成立分支才會紀錄真正_else_選擇() {
        let code = "fn main()->int{\nif (false){\nreturn 1\n}else{\nreturn 2\n}\nreturn 0\n}\n";
        let outcome = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap()
            .execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(2)));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ControlEvent::ElseSelected { .. })).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ControlEvent::Returned { .. })).count(), 1);
    }

    #[test]
    fn p18_巢狀真函式的進出順序與返回值一致() {
        let code = "fn inner()->int{\nreturn 2\n}\nfn main()->int{\nreturn inner()+1\n}\n";
        let outcome = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap()
            .execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(3)));
        let calls: Vec<_> = outcome.events.iter().filter_map(|event| match event {
            ControlEvent::FunctionEntered { name, .. } =>
                Some(format!("進入:{name}")),
            ControlEvent::FunctionExited { name, returned, .. } =>
                Some(format!("退出:{name}:{returned}")),
            _ => None,
        }).collect();
        assert_eq!(calls, ["進入:main", "進入:inner",
                           "退出:inner:true", "退出:main:true"]);
    }

    #[test]
    fn p18_執行期錯誤保留真實函式退出且不冒稱_return_成功() {
        let code = "fn explode()->int{\nvar n:int = 9223372036854775807\nreturn n+1\n}\nfn main()->int{\nreturn explode()\n}\n";
        let outcome = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap()
            .execute_with_trace();
        assert_eq!(outcome.result.unwrap_err().reason,
            "執行時有號 64 位元整數溢位");
        let exits: Vec<_> = outcome.events.iter().filter_map(|event| match event {
            ControlEvent::FunctionExited { name, returned, .. } =>
                Some((name.as_str(), *returned)),
            _ => None,
        }).collect();
        assert_eq!(exits, [("explode", false), ("main", false)]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, ControlEvent::Returned { .. })));
    }

    #[test]
    fn p18_驗證來源被改寫時拒絕且不得產生任何控制事件() {
        let mut plan = PreparedProgram::prepare(
            b"fn main()->int{\nreturn 2\n}\n".to_vec()).unwrap();
        plan.original_bytes[0] = b'x';
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p18_實際真分支與提前返回事件按路徑順序登錄() {
        let code = "fn main()->int{\nif (true){\nreturn 7\n}else{\nreturn 9\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(7)));
        assert_eq!(outcome.events.len(), 4);
        assert!(matches!(&outcome.events[0], ControlEvent::FunctionEntered { name, .. } if name == "main"));
        assert!(matches!(&outcome.events[1], ControlEvent::ConditionChecked { selected: true, .. }));
        assert!(matches!(&outcome.events[2], ControlEvent::Returned { .. }));
        assert!(matches!(&outcome.events[3], ControlEvent::FunctionExited { returned: true, .. }));
        assert!(!outcome.events.iter().any(|event| matches!(event, ControlEvent::ElseSelected { .. })));
    }

    #[test]
    fn p18_實際_false_分支只登錄_else_及被執行的返回() {
        let code = "fn main()->int{\nif (false){\nreturn 99\n}else{\nreturn 4\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(4)));
        assert_eq!(outcome.events.len(), 5);
        assert!(matches!(&outcome.events[1], ControlEvent::ConditionChecked { selected: false, .. }));
        assert!(matches!(&outcome.events[2], ControlEvent::ElseSelected { .. }));
        assert!(matches!(&outcome.events[3], ControlEvent::Returned { .. }));
        assert!(matches!(&outcome.events[4], ControlEvent::FunctionExited { returned: true, .. }));
    }

    #[test]
    fn p18_多層函式實際事件保持嚴格進出堆疊順序() {
        let code = "fn main()->int{\nreturn helper()+1\n}\nfn helper()->int{\nreturn 5\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(6)));
        let simplified: Vec<_> = outcome.events.iter().map(|event| match event {
            ControlEvent::FunctionEntered { name, .. } => format!("enter:{name}"),
            ControlEvent::FunctionExited { name, .. } => format!("exit:{name}"),
            ControlEvent::Returned { .. } => "return".into(),
            _ => "condition".into(),
        }).collect();
        assert_eq!(simplified, ["enter:main", "enter:helper", "return", "exit:helper", "return", "exit:main"]);
    }

    #[test]
    fn p18_動態溢位保留真實已進入函式且未成功退出的控制紀錄() {
        let code = "fn main()->int{\nvar n:int = 9223372036854775807\nif (true){\nreturn n+1\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result.unwrap_err().reason, "執行時有號 64 位元整數溢位");
        assert_eq!(outcome.events.len(), 3);
        assert!(matches!(&outcome.events[0], ControlEvent::FunctionEntered { .. }));
        assert!(matches!(&outcome.events[1], ControlEvent::ConditionChecked { selected: true, .. }));
        assert!(matches!(&outcome.events[2], ControlEvent::FunctionExited { returned: false, .. }));
        assert!(!outcome.events.iter().any(|event| matches!(event, ControlEvent::Returned { .. })));
    }

    #[test]
    fn p18_來源或_hir_失效時事件簿必須完全為空() {
        let mut plan = PreparedProgram::prepare(b"fn main()->int{\nreturn 1\n}\n".to_vec()).unwrap();
        plan.typed = VerifiedScalarHir::default();
        let outcome = plan.execute_with_trace();
        assert!(outcome.result.is_err());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn p18_多重_else_if_僅登錄實際檢查至成立的分支() {
        let code = "fn main()->int{\nif (false){\nreturn 1\n}else if (true){\nreturn 2\n}else if (true){\nreturn 3\n}else{\nreturn 4\n}\nreturn 0\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace();
        assert_eq!(outcome.result, Ok(Value::Int(2)));
        let decisions: Vec<_> = outcome.events.iter().filter_map(|event| match event {
            ControlEvent::ConditionChecked { selected, .. } => Some(*selected),
            _ => None,
        }).collect();
        assert_eq!(decisions, [false, true]);
        assert_eq!(outcome.events.iter().filter(|event| matches!(event, ControlEvent::Returned { .. })).count(), 1);
        assert!(!outcome.events.iter().any(|event| matches!(event, ControlEvent::ElseSelected { .. })));
    }



    #[test]
    fn p18_已核准直接遞迴_可由全域_bool_終止並逐框架返回() {
        let code = "var once:bool = true\nfn again()->int{\nif (once){\nonce = false\nreturn again()\n}\nreturn 7\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 3, max_steps: 100,
        });
        assert_eq!(outcome.result, Ok(Value::Int(7)));
        let names: Vec<_> = outcome.events.iter().filter_map(|e| match e {
            ControlEvent::FunctionEntered {name, ..} => Some(format!("進入:{name}")),
            ControlEvent::FunctionExited {name, returned, ..} =>
                Some(format!("退出:{name}:{returned}")),
            _ => None,
        }).collect();
        assert_eq!(names, [
            "進入:main", "進入:again", "進入:again",
            "退出:again:true", "退出:again:true", "退出:main:true",
        ]);
        assert_eq!(trace::audit_control_events_against_source(
            &outcome.events, &plan.ast, code), Ok(()));
    }

    #[test]
    fn p18_已核准互相遞迴_使用原函式宣告與真動態框架() {
        let code = "var flag:bool = true\nfn first()->int{\nif (flag){\nflag = false\nreturn second()\n}\nreturn 4\n}\nfn second()->int{\nreturn first()\n}\nfn main()->int{\nreturn first()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 4, max_steps: 150,
        });
        assert_eq!(outcome.result, Ok(Value::Int(4)));
        let call_order: Vec<_> = outcome.events.iter().filter_map(|e| match e {
            ControlEvent::FunctionEntered {name, ..} => Some(name.as_str()),
            _ => None,
        }).collect();
        assert_eq!(call_order, ["main", "first", "second", "first"]);
        assert_eq!(trace::audit_control_events_against_source(
            &outcome.events, &plan.ast, code), Ok(()));
    }

    #[test]
    fn p18_遞迴框架預算小於合法真呼叫層數也須原子報錯() {
        let code = "var once:bool = true\nfn again()->int{\nif(once){\nonce=false\nreturn again()\n}\nreturn 7\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 2, max_steps: 100,
        });
        assert_eq!(outcome.result.unwrap_err().reason, "遞迴呼叫框架預算耗盡");
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, ControlEvent::FunctionExited {returned: false, ..})).count(), 2);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, ControlEvent::Returned { .. })));
        assert_eq!(trace::audit_control_events_against_source(
            &outcome.events, &plan.ast, code), Ok(()));
    }

    #[test]
    fn p18_步數預算耗盡須沿真函式逐層退出不偽造成功() {
        let code = "fn again()->int{\nreturn again()\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let outcome = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 20, max_steps: 5,
        });
        assert_eq!(outcome.result.unwrap_err().reason, "執行步數預算耗盡");
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, ControlEvent::FunctionEntered { .. })).count(),
            outcome.events.iter().filter(|e|
            matches!(e, ControlEvent::FunctionExited { returned: false, .. })).count());
        assert_eq!(trace::audit_control_events_against_source(
            &outcome.events, &plan.ast, code), Ok(()));
    }

    #[test]
    fn p18_同一已核准遞迴來源使用不同預算不改變語言語意() {
        let code = "var once:bool = true\nfn again()->int{\nif(once){\nonce=false\nreturn again()\n}\nreturn 7\n}\nfn main()->int{\nreturn again()\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let enough = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 3, max_steps: 100,
        });
        let short = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 2, max_steps: 100,
        });
        assert_eq!(enough.result, Ok(Value::Int(7)));
        assert_eq!(short.result.unwrap_err().reason, "遞迴呼叫框架預算耗盡");
        assert_eq!(trace::audit_control_events_against_source(
            &enough.events, &plan.ast, code), Ok(()));
        assert_eq!(trace::audit_control_events_against_source(
            &short.events, &plan.ast, code), Ok(()));
    }

    #[test]
    fn p18_零預算與遭竄改來源不得產生假進入函式事件() {
        let code = "fn main()->int{\nreturn 3\n}\n";
        let plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        for budget in [
            ExecutionBudget { max_call_frames: 0, max_steps: 20 },
            ExecutionBudget { max_call_frames: 3, max_steps: 0 },
        ] {
            let result = plan.execute_with_trace_budget(budget);
            assert!(result.result.is_err());
            assert!(result.events.is_empty());
        }
        let mut plan = PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        plan.source.bytes[0] = b'x';
        let result = plan.execute_with_trace_budget(ExecutionBudget {
            max_call_frames: 3, max_steps: 100,
        });
        assert!(result.result.is_err());
        assert!(result.events.is_empty());
    }
}
