//! P13 受限結構體所有權前端：真正 .voil → 私有 AST → 型別 → 所有權狀態。
//! 只接受可以完整驗證的子集，未支援的語法拒絕，不影響 MVP-A check_file。
//! 正式內部入口依 HA13-A／str 方案 A 使用核准政策；自訂能力僅限 Rust 測試。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;
use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot, ValueState};
use crate::ownership_ledger::DeclarationId;
use crate::borrow_safety::{BorrowFact, BorrowKind, BorrowSafety, StaticBorrowSnapshot};
use crate::source::Source;
use crate::struct_ownership::{self, MoveTarget, SelfAction};

#[derive(Debug, Clone)]
struct Field {
    name: String,
    ty: String,
    immutable: bool,
    default: Option<Term>,
}

#[derive(Debug, Clone)]
struct StructDecl {
    fields: Vec<Field>,
    methods: Vec<Function>,
    // 解析期維持原宣告順序，另建只讀名稱索引以減少重複線性搜尋。
    field_index: HashMap<String, usize>,
    method_index: HashMap<String, usize>,
}
impl StructDecl {
    fn new(fields: Vec<Field>, methods: Vec<Function>) -> Self {
        let field_index = fields.iter().enumerate()
            .map(|(index, field)| (field.name.clone(), index)).collect();
        let method_index = methods.iter().enumerate()
            .map(|(index, method)| (method.name.clone(), index)).collect();
        Self { fields, methods, field_index, method_index }
    }
    fn field(&self, name: &str) -> Option<&Field> {
        self.field_index.get(name).and_then(|index| self.fields.get(*index))
    }
    fn method(&self, name: &str) -> Option<&Function> {
        self.method_index.get(name).and_then(|index| self.methods.get(*index))
    }
}

/// R02 F03：私有函式 AST 與公開函式宣告直接使用同一
/// ParsedFunction 泛型核心；私有仍保留 params、struct 與安全
/// Checker 已核准的差異化能力，沒有將私有語法公開。
type Function = crate::syntax_shared::ParsedFunction<
    Vec<Parameter>, String, crate::syntax_shared::ParsedBlock<Stmt>
>;

#[derive(Debug, Clone)]
struct Parameter {
    name: String,
    ty: String,
    declaration_start: usize,
}

/// 私有安全語句 AST 共用相同具位置的節點外殼。
type Stmt = crate::common_ir::SpannedNode<StmtKind>;

#[derive(Debug, Clone)]
enum StmtKind {
    Declare(crate::syntax_shared::ParsedDeclaration<String, Term>),
    Assign(crate::syntax_shared::ParsedNamedAssignment<Term>),
    FieldAssign { receiver: String, field: String, value: Term },
    If {
        branch: crate::syntax_shared::ParsedIfArm<Term, crate::syntax_shared::ParsedBlock<Stmt>>,
        no: Option<Vec<Stmt>>,
    },
    /// P15：只在能證明反向邊的所有權狀態不變時接受的 while 子集。
    While {
        condition: Term,
        body: crate::syntax_shared::ParsedBlock<Stmt>,
    },
    /// D04 右端點不含的整數常值區間；P15 僅建構靜態借用證據。
    ForRange { iterator: String, iterator_start: usize,
        lower: String, upper: String,
        body: crate::syntax_shared::ParsedBlock<Stmt>,
    },
    Return(crate::syntax_shared::ParsedReturn<Term>),
    /// 既有 for／while 的最內層控制轉移；無迴圈時必須拒絕。
    Break,
    Continue,
}

/// 私有受限運算式與公開 Expr 使用同一 AST 外殼，仍保留私有節點種類。
type Term = crate::common_ir::SpannedNode<TermKind>;

