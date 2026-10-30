//! P13 受限結構體所有權前端：真正 .voil → 私有 AST → 型別 → 所有權狀態。
//! 只接受可以完整驗證的子集，未支援的語法拒絕，不影響 MVP-A check_file。
//! 正式內部入口依 HA13-A／str 方案 A 使用核准政策；自訂能力僅限 Rust 測試。
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot, ValueState};
use crate::ownership_ledger::DeclarationId;
use crate::borrow_safety::{BorrowKind, BorrowSafety};
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

#[derive(Debug, Clone)]
struct Stmt {
    kind: StmtKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
enum StmtKind {
    Declare { name: String, ty: String, mutable: bool, init: Option<Term> },
    Assign { name: String, value: Term },
    FieldAssign { receiver: String, field: String, value: Term },
    If { condition: Term, yes: Vec<Stmt>, no: Option<Vec<Stmt>> },
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

#[derive(Debug, Clone)]
struct Term {
    kind: TermKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
enum TermKind {
    Literal(&'static str),
    Name(String),
    /// P01 已核准的 int 加減；計算前必須依序驗證兩端所有權。
    BinaryInt { left: Box<Term>, operator: char, right: Box<Term> },
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
    source: usize,
    kind: BorrowKind,
    active: bool,
}

#[derive(Clone)]
struct Binding {
    declaration_start: usize,
    ty: String,
    mutable: bool,
    value: ValueSlot,
    /// 僅以合法 struct 建立式已知的未初始化 var 欄位集合。
    /// None 表示不存在可追蹤的部分建立證明；不等於全部欄位可用。
    pending_fields: Option<HashSet<String>>,
    /// 非擁有的借用別名，不擁有來源值，不允許 .move()。
    loan: Option<LoanBinding>,
}

/// 各大括號區塊都有獨立詞彙作用域；內層同名宣告不得覆蓋外層所有權來源。
// 分支與迴圈快照共用不可變狀態；只有實際寫入的詞彙作用域才複製。
type Scopes = Vec<Rc<HashMap<String, Binding>>>;

/// 資料流需區分正常後繼、函式返回與迴圈轉移；後兩者不能混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowState { Fallthrough, Returned, LoopTransfer }

struct Checker<'a, C> {
    source: &'a Source,
    structs: &'a HashMap<String, StructDecl>,
    functions: &'a [Function],
    function_index: HashMap<String, usize>,
    ability: C,
}

// P13～P16 的受限靜態安全檢查；下一步再分離型別／資料流責任。
include!("struct_stage/safety_checker.rs");

// P16 真實 UTF-8 資源執行期橋接，僅在內部測試建置時引入。
#[cfg(test)]
include!("struct_stage/resource_trace.rs");

/// 已通過同一組原始來源、解析、型別、所有權、借用及有效期檢查的私有安全事實。
/// P16 資源橋接只可消耗此成果，不可單獨從未驗證的 AST 假造安全來源。
struct VerifiedPrivateSource {
    source: Source,
    structs: HashMap<String, StructDecl>,
    functions: Vec<Function>,
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
    Checker {
        source: &source, structs: &structs, functions: &functions,
        function_index, ability,
    }.all(&functions)?;
    Ok(VerifiedPrivateSource { source, structs, functions })
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
