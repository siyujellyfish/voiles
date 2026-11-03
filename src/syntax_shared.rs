//! P16.6-R02：公開 MVP-A 與私有 P13～P16 共用的詞彙判斷和 int64 字面值規則。
//! 只統一兩者均已核准的基礎契約；語法擴充、資源語意和入口權限不由此模組決定。

use crate::lexer::{Token, TokenKind};

pub(crate) fn symbol(tokens: &[Token], cursor: usize, ch: char) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Symbol(c)) if *c == ch)
}

pub(crate) fn keyword(tokens: &[Token], cursor: usize, word: &str) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Keyword(s)) if s == word)
}

/// 兩種正式解析器共用：只有指定 Keyword 才推進，失敗游標不變。
pub(crate) fn take_keyword(tokens: &[Token], cursor: &mut usize, word: &str) -> Option<usize> {
    let token = tokens.get(*cursor)?;
    if !matches!(&token.kind, TokenKind::Keyword(value) if value == word) { return None; }
    *cursor += 1;
    Some(token.start)
}

/// 公私正式函式簽章共用的 Arrow 消耗：失敗不推進游標。
pub(crate) fn take_return_arrow(tokens: &[Token], cursor: &mut usize) -> Option<usize> {
    let token = tokens.get(*cursor)?;
    if !matches!(token.kind, TokenKind::Arrow) { return None; }
    *cursor += 1;
    Some(token.end)
}

/// 公私共同 if/else-if 文法產生的單一條件分支。
#[derive(Debug, Clone)]
pub(crate) struct ParsedIfArm<E, B> {
    pub(crate) start: usize,
    pub(crate) branch: crate::common_ir::ConditionalBranch<E, B>,
}
/// 公私 Checker 只讀取共同分支的 condition/body，不自行改寫；
/// 同時保留真正解析器產生的 if/else-if 每一分支原始來源 start。
impl<E,B> std::ops::Deref for ParsedIfArm<E,B> {
    type Target = crate::common_ir::ConditionalBranch<E,B>;
    fn deref(&self) -> &Self::Target { &self.branch }
}

/// 不依賴任何特定 Parser 的完整已核准 if/else-if 鏈。
#[derive(Debug, Clone)]
pub(crate) struct ParsedIfChain<E, B> {
    pub(crate) arms: Vec<ParsedIfArm<E, B>>,
    pub(crate) otherwise: Option<B>,
    pub(crate) end: usize,
}

/// 入口各自保留條件限制與診斷；真正共同的 if/else-if 順序、
/// 可選 else、換行回退與節點順序由以下解析器唯一維護。
pub(crate) trait IfSyntax {
    type Condition;
    type Body;
    fn current_if_start(&self) -> usize;
    fn begin_if(&mut self) -> Result<(), crate::Diagnostic>;
    fn if_condition(&mut self) -> Result<Self::Condition, crate::Diagnostic>;
    fn if_body(&mut self) -> Result<Self::Body, crate::Diagnostic>;
    fn maybe_else(&mut self) -> bool;
    fn next_is_if(&self) -> bool;
    fn last_end(&self) -> usize;
}

pub(crate) fn parse_if_chain<P: IfSyntax>(
    parser: &mut P,
) -> Result<ParsedIfChain<P::Condition, P::Body>, crate::Diagnostic> {
    let mut arms = Vec::new();
    loop {
        let start = parser.current_if_start();
        parser.begin_if()?;
        let condition = parser.if_condition()?;
        let body = parser.if_body()?;
        arms.push(ParsedIfArm {
            start, branch: crate::common_ir::ConditionalBranch::new(condition, body),
        });
        if !parser.maybe_else() {
            return Ok(ParsedIfChain {
                arms, otherwise: None, end: parser.last_end(),
            });
        }
        if !parser.next_is_if() {
            let otherwise = parser.if_body()?;
            return Ok(ParsedIfChain {
                arms, otherwise: Some(otherwise), end: parser.last_end(),
            });
        }
    }
}

/// F03：正式 if 的 else 可以是真實 Parser 直接交付的大括號區塊，
/// 或 else-if 既有 CFG 單一巢狀語句；兩種情況不得再將區塊
/// 拆成純 Vec 而遺失其來源跨度。私有 Checker 仍可唯讀走訪 slice。
#[derive(Debug, Clone)]
pub(crate) enum ParsedElse<S> {
    Block(ParsedBlock<S>),
    ElseIf(Box<S>),
    /// 僅相容歷史手工建立 CFG 的 Rust 測試，不允許正式 Parser 產生。
    #[cfg(test)]
    SyntheticTest(Vec<S>),
}
impl<S> ParsedElse<S> {
    pub(crate) fn as_slice(&self) -> &[S] {
        match self {
            Self::Block(block) => &block.statements,
            Self::ElseIf(stmt) => std::slice::from_ref(stmt),
            #[cfg(test)]
            Self::SyntheticTest(stmts) => stmts,
        }
    }
}
impl<S> std::ops::Deref for ParsedElse<S> {
    type Target = [S];
    fn deref(&self) -> &Self::Target { self.as_slice() }
}
#[cfg(test)]
impl<S> From<Vec<S>> for ParsedElse<S> {
    fn from(stmts: Vec<S>) -> Self { Self::SyntheticTest(stmts) }
}

