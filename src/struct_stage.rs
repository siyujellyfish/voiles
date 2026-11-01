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

#[derive(Debug, Clone)]
struct Function {
    name: String,
    params: Vec<Parameter>,
    result_ty: String,
    body: Vec<Stmt>,
}

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
    Declare { name: String, ty: String, mutable: bool, init: Option<Term> },
    Assign { name: String, value: Term },
    FieldAssign { receiver: String, field: String, value: Term },
    If {
        branch: crate::common_ir::ConditionalBranch<Term, Vec<Stmt>>,
        no: Option<Vec<Stmt>>,
    },
    /// P15：只在能證明反向邊的所有權狀態不變時接受的 while 子集。
    While { condition: Term, body: Vec<Stmt> },
    /// D04 右端點不含的整數常值區間；P15 僅建構靜態借用證據。
    ForRange { iterator: String, iterator_start: usize,
        lower: String, upper: String, body: Vec<Stmt> },
    Return(Term),
    /// 既有 for／while 的最內層控制轉移；無迴圈時必須拒絕。
    Break,
    Continue,
}

/// 私有受限運算式與公開 Expr 使用同一 AST 外殼，仍保留私有節點種類。
type Term = crate::common_ir::SpannedNode<TermKind>;

#[derive(Debug, Clone)]
enum TermKind {
    Literal(&'static str),
    Name(String),
    /// P01 已核准的 int 加減；計算前必須依序驗證兩端所有權。
    BinaryInt(crate::common_ir::ArithmeticNode<Term>),
    Construct(String, Vec<(String, Term)>),
    /// 一般函式位置引數；呼叫端所有權資料流完成前不得接受。
    Call(String, Vec<Term>),
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

#[derive(Clone)]
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
    /// 正式私有 Checker 的詞彙已解析宣告及受限型別，與 P09→P10/P11
    /// 共用同一 HIR 引用契約；實際所有權安全仍以本綁定的 ValueSlot/loan 為準。
    fn semantic_use_at(&self, span: crate::common_ir::NodeSpan)
        -> crate::common_ir::TypedUse<&str> {
        crate::common_ir::TypedUse::new(
            span, crate::common_ir::ResolvedUse::new(
                self.declaration_start, self.ty.as_str(),
            ),
        )
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
    recorded_declarations: RefCell<BTreeMap<DeclarationId, (String, bool)>>,
    /// F11：具名 return .move() 經來源身分、型別、借用與有效期驗證後的證據。
    recorded_named_returns: RefCell<BTreeSet<(DeclarationId, usize)>>,
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

/// F12：無 Checker 已驗證轉移事實時不得寫入任何資源安全帳簿。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifiedMoveError {
    MissingEvidence,
    Pipeline(crate::resource_borrow_pipeline::PipelineError),
}

struct VerifiedDeclarationFacts {
    by_source: BTreeMap<DeclarationId, (String, bool)>,
    /// 目前僅記錄 Checker 已接受、同一函式直線語句中的具名整筆移動。
    direct_moves: BTreeSet<(DeclarationId, DeclarationId)>,
    /// 僅保存 Checker 成功提交的具名整筆返回來源及 return 運算式位置。
    named_returns: BTreeSet<(DeclarationId, usize)>,
}
impl VerifiedDeclarationFacts {
    /// F11：本表僅消費 Checker 成功的宣告與直接轉移證據；
    /// 不能再從 AST 或名稱文字自行重建安全事實。
    fn from_checker(
        by_source: BTreeMap<DeclarationId, (String, bool)>,
        direct_moves: BTreeSet<(DeclarationId, DeclarationId)>,
        named_returns: BTreeSet<(DeclarationId, usize)>,
    ) -> Self {
        Self { by_source, direct_moves, named_returns }
    }

    /// 一筆具名返回須由 Checker 在真正成功驗證 `return name.move()`
    /// 後按來源 DeclarationId 及原始運算式位置直接簽出證據。
    fn verified_named_return(
        &self, source: DeclarationId, return_start: usize, ty: &str,
    ) -> Option<VerifiedReturnEvidence> {
        self.by_source.get(&source)
            .filter(|(actual, _)| actual == ty)?;
        self.named_returns.contains(&(source, return_start))
            .then_some(VerifiedReturnEvidence { source, return_start })
    }

    fn verified_direct_move(
        &self, source: DeclarationId, target: DeclarationId,
        ty: &str,
    ) -> Option<VerifiedMoveEvidence> {
        let (from_type, _) = self.by_source.get(&source)?;
        let (to_type, _) = self.by_source.get(&target)?;
        (source != target && from_type == ty && to_type == ty
            && self.direct_moves.contains(&(source, target)))
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
        self.by_source.get(&source)
            .is_some_and(|(actual, can_write)| actual == ty && *can_write == mutable)
    }
}

struct VerifiedPrivateSource {
    source: Source,
    structs: HashMap<String, StructDecl>,
    functions: Vec<Function>,
    declarations: VerifiedDeclarationFacts,
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
        recorded_named_returns: RefCell::new(BTreeSet::new()),
    };
    checker.all(&functions)?;
    let by_source = checker.recorded_declarations.into_inner();
    let direct_moves = checker.recorded_direct_moves.into_inner();
    let named_returns = checker.recorded_named_returns.into_inner();
    let declarations = VerifiedDeclarationFacts::from_checker(
        by_source, direct_moves, named_returns);
    Ok(VerifiedPrivateSource { source, structs, functions, declarations })
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

// P16.6-R02.2：僅投影已核准公開／私有交集，不放寬 P13 私有邊界。
include!("struct_stage/ir_projection.rs");