#[derive(Debug, Clone)]
enum TermKind {
    /// F02：正式私有 Parser 與公開 Parser 共用完整 int/bool 常值 AST。
    Scalar(crate::syntax_shared::ScalarLiteral),
    /// 保留私有 float、str 與既有內部手造 AST 的相容入口。
    Literal(&'static str),
    Name(String),
    /// P01 已核准的 int 加減；計算前必須依序驗證兩端所有權。
    BinaryInt(crate::common_ir::ArithmeticNode<Term>),
    Construct(String, Vec<(String, Term)>),
    /// 一般函式位置引數；呼叫端所有權資料流完成前不得接受。
    Call(crate::syntax_shared::ParsedNamedCall<Vec<Term>>),
    Move(String),
    /// VA14-01-A：只於具型別 var／const 直接初始化時建立非擁有借用。
    Borrow(String),
    FieldMove(String, String),
    /// 只讀欄位值的複製；需另外通過已初始化與 HA13/S19 Copyable 檢查。
    FieldRead(String, String),
    /// S16 已核准的 instance.method() 零參數子集；此處不推定其他簽章。
    MethodCall(String, String, Vec<Term>),
}

struct Parser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    cursor: usize,
}

// 受限私有文法解析實作；目前使用同層 include! 以保持所有既有型別可見性。
include!("struct_stage/syntax.rs");

#[derive(Clone, PartialEq, Eq)]
struct LoanBinding {
    fact: BorrowFact,
    active: bool,
}

#[derive(Clone)]
struct Binding {
    declaration_start: DeclarationId,
    ty: String,
    mutable: bool,
    value: ValueSlot,
    /// 僅以合法 struct 建立式已知的未初始化 var 欄位集合。
    /// None 表示不存在可追蹤的部分建立證明；不等於全部欄位可用。
    pending_fields: Option<HashSet<String>>,
    /// 非擁有的借用別名，不擁有來源值，不允許 .move()。
    loan: Option<LoanBinding>,
}

impl Binding {
    /// F08／F09：整筆 move 只能委託 ValueSlot 的合法狀態轉移。
    /// 未完整建立的結構體不可移走；失敗不得修改 pending_fields
    /// 或清除借用來源事實。此處只管理靜態詞彙綁定，不創造動態資源。
    fn move_whole(&mut self) -> Result<(), ModelError> {
        self.value.move_whole()?;
        Ok(())
    }

    /// F09：已經在 Checker 預檢 RHS、目標 var 與資源覆寫限制後
    /// 進行唯一的重新綁定提交；值恢復與局部欄位責任清除不可分離。
    fn commit_rebind(&mut self) {
        self.value.mark_reinitialized();
        self.pending_fields = None;
    }

    /// F08／F09：欄位首次初始化只能移除既存的待完成欄位。
    /// 先完整唯讀驗證再更動；最後一欄才讓整筆 ValueSlot 可讀。
    /// 未經核准欄位／重複初始化必須維持原狀，不能半提交。
    fn commit_first_field(&mut self, field: &str) -> bool {
        if self.value.state() != ValueState::Uninitialized
            || !self.pending_fields.as_ref()
                .is_some_and(|pending| pending.contains(field)) {
            return false;
        }
        let pending = self.pending_fields.as_mut()
            .expect("已完成待初始化欄位預檢");
        pending.remove(field);
        if pending.is_empty() { self.commit_rebind(); }
        true
    }

    /// F08／F10：所有 CFG 反向邊、break／continue 出口及單次 for
    /// 分支合流都必須使用同一份來源安全狀態相等契約。除了原有
    /// ValueSlot、待初始化欄位和借用有效期，也要驗證真正宣告
    /// 身分、型別及 var/const 權限；不得讓局部比較產生不同權威。
    fn same_safety_state(&self, other: &Self) -> bool {
        self.declaration_start == other.declaration_start
            && self.ty == other.ty
            && self.mutable == other.mutable
            && self.value == other.value
            && self.pending_fields == other.pending_fields
            && self.loan == other.loan
    }

    /// 正式私有 Checker 的詞彙已解析宣告及受限型別，與 P09→P10/P11
    /// 共用同一 HIR 引用契約；實際所有權安全仍以本綁定的 ValueSlot/loan 為準。
    fn semantic_declaration(&self) -> crate::common_ir::TypedDeclaration<&str> {
        crate::common_ir::TypedDeclaration::new(
            self.declaration_start, self.ty.as_str(), self.mutable,
        )
    }

    fn semantic_use_at(&self, span: crate::common_ir::NodeSpan)
        -> crate::common_ir::TypedUse<&str> {
        self.semantic_declaration().use_at(span)
    }
}

/// 各大括號區塊都有獨立詞彙作用域；內層同名宣告不得覆蓋外層所有權來源。
// 分支與迴圈快照共用不可變狀態；只有實際寫入的詞彙作用域才複製。
type Scopes = Vec<Rc<HashMap<String, Binding>>>;