/// 正式函式簽章及本體的共同產物；各入口仍保留專屬參數、回傳型別與 body AST。
#[derive(Debug, Clone)]
pub(crate) struct ParsedFunction<P, R, B> {
    pub(crate) name: String,
    pub(crate) params: P,
    pub(crate) result_ty: R,
    pub(crate) body: B,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl<P, R, B> ParsedFunction<P, R, B> {
    /// 從共用正式解析產物取得整個函式原始跨度及入口模式。
    pub(crate) fn checked_identity(
        &self, mode: crate::common_ir::FrontendMode, source: &str,
    ) -> Option<crate::common_ir::ModeNodeIdentity> {
        crate::common_ir::ModeNodeIdentity::checked(
            mode, crate::common_ir::NodeSpan::new(self.start, self.end),
            crate::common_ir::NodeRole::Function, source,
        )
    }
}

/// 唯一正式函式宣告組構順序：fn→名稱→(參數)→回傳箭號→型別→本體。
pub(crate) trait FunctionSyntax {
    type Params;
    type ResultType;
    type Body;
    fn begin_function(&mut self) -> Result<usize, crate::Diagnostic>;
    fn function_name(&mut self) -> Result<String, crate::Diagnostic>;
    fn function_left_paren(&mut self) -> Result<(), crate::Diagnostic>;
    fn function_params(&mut self) -> Result<Self::Params, crate::Diagnostic>;
    fn function_right_paren(&mut self) -> Result<(), crate::Diagnostic>;
    fn function_return_arrow(&mut self) -> Result<(), crate::Diagnostic>;
    fn function_result_type(&mut self) -> Result<Self::ResultType, crate::Diagnostic>;
    fn function_body(&mut self) -> Result<Self::Body, crate::Diagnostic>;
    fn function_last_end(&self) -> usize;
}

pub(crate) fn parse_function_decl<P: FunctionSyntax>(
    parser: &mut P,
) -> Result<ParsedFunction<P::Params, P::ResultType, P::Body>, crate::Diagnostic> {
    let start = parser.begin_function()?;
    let name = parser.function_name()?;
    parser.function_left_paren()?;
    let params = parser.function_params()?;
    parser.function_right_paren()?;
    parser.function_return_arrow()?;
    let result_ty = parser.function_result_type()?;
    let body = parser.function_body()?;
    Ok(ParsedFunction {
        name, params, result_ty, body,
        start, end: parser.function_last_end(),
    })
}

/// 兩套正式 Parser 的區塊解析結果，保留原始來源起訖位置。
#[derive(Debug, Clone)]
pub(crate) struct ParsedBlock<S> {
    pub(crate) statements: Vec<S>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl<S> ParsedBlock<S> {
    /// 區塊含真正左右大括號，而非僅依子語句反推的合成跨度。
    pub(crate) fn checked_identity(
        &self, mode: crate::common_ir::FrontendMode, source: &str,
    ) -> Option<crate::common_ir::ModeNodeIdentity> {
        crate::common_ir::ModeNodeIdentity::checked(
            mode, crate::common_ir::NodeSpan::new(self.start, self.end),
            crate::common_ir::NodeRole::Block, source,
        )
    }
}

/// F01：正式兩模式區塊內的各個「直接語句」都必須是真正 Parser
/// 建立的 UTF-8 半開跨度，不能只驗證函式或大括號而放過假造位置。
/// 巢狀語句仍須由原有 Parser／Checker 負責其語法與安全合法性；
/// 此函式不授權任何新型別、借用或控制流程。
impl<K> ParsedBlock<crate::common_ir::SpannedNode<K>> {
    pub(crate) fn checked_direct_statements(
        &self, mode: crate::common_ir::FrontendMode, source: &str,
    ) -> bool {
        self.checked_identity(mode, source).is_some()
            && self.statements.iter().all(|statement|
                statement.checked_mode_identity(
                    mode, crate::common_ir::NodeRole::Statement, source
                ).is_some()
                && statement.start >= self.start
                && statement.end <= self.end
            )
    }
}

/// F03：原私有 Checker 已核准的唯讀區塊走訪仍可使用
/// Vec<S> 的迭代、切片和索引介面，但必須讀取正式 Parser
/// 唯一交付的 ParsedBlock，不得事後複製成第二本 body AST。
impl<S> std::ops::Deref for ParsedBlock<S> {
    type Target = Vec<S>;
    fn deref(&self) -> &Vec<S> { &self.statements }
}

/// 公開／私有只負責自身診斷、分句與具體語句，區塊走訪流程由共通核心負責。
pub(crate) trait BlockSyntax {
    type Statement;
    fn begin_block(&mut self) -> Result<usize, crate::Diagnostic>;
    fn block_at_end(&self) -> bool;
    fn block_at_close(&self) -> bool;
    fn block_missing_close(&self) -> crate::Diagnostic;
    fn parse_block_statement(&mut self) -> Result<Self::Statement, crate::Diagnostic>;
    fn finish_block_statement(&mut self) -> Result<(), crate::Diagnostic>;
    fn finish_block(&mut self, statement_count: usize)
        -> Result<usize, crate::Diagnostic>;
}

pub(crate) fn parse_block<P: BlockSyntax>(
    parser: &mut P,
) -> Result<ParsedBlock<P::Statement>, crate::Diagnostic> {
    let start = parser.begin_block()?;
    let mut statements = Vec::new();
    loop {
        if parser.block_at_end() { return Err(parser.block_missing_close()); }
        if parser.block_at_close() {
            let end = parser.finish_block(statements.len())?;
            return Ok(ParsedBlock { statements, start, end });
        }
        statements.push(parser.parse_block_statement()?);
        parser.finish_block_statement()?;
    }
}

/// 公私兩套正式運算式解析器的共同括號解析順序。
/// 兩個入口都在進入此函式前消耗左括號；失敗診斷及 AST 模式由入口決定。
pub(crate) trait GroupSyntax {
    type Expression;
    fn group_expression(&mut self) -> Result<Self::Expression, crate::Diagnostic>;
    fn group_right_paren(&mut self) -> Result<usize, crate::Diagnostic>;
    fn group_result(&mut self, expression: Self::Expression, start: usize, end: usize)
        -> Self::Expression;
}

pub(crate) fn parse_group<P: GroupSyntax>(
    parser: &mut P, start: usize,
) -> Result<P::Expression, crate::Diagnostic> {
    let expression = parser.group_expression()?;
    let end = parser.group_right_paren()?;
    Ok(P::group_result(parser, expression, start, end))
}

/// 公私正式返回 AST：保存共同 return 核心直接建構的唯一
/// 運算式節點；公私語意差異與資源移交仍在原核准 Checker 驗證。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedReturn<E> {
    pub(crate) expression: E,
}
impl<E> ParsedReturn<E> {
    pub(crate) fn new(expression: E) -> Self { Self { expression } }
}
impl<E> std::ops::Deref for ParsedReturn<E> {
    type Target = E;
    fn deref(&self) -> &E { &self.expression }
}

/// 公私 return 由同一正式核心依序消耗關鍵字、解析運算式、檢查模式權限。
pub(crate) trait ReturnSyntax {
    type Expression;
    fn begin_return(&mut self) -> Result<(), crate::Diagnostic>;
    fn return_expression(&mut self) -> Result<Self::Expression, crate::Diagnostic>;
    fn validate_return(&self, expression: &Self::Expression) -> Result<(), crate::Diagnostic>;
}
pub(crate) fn parse_return<P: ReturnSyntax>(
    parser: &mut P,
) -> Result<ParsedReturn<P::Expression>, crate::Diagnostic> {
    parser.begin_return()?;
    let expression = parser.return_expression()?;
    parser.validate_return(&expression)?;
    Ok(ParsedReturn::new(expression))
}

/// 公私 var/const 的唯一共同解析與初始化先後順序；型別與來源限制由入口決定。
#[derive(Debug, Clone)]
pub(crate) struct ParsedDeclaration<T, E> {
    pub(crate) name: String,
    pub(crate) ty: T,
    pub(crate) mutable: bool,
    pub(crate) initializer: Option<E>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
impl<T, E> ParsedDeclaration<T, E> {
    pub(crate) fn checked_identity(
        &self, mode: crate::common_ir::FrontendMode, source: &str,
    ) -> Option<crate::common_ir::ModeNodeIdentity> {
        crate::common_ir::ModeNodeIdentity::checked(
            mode, crate::common_ir::NodeSpan::new(self.start, self.end),
            crate::common_ir::NodeRole::Declaration, source,
        )
    }
}

pub(crate) trait DeclarationSyntax {
    type Type;
    type Expression;
    fn declaration_start(&self) -> usize;
    fn begin_declaration(&mut self) -> Result<bool, crate::Diagnostic>;
    fn declaration_name(&mut self) -> Result<String, crate::Diagnostic>;
    fn declaration_colon(&mut self) -> Result<(), crate::Diagnostic>;
    fn declaration_type(&mut self) -> Result<Self::Type, crate::Diagnostic>;
    fn declaration_has_initializer(&self) -> bool;
    fn declaration_equals(&mut self);
    fn declaration_expression(&mut self) -> Result<Self::Expression, crate::Diagnostic>;
    fn validate_declaration_initializer(
        &self, mutable: bool, initializer: &Option<Self::Expression>,
    ) -> Result<(), crate::Diagnostic>;
    fn declaration_end(&self, initializer: &Option<Self::Expression>) -> usize;
}
pub(crate) fn parse_declaration<P: DeclarationSyntax>(
    parser: &mut P,
) -> Result<ParsedDeclaration<P::Type, P::Expression>, crate::Diagnostic> {
    let start = parser.declaration_start();
    let mutable = parser.begin_declaration()?;
    let name = parser.declaration_name()?;
    parser.declaration_colon()?;
    let ty = parser.declaration_type()?;
    let initializer = if parser.declaration_has_initializer() {
        parser.declaration_equals();
        Some(parser.declaration_expression()?)
    } else { None };
    parser.validate_declaration_initializer(mutable, &initializer)?;
    let end = parser.declaration_end(&initializer);
    Ok(ParsedDeclaration { name, ty, mutable, initializer, start, end })
}

/// F03：正式公私 Parser 唯一的共同語句前綴分類。
/// 僅識別原有交集及私有延伸的外形，絕不替任何前端核准語意。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatementHead {
    Declaration,
    Conditional,
    Return,
    Named,
    ModeSpecific,
}
pub(crate) fn statement_head(
    tokens: &[Token], cursor: usize, mode: crate::common_ir::FrontendMode,
) -> Option<StatementHead> {
    let token = tokens.get(cursor)?;
    match &token.kind {
        TokenKind::Keyword(word) if word == "var" || word == "const" =>
            Some(StatementHead::Declaration),
        TokenKind::Keyword(word) if word == "if" =>
            Some(StatementHead::Conditional),
        TokenKind::Keyword(word) if word == "return" =>
            Some(StatementHead::Return),
        TokenKind::Name(_) => Some(StatementHead::Named),
        // 僅讓私有 Parser 辨識已核准的延伸語句種類，權限
        // 依然由原 P13 Parser + Checker 判斷，不放寬公用入口。
        TokenKind::Keyword(word) if mode == crate::common_ir::FrontendMode::PrivateVerified
            && matches!(word.as_str(), "for" | "while" | "break" | "continue") =>
            Some(StatementHead::ModeSpecific),
        _ => None,
    }
}

/// F03：公私皆必須透過本共同建構器依序讀取賦值名稱／=／RHS。
/// 保留原 Name 與型別／初始化的專屬診斷。私有欄位賦值
/// 仍先由 P13 確認 . 後綴並轉入原有欄位寫入規則。
#[derive(Debug, Clone)]
pub(crate) struct ParsedNamedAssignment<E> {
    pub(crate) name: String,
    pub(crate) expression: E,
}
pub(crate) trait NamedAssignmentSyntax: AssignmentSyntax {
    fn named_assignment_name(&mut self) -> Result<String, crate::Diagnostic>;
}
pub(crate) fn parse_named_assignment<P: NamedAssignmentSyntax>(
    parser: &mut P,
) -> Result<ParsedNamedAssignment<P::Expression>, crate::Diagnostic> {
    let name = parser.named_assignment_name()?;
    let expression = parse_assignment(parser)?;
    Ok(ParsedNamedAssignment { name, expression })
}

/// 公私已核准具名整筆賦值尾段的共同解析核心。
/// 名稱在進入前已解析；欄位賦值由私有入口獨立保留。
pub(crate) trait AssignmentSyntax {
    type Expression;
    fn assignment_equals(&mut self) -> Result<(), crate::Diagnostic>;
    fn assignment_expression(&mut self) -> Result<Self::Expression, crate::Diagnostic>;
    fn validate_assignment(&self, expression: &Self::Expression) -> Result<(), crate::Diagnostic>;
}
pub(crate) fn parse_assignment<P: AssignmentSyntax>(
    parser: &mut P,
) -> Result<P::Expression, crate::Diagnostic> {
    parser.assignment_equals()?;
    let expression = parser.assignment_expression()?;
    parser.validate_assignment(&expression)?;
    Ok(expression)
}

/// 公私正式函式呼叫共用已消耗左括號之後的引數與右括號解析順序。
/// F02：公私真正函式呼叫共同 AST；零參數公開 P01 的
/// A=()，私有 P13 已核准位置引數的 A=Vec<Term>。
/// 尚未核准的具名引數／方法語法不藉此擴張。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedNamedCall<A> {
    pub(crate) name: String,
    pub(crate) arguments: A,
}

