//! P16.6-R02.2：公開 MVP-A 與私有安全前端共用的「已核准交集」語意 AST。
//! 兩套舊解析器目前仍存在，且各自的完整語法、診斷和入口權限不變。
//! 只有可以保留原宣告位置、順序與語意的結點才可以投影到此結構。
// R02.2 此刻只由私有／公開差異回歸消耗；正式 Checker 遷移前，避免非測試編譯死碼警告。
#![allow(dead_code)]

use crate::expr::{Expr as PublicExpr, ExprKind as PublicExprKind};
use crate::parser::{self, Item, StmtKind as PublicStmtKind};
use crate::ownership_ledger::DeclarationId;

/// 原始 UTF-8 位元組半開範圍，供共同解析核心與各模式正式 AST 使用。
/// 此型別不是已換算為一基行列的公開診斷位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}
impl NodeSpan {
    pub(crate) fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "節點來源半開範圍不可反向");
        Self { start, end }
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

#[derive(Debug, Default)]
pub(crate) struct VerifiedScalarHir {
    references: std::collections::BTreeMap<usize, TypedUse<ScalarType>>,
    assignments: std::collections::BTreeMap<usize, TypedUse<ScalarType>>,
    expressions: std::collections::BTreeMap<(usize, usize), VerifiedScalarExpression>,
}
impl VerifiedScalarHir {
    pub(crate) fn record_expression(&mut self, span: NodeSpan, ty: ScalarType) {
        let fact = VerifiedScalarExpression { span, ty };
        let previous = self.expressions.insert((span.start, span.end), fact);
        debug_assert!(previous.is_none_or(|old| old == fact),
            "同一個來源位置不能有兩種衝突的運算式型別");
    }
    pub(crate) fn expression(&self, span: NodeSpan) -> Option<VerifiedScalarExpression> {
        self.expressions.get(&(span.start, span.end)).copied()
    }
    pub(crate) fn record_reference(&mut self, use_at: TypedUse<ScalarType>) {
        let previous = self.references.insert(use_at.span.start, use_at);
        debug_assert!(previous.is_none_or(|old| old == use_at),
            "同一個來源讀取位置不能有兩個相衝突的具型別身份");
    }
    pub(crate) fn record_assignment(&mut self, use_at: TypedUse<ScalarType>) {
        let previous = self.assignments.insert(use_at.span.start, use_at);
        debug_assert!(previous.is_none_or(|old| old == use_at),
            "同一個賦值來源位置不能有兩個相衝突的目標身份");
    }
    pub(crate) fn reference(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.references.get(&span.start).copied().filter(|entry| entry.span == span)
    }
    pub(crate) fn assignment(&self, span: NodeSpan) -> Option<TypedUse<ScalarType>> {
        self.assignments.get(&span.start).copied().filter(|entry| entry.span == span)
    }

    /// F11：只讀稽核介面，保留原 HIR 已驗證位置而不交出可寫 map。
    pub(crate) fn verified_references(&self)
        -> impl Iterator<Item = TypedUse<ScalarType>> + '_ {
        self.references.values().copied()
    }
    pub(crate) fn verified_assignments(&self)
        -> impl Iterator<Item = TypedUse<ScalarType>> + '_ {
        self.assignments.values().copied()
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
pub(crate) enum ScalarExpr {
    Int(i64),
    Bool(bool),
    Name(String),
    Binary { left: Box<ScalarExpr>, op: char, right: Box<ScalarExpr> },
    CallZero(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
pub(crate) struct ScalarFunction {
    pub(crate) name: String,
    pub(crate) result_ty: ScalarType,
    pub(crate) body: Vec<ScalarStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedAst {
    pub(crate) functions: Vec<ScalarFunction>,
}

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
        PublicExprKind::ZeroArgCall(name) => ScalarExpr::CallZero(name.clone()),
        _ => return None,
    };
    Some(result)
}

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
            PublicStmtKind::Assignment { name, value } => ScalarStmt::Assign {
                name: name.clone(), value: public_expression(value)?,
            },
            PublicStmtKind::IfChain { branches, otherwise } => {
                // P01 的平面 else-if 與私有 P13 的巢狀 If(no=[If])
                // 必須產生同一棵保留原分支順序的共同 AST。
                if branches.is_empty() { return None; }
                let mut tail = map_optional(otherwise.as_ref(),
                    |block| public_statements(&block.statements))?;
                for arm in branches.iter().rev() {
                    tail = Some(vec![ScalarStmt::If {
                        condition: public_expression(&arm.condition)?,
                        yes: public_statements(&arm.body.statements)?,
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
pub(crate) fn from_public(file: &parser::File) -> Option<SharedAst> {
    let functions = file.items.iter().map(|item| {
        let Item::Function(fun) = item else { return None };
        Some(ScalarFunction {
            name: fun.name.clone(),
            result_ty: fun.returns,
            body: public_statements(&fun.body.statements)?,
        })
    }).collect::<Option<Vec<_>>>()?;
    Some(SharedAst { functions })
}

// 外層 Option 表示投影成功／失敗；內層 Option 才表示語法原本不存在。
// 不可把缺少 initializer 或缺少 else 誤當成「無法投影」。
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
