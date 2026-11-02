//! P16.6-R02.2：公開 MVP-A 與私有安全前端共用的「已核准交集」語意 AST。
//! 兩套舊解析器目前仍存在，且各自的完整語法、診斷和入口權限不變。
//! 只有可以保留原宣告位置、順序與語意的結點才可以投影到此結構。
// R02.2 此刻只由私有／公開差異回歸消耗；正式 Checker 遷移前，避免非測試編譯死碼警告。
#![allow(dead_code)]

#[cfg(test)]
use crate::expr::{Expr as PublicExpr, ExprKind as PublicExprKind};
#[cfg(test)]
use crate::parser::{self, Item, StmtKind as PublicStmtKind};
use crate::ownership_ledger::DeclarationId;

/// 原始 UTF-8 位元組半開範圍，供共同解析核心與各模式正式 AST 使用。
/// 此型別不是已換算為一基行列的公開診斷位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}
impl NodeSpan {
    pub(crate) fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "節點來源半開範圍不可反向");
        Self { start, end }
    }

    /// 共同來源身分的不可造假驗證：拒絕 EOF 以外或多位元組
    /// UTF-8 字元中間的位置。只用原始碼的真實 byte offset。
    pub(crate) fn checked(self, source: &str) -> Option<Self> {
        (self.start < self.end
            && self.end <= source.len()
            && source.is_char_boundary(self.start)
            && source.is_char_boundary(self.end)).then_some(self)
    }
}

/// F01：不同前端雖共用 AST／HIR，模式權限不能取語法聯集。
/// 私有受限語法的合法性仍由原有 P13～P16 Parser／Checker 決定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FrontendMode {
    PublicMvpA,
    PrivateVerified,
}

/// F01：在源檔內以語法類別及來源半開跨度識別一筆 AST／HIR 節點。
/// 同位置不同角色（例如 return 表達式和名稱讀取）不能混淆；
/// DeclarationId 只表示詞彙宣告，絕不等於執行期 ResourceId。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NodeRole {
    Expression,
    Reference,
    Assignment,
    Declaration,
    Statement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeIdentity {
    pub(crate) span: NodeSpan,
    pub(crate) role: NodeRole,
}
impl NodeIdentity {
    pub(crate) fn checked(
        span: NodeSpan, role: NodeRole, source: &str,
    ) -> Option<Self> {
        Some(Self { span: span.checked(source)?, role })
    }
}

/// R02.16：公開 Expr 與私有 Term 直接使用相同的正式 AST 外殼。
/// K 表示各入口已核准的節點種類，保持能力隔離，不讓語法聯集公開。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpannedNode<K> {
    pub(crate) kind: K,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
impl<K> SpannedNode<K> {
    pub(crate) fn node_span(&self) -> NodeSpan {
        NodeSpan::new(self.start, self.end)
    }
    pub(crate) fn node_identity(&self, role: NodeRole) -> NodeIdentity {
        NodeIdentity { span: self.node_span(), role }
    }
}


/// 兩條正式 Parser、型別與安全 Checker 共用的條件分支節點。
/// 保留各模式的條件與區塊型別；不把私有能力加入公開文法。
#[derive(Debug, Clone)]
pub(crate) struct ConditionalBranch<E, B> {
    pub(crate) condition: E,
    pub(crate) body: B,
}
impl<E, B> ConditionalBranch<E, B> {
    pub(crate) fn new(condition: E, body: B) -> Self {
        Self { condition, body }
    }
}

/// 公開／私有正式加減 AST 共用同一具型別二元節點。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArithmeticNode<N> {
    pub(crate) left: Box<N>,
    pub(crate) operator: char,
    pub(crate) right: Box<N>,
}
impl<N> ArithmeticNode<N> {
    pub(crate) fn new(left: N, operator: char, right: N) -> Self {
        debug_assert!(matches!(operator, '+' | '-'), "此階段只核准加減");
        Self { left: Box::new(left), operator, right: Box::new(right) }
    }
}

/// 兩條正式 Checker 共用的詞彙遮蔽查找：由最內層往最外層，
/// 只決定可見的當前名稱，不改變兩個入口各自的前向參照或型別規則。
pub(crate) fn lookup_lexical<'a, V: 'a>(
    scopes: impl DoubleEndedIterator<Item = &'a std::collections::HashMap<String, V>>,
    name: &str,
) -> Option<&'a V> {
    scopes.rev().find_map(|scope| scope.get(name))
}

