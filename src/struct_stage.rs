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
    /// 真正共通 ParsedDeclaration 的來源跨度，含 var/const。
    declaration_span: crate::common_ir::NodeSpan,
    /// 由原始 Parser 消耗的兩個具名 token 直接取得，不以字串搜尋推算。
    name_span: crate::common_ir::NodeSpan,
    type_span: crate::common_ir::NodeSpan,
}

#[derive(Debug, Clone)]
struct StructDecl {
    fields: Vec<Field>,
    methods: Vec<Function>,
    /// struct 宣告含左、右大括號的原始 UTF-8 半開跨度。
    source_span: crate::common_ir::NodeSpan,
    // 解析期維持原宣告順序，另建只讀名稱索引以減少重複線性搜尋。
    field_index: HashMap<String, usize>,
    /// S20：同名方法按原宣告順序保留全部候選，禁止 HashMap 覆寫。
    method_index: HashMap<String, Vec<usize>>,
}
impl StructDecl {
    fn new(
        fields: Vec<Field>, methods: Vec<Function>,
        source_span: crate::common_ir::NodeSpan,
    ) -> Self {
        let field_index = fields.iter().enumerate()
            .map(|(index, field)| (field.name.clone(), index)).collect();
        let mut method_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, method) in methods.iter().enumerate() {
            method_index.entry(method.name.clone()).or_default().push(index);
        }
        Self { fields, methods, source_span, field_index, method_index }
    }
    fn field(&self, name: &str) -> Option<&Field> {
        self.field_index.get(name).and_then(|index| self.fields.get(*index))
    }
    /// 只在已確認同名方法唯一時提供原有簡易查找，不能將多載
    /// 中最後一個同名方法當成唯一結果（舊 HashMap 覆寫問題）。
    fn method(&self, name: &str) -> Option<&Function> {
        let indices = self.method_index.get(name)?;
        (indices.len() == 1).then(|| self.methods.get(indices[0])).flatten()
    }
    /// D18-02-A：只讀來源宣告順序候選，真正選擇須由 Checker
    /// 的參數數量／完整靜態型別匹配，不能靠運行時值推斷。
    fn methods_named(&self, name: &str) -> impl Iterator<Item = &Function> + '_ {
        self.method_index.get(name).into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|index| self.methods.get(*index))
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
    /// 由真正 Parser 在名稱與型別 token 消耗時原位記錄。
    declaration_span: crate::common_ir::NodeSpan,
    name_span: crate::common_ir::NodeSpan,
    type_span: crate::common_ir::NodeSpan,
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
        no: Option<crate::syntax_shared::ParsedElse<Stmt>>,
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

// F02：本 enum 的已核准公私交集由 common_ir 唯一宣告產生。
crate::common_ir::define_shared_expression_kind!(
    TermKind, Term, Vec<Term>, [Debug, Clone], {
        /// 保留私有 decimal64、str 與既有 Rust 內部 AST 相容入口。
        Literal(&'static str),
        Construct(String, Vec<(String, Term)>),
        /// A2/F02：裸 name() 在 Parser 完成時尚不能知道是 struct
        /// 或前向宣告的零參數 fn。正式 AST 明示待語意解析，
        /// 只有 Checker 的符號權威可一次分類；不得猜測新語法。
        UnresolvedZeroArgs(String),
        /// 函式具位置引數的獨立 P13 已核准語意。
        Move(String),
        Borrow(String),
        FieldMove(String, String),
        FieldRead(String, String),
        MethodCall(String, String, Vec<Term>),
    }
);

struct Parser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    cursor: usize,
}

// 受限私有文法解析實作；目前使用同層 include! 以保持所有既有型別可見性。
include!("struct_stage/syntax.rs");

// P17-C：只有完整私有 Checker 安全驗證後才可實際分配資源。
include!("struct_stage/execute_str.rs");

// P17-F：只有真私有來源才可構成混合欄位執行計畫。
include!("struct_stage/execute_struct.rs");

// G17／P17.3：同一私有 Checker 的直線借用票據與真 UTF-8 資源實驗。
include!("struct_stage/execute_borrow.rs");

// G18／P18-B/C：已驗證有限 for／while 真控制與 P16 UTF-8 清理。
include!("struct_stage/execute_loop.rs");

// G18／R05：各執行器共同核實真函式進退、回滾與票據歸還。
include!("struct_stage/shared_call_exit.rs");
// G18／P18-D：原 Checker 每次函式呼叫借用票據真正執行和出口歸還。
include!("struct_stage/execute_call_borrow.rs");
include!("struct_stage/execute_call_scalar.rs");
// G18／P18.3：原 Checker bool 參數直接／互相遞迴真 UTF-8 逐框架作用域清理。
include!("struct_stage/execute_recursive_str.rs");
// G18／P18.6：正式 Checker 已核准同步唯讀 self 的真 BorrowTicket。
include!("struct_stage/execute_method_self.rs");

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoanBinding {
    fact: BorrowFact,
    active: bool,
}

/// F08/B05：真正私有 Checker 的單一可寫靜態安全狀態。
/// 借用、初始化和部分欄位責任均由同一份 Binding 持有；
/// 此結構不儲存真 BorrowTicket、ResourceId 或執行期事件。
#[derive(Clone)]
struct SafetyState {
    value: ValueSlot,
    /// 僅以合法 struct 建立式已知的未初始化 var 欄位集合。
    /// None 表示不存在可追蹤的部分建立證明；不等於全部欄位可用。
    pending_fields: Option<HashSet<String>>,
    /// 非擁有的借用別名，不擁有來源值，不允許 .move()。
    loan: Option<LoanBinding>,
}

#[derive(Clone)]
struct Binding {
    declaration_start: DeclarationId,
    ty: String,
    mutable: bool,
    safety: SafetyState,
}

// 既有 Checker 的型別、來源與診斷操作先沿用欄位名稱，
// 實際所有靜態值／借用／部分初始化資料已唯一存放於 SafetyState。
// 後續 F09 將逐一縮小寫入入口至預檢後的不可失敗提交。
impl std::ops::Deref for Binding {
    type Target = SafetyState;
    fn deref(&self) -> &Self::Target { &self.safety }
}
impl std::ops::DerefMut for Binding {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.safety }
}



/// F08/B05：唯一真正可寫 Binding 的**唯讀**靜態安全狀態視圖。
/// 此視圖借用現有資料而不複製所有權或借用票據；不能建立平行帳簿。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SafetyStateView<'a> {
    declaration: DeclarationId,
    ty: &'a str,
    mutable: bool,
    value: ValueSlot,
    pending_fields: Option<&'a HashSet<String>>,
    loan: Option<&'a LoanBinding>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