pub(crate) trait CallSyntax {
    type Arguments;
    fn call_arguments(&mut self) -> Result<Self::Arguments, crate::Diagnostic>;
    fn call_right_paren(&mut self) -> Result<usize, crate::Diagnostic>;
}
pub(crate) fn parse_call<P: CallSyntax>(
    parser: &mut P,
) -> Result<(P::Arguments, usize), crate::Diagnostic> {
    let args = parser.call_arguments()?;
    let end = parser.call_right_paren()?;
    Ok((args, end))
}

/// 已經消耗函式名稱與左圓括號時，由正式公私共同核心
/// 將名稱／已核准的參數列表合成同一呼叫 AST。
pub(crate) fn parse_named_call<P: CallSyntax>(
    parser: &mut P, name: String,
) -> Result<(ParsedNamedCall<P::Arguments>, usize), crate::Diagnostic> {
    let (arguments, end) = parse_call(parser)?;
    Ok((ParsedNamedCall { name, arguments }, end))
}

pub(crate) fn newline(tokens: &[Token], cursor: usize) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::Newline))
}

/// R02.13：兩套正式 Parser 共用的語句分隔規則。
/// 右大括號結束目前區塊但不可在此消耗，換行序列則須全數消耗；
/// 其它 token 一律拒絕並保留原來的錯誤游標。
pub(crate) fn finish_statement(tokens: &[Token], cursor: &mut usize) -> bool {
    if symbol(tokens, *cursor, '}') { return true; }
    if !newline(tokens, *cursor) { return false; }
    skip_newlines(tokens, cursor);
    true
}