/// 資料流需區分正常後繼、函式返回與迴圈轉移；後兩者不能混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowState {
    Fallthrough,
    Returned,
    /// 已驗證的本層迴圈提前離開，並非下一輪反向邊。
    Break,
    /// 已驗證的本層迴圈反向邊，並非正常後繼或 return。
    Continue,
    /// 多分支混合的跳轉，未能證明屬單一 break／continue 類別。
    LoopTransfer,
}

struct Checker<'a, C> {
    source: &'a Source,
    structs: &'a HashMap<String, StructDecl>,
    functions: &'a [Function],
    function_index: HashMap<String, usize>,
    ability: C,
    /// F11 前置：真實 Checker 成功評估並完成宣告後記錄的直線具名移交。
    recorded_direct_moves: RefCell<BTreeSet<(DeclarationId, DeclarationId)>>,
    /// 真正通過 Checker 的參數、宣告與迭代身分，而不是事後掃描 AST。
    recorded_declarations: RefCell<BTreeMap<DeclarationId, crate::common_ir::TypedDeclaration<String>>>,
    /// F11：相同 AST 宣告被不同路徑檢查時若型別／可變權限不一致，
    /// 不能以覆寫表項掩藏衝突，也不能只靠 debug_assert 保護。
    recorded_declaration_conflicts: RefCell<BTreeSet<DeclarationId>>,
    /// F11：具名 return .move() 經來源身分、型別、借用與有效期驗證後的證據。
    recorded_named_returns: RefCell<BTreeSet<(DeclarationId, usize)>>,
    /// 真正完成型別、寫入授權及 RHS 評估後的結構欄位提交證據。
    recorded_field_writes: RefCell<BTreeSet<(DeclarationId, usize, String)>>,
    /// F04：私有 Checker 真正求值成功才產生共同 int／bool 運算式型別 HIR。
    recorded_scalar_hir: RefCell<crate::common_ir::VerifiedScalarHir>,
    /// 已經真正使用 Checker TypedDeclaration 並與 BorrowTicket tracker
    /// 對照成功的呼叫預設借用，不包含執行期票據 serial／tracker_id。
    recorded_call_borrows: RefCell<BTreeMap<usize, VerifiedCallBorrowPlan>>,
    recorded_call_borrow_conflicts: RefCell<BTreeSet<usize>>,
}

// P13～P16 的受限靜態安全檢查；下一步再分離型別／資料流責任。
include!("struct_stage/safety_checker.rs");

// P16 真實 UTF-8 資源執行期橋接，僅在內部測試建置時引入。
#[cfg(test)]
include!("struct_stage/resource_trace.rs");

/// 已通過同一組原始來源、解析、型別、所有權、借用及有效期檢查的私有安全事實。
/// P16 資源橋接只可消耗此成果，不可單獨從未驗證的 AST 假造安全來源。
/// R03.15：只在完整 Checker 成功後由已解析宣告建立的權威來源身分表。
/// 此處保證宣告來源、型別與可變權限，不假設動態持有者或資源實例。
/// 已驗證私有來源的一筆具名整筆轉移；只能由完成 Checker 的事實表取得。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedMoveEvidence {
    source: DeclarationId,
    target: DeclarationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedReturnEvidence {
    source: DeclarationId,
    return_start: usize,
}
/// 具名接收者的欄位提交已經 Checker 驗證；不隱含動態資源釋放順序。
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedFieldWriteEvidence {
    owner: DeclarationId,
    statement_start: usize,
    field: String,
}