enum StaticTransition<'a> {
    MoveWhole,
    CommitRebindChecked,
    InitializeField(&'a str),
    ExpireAliasChecked,
    /// F13/B10：只有 Checker 已決定可達分支、完成全部型別與欄位
    /// 責任預檢後才可提交；三個靜態安全維度必須一起更新。
    JoinReachable {
        value: ValueSlot,
        pending_fields: Option<HashSet<String>>,
        loan_active: Option<bool>,
    },
}

impl SafetyState {
    /// F09/B06：唯一靜態可寫安全狀態的受限轉移核心。
    /// 移動及欄位初始化先唯讀驗證，成功後不再有可回傳錯誤的步驟。
    /// 本模型不簽發或歸還真 BorrowTicket，也不產生 ResourceEvent。
    fn transition(&mut self, action: StaticTransition<'_>) -> Result<bool, ModelError> {
        match action {
            StaticTransition::MoveWhole => {
                self.value.move_whole()?;
                Ok(true)
            }
            StaticTransition::CommitRebindChecked => {
                self.value.mark_reinitialized();
                self.pending_fields = None;
                Ok(true)
            }
            StaticTransition::InitializeField(field) => {
                if self.value.state() != ValueState::Uninitialized
                    || !self.pending_fields.as_ref()
                        .is_some_and(|pending| pending.contains(field)) {
                    return Ok(false);
                }
                let pending = self.pending_fields.as_mut()
                    .expect("已完整預檢的待初始化欄位集合");
                pending.remove(field);
                if pending.is_empty() {
                    // 只在所有欄位均完整初始化後，才一起恢復整筆可讀。
                    self.value.mark_reinitialized();
                    self.pending_fields = None;
                }
                Ok(true)
            }
            StaticTransition::ExpireAliasChecked => {
                if let Some(loan) = self.loan.as_mut() {
                    if loan.active {
                        loan.active = false;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            StaticTransition::JoinReachable {
                value, pending_fields, loan_active,
            } => {
                // 錯誤的有無 loan 證據不可先改值／欄位再發現失配。
                if loan_active.is_some() != self.loan.is_some() {
                    return Ok(false);
                }
                // 以上完成所有可失敗預檢；以下提交不再回傳 Err。
                self.value = value;
                self.pending_fields = pending_fields;
                if let (Some(loan), Some(active)) = (self.loan.as_mut(), loan_active) {
                    loan.active = active;
                }
                Ok(true)
            }
        }
    }
}

impl Binding {
    fn new(
        declaration_start: DeclarationId, ty: String, mutable: bool,
        value: ValueSlot, pending_fields: Option<HashSet<String>>,
        loan: Option<LoanBinding>,
    ) -> Self {
        Self {
            declaration_start, ty, mutable,
            safety: SafetyState { value, pending_fields, loan },
        }
    }

    /// 完整由唯一 SafetyState 操作；錯誤時維持來源與欄位責任原狀。
    fn move_whole(&mut self) -> Result<(), ModelError> {
        self.safety.transition(StaticTransition::MoveWhole).map(|_| ())
    }

    /// 原 Checker 已完成 RHS、var 與資源覆寫預檢後唯一合法提交入口。
    fn commit_rebind(&mut self) {
        self.safety.transition(StaticTransition::CommitRebindChecked)
            .expect("已驗證重新綁定提交不得失敗");
    }

    /// 首次寫入合法欄位、拒絕重複或未知欄位且絕不半提交。
    fn commit_first_field(&mut self, field: &str) -> bool {
        self.safety.transition(StaticTransition::InitializeField(field))
            .expect("部分欄位安全轉移不得失敗")
    }

    /// 只有明確不是函式參數／self 的已授權別名才能於最後使用點失效。
    /// 此處只改變靜態有效期，原執行期 BorrowTicket 由呼叫端 tracker 歸還。
    fn expire_alias_checked(&mut self) -> bool {
        if self.safety.loan.as_ref().is_some_and(|loan|
            loan.fact.source != self.declaration_start && loan.active) {
            self.safety.transition(StaticTransition::ExpireAliasChecked)
                .expect("已驗證別名有效期轉移不得失敗")
        } else { false }
    }

    /// F13/B10：真正 CFG 來源 Checker 的一組合流安全責任一次提交。
    /// 此函式只是唯一 SafetyState Transition 的轉接，不維護第二本表。
    fn commit_reachable_join(
        &mut self, value: ValueSlot, pending_fields: Option<HashSet<String>>,
        loan_active: Option<bool>,
    ) -> bool {
        self.safety.transition(StaticTransition::JoinReachable {
            value, pending_fields, loan_active,
        }).expect("完成來源預檢後的合流提交不得失敗")
    }

    /// F08／F10：所有 CFG 反向邊、break／continue 出口及單次 for
    /// 分支合流都必須使用同一份來源安全狀態相等契約。除了原有
    /// ValueSlot、待初始化欄位和借用有效期，也要驗證真正宣告
    /// 身分、型別及 var/const 權限；不得讓局部比較產生不同權威。
    fn safety_state(&self) -> SafetyStateView<'_> {
        SafetyStateView {
            declaration: self.declaration_start,
            ty: &self.ty,
            mutable: self.mutable,
            value: self.value,
            pending_fields: self.pending_fields.as_ref(),
            loan: self.loan.as_ref(),
        }
    }

    fn same_safety_state(&self, other: &Self) -> bool {
        self.safety_state() == other.safety_state()
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

/// F10/B07：跨作用域反向邊及 break/continue 共用同一安全狀態差異分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OuterSafetyDiff {
    MissingBinding,
    ChangedState,
}

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

/// F11/B08：真正 Checker 在已成功驗證**整個函式**後簽發的
/// 靜態 CFG 退出分類。此證據只是可達性義務，不是已執行返回事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedFunctionExit {
    span: crate::common_ir::NodeSpan,
    flow: FlowState,
}

/// G4 第一批：正式 Checker 真正走訪且安全核對成功的靜態控制轉移。
/// 只有三類明確轉移：return、break、continue；是靜態可達證據，
/// 絕非真實執行 Return、ResourceEvent 或 BorrowTicket。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedControlEdge {
    span: crate::common_ir::NodeSpan,
    flow: FlowState,
}

/// G4 第二批：受限方法與普通函式分開簽出 self 借用模式及
/// 真正成功檢查的各個 return 位置；不暗示方法執行或析構。
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedMethodSafety {
    span: crate::common_ir::NodeSpan,
    self_borrow: BorrowKind,
    checked_returns: BTreeSet<crate::common_ir::NodeSpan>,
}

/// G4 第三批：每條 Checker 真正已安全核對的控制轉移，
/// 封存該位置的完整詞彙 SafetyState 靜態事實。不建立或歸還真票據。
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedExitBinding {
    scope_depth: usize,
    declaration: DeclarationId,
    ty: String,
    mutable: bool,
    value: ValueSlot,
    pending_fields: Option<Vec<String>>,
    loan: Option<LoanBinding>,
}

/// 只對內部靜態計畫表示未來需要的清理，絕不是已執行 Released。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticCleanupNeed {
    Required,
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StaticCleanupObligation {
    declaration: DeclarationId,
    need: StaticCleanupNeed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedExitSnapshot {
    edge: VerifiedControlEdge,
    /// 與當時同一份 Checker Scopes 的逐深度宣告數量對帳，
    /// 避免刪除一個持有者後由空集合假稱整個返回責任閉合。
    scope_sizes: Vec<usize>,
    bindings: Vec<VerifiedExitBinding>,
    /// 仍持有可清理資源的靜態 Owner；不含返回時已完成 move 的來源。
    cleanup_owners: Vec<StaticCleanupObligation>,
    /// 還存活的靜態借用別名，不含 tracker serial 或真 BorrowTicket。
    active_borrows: Vec<(DeclarationId, BorrowFact)>,
}

impl VerifiedExitSnapshot {
    fn cleanup_from_bindings(bindings: &[VerifiedExitBinding])
        -> Vec<StaticCleanupObligation> {
        bindings.iter().filter_map(|binding| {
            if binding.loan.is_some()
                || binding.value.copy_permission() == CopyPermission::Allowed {
                return None;
            }
            let need = match binding.value.state() {
                ValueState::Available => StaticCleanupNeed::Required,
                ValueState::MayBeUnavailable => StaticCleanupNeed::Conditional,
                ValueState::Uninitialized if binding.pending_fields.is_some() =>
                    StaticCleanupNeed::Conditional,
                ValueState::Uninitialized | ValueState::Moved => return None,
            };
            Some(StaticCleanupObligation {
                declaration: binding.declaration, need,
            })
        }).collect()
    }

    fn active_borrows_from_bindings(bindings: &[VerifiedExitBinding])
        -> Vec<(DeclarationId, BorrowFact)> {
        bindings.iter().filter_map(|binding|
            binding.loan.as_ref().filter(|loan| loan.active)
                .map(|loan| (binding.declaration, loan.fact))).collect()
    }

    /// 只有真 Checker 於每次出口呼叫此建構器；由同一 Scopes
    /// 正規化狀態及清理義務，不允許橋接依 AST 繞過來源身分認領。
    fn from_checked_scopes(
        vars: &Scopes, span: crate::common_ir::NodeSpan, flow: FlowState,
    ) -> Self {
        let mut bindings: Vec<VerifiedExitBinding> = vars.iter().enumerate()
            .flat_map(|(depth, scope)| scope.values().map(move |binding| {
                let pending_fields = binding.pending_fields.as_ref().map(|fields| {
                    let mut sorted: Vec<String> = fields.iter().cloned().collect();
                    sorted.sort();
                    sorted
                });
                VerifiedExitBinding {
                    scope_depth: depth,
                    declaration: binding.declaration_start,
                    ty: binding.ty.clone(),
                    mutable: binding.mutable,
                    value: binding.value,
                    pending_fields,
                    loan: binding.loan.clone(),
                }
            })).collect();
        bindings.sort_by_key(|binding| (binding.scope_depth, binding.declaration));
        let cleanup_owners = Self::cleanup_from_bindings(&bindings);
        let active_borrows = Self::active_borrows_from_bindings(&bindings);
        Self {
            edge: VerifiedControlEdge { span, flow },
            scope_sizes: vars.iter().map(|scope| scope.len()).collect(),
            bindings, cleanup_owners, active_borrows,
        }
    }
}

struct Checker<'a, C> {
    source: &'a Source,
    structs: &'a HashMap<String, StructDecl>,
    functions: &'a [Function],
    function_index: HashMap<String, usize>,
    ability: C,
    /// F11 前置：真實 Checker 成功評估並完成宣告後記錄的直線具名移交。
    recorded_function_exits: RefCell<BTreeMap<usize, VerifiedFunctionExit>>,
    recorded_control_edges: RefCell<BTreeMap<usize, VerifiedControlEdge>>,
    recorded_control_conflicts: RefCell<BTreeSet<usize>>,
    recorded_method_safety: RefCell<BTreeMap<usize, VerifiedMethodSafety>>,
    recorded_method_safety_conflicts: RefCell<BTreeSet<usize>>,
    recorded_exit_snapshots: RefCell<BTreeMap<usize, Vec<VerifiedExitSnapshot>>>,
    recorded_direct_moves: RefCell<BTreeSet<(DeclarationId, DeclarationId)>>,
    recorded_direct_borrows: RefCell<BTreeMap<DeclarationId, BorrowFact>>,
    /// 真正通過 Checker 的參數、宣告與迭代身分，而不是事後掃描 AST。
    recorded_declarations: RefCell<BTreeMap<DeclarationId, crate::common_ir::TypedDeclaration<String>>>,
    /// F01：真正私有 Checker 已驗證的欄位／方法及函式參數
    /// 名稱、型別 token 來源身分。唯讀型別註記，不是資源持有者。
    recorded_member_types: RefCell<BTreeMap<
        crate::common_ir::ModeNodeIdentity, crate::common_ir::VerifiedTypedMember
    >>,
    recorded_member_conflicts: RefCell<BTreeSet<usize>>,
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
    /// 僅保存 Checker 已接受的直線 borrow 宣告：目標 ID 對原來源 BorrowFact。
    /// 不是實際簽發的 BorrowTicket，也不代表巢狀分支一定執行。
    direct_borrows: BTreeMap<DeclarationId, BorrowFact>,
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
            direct_moves, direct_borrows: BTreeMap::new(), named_returns, field_writes }
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
        && self.direct_borrows.iter().all(|(alias, fact)| {
            fact.source != *alias && fact.source.0 < alias.0
                && self.by_source.get(&fact.source).zip(self.by_source.get(alias))
                    .is_some_and(|(from, to)| from.declaration == fact.source
                        && to.declaration == *alias && from.ty == to.ty
                        && from.mutable == to.mutable
                        && matches!((from.mutable, fact.kind),
                            (true, BorrowKind::ExclusiveWrite)
                            | (false, BorrowKind::SharedRead)))
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

    /// A4/F04-04：已經由 Checker 簽出型別與安全前置的資源操作，
    /// 在最終計畫交付時還必須能對回「同一份真正 Parser AST」的操作形狀。
    /// 本稽核只讀取現有 AST 以排除偽造／錯配的操作位置，絕不藉由
    /// AST 自行推論來源持有者、借用票據或執行期 ResourceId。
    fn operations_match_verified_ast(
        &self, structs: &HashMap<String, StructDecl>, functions: &[Function],
    ) -> bool {
        fn visit(
            statements: &[Stmt],
            direct_targets: &mut BTreeSet<DeclarationId>,
            borrow_targets: &mut BTreeSet<DeclarationId>,
            named_returns: &mut BTreeSet<usize>,
            field_writes: &mut BTreeSet<(usize, String)>,
        ) {
            for statement in statements {
                match &statement.kind {
                    StmtKind::Declare(declaration) => {
                        match declaration.initializer.as_ref().map(|term| &term.kind) {
                            Some(TermKind::Move(_)) => {
                                direct_targets.insert(DeclarationId(statement.start));
                            }
                            Some(TermKind::Borrow(_)) => {
                                borrow_targets.insert(DeclarationId(statement.start));
                            }
                            _ => {}
                        }
                    }
                    StmtKind::Return(term) => {
                        if matches!(&term.kind, TermKind::Move(_)) {
                            named_returns.insert(term.start);
                        }
                    }
                    StmtKind::FieldAssign { field, .. } => {
                        field_writes.insert((statement.start, field.clone()));
                    }
                    StmtKind::If { branch, no } => {
                        visit(&branch.body, direct_targets, borrow_targets, named_returns, field_writes);
                        if let Some(otherwise) = no {
                            visit(otherwise.as_slice(), direct_targets, borrow_targets, named_returns, field_writes);
                        }
                    }
                    StmtKind::While { body, .. } | StmtKind::ForRange { body, .. } => {
                        visit(body, direct_targets, borrow_targets, named_returns, field_writes);
                    }
                    StmtKind::Assign(_) | StmtKind::Break | StmtKind::Continue => {}
                }
            }
        }

        let mut direct_targets = BTreeSet::new();
        let mut borrow_targets = BTreeSet::new();
        let mut named_returns = BTreeSet::new();
        let mut field_writes = BTreeSet::new();
        for function in functions.iter().chain(
            structs.values().flat_map(|definition| definition.methods.iter())
        ) {
            visit(&function.body, &mut direct_targets, &mut borrow_targets, &mut named_returns, &mut field_writes);
        }
        self.direct_moves.iter().all(|(_, target)| direct_targets.contains(target))
            && self.direct_borrows.keys().all(|target| borrow_targets.contains(target))
            && self.named_returns.iter().all(|(_, at)| named_returns.contains(at))
            && self.field_writes.iter().all(|(_, at, field)|
                field_writes.contains(&(*at, field.clone())))
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
        hir.source_spans_are_valid_for_mode(
            crate::common_ir::FrontendMode::PrivateVerified, text,
        ) && hir.verified_references().all(|entry| {
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
    /// 不可從真 AST 或橋接倒推；唯一從 Checker::function 成功
    /// 的整個函式控制流驗證出口取得。方法另保留原受限安全路徑。
    function_exits: BTreeMap<usize, VerifiedFunctionExit>,
    /// 每筆皆由真 Checker 已成功驗證的 return/break/continue
    /// 原位交付，供正式安全計畫唯讀核對原 Parser AST 位置。
    control_edges: BTreeMap<usize, VerifiedControlEdge>,
    /// 方法的 self/參數受限 safety 授權只由 Checker::check_method 完成後認領。
    method_safety: BTreeMap<usize, VerifiedMethodSafety>,
    /// 控制轉移可達時由唯一 Checker 詞彙 Scopes 直接封存的純靜態狀態；
    /// 同一位置不同可達路徑保留不同候選，絕不拼成單一執行序列。
    exit_snapshots: BTreeMap<usize, Vec<VerifiedExitSnapshot>>,
    scalar_hir: crate::common_ir::VerifiedScalarHir,
    /// F01/F04：僅由真 Checker 驗證欄位／參數後簽出、與借用
    /// 資源持有者分離的型別 token 來源證據。
    members: BTreeMap<
        crate::common_ir::ModeNodeIdentity, crate::common_ir::VerifiedTypedMember
    >,
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
        members: BTreeMap<
            crate::common_ir::ModeNodeIdentity, crate::common_ir::VerifiedTypedMember
        >,
    ) -> Self {
        let expression_spans =
            scalar_hir.verified_expressions().map(|fact| fact.span).collect();
        let reference_spans =
            scalar_hir.verified_references().map(|fact| fact.span).collect();
        let assignment_spans =
            scalar_hir.verified_assignments().map(|fact| fact.span).collect();
        Self { declarations, scalar_hir, members,
            function_exits: BTreeMap::new(),
            control_edges: BTreeMap::new(),
            method_safety: BTreeMap::new(),
            exit_snapshots: BTreeMap::new(),
            expression_spans, reference_spans, assignment_spans }
    }

    /// 公私共通 SourceIdentity 契約的私有 typed 成員核對。
    /// 唯一真 Checker 從已核准型別收集的事實不可被移除、
    /// 更改型別、冒充其它模式或虛構未出現的欄位/參數。
    /// 不把欄位註記插入可借用的 Binding 來源表。
    fn member_sources_are_consistent(
        &self, structs: &HashMap<String, StructDecl>,
        functions: &[Function], text: &str,
    ) -> bool {
        use crate::common_ir::{NodeRole, VerifiedTypedMember};
        let mut expected = BTreeMap::new();
        let mut add = |role, span, name: &str, name_span, ty: &str, type_span, mutable| {
            let Some(fact) = VerifiedTypedMember::checked_private(
                text, role, span, name, name_span, ty, type_span, mutable,
            ) else { return false; };
            expected.insert(fact.identity, fact).is_none()
        };
        let fields_valid = structs.values().all(|definition| {
            definition.fields.iter().all(|field| add(
                NodeRole::Field, field.declaration_span,
                &field.name, field.name_span,
                &field.ty, field.type_span, !field.immutable,
            ))
        });
        if !fields_valid { return false; }
        let params_valid = functions.iter().chain(
            structs.values().flat_map(|def| def.methods.iter())
        ).all(|fun| {
            fun.params.iter().all(|parameter| add(
                NodeRole::Parameter, parameter.declaration_span,
                &parameter.name, parameter.name_span,
                &parameter.ty, parameter.type_span, false,
            ))
        });
        if !params_valid || self.members != expected
            || !self.members.values().all(|fact| fact.source_is_consistent(text)) {
            return false;
        }
        // 一般函式參數的詞彙 Binding 仍必須來自相同 Checker 的
        // 已解析 DeclarationId；不准兩份型別表以相同名稱投票。
        functions.iter().flat_map(|fun| fun.params.iter()).all(|parameter| {
            self.declarations.typed_declaration(
                DeclarationId(parameter.declaration_start),
            ).is_some_and(|binding|
                binding.ty == parameter.ty
                    && !binding.mutable
                    && binding.declaration.0 == parameter.declaration_span.start
            )
        })
    }

    /// 唯讀交叉核對：只接受原 AST 真正存在且跨度吻合的控制語句，
    /// 不從 AST 建立缺失的 Checker 退出證據，也不猜測執行順序。
    fn control_edge_matches_ast(body: &[Stmt], edge: VerifiedControlEdge) -> bool {
        body.iter().any(|stmt| {
            let same_span = stmt.start == edge.span.start && stmt.end == edge.span.end;
            let same_flow = matches!(
                (&stmt.kind, edge.flow),
                (StmtKind::Return(_), FlowState::Returned)
                    | (StmtKind::Break, FlowState::Break)
                    | (StmtKind::Continue, FlowState::Continue)
            );
            (same_span && same_flow) || match &stmt.kind {
                StmtKind::If { branch, no } =>
                    Self::control_edge_matches_ast(&branch.body, edge)
                        || no.as_ref().is_some_and(|otherwise|
                            Self::control_edge_matches_ast(otherwise, edge)),
                StmtKind::ForRange { body, .. } | StmtKind::While { body, .. } =>
                    Self::control_edge_matches_ast(body, edge),
                _ => false,
            }
        })
    }

    fn control_edges_are_consistent(&self, functions: &[Function], text: &str) -> bool {
        self.control_edges.iter().all(|(&start, &edge)| {
            edge.span.start == start
                && edge.span.start < edge.span.end
                && edge.span.end <= text.len()
                && text.is_char_boundary(edge.span.start)
                && text.is_char_boundary(edge.span.end)
                && matches!(edge.flow, FlowState::Returned
                    | FlowState::Break | FlowState::Continue)
                && functions.iter().filter(|fun|
                    Self::control_edge_matches_ast(&fun.body, edge)).count() == 1
        }) && functions.iter().all(|fun|
            self.control_edges.values().any(|edge|
                edge.flow == FlowState::Returned
                    && Self::control_edge_matches_ast(&fun.body, *edge)))
    }

    /// 唯讀模型交叉拒假：方法中的寫入若已由 Checker 安全核准，
    /// self 借用模式不得被轉成唯讀；中途確定 return 後不再猜測後繼。
    fn method_requires_write(body: &[Stmt]) -> bool {
        let mut writable = false;
        for statement in body {
            match &statement.kind {
                StmtKind::FieldAssign { receiver, .. } if receiver == "self" =>
                    writable = true,
                StmtKind::If { branch, no } => {
                    writable |= Self::method_requires_write(&branch.body);
                    if let Some(otherwise) = no {
                        writable |= Self::method_requires_write(otherwise);
                    }
                }
                StmtKind::Return(_) => return writable,
                _ => {}
            }
        }
        writable
    }

    fn method_safety_is_consistent(
        &self, structs: &HashMap<String, StructDecl>, text: &str,
    ) -> bool {
        let methods: Vec<&Function> = structs.values()
            .flat_map(|definition| definition.methods.iter()).collect();
        self.method_safety.len() == methods.len()
            && methods.iter().all(|method| {
                let Some(fact) = self.method_safety.get(&method.start) else {
                    return false;
                };
                let final_span = method.body.last().map(|statement| statement.node_span());
                fact.span == crate::common_ir::NodeSpan::new(method.start, method.end)
                    && fact.span.start < fact.span.end
                    && fact.span.end <= text.len()
                    && text.is_char_boundary(fact.span.start)
                    && text.is_char_boundary(fact.span.end)
                    && final_span.is_some_and(|span| fact.checked_returns.contains(&span))
                    && !fact.checked_returns.is_empty()
                    && fact.checked_returns.iter().all(|&span|
                        Self::control_edge_matches_ast(&method.body,
                            VerifiedControlEdge { span, flow: FlowState::Returned }))
                    && fact.self_borrow == if Self::method_requires_write(&method.body) {
                        BorrowKind::ExclusiveWrite
                    } else {
                        BorrowKind::SharedRead
                    }
            })
    }

    /// 真 Checker 出口的宣告／所有權／有效借用狀態只可唯讀對照，
    /// 不把可能執行分支的快照當成任一已發生的 ResourceEvent。
    fn exit_snapshots_are_consistent(&self) -> bool {
        self.exit_snapshots.len() == self.control_edges.len()
            && self.control_edges.iter().all(|(&start, &edge)| {
                let Some(alternatives) = self.exit_snapshots.get(&start) else {
                    return false;
                };
                !alternatives.is_empty() && alternatives.iter().enumerate().all(|(i, snapshot)| {
                    if snapshot.edge != edge
                        || alternatives[..i].iter().any(|older| older == snapshot) {
                        return false;
                    }
                    let bindings = &snapshot.bindings;
                    let size_matches = !snapshot.scope_sizes.is_empty()
                        && snapshot.scope_sizes.iter().sum::<usize>() == bindings.len()
                        && snapshot.scope_sizes.iter().enumerate().all(|(depth, expected)|
                            bindings.iter().filter(|item| item.scope_depth == depth).count()
                                == *expected);
                    let ordered = bindings.windows(2).all(|pair|
                        (pair[0].scope_depth, pair[0].declaration)
                            < (pair[1].scope_depth, pair[1].declaration));
                    let typed = bindings.iter().all(|item|
                        self.declarations.typed_declaration(item.declaration)
                            .is_some_and(|fact|
                                fact.ty == item.ty.as_str()
                                    && fact.mutable == item.mutable));
                    let valid_loans = StaticBorrowSnapshot::from_aliases(bindings.iter()
                        .filter_map(|item| item.loan.as_ref().map(|loan|
                            (item.declaration, loan.fact, loan.active))))
                        .validate().is_ok();
                    let obligations = snapshot.cleanup_owners
                        == VerifiedExitSnapshot::cleanup_from_bindings(bindings)
                        && snapshot.active_borrows
                            == VerifiedExitSnapshot::active_borrows_from_bindings(bindings);
                    size_matches && ordered && typed && valid_loans && obligations
                })
            })
    }

    /// 同一完整來源與同一 Checker 交付的兩份安全證據一起核對；
    /// 不允許橋接只看其中一份而忽略不一致的型別／借用權威。
    fn is_consistent(
        &self, structs: &HashMap<String, StructDecl>, functions: &[Function],
        text: &str,
    ) -> bool {
        // F11：每一個被原 Checker 接受的函式都必須有唯一的真
        // 可達退出分類；不能刪掉其中一條再拿剩餘 AST 重建。
        // 靜態返回不意味真的執行返回或釋放 ResourceId。
        self.function_exits.len() == functions.len()
            && functions.iter().all(|function| {
                self.function_exits.get(&function.start).is_some_and(|exit| {
                    exit.span == crate::common_ir::NodeSpan::new(
                        function.start, function.end)
                        && exit.flow == FlowState::Returned
                        && function.start < function.end
                        && function.end <= text.len()
                        && text.is_char_boundary(function.start)
                        && text.is_char_boundary(function.end)
                })
            })
            && self.control_edges_are_consistent(functions, text)
            && self.method_safety_is_consistent(structs, text)
            && self.exit_snapshots_are_consistent()
            && self.scalar_hir.mode_is(crate::common_ir::FrontendMode::PrivateVerified)
            && self.declarations.evidence_is_consistent(structs, text)
            && self.declarations.operations_match_verified_ast(structs, functions)
            && self.member_sources_are_consistent(structs, functions, text)
            && self.declarations.scalar_hir_is_consistent(&self.scalar_hir, text)
            && verified_private_syntax_roles(text, structs, functions)
                .is_some_and(|roles|
                    crate::common_ir::syntax_covers_verified_scalar_hir(
                        &roles, &self.scalar_hir,
                        crate::common_ir::FrontendMode::PrivateVerified, text,
                    ))
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
    /// 保留同一正式 Lexer 交給私有 Parser 的原始 token，避免第二套解析。
    tokens: Vec<Token>,
    structs: HashMap<String, StructDecl>,
    functions: Vec<Function>,
    /// 與真正通過驗證的完整原始來源一起封裝，不能獨立從 AST 重建。
    plan: VerifiedSafetyPlan,
}

// F01：私有真正 Parser 產生後的所有來源角色身分均用相同契約。
include!("struct_stage/source_identity.rs");
include!("struct_stage/token_provenance.rs");

fn verify_source_core_with_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<VerifiedPrivateSource, Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() { return Err(error); }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() { return Err(error); }
    let (structs, functions) =
        Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file()?;
    // F01：正式私有 Parser 的函式與區塊使用共同模式身分驗證，
    // 來源跨度必須是真實 UTF-8 半開範圍；保留原私有安全語意。
    if functions.iter().chain(
        structs.values().flat_map(|definition| definition.methods.iter())
    ).any(|fun|
        fun.checked_identity(crate::common_ir::FrontendMode::PrivateVerified, text)
            .is_none()
        || !fun.body.checked_direct_statements(
            crate::common_ir::FrontendMode::PrivateVerified, text)
    ) {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "私有函式或區塊的原始來源身分不合法".into(),
            span: None,
        });
    }
    if !verified_private_syntax_tree(text, &structs, &functions)
        || !verified_private_source_tokens(text, &tokens.tokens, &structs, &functions) {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "私有 AST 的結構、函式標頭、宣告型別 token 或巢狀來源身分不一致".into(),
            span: None,
        });
    }
    let function_index = functions.iter().enumerate()
        .map(|(i, f)| (f.name.clone(), i)).collect();
    let checker = Checker {
        source: &source, structs: &structs, functions: &functions,
        function_index, ability,
        recorded_function_exits: RefCell::new(BTreeMap::new()),
        recorded_control_edges: RefCell::new(BTreeMap::new()),
        recorded_control_conflicts: RefCell::new(BTreeSet::new()),
        recorded_method_safety: RefCell::new(BTreeMap::new()),
        recorded_method_safety_conflicts: RefCell::new(BTreeSet::new()),
        recorded_exit_snapshots: RefCell::new(BTreeMap::new()),
        recorded_direct_moves: RefCell::new(BTreeSet::new()),
        recorded_direct_borrows: RefCell::new(BTreeMap::new()),
        recorded_declarations: RefCell::new(BTreeMap::new()),
        recorded_member_types: RefCell::new(BTreeMap::new()),
        recorded_member_conflicts: RefCell::new(BTreeSet::new()),
        recorded_declaration_conflicts: RefCell::new(BTreeSet::new()),
        recorded_named_returns: RefCell::new(BTreeSet::new()),
        recorded_field_writes: RefCell::new(BTreeSet::new()),
        recorded_scalar_hir: RefCell::new(crate::common_ir::VerifiedScalarHir::for_mode(
            crate::common_ir::FrontendMode::PrivateVerified)),
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
    if !checker.recorded_member_conflicts.borrow().is_empty() {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：私有欄位或參數名稱與型別來源證據衝突".into(),
            span: None,
        });
    }
    if !checker.recorded_control_conflicts.borrow().is_empty() {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：同一來源的 Checker 靜態轉移不可被兩種不相容的控制出口認領".into(),
            span: None,
        });
    }
    if !checker.recorded_method_safety_conflicts.borrow().is_empty() {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：同一方法的 Checker self 借用或退出義務出現來源衝突".into(),
            span: None,
        });
    }
    let members = checker.recorded_member_types.into_inner();
    let by_source = checker.recorded_declarations.into_inner();
    let direct_moves = checker.recorded_direct_moves.into_inner();
    let direct_borrows = checker.recorded_direct_borrows.into_inner();
    let named_returns = checker.recorded_named_returns.into_inner();
    let field_writes = checker.recorded_field_writes.into_inner();
    let scalar_hir = checker.recorded_scalar_hir.into_inner();
    let mut declarations = VerifiedDeclarationFacts::from_checker(
        by_source, direct_moves, named_returns, field_writes);
    // 原始 F11 四參數建構契約不變；這份借用事實只能由 Checker 出口交付。
    declarations.direct_borrows = direct_borrows;
    // 只由真正 Checker 在每次已驗證的呼叫出口交付來源事實，
    // 不從任何 AST 呼叫外形或模擬資源事件逆向重建。
    declarations.call_borrows = checker.recorded_call_borrows.into_inner();
    // 雙帳簿與 P16 來源橋接只能拿到整份通過同一交叉驗證的計畫。
    // 真正 Checker 的資訊由此單一所有權移入，不能傳遞獨立可換的兩本表。
    let mut plan = VerifiedSafetyPlan::from_checker(declarations, scalar_hir, members);
    plan.function_exits = checker.recorded_function_exits.into_inner();
    plan.control_edges = checker.recorded_control_edges.into_inner();
    plan.method_safety = checker.recorded_method_safety.into_inner();
    plan.exit_snapshots = checker.recorded_exit_snapshots.into_inner();
    if !plan.is_consistent(&structs, &functions, text) {
        return Err(Diagnostic {
            kind: DiagnosticKind::Static,
            message: "P16：Checker 安全證據與宣告、型別 HIR 或資源權限不一致".into(),
            span: None,
        });
    }
    Ok(VerifiedPrivateSource { source, tokens: tokens.tokens, structs, functions, plan })
}