/// P09 已完成的詞彙名稱解析證據，供正式 P10 型別檢查直接消耗。
/// 名稱引用以實際原始位元組起點綁定 DeclarationId，絕不依同名字串猜測遮蔽。
#[derive(Debug, Default)]
pub(crate) struct ResolvedNames {
    pub(crate) references: std::collections::BTreeMap<usize, DeclarationId>,
    /// 賦值語句原始位元組起點對應的目標宣告身分。
    pub(crate) assignments: std::collections::BTreeMap<usize, DeclarationId>,
    pub(crate) declarations: std::collections::BTreeMap<DeclarationId, TypedDeclaration<ScalarType>>,
}
/// 兩條正式 Checker 都使用的已解析宣告身分與型別資料形狀。
/// P09 的資料須經名稱解析產生；私有路徑的型別還須經 P13 安全檢查。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedUse<T> {
    pub(crate) declaration: DeclarationId,
    pub(crate) ty: T,
}
impl<T> ResolvedUse<T> {
    pub(crate) fn new(declaration: DeclarationId, ty: T) -> Self {
        Self { declaration, ty }
    }
}

/// F04：雙 Checker 實際詞彙環境共用的具型別宣告身分與 var／const 權限。
/// P10 以 ScalarType 具體化，P11 僅需要身分及可變權限可用單位型別；
/// P13～P16 的非 Copyable 型別保留私有的 &str 名稱而不擴張公開文法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypedDeclaration<T> {
    pub(crate) declaration: DeclarationId,
    pub(crate) ty: T,
    pub(crate) mutable: bool,
}
impl<T> TypedDeclaration<T> {
    pub(crate) fn new(declaration: DeclarationId, ty: T, mutable: bool) -> Self {
        Self { declaration, ty, mutable }
    }
    pub(crate) fn owns(&self, declaration: DeclarationId) -> bool {
        self.declaration == declaration
    }
}
impl<T: Copy + PartialEq> TypedDeclaration<T> {
    pub(crate) fn matches_use(&self, resolved: ResolvedUse<T>) -> bool {
        self.owns(resolved.declaration) && self.ty == resolved.ty
    }
    pub(crate) fn use_at(&self, span: NodeSpan) -> TypedUse<T> {
        TypedUse::new(span, ResolvedUse::new(self.declaration, self.ty))
    }
}

/// F04：正式型別化引用的來源半開跨度與已解析宣告必須同行傳遞。
/// 不以名稱文字重新推測擁有者；具體型別能力仍由不同模式的 Checker 驗證。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypedUse<T> {
    pub(crate) span: NodeSpan,
    pub(crate) resolved: ResolvedUse<T>,
}
impl<T> TypedUse<T> {
    pub(crate) fn new(span: NodeSpan, resolved: ResolvedUse<T>) -> Self {
        Self { span, resolved }
    }
}

/// F04：P10 成功完成型別檢查後才可交付 P11 的正式具型別讀寫 HIR。
/// 只保存原始來源跨度及 P09 已解析並在 P10 驗證的宣告身分與型別。
/// 不表示任何私有借用／移動資源已獲准。
/// 已驗證純值運算式完整節點的來源跨度及 P10 推導型別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VerifiedScalarExpression {
    pub(crate) span: NodeSpan,
    pub(crate) ty: ScalarType,
}

/// R02 F04：由真正公私 Checker 驗證的同一個 int 加減節點及其
/// 左右運算元來源；不把這個純值事實冒充可移動資源的票據。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VerifiedScalarArithmetic {
    pub(crate) span: NodeSpan,
    pub(crate) left: NodeSpan,
    pub(crate) right: NodeSpan,
    pub(crate) operator: char,
}

#[derive(Debug, Default)]
pub(crate) struct VerifiedScalarHir {
    /// F01／F04：讀取與賦值由同一筆 NodeIdentity 索引表管理，
    /// (語意角色, 來源起點) 唯一；同位置範圍、來源、型別不合
    /// 只標記衝突，不允許後一次合法重訪覆寫首次授權。
    uses: std::collections::BTreeMap<
        (NodeRole, usize), (NodeIdentity, TypedUse<ScalarType>)
    >,
    expressions: std::collections::BTreeMap<NodeIdentity, VerifiedScalarExpression>,
    /// 同一來源起點不得記錄兩筆不同終點／運算元／運算子；
    /// 與共通 Typed HIR 一起交付，拒絕事後由 AST 逆推合法加減。
    arithmetic: std::collections::BTreeMap<usize, VerifiedScalarArithmetic>,
    conflicted: bool,
}
impl VerifiedScalarHir {
    pub(crate) fn is_consistent(&self) -> bool { !self.conflicted }