/// R02.12：兩套正式 Parser 共用連續換行略過。
/// 只略過實際 Newline token；不吞掉 End、符號及其它詞彙。
pub(crate) fn skip_newlines(tokens: &[Token], cursor: &mut usize) {
    while newline(tokens, *cursor) { *cursor += 1; }
}

/// R02.12：公開運算式與私有結構解析器共用唯一 bool 常值判讀。
/// 不將普通 Name、其它保留字或大小寫變體當成布林常值。
/// 共同純值字面值 AST；正式公私 Parser 直接建構此節點後保留各自語意節點種類。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarLiteral {
    Integer(String),
    Boolean(bool),
}
impl ScalarLiteral {
    pub(crate) fn type_name(&self) -> &'static str {
        match self { Self::Integer(_) => "int", Self::Boolean(_) => "bool" }
    }
}

/// 不消耗 token 的共同字面值判讀，保留 UTF-8 原始位元組跨度。
/// 私有 Parser 在實際消耗之前仍可執行已核准的 i64 溢位拒絕。
pub(crate) fn peek_scalar_literal(
    tokens: &[Token], cursor: usize,
) -> Option<crate::common_ir::SpannedNode<ScalarLiteral>> {
    let token = tokens.get(cursor)?;
    let kind = match &token.kind {
        TokenKind::Integer(text) => ScalarLiteral::Integer(text.clone()),
        TokenKind::Keyword(word) => ScalarLiteral::Boolean(boolean_keyword(word)?),
        _ => return None,
    };
    Some(crate::common_ir::SpannedNode {
        kind, start: token.start, end: token.end,
    })
}

/// F02：兩條正式 Parser 共用**同一個字面值節點產生及提交演算法**。
/// Parser 模式只負責 ScalarLiteral→正式 AST Kind 的轉換及已核准模式
/// 特有的型別限制；轉換失敗不消耗 token，保留既有私有診斷優先序。
pub(crate) fn parse_scalar_atom<K>(
    tokens: &[Token], cursor: &mut usize,
    convert: impl FnOnce(ScalarLiteral) -> Result<K, crate::Diagnostic>,
) -> Result<Option<crate::common_ir::SpannedNode<K>>, crate::Diagnostic> {
    let Some(node) = peek_scalar_literal(tokens, *cursor) else { return Ok(None); };
    let kind = convert(node.kind)?;
    *cursor += 1;
    Ok(Some(crate::common_ir::SpannedNode {
        kind, start: node.start, end: node.end,
    }))
}

/// 既有相容入口委派給相同正式節點產生器，不保留另一套游標邏輯。
pub(crate) fn take_scalar_literal(
    tokens: &[Token], cursor: &mut usize,
) -> Option<crate::common_ir::SpannedNode<ScalarLiteral>> {
    parse_scalar_atom(tokens, cursor, Ok)
        .expect("相同純值字面值不需附加模式驗證")
}