/// F00：唯一的正式私有來源入口維持原樣；只有內部測試開啟
/// 明確環境標記時，才輸出原始 UTF-8 來源、實際核准或拒絕的完整診斷。
/// 不在正常建置產生副作用，也不據此建構 AST／資源事件。
fn verify_source_with_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<VerifiedPrivateSource, Diagnostic> {
    let result = verify_source_core_with_ability(text, ability);
    #[cfg(test)]
    if std::env::var_os("VOILES_F00_PRIVATE_CAPTURE").is_some() {
        f00_記錄私有實際診斷(text, &result);
    }
    result
}

#[cfg(test)]
fn f00_記錄私有實際診斷(
    source: &str, result: &Result<VerifiedPrivateSource, Diagnostic>,
) {
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    let (state, kind, message, span): (&str, &str, &str, Option<crate::SourceSpan>) =
        match result {
            Ok(_) => ("Passed", "-", "", None),
            Err(diagnostic) => {
                let type_name = match diagnostic.kind {
                    DiagnosticKind::Lexical => "Lexical",
                    DiagnosticKind::Syntax => "Syntax",
                    DiagnosticKind::Static => "Static",
                };
                ("Failed", type_name, diagnostic.message.as_ref(), diagnostic.span)
            }
        };
    let (line1, col1, line2, col2) = match span {
        Some(value) => (
            value.start.line.to_string(), value.start.column.to_string(),
            value.end.line.to_string(), value.end.column.to_string(),
        ),
        None => ("-".into(), "-".into(), "-".into(), "-".into()),
    };
    println!(
        "F00_PRIVATE_SOURCE={}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        hex(source.as_bytes()), state, kind, hex(message.as_bytes()),
        line1, col1, line2, col2,
    );
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
// G18／S20：原 Parser／Checker 方法多載唯一靜態簽章與借用證據。
#[cfg(test)]
include!("struct_stage/method_overload_tests.rs");
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

// F00：只在內部測試建置採集八條真正私有來源的資源事件基準。
#[cfg(test)]
include!("struct_stage/f00_resource_capture.rs");

// F05：已刪除 SharedAst／ScalarExpr／ScalarStmt 的測試期重建權威。
// 以下回歸直接走兩條正式 Parser 與 Checker Typed HIR，不重組第三棵 AST。
#[cfg(test)]
include!("struct_stage/ir_projection.rs");

// G18／P18.1-F1：原六項契約第一條函式圖獨立真執行正反矩陣。
#[cfg(test)]
include!("struct_stage/g18_f1_obligation_tests.rs");

// G18／P18.1-F3：同一原 Checker 圖涵蓋 return／宣告／賦值／if／for／while 真呼叫位置。
#[cfg(test)]
include!("struct_stage/g18_f3_call_site_tests.rs");

// G18／P18.1-F2：原始混合參數真執行及正常／失敗資源責任矩陣。
#[cfg(test)]
include!("struct_stage/g18_f2_obligation_tests.rs");

// G18／P18.4 B1：原 Checker Copyable 不消耗來源的獨立真正反矩陣。
#[cfg(test)]
include!("struct_stage/g18_b1_obligation_tests.rs");

// G18／P18.4 B4：原 Checker 同一詞彙路徑之普通函式與同步 self 動態橋接。
#[cfg(test)]
include!("struct_stage/g18_b4_mixed_self_call_tests.rs");

// G18／P18.4 B4：普通函式先在 main 返回，後進入同步 self 的真借用框架。
#[cfg(test)]
include!("struct_stage/g18_b4_main_call_method_tests.rs");

// G18／P18.4-B4：原 Checker 純 int helper 加減及 main／方法內溢位退出。
#[cfg(test)]
include!("struct_stage/g18_b4_helper_arithmetic_unwind_tests.rs");

// G18／P18.4-B4：原 Checker var int 真重賦已核准純 helper 的 RHS 及失敗原子性。
#[cfg(test)]
include!("struct_stage/g18_b4_helper_assignment_unwind_tests.rs");

// G18／P18.4-B4：原 Checker 零／單 int 形參 helper 真綁定及跨 self 失敗退出。
#[cfg(test)]
include!("struct_stage/g18_b4_single_int_helper_parameter_tests.rs");

// G18／P18.4-B4：原 Checker 兩 int 形參依宣告順序真綁定與實參溢位。
#[cfg(test)]
include!("struct_stage/g18_b4_two_int_helper_argument_order_tests.rs");

// G18／P18.4-B4：完整原 Checker 純 int 形參簽章的共通動態框架與頭尾 Unwind。
#[cfg(test)]
include!("struct_stage/g18_b4_verified_int_helper_arity_tests.rs");

// G18／P18.4-B4／M3：原 Checker 方法末尾 Return 真普通 helper，
// 求參與返回錯誤各自逐框架逆序退出（沿用原唯一 BorrowTicket）。
#[cfg(test)]
include!("struct_stage/g18_b4_method_return_helper_exit_tests.rs");

// G18／P18.4-B4／M3：原 Checker 真 if 選中提前 Return helper、
// 未選葉不得有普通函式／資源事件。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_if_selected_helper_return_tests.rs");

// G18／P18.4-B4／M3：原 Checker 真 if/else 選中 else 純 helper Return，
// 未選葉無假普通函式或溢位，獨占寫入仍單票歸還。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_if_else_helper_return_tests.rs");

// G18／P18.4-B4／M3：原 Checker 真巢狀 if 雙葉純 helper，
// 只選中葉執行、未選葉不得生假 Return／Unwind。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_nested_if_helper_return_tests.rs");

// G18／P18.4-B4／M3：原 Checker 無局部方法末尾直接 helper Return。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_final_helper_without_locals_tests.rs");

// G18／P18.4-B4／M3：原 Checker 方法真 int 形參在直接
// Return helper 的動態 HIR 中保持 DeclarationId／真值。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_dynamic_int_return_helper_tests.rs");

// G18／P18.6-M2／P18.4-B4-M3：原 Parser 混合 bool/int
// 位置真方法形參、控制選葉與普通 helper 同一 R05 歸票。
#[cfg(test)]
include!("struct_stage/g18_m2_m3_mixed_bool_int_method_args_tests.rs");

// G18／P18.5-T3／R05：真 BorrowSafety 票據的跨函式
// 深度歸還序及同幀合法最後使用點正反稽核。
#[cfg(test)]
include!("struct_stage/g18_r05_t3_nested_ticket_order_tests.rs");

// G18／P18.5-T3／R05：真 HC14 票據離域前必退與歸還後不得重新借入。
#[cfg(test)]
include!("struct_stage/g18_r05_t3_ticket_frame_lifetime_tests.rs");

// G18／P18.5-T4／V7-A2：已執行 async 取消工作退休須真 R05／P16／HC14 雙帳。
#[cfg(test)]
include!("struct_stage/g18_v7_cancel_retire_proof_tests.rs");

// G18／P18.5-T4／V7-E2：wake／drive_one 均拒絕偽 FIFO 來源，
// 未選工作與已完成結果不得被重複消耗。
#[cfg(test)]
include!("struct_stage/g18_t4_verified_wake_drive_history_tests.rs");
include!("struct_stage/g18_t4_wake_token_ledger_tests.rs");

// G18／P18.5-T4／V7-E3：待取消 Prepared／Suspended 真證據與整批安全前置守門。
#[cfg(test)]
include!("struct_stage/g18_t4_pending_cancel_evidence_tests.rs");

// G18／P18.5-T4／V7-E2：整條 FIFO 驅動歷史不可重複 Completed／事後 Suspended。
#[cfg(test)]
include!("struct_stage/g18_t4_fifo_terminal_poll_history_tests.rs");

// G18／P18.5-T4／V7-A2：整批 shutdown 預檢終態 R05/P16 真證據先於任何取消。
#[cfg(test)]
include!("struct_stage/g18_v7_shutdown_terminal_evidence_tests.rs");

// G18／P18.6-M2／M3：多個原 Checker bool 方法形參
// 各自 DeclId 驅動巢狀真選枝及共通純 helper 返回。
#[cfg(test)]
include!("struct_stage/g18_m2_m3_multiple_bool_checker_frames_tests.rs");

// G18／P18.4-B4／M3：原 Checker main 純 helper 作直接
// 同步 self 方法 int 實參，必須先完成子函式再借票。
#[cfg(test)]
include!("struct_stage/g18_b4_m3_helper_method_argument_tests.rs");

// G18／P18.6 M2：原 Checker 同步 self 的 if/提前 return 真控制葉。
#[cfg(test)]
include!("struct_stage/g18_m2_self_if_return_tests.rs");

// G18／P18.6 M2：同步 self bool Copyable 形參驅動真 if 控制與清理。
#[cfg(test)]
include!("struct_stage/g18_m2_bool_parameter_tests.rs");

// G18／P18.6-M2：原 Checker 的同步 self if/else 真選枝及提前返回正反矩陣。
#[cfg(test)]
include!("struct_stage/g18_m2_if_else_tests.rs");

// G18／P18.6 M2：原 Checker 條件真葉獨占 self.int 寫入與保守安全負例。
#[cfg(test)]
include!("struct_stage/g18_m2_conditional_write_tests.rs");

// G18／P18.6-M2：真 self 寫入後返回算術的受控溢位與票據退出驗收。
#[cfg(test)]
include!("struct_stage/g18_m2_post_write_overflow_tests.rs");

// G18／P18.6-M2：真 else 葉獨占 self 寫入、雙葉互斥及唯一 BorrowTicket 安全。
#[cfg(test)]
include!("struct_stage/g18_m2_else_exclusive_write_tests.rs");

// G18／P18.6-M2：原 else 獨占寫入後真返回算術溢位及退出正反。
#[cfg(test)]
include!("struct_stage/g18_m2_else_post_write_overflow_tests.rs");

// G18／P18.6 M2：原 Checker 真巢狀 if 雙葉安全控制及 Return/Unwind。
#[cfg(test)]
include!("struct_stage/g18_m2_nested_self_if_tests.rs");

// G18／P18.6-M2：原 Checker bool 形參真巢狀選葉及 BorrowTicket 歸還。
#[cfg(test)]
include!("struct_stage/g18_m2_nested_dynamic_bool_tests.rs");

// G18／P18.6-M2：原巢狀 else 真加減與獨立溢位 Unwind 安全。
#[cfg(test)]
include!("struct_stage/g18_m2_nested_else_arithmetic_tests.rs");

// G18／P18.6-M2：原 Checker 巢狀 true／else 整數常值返回與唯一票據。
#[cfg(test)]
include!("struct_stage/g18_m2_nested_constant_return_tests.rs");

// G18／P18.6-M2：原 Checker 頂層 else 直接 self.int 返回與加減。
#[cfg(test)]
include!("struct_stage/g18_m2_direct_else_self_return_tests.rs");

// G18／P18.6-M2：巢狀可寫 self 雙葉互斥與 Unwind 歸票。
#[cfg(test)]
include!("struct_stage/g18_m2_nested_exclusive_write_tests.rs");

// G18／F2：原 Checker 真 bool 返回與 bool 實參使用同一動態進退帳。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_return_tests.rs");

// G18／F2：原 Checker bool-return 函式的局部綁定、更新與真資源逆序收尾。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_local_tests.rs");

// G18 F2：原 Checker bool-return 真 if／else-if／while 條件八項正反驗收。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_condition_tests.rs");

// G18／P18.1-F2：布林回傳的 int／bool 混型別形參真動態來源正反。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_mixed_tests.rs");

// G18 F2：原 Checker bool 返回 str 借用與 Move 實際 ResourceId／BorrowTicket 八項。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_str_tests.rs");

// G18／F2：原 bool-return 多語句、提前分支及 str 資源真正反九項。
#[cfg(test)]
include!("struct_stage/g18_f2_bool_body_tests.rs");

// G18／F2：原 Checker str Return→OwnedUtf8 接管與 ResourceId 身分閉合五項正反。
#[cfg(test)]
include!("struct_stage/g18_f2_str_return_graph_tests.rs");

// G18／P18.1-F2：str 擁有返回帶 int／bool／str 實參八項真資源與失敗證據。
#[cfg(test)]
include!("struct_stage/g18_f2_str_return_args_tests.rs");

// G18／P18.1-F2：原 Checker str-return 覆寫既有 var 的真 OwnedUtf8 正反驗收。
#[cfg(test)]
include!("struct_stage/g18_f2_str_return_assign_tests.rs");


// G18／P18.1-F2：原 Checker 合法 str 提前返回字面／Move 八項同圖正反。
#[cfg(test)]
include!("struct_stage/g18_f2_str_early_return_tests.rs");

// G18／P18.1-F2：多個 str ResourceId 與 BorrowTicket 在相同真函式工作圖的交叉正反。
#[cfg(test)]
include!("struct_stage/g18_f2_str_multi_resource_tests.rs");
include!("struct_stage/g18_f2_nested_str_temporary_tests.rs");
include!("struct_stage/g18_f2_literal_str_temporary_tests.rs");
include!("struct_stage/g18_f2_str_forward_return_tests.rs");
include!("struct_stage/g18_f2_str_forward_budget_tests.rs");