    /// F01／F04：公私正式 Typed HIR 共同使用唯一 UTF-8
    /// NodeIdentity 來源驗證器；同跨度具名引用必須存在同型別
    /// 的成功 Checker 運算式，任一缺漏都不可交付後續階段。
    pub(crate) fn source_spans_are_valid(&self, source: &str) -> bool {
        self.is_consistent()
            && self.verified_expressions().all(|expr|
                NodeIdentity::checked(expr.span, NodeRole::Expression, source).is_some())
            && self.verified_references().all(|reference|
                NodeIdentity::checked(reference.span, NodeRole::Reference, source).is_some()
                    && self.expression(reference.span)
                        .is_some_and(|expr| expr.ty == reference.resolved.ty))
            && self.verified_assignments().all(|assignment|
                NodeIdentity::checked(assignment.span, NodeRole::Assignment, source).is_some())
            && self.verified_arithmetic().all(|fact| {
                NodeIdentity::checked(fact.span, NodeRole::Expression, source).is_some()
                    && NodeIdentity::checked(fact.left, NodeRole::Expression, source).is_some()
                    && NodeIdentity::checked(fact.right, NodeRole::Expression, source).is_some()
                    && fact.span.start <= fact.left.start
                    && fact.left.end <= fact.right.start
                    && fact.right.end <= fact.span.end
                    && matches!(fact.operator, '+' | '-')
                    && source.get(fact.left.end..fact.right.start)
                        .is_some_and(|between| between.contains(fact.operator))
                    && [fact.span, fact.left, fact.right].into_iter().all(|span|
                        self.expression(span).is_some_and(|entry| entry.ty == ScalarType::Int))
            })
    }

    /// 已有來源跨度／左右值型別／副作用完成檢查後才記錄。
    /// 同起點不同加減事實永久矛盾，不准後來的合法重訪洗掉。
    pub(crate) fn record_arithmetic(&mut self, fact: VerifiedScalarArithmetic) {
        if let Some(previous) = self.arithmetic.get(&fact.span.start) {
            if *previous != fact { self.conflicted = true; }
        } else {
            self.arithmetic.insert(fact.span.start, fact);
        }
    }

    pub(crate) fn verified_arithmetic(&self)
        -> impl Iterator<Item = VerifiedScalarArithmetic> + '_ {
        self.arithmetic.values().copied()
    }

    pub(crate) fn arithmetic(&self, span: NodeSpan) -> Option<VerifiedScalarArithmetic> {
        self.arithmetic.get(&span.start).copied().filter(|fact| fact.span == span)
    }

    pub(crate) fn record_expression(&mut self, span: NodeSpan, ty: ScalarType) {
        let fact = VerifiedScalarExpression { span, ty };
        let key = NodeIdentity { span, role: NodeRole::Expression };
        if let Some(previous) = self.expressions.get(&key) {
            if *previous != fact { self.conflicted = true; }
        } else {
            self.expressions.insert(key, fact);
        }
    }

    pub(crate) fn expression(&self, span: NodeSpan) -> Option<VerifiedScalarExpression> {
        self.expressions.get(&NodeIdentity {
            span, role: NodeRole::Expression,
        }).copied()
    }

    fn record_use(&mut self, role: NodeRole, fact: TypedUse<ScalarType>) {
        debug_assert!(matches!(role, NodeRole::Reference | NodeRole::Assignment));
        let node = NodeIdentity { span: fact.span, role };
        let key = (node.role, node.span.start);
        if let Some(previous) = self.uses.get(&key) {
            if *previous != (node, fact) { self.conflicted = true; }
        } else {
            self.uses.insert(key, (node, fact));
        }
    }

    pub(crate) fn record_reference(&mut self, use_at: TypedUse<ScalarType>) {
        self.record_use(NodeRole::Reference, use_at);
    }
    pub(crate) fn record_assignment(&mut self, use_at: TypedUse<ScalarType>) {
        self.record_use(NodeRole::Assignment, use_at);
    }

    fn verified_use(&self, span: NodeSpan, role: NodeRole)
        -> Option<TypedUse<ScalarType>> {
        let (node, fact) = self.uses.get(&(role, span.start))?;
        (node.span == span && node.role == role).then_some(*fact)
    }

    pub(crate) fn reference(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.verified_use(span, NodeRole::Reference)
    }
    pub(crate) fn assignment(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.verified_use(span, NodeRole::Assignment)
    }

    /// 三種類型的來源節點都只可經由此單一已驗證資料結構消費。
    pub(crate) fn verified_references(&self)
        -> impl Iterator<Item = TypedUse<ScalarType>> + '_ {
        self.uses.iter().filter_map(|((role, _), (_, fact))|
            (*role == NodeRole::Reference).then_some(*fact))
    }
    pub(crate) fn verified_assignments(&self)
        -> impl Iterator<Item = TypedUse<ScalarType>> + '_ {
        self.uses.iter().filter_map(|((role, _), (_, fact))|
            (*role == NodeRole::Assignment).then_some(*fact))
    }
    pub(crate) fn verified_expressions(&self)
        -> impl Iterator<Item = VerifiedScalarExpression> + '_ {
        self.expressions.values().copied()
    }
}