/// F12：無 Checker 已驗證轉移事實時不得寫入任何資源安全帳簿。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifiedMoveError {
    MissingEvidence,
    Pipeline(crate::resource_borrow_pipeline::PipelineError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedCallBorrowPlan {
    span: crate::common_ir::NodeSpan,
    facts: Vec<BorrowFact>,
}

struct VerifiedDeclarationFacts {
    by_source: BTreeMap<DeclarationId, crate::common_ir::TypedDeclaration<String>>,
    /// 僅儲存真實私有 Checker 呼叫成功、與當次真實票據核對後的
    /// 靜態來源責任；不同分支的靜態計畫不表示兩分支均已執行。
    call_borrows: BTreeMap<usize, VerifiedCallBorrowPlan>,
    /// 目前僅記錄 Checker 已接受、同一函式直線語句中的具名整筆移動。
    direct_moves: BTreeSet<(DeclarationId, DeclarationId)>,
    /// 僅保存 Checker 成功提交的具名整筆返回來源及 return 運算式位置。
    named_returns: BTreeSet<(DeclarationId, usize)>,
    field_writes: BTreeSet<(DeclarationId, usize, String)>,
}
impl VerifiedDeclarationFacts {
    /// F11：本表僅消費 Checker 成功的宣告與直接轉移證據；
    /// 不能再從 AST 或名稱文字自行重建安全事實。
    fn from_checker(
        by_source: BTreeMap<DeclarationId, crate::common_ir::TypedDeclaration<String>>,
        direct_moves: BTreeSet<(DeclarationId, DeclarationId)>,
        named_returns: BTreeSet<(DeclarationId, usize)>,
        field_writes: BTreeSet<(DeclarationId, usize, String)>,
    ) -> Self {
        Self { by_source, call_borrows: BTreeMap::new(),
            direct_moves, named_returns, field_writes }
    }

    /// 公私雙 Checker 共用的完整宣告 HIR。避免驗證資源證據時
    /// 再將 (型別, var/const) 裝成另一個沒有 DeclarationId 的狀態模型。
    /// F11：由 Checker 實際呼叫驗證後的借用快照；只供已驗證來源
    /// 安全計畫消耗，不允許把推測 CFG 跨路徑快照當作實際簽發票據。
    fn call_borrows_at(&self, span: crate::common_ir::NodeSpan) -> Option<&[BorrowFact]> {
        self.call_borrows.get(&span.start)
            .filter(|plan| plan.span == span).map(|plan| plan.facts.as_slice())
    }

    fn typed_declaration(&self, source: DeclarationId)
        -> Option<crate::common_ir::TypedDeclaration<&str>> {
        let fact = self.by_source.get(&source)?;
        fact.owns(source).then(|| crate::common_ir::TypedDeclaration::new(
            source, fact.ty.as_str(), fact.mutable,
        ))
    }

    /// F11：Checker 證據只有在全表互相吻合時才可交給 P16 資源橋接。
    /// 不得把沒有宣告來源或型別不同的分支推測事件當成安全計畫。
    fn evidence_is_consistent(&self, structs: &HashMap<String, StructDecl>, text: &str) -> bool {
        // F11：來源身分及每個資源操作位置都必須屬於真正的 UTF-8 原始碼。
        // 不可以只檢查前後順序，讓超出 EOF 或落在多位元組字元中間的
        // 位置冒充已驗證的持有者／清理事件證據。
        let valid_position = |offset: usize| {
            offset < text.len() && text.is_char_boundary(offset)
        };
        // 同一正式目標宣告、return 來源位置或欄位賦值語句只能
        // 對應一筆已驗證轉移；不同 Checker 路徑的雙重主張須保守拒絕，
        // 不可將它們混成同一執行期資源事件或憑集合去重掩蓋衝突。
        let mut move_targets = BTreeSet::new();
        let mut return_positions = BTreeSet::new();
        let mut field_write_positions = BTreeSet::new();
        self.by_source.iter().all(|(id, fact)| fact.owns(*id) && valid_position(id.0))
        && self.direct_moves.iter().all(|(source, target)| {
            move_targets.insert(*target)
                && source != target && source.0 < target.0
                && self.by_source.get(source).zip(self.by_source.get(target))
                    .is_some_and(|(from, to)| from.declaration == *source
                        && to.declaration == *target && from.ty == to.ty)
        })
        && self.named_returns.iter().all(|(source, return_start)| {
            return_positions.insert(*return_start)
                && self.by_source.contains_key(source) && *return_start >= source.0
                && valid_position(*return_start)
        })
        && self.field_writes.iter().all(|(source, statement_start, field)| {
            field_write_positions.insert(*statement_start)
                && self.by_source.get(source).is_some_and(|fact| {
                fact.declaration == *source && fact.mutable && *statement_start >= source.0
                    && valid_position(*statement_start)
                    && structs.get(&fact.ty).and_then(|definition| definition.field(field))
                        .is_some_and(|member| !member.immutable)
            })
        })
        && self.call_borrows.iter().all(|(start, plan)| {
            plan.span.start == *start
                && plan.span.start < plan.span.end
                && plan.span.end <= text.len()
                && text.is_char_boundary(plan.span.start)
                && text.is_char_boundary(plan.span.end)
                && crate::borrow_safety::BorrowSafety::validate_active_set(
                    plan.facts.iter().map(|fact| (fact.source, fact.kind))
                ).is_ok()
                && plan.facts.iter().all(|fact| self.by_source.get(&fact.source)
                    .is_some_and(|decl| decl.owns(fact.source)
                        && fact.source.0 <= plan.span.start
                        && (fact.kind != BorrowKind::ExclusiveWrite || decl.mutable)))
        })
    }

    /// F11：已成功型別檢查的所有純值 HIR 必須與同一 Checker 的
    /// 宣告／var-const 權威完全一致；只讀交叉稽核，不從 AST 猜測
    /// 動態資源實例、所有權或尚未核准的完整 CFG。
    fn scalar_hir_is_consistent(
        &self, hir: &crate::common_ir::VerifiedScalarHir, text: &str,
    ) -> bool {
        // 由同一個公私正式 Typed HIR 閘門驗證來源 UTF-8、
        // 三類 NodeIdentity 與已讀取運算式的型別一致性。
        // 本層僅額外核對私有宣告所有權與 var/const 授權。
        hir.source_spans_are_valid(text)
            && hir.verified_references().all(|entry| {
                self.typed_declaration(entry.resolved.declaration)
                        .is_some_and(|decl| decl.declaration.0 <= entry.span.start
                            && crate::common_ir::ScalarType::from_name(decl.ty)
                                == Some(entry.resolved.ty))
            })
            && hir.verified_assignments().all(|entry| {
                self.typed_declaration(entry.resolved.declaration)
                        .is_some_and(|decl| decl.mutable
                            && decl.declaration.0 <= entry.span.start
                            && crate::common_ir::ScalarType::from_name(decl.ty)
                                == Some(entry.resolved.ty))
            })
    }

    /// 一筆具名返回須由 Checker 在真正成功驗證 `return name.move()`
    /// 後按來源 DeclarationId 及原始運算式位置直接簽出證據。
    fn verified_named_return(
        &self, source: DeclarationId, return_start: usize, ty: &str,
    ) -> Option<VerifiedReturnEvidence> {
        self.typed_declaration(source).filter(|fact| fact.ty == ty)?;
        (self.named_returns.contains(&(source, return_start))
            && self.named_returns.iter().filter(|(_, pos)| *pos == return_start).count() == 1)
            .then_some(VerifiedReturnEvidence { source, return_start })
    }

    /// 結構欄位寫入必須是已簽發的接收者身分、型別及原位置三元事實。
    fn verified_field_write(
        &self, owner: DeclarationId, statement_start: usize,
        ty: &str, field: &str,
    ) -> Option<VerifiedFieldWriteEvidence> {
        self.typed_declaration(owner).filter(|fact|
            fact.ty == ty && fact.mutable)?;
        (self.field_writes.contains(&(owner, statement_start, field.to_owned()))
            && self.field_writes.iter().filter(|(_, pos, _)| *pos == statement_start)
                .count() == 1)
            .then_some(VerifiedFieldWriteEvidence {
                owner, statement_start, field: field.to_owned(),
            })
    }

    fn verified_direct_move(
        &self, source: DeclarationId, target: DeclarationId,
        ty: &str,
    ) -> Option<VerifiedMoveEvidence> {
        let from = self.typed_declaration(source)?;
        let to = self.typed_declaration(target)?;
        (source != target && source.0 < target.0 && from.ty == ty && to.ty == ty
            && self.direct_moves.contains(&(source, target))
            && self.direct_moves.iter().filter(|(_, to)| *to == target).count() == 1)
            .then_some(VerifiedMoveEvidence { source, target })
    }

    /// 唯一可由已核准直線 Checker 移交事實接入抽象資源管線的操作。
    /// 拒絕錯誤來源、型別與方向，且在 Pipeline 任一側失敗時不提交。
    fn apply_verified_direct_move(
        &self, pipeline: &mut crate::resource_borrow_pipeline::SafetyPipeline,
        source: DeclarationId, target: DeclarationId, ty: &str,
    ) -> Result<(), VerifiedMoveError> {
        let proof = self.verified_direct_move(source, target, ty)
            .ok_or(VerifiedMoveError::MissingEvidence)?;
        pipeline.move_into(proof.source, proof.target)
            .map_err(VerifiedMoveError::Pipeline)
    }

    fn has_direct_move(&self, source: DeclarationId, target: DeclarationId) -> bool {
        self.direct_moves.contains(&(source, target))
    }

    fn matches(&self, source: DeclarationId, ty: &str, mutable: bool) -> bool {
        self.typed_declaration(source)
            .is_some_and(|fact| fact.ty == ty && fact.mutable == mutable)
    }
}

/// F11：正式內部 Checker 唯一交付的**已驗證靜態安全計畫**。
/// 以同一封閉物件承載來源身分／所有權移交／欄位寫入／真實呼叫借用
/// 與 Typed HIR，供 P16 已核准受限橋接消耗；不是動態 ResourceId 或
/// 一般 CFG 執行器，也不得由橋接自行用 AST 推斷持有者。
struct VerifiedSafetyPlan {
    declarations: VerifiedDeclarationFacts,
    scalar_hir: crate::common_ir::VerifiedScalarHir,
    /// Checker 交付時即封存的已驗證 HIR 位置契約；避免橋接錯誤
    /// 丟失任何一類證據後，僅靠剩餘的相容項目通過空集合檢查。
    expression_spans: Vec<crate::common_ir::NodeSpan>,
    reference_spans: Vec<crate::common_ir::NodeSpan>,
    assignment_spans: Vec<crate::common_ir::NodeSpan>,
}

impl VerifiedSafetyPlan {
    fn from_checker(
        declarations: VerifiedDeclarationFacts,
        scalar_hir: crate::common_ir::VerifiedScalarHir,
    ) -> Self {
        let expression_spans =
            scalar_hir.verified_expressions().map(|fact| fact.span).collect();
        let reference_spans =
            scalar_hir.verified_references().map(|fact| fact.span).collect();
        let assignment_spans =
            scalar_hir.verified_assignments().map(|fact| fact.span).collect();
        Self { declarations, scalar_hir,
            expression_spans, reference_spans, assignment_spans }
    }

    /// 同一完整來源與同一 Checker 交付的兩份安全證據一起核對；
    /// 不允許橋接只看其中一份而忽略不一致的型別／借用權威。
    fn is_consistent(&self, structs: &HashMap<String, StructDecl>, text: &str) -> bool {
        self.declarations.evidence_is_consistent(structs, text)
            && self.declarations.scalar_hir_is_consistent(&self.scalar_hir, text)
            && self.expression_spans == self.scalar_hir.verified_expressions()
                .map(|fact| fact.span).collect::<Vec<_>>()
            && self.reference_spans == self.scalar_hir.verified_references()
                .map(|fact| fact.span).collect::<Vec<_>>()
            && self.assignment_spans == self.scalar_hir.verified_assignments()
                .map(|fact| fact.span).collect::<Vec<_>>()
    }
}

struct VerifiedPrivateSource {
    source: Source,
    structs: HashMap<String, StructDecl>,
    functions: Vec<Function>,
    /// 與真正通過驗證的完整原始來源一起封裝，不能獨立從 AST 重建。
    plan: VerifiedSafetyPlan,
}

fn verify_source_with_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<VerifiedPrivateSource, Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() { return Err(error); }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() { return Err(error); }
    let (structs, functions) =
        Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file()?;
    let function_index = functions.iter().enumerate()
        .map(|(i, f)| (f.name.clone(), i)).collect();
    let checker = Checker {
        source: &source, structs: &structs, functions: &functions,
        function_index, ability,
        recorded_direct_moves: RefCell::new(BTreeSet::new()),
        recorded_declarations: RefCell::new(BTreeMap::new()),
        recorded_declaration_conflicts: RefCell::new(BTreeSet::new()),
        recorded_named_returns: RefCell::new(BTreeSet::new()),
        recorded_field_writes: RefCell::new(BTreeSet::new()),
        recorded_scalar_hir: RefCell::new(crate::common_ir::VerifiedScalarHir::default()),
        recorded_call_borrows: RefCell::new(BTreeMap::new()),
        recorded_call_borrow_conflicts: RefCell::new(BTreeSet::new()),
    };
    checker.all(&functions)?;
    // 任何路徑的同一 DeclarationId 證據若有衝突，整份成功
    // Checked HIR 不能交給 P16。不能讓最後一次寫入覆蓋異常。
    if !checker.recorded_declaration_conflicts.borrow().is_empty() {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：不同控制流程路徑的宣告型別或寫入權限證據衝突".into(),
            span: None,
        });
    }
    if !checker.recorded_call_borrow_conflicts.borrow().is_empty() {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：相同呼叫位置的借用來源或權限證據衝突".into(),
            span: None,
        });
    }
    let by_source = checker.recorded_declarations.into_inner();
    let direct_moves = checker.recorded_direct_moves.into_inner();
    let named_returns = checker.recorded_named_returns.into_inner();
    let field_writes = checker.recorded_field_writes.into_inner();
    let scalar_hir = checker.recorded_scalar_hir.into_inner();
    let mut declarations = VerifiedDeclarationFacts::from_checker(
        by_source, direct_moves, named_returns, field_writes);
    // 只由真正 Checker 在每次已驗證的呼叫出口交付來源事實，
    // 不從任何 AST 呼叫外形或模擬資源事件逆向重建。
    declarations.call_borrows = checker.recorded_call_borrows.into_inner();
    // 雙帳簿與 P16 來源橋接只能拿到整份通過同一交叉驗證的計畫。
    // 真正 Checker 的資訊由此單一所有權移入，不能傳遞獨立可換的兩本表。
    let plan = VerifiedSafetyPlan::from_checker(declarations, scalar_hir);
    if !plan.is_consistent(&structs, text) {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：Checker 安全證據與宣告、型別 HIR 或資源權限不一致".into(),
            span: None,
        });
    }
    Ok(VerifiedPrivateSource { source, structs, functions, plan })
}