/// F02：兩套正式 Parser 的無後綴具名原子直接由單一 AST 產生器
/// 建構 SpannedNode<K>。先以共通名稱讀取預覽下一個 token，遇到
/// 可能含函式呼叫／方法／索引等模式專屬後綴時不消耗游標；
/// 仍由各模式原有的後綴及權限規則獨立解析。
pub(crate) fn parse_bare_name<K>(
    tokens: &[Token], cursor: &mut usize, convert: impl FnOnce(String) -> K,
) -> Option<crate::common_ir::SpannedNode<K>> {
    let mut following = *cursor;
    let name = take_spanned_name(tokens, &mut following)?;
    if matches!(
        tokens.get(following).map(|token| &token.kind),
        Some(TokenKind::Symbol('(' | '.' | '['))
    ) {
        return None;
    }
    *cursor = following;
    Some(crate::common_ir::SpannedNode {
        kind: convert(name.kind), start: name.start, end: name.end,
    })
}

pub(crate) fn boolean_keyword(word: &str) -> Option<bool> {
    match word {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub(crate) fn end(tokens: &[Token], cursor: usize) -> bool {
    matches!(tokens.get(cursor).map(|t| &t.kind), Some(TokenKind::End) | None)
}

/// R02.7：真正供公開 P06 與私有 P13 運算式解析器共用的
/// 整數加減運算子判讀，僅允許兩者均已核准的 +／-。
/// 左結合及來源跨度由呼叫者既有遞迴解析程序維持。
pub(crate) fn additive_operator(tokens: &[Token], cursor: usize) -> Option<char> {
    match tokens.get(cursor).map(|token| &token.kind) {
        Some(TokenKind::Symbol(ch)) if *ch == '+' || *ch == '-' => Some(*ch),
        _ => None,
    }
}

/// R02.11：共用真正左結合算術解析折疊。各 Parser 仍只由自身
/// 的 operand 解析器決定可用後綴、AST 型別及語法診斷；本層
/// 只在 next 成功提供一組已核准運算子／右運算元時建立左結合節點。
/// 任一右運算元解析失敗須立即中止，不可吞掉失敗變成部分 AST。
pub(crate) fn fold_additive<T, E>(
    mut left: T,
    mut next: impl FnMut() -> Result<Option<(char, T)>, E>,
    mut combine: impl FnMut(T, char, T) -> T,
) -> Result<T, E> {
    while let Some((op, right)) = next()? {
        debug_assert!(matches!(op, '+' | '-'), "僅接受核准的加減運算子");
        left = combine(left, op, right);
    }
    Ok(left)
}

/// R02.10：公開檔案 Parser、公開算式 Parser、私有 P13 Parser
/// 真正共用的符號消耗原語。僅精確匹配指定 Symbol 時推進一次，
/// 並返回來源原有 token 終點；失敗不消耗任何符號或改寫錯誤位置。
/// R02.15：真實公開與私有加減 AST 共用半開來源範圍合成。
/// 建樹仍由原 Parser 決定，以保留不同的後綴及錯誤診斷契約。
pub(crate) fn fold_additive_spanned<T, E>(
    left: T,
    next: impl FnMut() -> Result<Option<(char, T)>, E>,
    span_of: impl Fn(&T) -> crate::common_ir::NodeSpan,
    mut combine: impl FnMut(T, char, T, crate::common_ir::NodeSpan) -> T,
) -> Result<T, E> {
    fold_additive(left, next, |left, op, right| {
        let span = crate::common_ir::NodeSpan::new(
            span_of(&left).start, span_of(&right).end,
        );
        combine(left, op, right, span)
    })
}

pub(crate) fn take_symbol(
    tokens: &[Token], cursor: &mut usize, expected: char,
) -> Option<usize> {
    let token = tokens.get(*cursor)?;
    if !matches!(&token.kind, TokenKind::Symbol(ch) if *ch == expected) {
        return None;
    }
    *cursor += 1;
    Some(token.end)
}

/// R02.8：公開與私有 Parser 實際共用的名稱 token 消耗原語。
/// 僅在 Name 時推進游標，失敗不能消耗任何 token；
/// 診斷文字／位置由呼叫端保留原先所屬的解析模式。
pub(crate) fn take_name(tokens: &[Token], cursor: &mut usize) -> Option<String> {
    let TokenKind::Name(value) = &tokens.get(*cursor)?.kind else { return None };
    *cursor += 1;
    Some(value.clone())
}

/// 公私真正共用的具名 AST 原子，返回帶有 UTF-8 原始半開跨度的名稱節點。
/// 後綴語法由各 Parser 分別解釋，不因辨識名稱而放行借用／移動。
pub(crate) fn take_spanned_name(
    tokens: &[Token], cursor: &mut usize,
) -> Option<crate::common_ir::SpannedNode<String>> {
    let token = tokens.get(*cursor)?;
    let TokenKind::Name(name) = &token.kind else { return None };
    let result = crate::common_ir::SpannedNode {
        kind: name.clone(), start: token.start, end: token.end,
    };
    *cursor += 1;
    Some(result)
}

/// R02.9：公開 MVP-A 的純值型別消耗原語，唯一合法集合由
/// 共通 ScalarType 定義。私有 P13 的 str／struct 型別不由此函式擴張。
/// 若 token 不屬 int／bool，游標及錯誤位置必須完全不變。
pub(crate) fn take_scalar_type(
    tokens: &[Token], cursor: &mut usize,
) -> Option<crate::common_ir::ScalarType> {
    let TokenKind::Name(name) = &tokens.get(*cursor)?.kind else { return None };
    let ty = crate::common_ir::ScalarType::from_name(name)?;
    *cursor += 1;
    Some(ty)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerError {
    InvalidDigits,
    OutOfRange,
}

/// 不更動十進位、二／八／十六進位、底線分隔與有號 64 位元上限的既有規則。
pub(crate) fn integer_i64(literal: &str) -> Result<i64, IntegerError> {
    let (radix, digits) = if let Some(rest) = literal.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = literal.strip_prefix("0b") {
        (2, rest)
    } else if let Some(rest) = literal.strip_prefix("0o") {
        (8, rest)
    } else {
        (10, literal)
    };
    let normalized: String = digits.chars().filter(|c| *c != '_').collect();
    if normalized.is_empty() || !normalized.chars().all(|c| c.to_digit(radix).is_some()) {
        return Err(IntegerError::InvalidDigits);
    }
    let value = u128::from_str_radix(&normalized, radix)
        .map_err(|_| IntegerError::OutOfRange)?;
    if value > i64::MAX as u128 {
        return Err(IntegerError::OutOfRange);
    }
    Ok(value as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, source::Source};

    #[test]
    fn 共通具名節點保留中文字元來源位置且錯誤游標不變() {
        let tokens = [
            Token { kind: TokenKind::Name("字".into()), start: 0, end: 3 },
            Token { kind: TokenKind::Keyword("true".into()), start: 4, end: 8 },
        ];
        let mut at = 0;
        let name = take_spanned_name(&tokens, &mut at).unwrap();
        assert_eq!(name.kind, "字");
        assert_eq!(name.node_span(), crate::common_ir::NodeSpan::new(0, 3));
        assert_eq!(at, 1);
        assert_eq!(take_spanned_name(&tokens, &mut at), None);
        assert_eq!(at, 1);
    }

    #[test]
    fn 公私共同純值字面值_ast_保留_utf8_位元組跨度與失敗游標() {
        let code = "中文字 true 0xFF";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source).tokens;
        let boolean_pos = tokens.iter().position(|t| matches!(
            &t.kind, TokenKind::Keyword(word) if word == "true")).unwrap();
        let mut cursor = boolean_pos;
        let boolean = take_scalar_literal(&tokens, &mut cursor).unwrap();
        assert_eq!(boolean.kind, ScalarLiteral::Boolean(true));
        assert_eq!((boolean.start, boolean.end),
            (code.find("true").unwrap(), code.find("true").unwrap() + 4));
        let number_pos = tokens.iter().position(|t| matches!(
            t.kind, TokenKind::Integer(_))).unwrap();
        let number = peek_scalar_literal(&tokens, number_pos).unwrap();
        assert_eq!(number.kind, ScalarLiteral::Integer("0xFF".into()));
        assert_eq!(number.start, code.find("0xFF").unwrap());
        let mut failed = tokens.len();
        assert_eq!(take_scalar_literal(&tokens, &mut failed), None);
        assert_eq!(failed, tokens.len());
    }

    #[test]
    fn 關鍵字只能精確消耗且失敗保留游標() {
        let tokens = [
            Token {kind:TokenKind::Name("fn".into()),start:0,end:2},
            Token {kind:TokenKind::Keyword("fn".into()),start:3,end:5},
            Token {kind:TokenKind::Keyword("if".into()),start:6,end:8},
            Token {kind:TokenKind::End,start:8,end:8},
        ];
        let mut at=0;
        assert_eq!(take_keyword(&tokens,&mut at,"fn"),None);
        assert_eq!(at,0);
        at=1;
        assert_eq!(take_keyword(&tokens,&mut at,"if"),None);
        assert_eq!(at,1);
        assert_eq!(take_keyword(&tokens,&mut at,"fn"),Some(3));
        assert_eq!(at,2);
        assert_eq!(take_keyword(&tokens,&mut at,"if"),Some(6));
        assert_eq!(at,3);
        for pos in [3,4] {
            let mut cursor=pos;
            assert_eq!(take_keyword(&tokens,&mut cursor,"fn"),None);
            assert_eq!(cursor,pos);
        }
    }

    #[test]
    fn 指定符號只在完全匹配時移動且回傳原本_utf8_位元組終點() {
        let tokens=[
            Token {kind:TokenKind::Name("字".into()),start:0,end:3},
            Token {kind:TokenKind::Symbol('('),start:3,end:4},
            Token {kind:TokenKind::Symbol(')'),start:4,end:5},
            Token {kind:TokenKind::End,start:5,end:5},
        ];
        let mut cursor=0;
        assert_eq!(take_symbol(&tokens,&mut cursor,'('),None);
        assert_eq!(cursor,0);
        cursor=1;
        assert_eq!(take_symbol(&tokens,&mut cursor,')'),None);
        assert_eq!(cursor,1);
        assert_eq!(take_symbol(&tokens,&mut cursor,'('),Some(4));
        assert_eq!(cursor,2);
        assert_eq!(take_symbol(&tokens,&mut cursor,')'),Some(5));
        assert_eq!(cursor,3);
        for pos in [3,4] {
            let mut at=pos;
            assert_eq!(take_symbol(&tokens,&mut at,'('),None);
            assert_eq!(at,pos,"結束 token／超界不可以推進游標");
        }
    }

    #[test]
    fn 公開純值型別須由共同_scalar_type_決定且拒絕私有型別() {
        let tokens=[
            Token {kind:TokenKind::Name("int".into()),start:0,end:3},
            Token {kind:TokenKind::Name("bool".into()),start:4,end:8},
            Token {kind:TokenKind::Name("str".into()),start:9,end:12},
            Token {kind:TokenKind::Name("Custom".into()),start:13,end:19},
            Token {kind:TokenKind::Keyword("return".into()),start:20,end:26},
            Token {kind:TokenKind::End,start:26,end:26},
        ];
        let mut cursor=0;
        assert_eq!(take_scalar_type(&tokens,&mut cursor),
            Some(crate::common_ir::ScalarType::Int));
        assert_eq!(take_scalar_type(&tokens,&mut cursor),
            Some(crate::common_ir::ScalarType::Bool));
        assert_eq!(cursor,2);
        for _ in 0..3 {
            assert_eq!(take_scalar_type(&tokens,&mut cursor),None);
            assert_eq!(cursor,2,"私有 str 不能成為公開 MVP-A 型別");
        }
        for pos in 3..=tokens.len() {
            let mut at=pos;
            assert_eq!(take_scalar_type(&tokens,&mut at),None);
            assert_eq!(at,pos);
        }
    }

    #[test]
    fn 名稱原語只消耗真正_name_其他_token_及結束位置不變() {
        let tokens = [
            Token {kind:TokenKind::Name("水母_7".into()),start:0,end:8},
            Token {kind:TokenKind::Symbol('('),start:8,end:9},
            Token {kind:TokenKind::Keyword("return".into()),start:9,end:15},
            Token {kind:TokenKind::End,start:15,end:15},
        ];
        let mut cursor=0usize;
        assert_eq!(take_name(&tokens,&mut cursor),Some("水母_7".into()));
        assert_eq!(cursor,1);
        for _ in 0..3 {
            assert_eq!(take_name(&tokens,&mut cursor),None);
            assert_eq!(cursor,1,"不合法名稱絕不能跳過原先錯誤位置");
        }
        for position in [2,3,4] {
            let mut cursor=position;
            assert_eq!(take_name(&tokens,&mut cursor),None);
            assert_eq!(cursor,position);
        }
    }

    #[test]
    fn 布林常值不誤認其它保留字或不同大小寫名稱() {
        assert_eq!(boolean_keyword("true"), Some(true));
        assert_eq!(boolean_keyword("false"), Some(false));
        for word in ["True", "FALSE", "if", "return", "bool", "true_"] {
            assert_eq!(boolean_keyword(word), None);
        }
    }

    #[test]
    fn 共同語句分隔_右大括號不消耗_換行連續消耗_非法來源保持位置() {
        let tokens=[
            Token {kind:TokenKind::Newline,start:0,end:1},
            Token {kind:TokenKind::Newline,start:1,end:2},
            Token {kind:TokenKind::Symbol('}'),start:2,end:3},
            Token {kind:TokenKind::Keyword("return".into()),start:4,end:10},
            Token {kind:TokenKind::End,start:10,end:10},
        ];
        let mut cursor=0;
        assert!(finish_statement(&tokens,&mut cursor));
        assert_eq!(cursor,2);
        assert!(finish_statement(&tokens,&mut cursor));
        assert_eq!(cursor,2,"右大括號應由所屬 block 自己消耗");
        for pos in [3,4,5] {
            let mut cursor=pos;
            assert!(!finish_statement(&tokens,&mut cursor));
            assert_eq!(cursor,pos,"錯誤分隔不可跨越保留字或 End");
        }
    }

    #[test]
    fn 連續換行只消耗_newline_不跨越原來的其它_token() {
        let tokens = [
            Token {kind:TokenKind::Newline,start:0,end:1},
            Token {kind:TokenKind::Newline,start:1,end:2},
            Token {kind:TokenKind::Keyword("return".into()),start:2,end:8},
            Token {kind:TokenKind::Newline,start:8,end:9},
            Token {kind:TokenKind::End,start:9,end:9},
        ];
        let mut cursor=0;
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,2);
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,2,"不可跨越 return 保留字");
        cursor=3;
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,4);
        skip_newlines(&tokens,&mut cursor);
        assert_eq!(cursor,4,"不可消耗 End");
    }

    #[test]
    fn 共通加減節點半開跨度以最左運算元及最右運算元合成() {
        use crate::common_ir::NodeSpan;
        let mut operands = [
            ('-', NodeSpan::new(9, 15)),
            ('+', NodeSpan::new(19, 22)),
        ].into_iter();
        let tree = fold_additive_spanned(
            NodeSpan::new(2, 7),
            || Ok::<_, ()>(operands.next()),
            |node| *node,
            |_, _, _, span| span,
        ).unwrap();
        assert_eq!(tree, NodeSpan::new(2, 22));
    }

    #[test]
    fn 回傳箭號只精確消耗_arrow_且保留原位元組結尾() {
        let source = Source::new("fn main()->int{}".as_bytes().to_vec());
        let scan = lexer::scan(&source);
        let tokens = &scan.tokens;
        let position = tokens.iter().position(|token|
            matches!(token.kind, TokenKind::Arrow)).unwrap();
        let mut cursor = position;
        assert_eq!(take_return_arrow(tokens, &mut cursor), Some(tokens[position].end));
        assert_eq!(cursor, position + 1);
        assert_eq!(take_return_arrow(tokens, &mut cursor), None);
        assert_eq!(cursor, position + 1);
        let mut beyond = tokens.len();
        assert_eq!(take_return_arrow(tokens, &mut beyond), None);
        assert_eq!(beyond, tokens.len());
    }

    #[test]
    fn 真正左結合折疊保留求值順序及錯誤立即傳回() {
        let mut operators = [('-', 3_i64), ('+', 2), ('-', 4)].into_iter();
        let value = fold_additive(
            10_i64,
            || Ok::<_, &'static str>(operators.next()),
            |left, op, right| if op == '+' {left + right} else {left - right},
        ).unwrap();
        assert_eq!(value, 5, "10 - 3 + 2 - 4 必須依左結合求值");

        let mut called = 0;
        let result = fold_additive(
            7_i64,
            || {
                called += 1;
                if called == 1 { Ok(Some(('+', 5))) }
                else { Err("右運算元語法錯誤") }
            },
            |left, _, right| left + right,
        );
        assert_eq!(result, Err("右運算元語法錯誤"));
        assert_eq!(called, 2, "右運算元失敗不得被忽略而輸出部分樹");
    }

    #[test]
    fn 兩套真正解析器共用的加減規則只能回傳核准二元運算子() {
        let source = Source::new(b"+ - * /".to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        assert_eq!(additive_operator(&tokens.tokens, 0), Some('+'));
        assert_eq!(additive_operator(&tokens.tokens, 1), Some('-'));
        for cursor in 2..=tokens.tokens.len() {
            assert_eq!(additive_operator(&tokens.tokens, cursor), None);
        }
    }

    #[test]
    fn p16_6_r02_字面值正反界線在共用規則中一致() {
        for (raw, expected) in [
            ("0", Ok(0)), ("001", Ok(1)), ("0x2a", Ok(42)),
            ("0b101010", Ok(42)), ("0o52", Ok(42)),
            ("9_223_372_036_854_775_807", Ok(i64::MAX)),
            ("0x", Err(IntegerError::InvalidDigits)),
            ("0b102", Err(IntegerError::InvalidDigits)),
            ("9223372036854775808", Err(IntegerError::OutOfRange)),
        ] {
            assert_eq!(integer_i64(raw), expected, "整數 {raw}");
        }
    }

    #[test]
    fn p16_6_r02_兩種解析模式使用相同詞彙位置及結束標記() {
        let source = Source::new(b"fn main()->int{\nreturn 0\n}\n".to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        assert!(keyword(&tokens.tokens, 0, "fn"));
        assert!(!symbol(&tokens.tokens, 0, 'f'));
        let end_pos = tokens.tokens.len() - 1;
        assert!(end(&tokens.tokens, end_pos));
        assert!(end(&tokens.tokens, tokens.tokens.len()));
        assert!(!newline(&tokens.tokens, 0));
        assert!(tokens.tokens.iter().enumerate().any(|(i, _)| newline(&tokens.tokens, i)));
    }
}

#[cfg(test)]
mod f02_真正公私純值字面值共同建構器 {
    use super::*;
    use crate::{lexer, source::Source, Diagnostic, DiagnosticKind};

    #[test]
    fn 相同_utf8_節點跨度與游標只有成功後提交() {
        let src = Source::new("true\n42\n".as_bytes().to_vec());
        let toks = lexer::scan(&src);
        assert!(toks.diagnostics.is_empty());
        let mut cursor = 0;
        let first = parse_scalar_atom(&toks.tokens, &mut cursor,
            |atom| -> Result<ScalarLiteral, Diagnostic> { Ok(atom) })
            .unwrap().unwrap();
        assert_eq!(first.kind, ScalarLiteral::Boolean(true));
        assert_eq!(first.start, 0);
        assert_eq!(first.end, 4);
        assert_eq!(cursor, 1);

        let mut denied = 2;
        let err = parse_scalar_atom::<ScalarLiteral>(&toks.tokens, &mut denied, |_| {
            Err(Diagnostic {
                kind: DiagnosticKind::Syntax,
                message: "私有模式拒絕".into(),
                span: None,
            })
        }).unwrap_err();
        assert_eq!(err.message.as_ref(), "私有模式拒絕");
        assert_eq!(denied, 2, "模式驗證失敗必須保留原詞彙位置");
        let allowed = parse_scalar_atom(&toks.tokens, &mut denied,
            |atom| -> Result<ScalarLiteral, Diagnostic> { Ok(atom) })
            .unwrap().unwrap();
        assert_eq!(allowed.kind, ScalarLiteral::Integer("42".to_owned()));
        assert_eq!(denied, 3);
    }
}

#[cfg(test)]
mod f02_正式無後綴名稱共通組樹 {
    use super::*;
    use crate::{lexer, source::Source};

    #[test]
    fn 無後綴名稱公私使用同一半開跨度_後綴形式不能被預先消耗() {
        let source = Source::new("value\ncall()\nowner.move()\narr[0]\n".as_bytes().to_vec());
        let scanned = lexer::scan(&source);
        assert!(scanned.diagnostics.is_empty());
        let toks = &scanned.tokens;
        let start = toks.iter().position(|t| matches!(&t.kind, TokenKind::Name(s) if s=="value"))
            .unwrap();
        let mut at = start;
        let name = parse_bare_name(toks, &mut at, |s| s).unwrap();
        assert_eq!(name.kind, "value");
        assert_eq!(name.start, 0);
        assert_eq!(name.end, 5);
        assert_eq!(at, start + 1);

        for expected in ["call", "owner", "arr"] {
            let pos = toks.iter().position(
                |t| matches!(&t.kind, TokenKind::Name(s) if s == expected)).unwrap();
            let mut cursor = pos;
            assert!(parse_bare_name(toks, &mut cursor, |s| s).is_none());
            assert_eq!(cursor, pos, "函式、方法、索引的專屬後綴不得被共通裸名解析器消耗");
        }
    }
}

#[cfg(test)]
mod r02_f01_f03_模式隔離及共同賦值分類 {
    use super::*;
    use crate::common_ir::{FrontendMode, NodeIdentity, NodeRole, NodeSpan};

    #[test]
    fn 公私語句前綴只有原核准交集相同_私有關鍵字不開放公開模式() {
        for word in ["var", "const", "if", "return"] {
            let tokens = [Token {
                kind: TokenKind::Keyword(word.into()), start: 0, end: word.len(),
            }];
            assert_eq!(
                statement_head(&tokens, 0, FrontendMode::PublicMvpA),
                statement_head(&tokens, 0, FrontendMode::PrivateVerified));
        }
        for word in ["for", "while", "break", "continue"] {
            let tokens = [Token {
                kind: TokenKind::Keyword(word.into()), start: 0, end: word.len(),
            }];
            assert_eq!(statement_head(&tokens, 0, FrontendMode::PublicMvpA), None);
            assert_eq!(statement_head(&tokens, 0, FrontendMode::PrivateVerified),
                Some(StatementHead::ModeSpecific));
        }
    }

    #[test]
    fn 節點身分由_utf8_半開位置及角色共同決定_無效位置不可取得() {
        let source = "中文\nvalue";
        let from = source.find("value").unwrap();
        let span = NodeSpan::new(from, from + 5);
        let reference = NodeIdentity::checked(span, NodeRole::Reference, source).unwrap();
        let assignment = NodeIdentity::checked(span, NodeRole::Assignment, source).unwrap();
        assert_ne!(reference, assignment);
        assert_eq!(reference.span, span);
        assert!(NodeIdentity::checked(NodeSpan::new(1, 2),
            NodeRole::Expression, source).is_none());
        assert!(NodeIdentity::checked(NodeSpan::new(from, source.len() + 1),
            NodeRole::Statement, source).is_none());
    }
}