/// 公開純值 HIR 引用是共同語意引用的 int／bool 專用形式。
pub(crate) type ResolvedScalarUse = ResolvedUse<ScalarType>;

impl ResolvedNames {
    fn use_for(&self, starts: &std::collections::BTreeMap<usize, DeclarationId>,
        start: usize) -> Option<ResolvedScalarUse> {
        let declaration = *starts.get(&start)?;
        let ty = self.declaration(declaration)?.ty;
        Some(ResolvedUse::new(declaration, ty))
    }

    /// P09 已解析宣告的唯一具型別來源；P10、P11 檢查實際詞彙綁定
    /// 是否與這筆已驗證事實完全相同（含 const/var 權限）。
    pub(crate) fn declaration(&self, id: DeclarationId)
        -> Option<TypedDeclaration<ScalarType>> {
        let fact = *self.declarations.get(&id)?;
        fact.owns(id).then_some(fact)
    }

    pub(crate) fn reference(&self, start: usize) -> Option<ResolvedScalarUse> {
        self.use_for(&self.references, start)
    }

    pub(crate) fn assignment(&self, start: usize) -> Option<ResolvedScalarUse> {
        self.use_for(&self.assignments, start)
    }

    /// 只接受 P09 已驗證的來源身分，保留正式 Parser 提供的半開來源跨度。
    pub(crate) fn reference_at(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.reference(span.start).map(|resolved| TypedUse::new(span, resolved))
    }

    pub(crate) fn assignment_at(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.assignment(span.start).map(|resolved| TypedUse::new(span, resolved))
    }

    /// R02 F04：唯一 P09→P10／P11 型別化來源安全授權入口。
    /// 一次核對原始位置、宣告表鍵、身分、型別與 var/const 權限。
    /// 這只表示靜態來源一致，不代表借用或資源操作已獲授權。
    fn verified_use(&self, span: NodeSpan, visible: TypedDeclaration<ScalarType>,
        assignment: bool) -> Option<TypedUse<ScalarType>> {
        let use_at = if assignment {
            self.assignment_at(span)?
        } else {
            self.reference_at(span)?
        };
        (self.declaration(visible.declaration) == Some(visible)
            && visible.matches_use(use_at.resolved)).then_some(use_at)
    }

    pub(crate) fn verified_reference(&self, span: NodeSpan,
        visible: TypedDeclaration<ScalarType>) -> Option<TypedUse<ScalarType>> {
        self.verified_use(span, visible, false)
    }

    pub(crate) fn verified_assignment(&self, span: NodeSpan,
        visible: TypedDeclaration<ScalarType>) -> Option<TypedUse<ScalarType>> {
        self.verified_use(span, visible, true)
    }

    pub(crate) fn reference_type(&self, start: usize) -> Option<ScalarType> {
        self.reference(start).map(|value| value.ty)
    }

    pub(crate) fn assignment_type(&self, start: usize) -> Option<ScalarType> {
        self.assignment(start).map(|value| value.ty)
    }
}

/// R02.5：只允許公開 MVP-A 與已核准 P13 交集的具體純值型別。
/// 其他型別（含非 Copyable str、struct、容器）不可用任意字串冒充。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType { Int, Bool }

impl ScalarType {
    pub(crate) fn from_name(ty: &str) -> Option<Self> {
        match ty { "int" => Some(Self::Int), "bool" => Some(Self::Bool), _ => None }
    }
}

// parser::Type 現為此型別的同一個名稱，不再需要轉換兩套等價型別。