#[cfg(test)]
fn verified_private_source(text: &str) -> Result<VerifiedPrivateSource, Diagnostic> {
    verify_source_with_ability(
        text, crate::ownership_policy::approved_leaf_copy_permission)
}

fn inspect_with_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<(), Diagnostic> {
    verify_source_with_ability(text, ability).map(|_| ())
}

/** P13 已核准 HA13-A/HB13-A 的真實來源受限靜態入口。
 * 目前僅內部使用；若有尚未實作的語意，仍必須保守拒絕。
 * 外部無法傳入臨時型別權限覆寫已核准 Copyable 政策。
 */
pub(crate) fn inspect(text: &str) -> Result<(), Diagnostic> {
    inspect_with_ability(text, crate::ownership_policy::approved_leaf_copy_permission)
}

/// 歷史測試保留假設能力注入，僅用於驗證 S19 合取及未知能力；
/// 正式核准入口一律使用上方 inspect，不接受外部能力覆寫。
#[cfg(test)]
fn inspect_with_test_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<(), Diagnostic> {
    inspect_with_ability(text, ability)
}


// 同層引入原有測試模組，以保留 Rust 測試完整識別名稱及驗收腳本過濾條件。
#[cfg(test)]
include!("struct_stage/tests_ownership.rs");
#[cfg(test)]
include!("struct_stage/tests_lifetime_resources.rs");

// P16.6-R04：靜態檢查效能基準與來源正反驗證；不屬公開入口。
#[cfg(test)]
include!("struct_stage/perf_bench.rs");

// P16.6-R02：公開與私有解析路徑交集的差異契約與入口隔離回歸。
#[cfg(test)]
include!("struct_stage/parity_tests.rs");

// P16.6-R03：相同來源身分的 UTF-8 事件與借用／資源責任表對照。
#[cfg(test)]
include!("struct_stage/resource_contract_tests.rs");

// F05：歷史共通 ScalarExpr／SharedAst 的 AST 事後投影只做
// 獨立公私差異回歸；正式內部 Checker 與 P16 橋接僅使用
// parser 直接建構的 AST＋VerifiedScalarHir／VerifiedSafetyPlan。
#[cfg(test)]
include!("struct_stage/ir_projection.rs");