#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) enum ScalarExpr {
    Int(i64),
    Bool(bool),
    Name(String),
    Binary { left: Box<ScalarExpr>, op: char, right: Box<ScalarExpr> },
    CallZero(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) enum ScalarStmt {
    Declare {
        declaration: DeclarationId,
        name: String,
        ty: ScalarType,
        mutable: bool,
        init: Option<ScalarExpr>,
    },
    Assign { name: String, value: ScalarExpr },
    If { condition: ScalarExpr, yes: Vec<ScalarStmt>, no: Option<Vec<ScalarStmt>> },
    Return(ScalarExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct ScalarFunction {
    pub(crate) name: String,
    pub(crate) result_ty: ScalarType,
    pub(crate) body: Vec<ScalarStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct SharedAst {
    pub(crate) functions: Vec<ScalarFunction>,
}

#[cfg(test)]
fn public_expression(expr: &PublicExpr) -> Option<ScalarExpr> {
    let result = match &expr.kind {
        PublicExprKind::IntLiteral(raw) =>
            ScalarExpr::Int(crate::syntax_shared::integer_i64(raw).ok()?),
        PublicExprKind::BoolLiteral(value) => ScalarExpr::Bool(*value),
        PublicExprKind::Name(value) => ScalarExpr::Name(value.clone()),
        PublicExprKind::Add(binary) | PublicExprKind::Subtract(binary) =>
            ScalarExpr::Binary {
                left: Box::new(public_expression(&binary.left)?),
                op: binary.operator,
                right: Box::new(public_expression(&binary.right)?),
            },
        PublicExprKind::Group(inner) => return public_expression(inner),
        PublicExprKind::ZeroArgCall(call) => ScalarExpr::CallZero(call.name.clone()),
        _ => return None,
    };
    Some(result)
}

#[cfg(test)]
fn public_statements(stmts: &[parser::Stmt]) -> Option<Vec<ScalarStmt>> {
    stmts.iter().map(|statement| {
        let value = match &statement.kind {
            PublicStmtKind::Binding(binding) => ScalarStmt::Declare {
                declaration: DeclarationId(binding.start),
                name: binding.name.clone(),
                ty: binding.ty,
                mutable: binding.mutable,
                init: map_optional(binding.initializer.as_ref(), public_expression)?,
            },
            PublicStmtKind::Assignment(assignment) => ScalarStmt::Assign {
                name: assignment.name.clone(),
                value: public_expression(&assignment.expression)?,
            },
            PublicStmtKind::IfChain(chain) => {
                // P01 直接消費共同 ParsedIfChain，保留每個分支跨度；
                // 私有 P13 歷史差異測試仍核對巢狀 if 的等價順序。
                if chain.arms.is_empty() { return None; }
                let mut tail = map_optional(chain.otherwise.as_ref(),
                    |block| public_statements(&block.statements))?;
                for arm in chain.arms.iter().rev() {
                    tail = Some(vec![ScalarStmt::If {
                        condition: public_expression(&arm.branch.condition)?,
                        yes: public_statements(&arm.branch.body.statements)?,
                        no: tail,
                    }]);
                }
                tail?.into_iter().next()?
            }
            PublicStmtKind::Return(value) => ScalarStmt::Return(public_expression(value)?),
        };
        Some(value)
    }).collect()
}

/// 僅供已驗證 P01 MVP-A 的 AST 做無副作用投影；未支援者回傳 None。
#[cfg(test)]
pub(crate) fn from_public(file: &parser::File) -> Option<SharedAst> {
    let functions = file.items.iter().map(|item| {
        let Item::Function(fun) = item else { return None };
        Some(ScalarFunction {
            name: fun.name.clone(),
            result_ty: fun.result_ty,
            body: public_statements(&fun.body.statements)?,
        })
    }).collect::<Option<Vec<_>>>()?;
    Some(SharedAst { functions })
}

// 外層 Option 表示投影成功／失敗；內層 Option 才表示語法原本不存在。
// 不可把缺少 initializer 或缺少 else 誤當成「無法投影」。
#[cfg(test)]
fn map_optional<T, U>(
    source: Option<&T>, project: impl FnOnce(&T) -> Option<U>,
) -> Option<Option<U>> {
    match source {
        Some(value) => Some(Some(project(value)?)),
        None => Some(None),
    }
}

#[cfg(test)]
mod f04_正式_p10_交付給_p11_的具型別存取證據 {
    use super::*;
    #[test]
    fn 讀取與賦值分開記錄完整來源跨度及宣告且不得混淆() {
        let a = TypedDeclaration::new(DeclarationId(4), ScalarType::Int, true);
        let read = NodeSpan::new(50, 56);
        let write = NodeSpan::new(80, 87);
        let mut hir = VerifiedScalarHir::default();
        hir.record_reference(a.use_at(read));
        hir.record_assignment(a.use_at(write));
        hir.record_expression(read, ScalarType::Int);
        assert_eq!(hir.expression(read),
            Some(VerifiedScalarExpression { span: read, ty: ScalarType::Int }));
        assert_eq!(hir.expression(NodeSpan::new(50, 57)), None);
        assert_eq!(hir.reference(read), Some(a.use_at(read)));
        assert_eq!(hir.assignment(write), Some(a.use_at(write)));
        assert_eq!(hir.reference(write), None);
        assert_eq!(hir.assignment(read), None);
        assert_eq!(hir.reference(NodeSpan::new(50, 57)), None,
            "相同起點不同結束位置不能冒充經型別檢查的來源");
        assert_eq!(hir.assignment(NodeSpan::new(80, 88)), None);
        assert!(hir.is_consistent());
    }

    #[test]
    fn 同一節點型別衝突永久標記_首次權威不可被覆寫() {
        let at = NodeSpan::new(60, 66);
        let mut hir = VerifiedScalarHir::default();
        hir.record_expression(at, ScalarType::Int);
        hir.record_expression(at, ScalarType::Int);
        assert!(hir.is_consistent(), "同一 AST 多條路徑重訪相同證據應冪等");
        hir.record_expression(at, ScalarType::Bool);
        assert!(!hir.is_consistent());
        assert_eq!(hir.expression(at).unwrap().ty, ScalarType::Int,
            "不得以第二筆型別覆寫首次證明");
        hir.record_expression(at, ScalarType::Int);
        assert!(!hir.is_consistent(), "後續相同證據不得清除歷史衝突");
    }

    #[test]
    fn 讀取或賦值相同起點不同身分及跨度都必須永久拒絕() {
        let owner = TypedDeclaration::new(DeclarationId(4), ScalarType::Int, true);
        let other = TypedDeclaration::new(DeclarationId(18), ScalarType::Int, true);
        let read = NodeSpan::new(100, 105);
        let write = NodeSpan::new(130, 140);
        let mut hir = VerifiedScalarHir::default();
        hir.record_reference(owner.use_at(read));
        hir.record_assignment(owner.use_at(write));
        hir.record_reference(owner.use_at(read));
        hir.record_assignment(owner.use_at(write));
        assert!(hir.is_consistent());
        hir.record_reference(other.use_at(read));
        hir.record_assignment(owner.use_at(NodeSpan::new(130, 141)));
        assert!(!hir.is_consistent());
        assert_eq!(hir.reference(read), Some(owner.use_at(read)));
        assert_eq!(hir.assignment(write), Some(owner.use_at(write)));
        hir.record_reference(owner.use_at(read));
        hir.record_assignment(owner.use_at(write));
        assert!(!hir.is_consistent());
    }
}

#[cfg(test)]
mod f04_公私共同語意的正式已解析存取權威 {
    use super::*;

    #[test]
    fn 同時驗證來源位置身分型別及可變權限_不能跨讀寫用途使用() {
        let owner = DeclarationId(100);
        let other = DeclarationId(200);
        let span_read = NodeSpan::new(310, 317);
        let span_write = NodeSpan::new(410, 420);
        let fact = TypedDeclaration::new(owner, ScalarType::Int, true);
        let mut names = ResolvedNames::default();
        names.declarations.insert(owner, fact);
        names.declarations.insert(other,
            TypedDeclaration::new(other, ScalarType::Int, false));
        names.references.insert(span_read.start, owner);
        names.assignments.insert(span_write.start, owner);
        assert_eq!(names.verified_reference(span_read, fact),
            Some(fact.use_at(span_read)));
        assert_eq!(names.verified_assignment(span_write, fact),
            Some(fact.use_at(span_write)));
        assert_eq!(names.verified_assignment(span_read, fact), None);
        assert_eq!(names.verified_reference(span_write, fact), None);
        assert_eq!(names.verified_reference(NodeSpan::new(311, 317), fact), None);
        assert_eq!(names.verified_reference(span_read,
            TypedDeclaration::new(other, ScalarType::Int, false)), None);
        assert_eq!(names.verified_reference(span_read,
            TypedDeclaration::new(owner, ScalarType::Bool, true)), None);
        assert_eq!(names.verified_reference(span_read,
            TypedDeclaration::new(owner, ScalarType::Int, false)), None);
        names.declarations.insert(owner,
            TypedDeclaration::new(other, ScalarType::Int, true));
        assert_eq!(names.verified_reference(span_read, fact), None);
        assert_eq!(names.verified_assignment(span_write, fact), None);
    }
}

#[cfg(test)]
mod f04_詞彙宣告與已解析引用身分一致性 {
    use super::*;

    #[test]
    fn 真正詞彙宣告同時核對來源型別及寫入權限() {
        let a = TypedDeclaration::new(DeclarationId(12), ScalarType::Int, true);
        let shadow = TypedDeclaration::new(DeclarationId(30), ScalarType::Int, false);
        let span = NodeSpan::new(80, 86);
        let typed = a.use_at(span);
        assert_eq!(typed.span, span);
        assert!(a.owns(DeclarationId(12)));
        assert!(a.matches_use(typed.resolved));
        assert!(!shadow.matches_use(typed.resolved), "同名同型別也不等於同一宣告");
        assert!(!a.matches_use(ResolvedUse::new(DeclarationId(12), ScalarType::Bool)),
            "不能透過捏造已解析型別改變詞彙綁定");
        assert!(a.mutable);
        assert!(!shadow.mutable);
    }
}

#[cfg(test)]
mod f04_正式具位置引用權威回歸 {
    use super::*;

    #[test]
    fn 已解析來源型別必須隨真正節點跨度同行傳遞() {
        let owner = DeclarationId(5);
        let shadow = DeclarationId(20);
        let mut facts = ResolvedNames::default();
        facts.declarations.insert(owner, TypedDeclaration::new(owner, ScalarType::Int, true));
        facts.declarations.insert(shadow, TypedDeclaration::new(shadow, ScalarType::Bool, false));
        facts.references.insert(50, owner);
        facts.assignments.insert(71, shadow);
        let use_span = NodeSpan::new(50, 56);
        let write_span = NodeSpan::new(71, 77);
        let reference = facts.reference_at(use_span).unwrap();
        let assignment = facts.assignment_at(write_span).unwrap();
        assert_eq!(reference.span, use_span);
        assert_eq!(reference.resolved, ResolvedUse::new(owner, ScalarType::Int));
        assert_eq!(assignment.span, write_span);
        assert_eq!(assignment.resolved, ResolvedUse::new(shadow, ScalarType::Bool));
        assert_eq!(facts.reference_at(NodeSpan::new(49, 56)), None);
        assert_eq!(facts.assignment_at(NodeSpan::new(70, 77)), None);
        // 兩個宣告即使名稱相同也不能以其中一個身分推定另一個的能力。
        facts.references.remove(&50);
        assert_eq!(facts.reference_at(use_span), None);
    }
}

#[cfg(test)]
mod f04_共通詞彙查找回歸 {
    use super::lookup_lexical;
    use std::collections::HashMap;

    #[test]
    fn 遮蔽查找由最內層到最外層且不推定型別() {
        let mut outer = HashMap::new();
        outer.insert("item".to_owned(), 1_i32);
        let mut inner = HashMap::new();
        inner.insert("item".to_owned(), 2_i32);
        let scopes = [outer, inner];
        assert_eq!(lookup_lexical(scopes.iter(), "item"), Some(&2));
        assert_eq!(lookup_lexical(scopes[..1].iter(), "item"), Some(&1));
        assert_eq!(lookup_lexical(scopes.iter(), "absent"), None);
    }
}

#[cfg(test)]
mod p16_6_r02_6_共通型別即公開解析型別 {
    use super::*;

    #[test]
    fn 公開_parser_與共同語意樹在編譯期就是相同型別() {
        let from_parser: crate::parser::Type = crate::parser::Type::Int;
        let common: ScalarType = from_parser;
        assert_eq!(common, ScalarType::Int);
        let back: crate::parser::Type = ScalarType::Bool;
        assert_eq!(back, crate::parser::Type::Bool);
        assert_eq!(ScalarType::from_name("int"), Some(from_parser));
        assert_eq!(ScalarType::from_name("str"), None);
    }
}

#[cfg(test)]
mod r02_f01_f04_單一_nodeidentity_typed_hir_權威 {
    use super::*;

    #[test]
    fn 相同來源跨度的讀寫由節點角色隔離但共用真正已解析表() {
        let source = TypedDeclaration::new(DeclarationId(21), ScalarType::Int, true);
        let span = NodeSpan::new(41, 45);
        let mut hir = VerifiedScalarHir::default();
        hir.record_reference(source.use_at(span));
        hir.record_assignment(source.use_at(span));
        hir.record_expression(span, ScalarType::Int);
        assert!(hir.is_consistent());
        assert_eq!(hir.reference(span), Some(source.use_at(span)));
        assert_eq!(hir.assignment(span), Some(source.use_at(span)));
        assert_eq!(hir.expression(span).unwrap().ty, ScalarType::Int);
        assert_eq!(hir.verified_references().count(), 1);
        assert_eq!(hir.verified_assignments().count(), 1);
    }

    #[test]
    fn 同語意角色同起點不同跨度或宣告身分不可藉新節點覆寫() {
        let first = TypedDeclaration::new(DeclarationId(15), ScalarType::Bool, false);
        let second = TypedDeclaration::new(DeclarationId(26), ScalarType::Bool, false);
        let original = NodeSpan::new(66, 70);
        let forged = NodeSpan::new(66, 71);
        let mut hir = VerifiedScalarHir::default();
        hir.record_reference(first.use_at(original));
        hir.record_reference(first.use_at(forged));
        hir.record_reference(second.use_at(original));
        hir.record_reference(first.use_at(original));
        assert!(!hir.is_consistent());
        assert_eq!(hir.reference(original), Some(first.use_at(original)));
        assert_eq!(hir.reference(forged), None);
        assert_eq!(hir.verified_references().count(), 1);
    }
}

#[cfg(test)]
mod r02_f01_f04_公私唯一_hir_來源閉合 {
    use super::*;
    #[test]
    fn utf8_字元內部跨度_越界_沒有對應運算式的引用全部拒絕() {
        let src = "🪼value";
        let start = src.find("value").unwrap();
        let valid = NodeSpan::new(start,start+5);
        let id = TypedDeclaration::new(DeclarationId(0), ScalarType::Int, true);
        let mut correct = VerifiedScalarHir::default();
        correct.record_expression(valid,ScalarType::Int);
        correct.record_reference(id.use_at(valid));
        assert!(correct.source_spans_are_valid(src));

        let mut fake = VerifiedScalarHir::default();
        fake.record_reference(id.use_at(valid));
        assert!(!fake.source_spans_are_valid(src),
            "單有來源引用不代表 Checker 已成功建立同跨度型別 HIR");
        fake.record_expression(valid,ScalarType::Bool);
        assert!(!fake.source_spans_are_valid(src),
            "同跨度不同型別的引用不能冒充成功的 Checker 證據");

        for span in [NodeSpan::new(1,2),
            NodeSpan::new(start,src.len()+1)] {
            let mut broken = VerifiedScalarHir::default();
            broken.record_expression(span,ScalarType::Int);
            assert!(!broken.source_spans_are_valid(src),
                "來源範圍不可指向 UTF-8 中間或 EOF 之外");
        }
    }
}

#[cfg(test)]
mod r02_f04_公私算術_hir_跨度操作與不可篡改契約 {
    use super::*;
    #[test]
    fn 合法_utf8_二元節點在同一_hir_驗證子節點類型及原始運算子() {
        let code="繁體\n4 + 5";
        let begin=code.find('4').unwrap();
        let lhs=NodeSpan::new(begin,begin+1);
        let rhs=NodeSpan::new(begin+4,begin+5);
        let parent=NodeSpan::new(begin,begin+5);
        let fact=VerifiedScalarArithmetic {
            span:parent,left:lhs,right:rhs,operator:'+',
        };
        let mut hir=VerifiedScalarHir::default();
        for span in [lhs,rhs,parent] { hir.record_expression(span,ScalarType::Int); }
        hir.record_arithmetic(fact);
        assert!(hir.source_spans_are_valid(code));
        assert_eq!(hir.arithmetic(parent),Some(fact));
        assert_eq!(hir.verified_arithmetic().count(),1);
        // 更換運算子或左右來源均不得覆寫已核准事實。
        hir.record_arithmetic(VerifiedScalarArithmetic {operator:'-',..fact});
        assert!(!hir.is_consistent());
        assert_eq!(hir.arithmetic(parent),Some(fact));
    }
    #[test]
    fn 來源不存在的運算子_或不同型別子節點必須拒絕() {
        let src="4 + 5";
        let left=NodeSpan::new(0,1);
        let right=NodeSpan::new(4,5);
        let span=NodeSpan::new(0,5);
        let mut wrong=VerifiedScalarHir::default();
        wrong.record_expression(left,ScalarType::Int);
        wrong.record_expression(right,ScalarType::Bool);
        wrong.record_expression(span,ScalarType::Int);
        wrong.record_arithmetic(VerifiedScalarArithmetic {
            span,left,right,operator:'-',
        });
        assert!(!wrong.source_spans_are_valid(src));
        let mut missing=VerifiedScalarHir::default();
        missing.record_expression(span,ScalarType::Int);
        missing.record_arithmetic(VerifiedScalarArithmetic {span,left,right,operator:'+'});
        assert!(!missing.source_spans_are_valid(src));
    }
}
