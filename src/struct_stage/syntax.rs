// 對應 P16.6-R01 無語意變更之實作拆檔。
impl Parser<'_> {
    fn tok(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }
    fn syntax(&self, msg: &str) -> Diagnostic {
        let t = self.tok();
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: msg.into(),
            span: Some(self.source.span(t.start, t.end)),
        }
    }
    fn keyword(&self, word: &str) -> bool {
        crate::syntax_shared::keyword(self.tokens, self.cursor, word)
    }
    fn symbol(&self, c: char) -> bool {
        crate::syntax_shared::symbol(self.tokens, self.cursor, c)
    }
    fn eof(&self) -> bool { crate::syntax_shared::end(self.tokens, self.cursor) }
    fn lines(&mut self) {
        crate::syntax_shared::skip_newlines(self.tokens, &mut self.cursor);
    }
    fn take(&mut self, ch: char, msg: &str) -> Result<(), Diagnostic> {
        crate::syntax_shared::take_symbol(self.tokens, &mut self.cursor, ch)
            .map(|_| ())
            .ok_or_else(|| self.syntax(msg))
    }
    fn name(&mut self) -> Result<String, Diagnostic> {
        crate::syntax_shared::take_name(self.tokens, &mut self.cursor)
            .ok_or_else(|| self.syntax("需要合法的名稱或型別名稱"))
    }
    fn end_statement(&mut self) -> Result<(), Diagnostic> {
        if crate::syntax_shared::finish_statement(self.tokens, &mut self.cursor) {
            Ok(())
        } else {
            Err(self.syntax("語句之間必須換行"))
        }
    }
    /// 和 MVP-A i64 有效整數字面值規則一致；底線只作分隔，
    /// 範圍溢位在靜態前端拒絕，不移交未知執行時處理。
    fn valid_integer(literal: &str) -> bool {
        crate::syntax_shared::integer_i64(literal).is_ok()
    }
    /// P13 已核准的 decimal64 有限常值子集：採十進位字元和最近偶數捨入，
    /// 不將宿主二進位 f64 當成 decimal64。僅驗證調整指數 -15..=15 的數值。
    /// 回傳 (16 位十進位係數, 十進位指數)，保留未決邊界之外的拒絕。
    fn decimal64_checked_subset(literal: &str) -> Option<(u64, i32)> {
        let cleaned: String = literal.chars().filter(|c| *c != '_').collect();
        let (integer, fraction) = cleaned.split_once('.')?;
        if integer.is_empty()
            || !integer.bytes().all(|b| b.is_ascii_digit())
            || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let digits = format!("{integer}{fraction}");
        let Some(first_nonzero) = digits.bytes().position(|b| b != b'0') else {
            return Some((0, 0));
        };
        let mut adjusted = integer.len() as i64 - first_nonzero as i64 - 1;
        if !(-15..=15).contains(&adjusted) { return None; }
        let significant = &digits[first_nonzero..];
        let mut coefficient = 0u64;
        let first_sixteen = significant.bytes().take(16).collect::<Vec<_>>();
        for digit in &first_sixteen {
            coefficient = coefficient * 10 + (digit - b'0') as u64;
        }
        for _ in first_sixteen.len()..16 { coefficient *= 10; }
        let following = significant.as_bytes().get(16).copied().unwrap_or(b'0');
        let further_nonzero = significant.as_bytes().get(17..)
            .unwrap_or(&[]).iter().any(|b| *b != b'0');
        let round_up = following > b'5'
            || (following == b'5' && (further_nonzero || coefficient % 2 == 1));
        if round_up {
            coefficient += 1;
            if coefficient == 10_000_000_000_000_000 {
                coefficient = 1_000_000_000_000_000;
                adjusted += 1;
                if adjusted > 15 { return None; }
            }
        }
        Some((coefficient, (adjusted - 15) as i32))
    }

    fn term(&mut self) -> Result<Term, Diagnostic> {
        // P01 加減為相同優先序、左結合；括號在原子解析處理。
        let left = self.atom()?;
        crate::syntax_shared::fold_additive_spanned(
            left,
            || {
                let Some(op) = crate::syntax_shared::additive_operator(
                    self.tokens, self.cursor) else { return Ok(None) };
                self.cursor += 1;
                Ok(Some((op, self.atom()?)))
            },
            |term| term.node_span(),
            |left, operator, right, span| Term {
                start: span.start, end: span.end,
                kind: TermKind::BinaryInt(
                    crate::common_ir::ArithmeticNode::new(left, operator, right),
                ),
            },
        )
    }

    fn atom(&mut self) -> Result<Term, Diagnostic> {
        // 無函式／方法／索引後綴時由公私共同核心直接建構裸名
        // AST；不能讓公開入口藉此接受私有 .move／.borrow 語法。
        if let Some(name) = crate::syntax_shared::parse_bare_name(
            self.tokens, &mut self.cursor, TermKind::Name,
        ) {
            return Ok(name);
        }
        let start = self.tok().start;
        // 與公開 ExprParser 共用真正節點產生及游標提交核心。
        // 私有 i64 必須仍在解析期拒絕；保留此前使用的繁體中文語法診斷。
        let invalid_integer = self.syntax("整數字面值無效或超出有號 64 位元範圍");
        let (kind, mut end) = if let Some(literal) =
            crate::syntax_shared::parse_scalar_atom(
                self.tokens, &mut self.cursor, |atom| match atom {
                    crate::syntax_shared::ScalarLiteral::Integer(ref text)
                        if !Self::valid_integer(text) => Err(invalid_integer),
                    atom => Ok(TermKind::Scalar(atom)),
                },
            )? {
            (literal.kind, literal.end)
        } else {
        match self.tok().kind.clone() {
            TokenKind::Decimal(ref text) => {
                if Self::decimal64_checked_subset(text).is_none() {
                    return Err(self.syntax(
                        "P13 decimal64 常值超出已驗證的有限安全子集；下溢、超界與特殊值仍待正式規格"
                    ));
                }
                let end = self.tok().end;
                self.cursor += 1;
                (TermKind::Literal("float"), end)
            }
            TokenKind::StringLiteral(_) => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("str"), end)
            }
            TokenKind::Symbol('(') => {
                self.cursor += 1;
                return crate::syntax_shared::parse_group(self, start);
            }
            TokenKind::Name(_) => {
                let name = crate::syntax_shared::take_spanned_name(
                    self.tokens, &mut self.cursor)
                    .expect("已預檢具名節點");
                let s = name.kind;
                let end = name.end;
                if self.symbol('(') {
                    self.cursor += 1;
                    // 只有 name = value 才是結構具名欄位建立式；
                    // 位置引數須另建 AST，避免錯當欄位或略過所有權檢查。
                    let named = matches!(self.tok().kind, TokenKind::Name(_))
                        && self.tokens.get(self.cursor + 1)
                            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol('=')));
                    if named || self.symbol(')') {
                        let mut args = Vec::new();
                        let mut names = HashSet::new();
                        while !self.symbol(')') {
                            let field = self.name()?;
                            if !names.insert(field.clone()) {
                                return Err(Diagnostic {
                                    kind: DiagnosticKind::Static,
                                    message: "VB16：DuplicateField，建立結構時不得重複指定同一欄位".into(),
                                    span: Some(self.source.span(self.tok().start, self.tok().end)),
                                });
                            }
                            self.take('=', "具名欄位引數需要等號")?;
                            let value = self.term()?;
                            args.push((field, value));
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("具名欄位引數之間需要逗號"));
                            }
                        }
                        let end = self.tok().end; self.cursor += 1;
                        (TermKind::Construct(s, args), end)
                    } else {
                        let (args, end) = crate::syntax_shared::parse_call(self)?;
                        (TermKind::Call(s, args), end)
                    }
                } else if self.symbol('.') {
                    self.cursor += 1;
                    let field_or_move = self.name()?;
                    if field_or_move == "borrow" && self.symbol('(') {
                        self.cursor += 1;
                        self.take(')', "一般借用 .borrow() 不可帶引數")?;
                        (TermKind::Borrow(s), self.tokens[self.cursor - 1].end)
                    } else if field_or_move == "move" && self.symbol('(') {
                        self.cursor += 1;
                        self.take(')', "明確移動目前只接受零參數")?;
                        (TermKind::Move(s), self.tokens[self.cursor - 1].end)
                    } else if self.symbol('.') {
                        self.cursor += 1;
                        let action = self.name()?;
                        if action == "borrow" {
                            self.take('(', "欄位借用呼叫缺少左括號")?;
                            self.take(')', "欄位借用呼叫缺少右括號")?;
                            return Err(Diagnostic {
                                kind: DiagnosticKind::Static,
                                message: "P15.4：直接借用結構欄位尚無獨立來源身分與有效期證明，保守拒絕".into(),
                                span: Some(self.source.span(start, self.tokens[self.cursor - 1].end)),
                            });
                        }
                        if action != "move" {
                            return Err(self.syntax("此子集只接受明確整筆移動"));
                        }
                        self.take('(', "欄位移動呼叫缺少左括號")?;
                        self.take(')', "欄位移動呼叫缺少右括號")?;
                        (TermKind::FieldMove(s, field_or_move),
                            self.tokens[self.cursor - 1].end)
                    } else if self.symbol('(') {
                        self.cursor += 1;
                        let (args, end) = crate::syntax_shared::parse_call(
                            &mut MethodCallSyntax { parser: self },
                        )?;
                        (TermKind::MethodCall(s, field_or_move, args), end)
                    } else {
                        (TermKind::FieldRead(s, field_or_move),
                            self.tokens[self.cursor - 1].end)
                    }
                } else if self.symbol('[') {
                    // P15.4：仍無索引項目的來源身分／存活期證明，
                    // 不得把 str、list 或 dict 的元素當成可直接借用的持有者。
                    return Err(self.syntax(
                        "P15.4：索引元素的借用／別名有效期尚未證明，私有所有權前端保守拒絕"));
                } else {
                    (TermKind::Name(s), end)
                }
            }
            _ => return Err(self.syntax("此階段不接受的運算式或來源形式")),
        }
        };
        if self.symbol('.') || self.symbol('(') || self.symbol('[') {
            return Err(self.syntax("此階段尚不支援複合接收者或額外後綴操作"));
        }
        if end < start { end = start; }
        Ok(Term { kind, start, end })
    }
    /// 讓已核准的 else if 與獨立 if 共用同一個可達控制流程節點；
    /// 不將 else if 視為不同所有權語法，三條分支均由後端合流追蹤。
    fn if_statement_kind(&mut self) -> Result<StmtKind, Diagnostic> {
        let parsed = crate::syntax_shared::parse_if_chain(self)?;
        let mut arms = parsed.arms.into_iter();
        let first = arms.next().expect("if 文法至少產生一條分支");
        let mut no = parsed.otherwise;
        // 公開平面鏈在私有既有 AST 中仍需折成原來的 else-if 巢狀鏈，
        // 合流規則與原始半開位置不變，不能憑此開放新語言能力。
        for arm in arms.collect::<Vec<_>>().into_iter().rev() {
            no = Some(vec![Stmt {
                kind: StmtKind::If { branch: arm.branch, no },
                start: arm.start,
                end: parsed.end,
            }]);
        }
        Ok(StmtKind::If { branch: first.branch, no })
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.tok().start;
        let head = crate::syntax_shared::statement_head(
            self.tokens, self.cursor, crate::common_ir::FrontendMode::PrivateVerified,
        );
        let kind = if head == Some(crate::syntax_shared::StatementHead::Declaration) {
            let parsed = crate::syntax_shared::parse_declaration(self)?;
            StmtKind::Declare(parsed)
        } else if head == Some(crate::syntax_shared::StatementHead::Conditional) {
            self.if_statement_kind()?
        } else if self.keyword("for") {
            self.cursor += 1;
            let iterator_start = self.tok().start;
            let iterator = self.name()?;
            if !matches!(&self.tok().kind, TokenKind::Name(word) if word == "in") {
                return Err(self.syntax("for 區間迭代變數後需要 in"));
            }
            self.cursor += 1;
            let lower = match self.tok().kind.clone() {
                TokenKind::Integer(number) if Self::valid_integer(&number) => number,
                _ => return Err(self.syntax("此子集 for 左端點必須是合法整數常值")),
            };
            self.cursor += 1;
            if !matches!(self.tok().kind, TokenKind::Range) {
                return Err(self.syntax("for 半開區間缺少 .. 範圍記號"));
            }
            self.cursor += 1;
            let upper = match self.tok().kind.clone() {
                TokenKind::Integer(number) if Self::valid_integer(&number) => number,
                _ => return Err(self.syntax("此子集 for 右端點必須是合法整數常值")),
            };
            self.cursor += 1;
            let body = self.body()?;
            StmtKind::ForRange { iterator, iterator_start, lower, upper, body }
        } else if self.keyword("while") {
            self.cursor += 1;
            self.take('(', "while 條件需要左括號")?;
            let condition = self.term()?;
            self.take(')', "while 條件需要右括號")?;
            let body = self.body()?;
            StmtKind::While { condition, body }
        } else if head == Some(crate::syntax_shared::StatementHead::Return) {
            StmtKind::Return(crate::syntax_shared::parse_return(self)?)
        } else if self.keyword("break") {
            self.cursor += 1;
            StmtKind::Break
        } else if self.keyword("continue") {
            self.cursor += 1;
            StmtKind::Continue
        } else if matches!(
            self.tokens.get(self.cursor + 1).map(|token| &token.kind),
            Some(TokenKind::Symbol('.'))
        ) {
            let name = self.name()?;
            self.cursor += 1;
            let field = self.name()?;
            self.take('=', "欄位賦值需要等號")?;
            let value = self.term()?;
            StmtKind::FieldAssign { receiver: name, field, value }
        } else {
            let parsed = crate::syntax_shared::parse_named_assignment(self)?;
            StmtKind::Assign(parsed)
        };
        let end = self.tokens[self.cursor - 1].end;
        self.end_statement()?;
        Ok(Stmt { kind, start, end })
    }
    fn body(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        Ok(crate::syntax_shared::parse_block(self)?.statements)
    }
    fn function(&mut self) -> Result<Function, Diagnostic> {
        crate::syntax_shared::parse_function_decl(self)
    }
    fn strukt(&mut self) -> Result<(String, StructDecl), Diagnostic> {
        self.cursor += 1; // struct
        let name = self.name()?;
        self.take('{', "結構名稱後需要左大括號；泛型尚未開放")?;
        self.lines();
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut names = HashSet::new();
        while !self.symbol('}') {
            if self.eof() { return Err(self.syntax("結構缺少右大括號")); }
            if self.keyword("struct") || self.keyword("share") {
                return Err(self.syntax("結構內不可宣告巢狀 struct 或 share 欄位"));
            }
            if self.keyword("fn") {
                let fun = self.function()?;
                if !names.insert(fun.name.clone()) {
                    return Err(self.syntax("欄位與方法不得共用名稱"));
                }
                methods.push(fun);
            } else {
                if !self.keyword("var") && !self.keyword("const") {
                    return Err(self.syntax("結構僅接受 var、const 欄位及 fn 方法"));
                }
                let parsed = crate::syntax_shared::parse_declaration(
                    &mut StructFieldDeclaration {
                        parser: self, declared_names: &mut names,
                    },
                )?;
                fields.push(Field {
                    name: parsed.name, ty: parsed.ty,
                    immutable: !parsed.mutable, default: parsed.initializer,
                });
            }
            self.end_statement()?;
            self.lines();
        }
        // K06-B：真正 struct 至少有一個欄位或方法；
        // 不把空大括號或只有註解、換行的結構當成已完成宣告。
        if fields.is_empty() && methods.is_empty() {
            return Err(self.syntax("K06：struct 至少需要一個欄位或方法"));
        }
        self.cursor += 1;
        Ok((name, StructDecl::new(fields, methods)))
    }
    fn file(mut self) -> Result<(HashMap<String, StructDecl>, Vec<Function>), Diagnostic> {
        let mut structs = HashMap::new();
        let mut funcs = Vec::new();
        let mut function_names = HashSet::new();
        self.lines();
        while !self.eof() {
            if self.keyword("struct") {
                let (name, def) = self.strukt()?;
                if structs.insert(name, def).is_some() {
                    return Err(self.syntax("不得重複宣告相同結構型別"));
                }
            } else if self.keyword("fn") {
                let fun = self.function()?;
                if !function_names.insert(fun.name.clone()) {
                    return Err(self.syntax("不得重複宣告函式名稱"));
                }
                funcs.push(fun);
            } else {
                return Err(self.syntax("受限 P13 來源僅接受 struct 與 fn 頂層宣告"));
            }
            if !self.eof() && !matches!(self.tok().kind, TokenKind::Newline) {
                return Err(self.syntax("頂層宣告之間必須換行"));
            }
            self.lines();
        }
        Ok((structs, funcs))
    }
}


/// 結構欄位也使用公私 var／const 正式共同宣告解析核心。
/// 欄位缺預設值、重複名稱及型別標註有獨立歷史診斷；
/// 使用輕量 adapter 避免將語句宣告的診斷／安全權限錯套至欄位。
struct StructFieldDeclaration<'p, 'src> {
    parser: &'p mut Parser<'src>,
    declared_names: &'p mut HashSet<String>,
}
impl crate::syntax_shared::DeclarationSyntax for StructFieldDeclaration<'_, '_> {
    type Type = String;
    type Expression = Term;

    fn declaration_start(&self) -> usize { self.parser.tok().start }
    fn begin_declaration(&mut self) -> Result<bool, Diagnostic> {
        let mutable = self.parser.keyword("var");
        // 入口已確認 var／const；不放寬 share 或未核准欄位形式。
        if !mutable && !self.parser.keyword("const") {
            return Err(self.parser.syntax("結構僅接受 var、const 欄位及 fn 方法"));
        }
        self.parser.cursor += 1;
        Ok(mutable)
    }
    fn declaration_name(&mut self) -> Result<String, Diagnostic> {
        let name = self.parser.name()?;
        // 保留原行為：名稱消耗後、型別及初始化解析之前就檢查重複。
        if !self.declared_names.insert(name.clone()) {
            return Err(self.parser.syntax("同一結構不可重複欄位名稱"));
        }
        Ok(name)
    }
    fn declaration_colon(&mut self) -> Result<(), Diagnostic> {
        self.parser.take(':', "結構欄位必須標註型別")
    }
    fn declaration_type(&mut self) -> Result<String, Diagnostic> { self.parser.name() }
    fn declaration_has_initializer(&self) -> bool { self.parser.symbol('=') }
    fn declaration_equals(&mut self) { self.parser.cursor += 1; }
    fn declaration_expression(&mut self) -> Result<Term, Diagnostic> { self.parser.term() }
    fn validate_declaration_initializer(
        &self, mutable: bool, initializer: &Option<Term>,
    ) -> Result<(), Diagnostic> {
        if !mutable && initializer.is_none() {
            Err(self.parser.syntax("const 結構欄位必須具有宣告時預設值"))
        } else { Ok(()) }
    }
    fn declaration_end(&self, _: &Option<Term>) -> usize {
        self.parser.tokens[self.parser.cursor - 1].end
    }
}

/// 私有安全子集使用共同區塊走訪核心，語句分隔由原 statement 消耗。
impl crate::syntax_shared::BlockSyntax for Parser<'_> {
    type Statement = Stmt;
    fn begin_block(&mut self) -> Result<usize, Diagnostic> {
        let start = self.tok().start;
        self.take('{', "函式需要左大括號")?;
        self.lines();
        Ok(start)
    }
    fn block_at_end(&self) -> bool { self.eof() }
    fn block_at_close(&self) -> bool { self.symbol('}') }
    fn block_missing_close(&self) -> Diagnostic { self.syntax("函式缺少右大括號") }
    fn parse_block_statement(&mut self) -> Result<Stmt, Diagnostic> { self.statement() }
    fn finish_block_statement(&mut self) -> Result<(), Diagnostic> { Ok(()) }
    fn finish_block(&mut self, count: usize) -> Result<usize, Diagnostic> {
        self.cursor += 1;
        if count == 0 {
            return Err(self.syntax("此階段函式必須具有明確 return"));
        }
        Ok(self.tokens[self.cursor - 1].end)
    }
}

/// 已核准的私有括號運算式沿用原節點，並由共同核心擴展來源跨度。
/// 公私函式呼叫與私有方法呼叫消費相同的 CallSyntax 序列，
/// 但方法尾段歷史診斷與一般函式不同，必須留在專用 adapter。
struct MethodCallSyntax<'p, 'src> { parser: &'p mut Parser<'src> }
impl crate::syntax_shared::CallSyntax for MethodCallSyntax<'_, '_> {
    type Arguments = Vec<Term>;
    fn call_arguments(&mut self) -> Result<Vec<Term>, Diagnostic> {
        let mut args = Vec::new();
        while !self.parser.symbol(')') {
            args.push(self.parser.term()?);
            if self.parser.symbol(',') { self.parser.cursor += 1; }
            else if !self.parser.symbol(')') {
                return Err(self.parser.syntax("方法位置引數之間需要逗號"));
            }
        }
        Ok(args)
    }
    fn call_right_paren(&mut self) -> Result<usize, Diagnostic> {
        self.parser.take(')', "方法呼叫缺少右括號")?;
        Ok(self.parser.tokens[self.parser.cursor - 1].end)
    }
}

impl crate::syntax_shared::CallSyntax for Parser<'_> {
    type Arguments = Vec<Term>;
    fn call_arguments(&mut self) -> Result<Vec<Term>, Diagnostic> {
        let mut args = Vec::new();
        while !self.symbol(')') {
            args.push(self.term()?);
            if self.symbol(',') { self.cursor += 1; }
            else if !self.symbol(')') {
                return Err(self.syntax("函式位置引數之間需要逗號"));
            }
        }
        Ok(args)
    }
    fn call_right_paren(&mut self) -> Result<usize, Diagnostic> {
        let end = self.tok().end;
        self.cursor += 1;
        Ok(end)
    }
}

impl crate::syntax_shared::GroupSyntax for Parser<'_> {
    type Expression = Term;
    fn group_expression(&mut self) -> Result<Term, Diagnostic> { self.term() }
    fn group_right_paren(&mut self) -> Result<usize, Diagnostic> {
        self.take(')', "括號運算式缺少右括號")?;
        Ok(self.tokens[self.cursor - 1].end)
    }
    fn group_result(&mut self, mut expression: Term, start: usize, end: usize) -> Term {
        expression.start = start;
        expression.end = end;
        expression
    }
}

/// 正式私有受限 Parser 與公開 Parser 實作同一條 if/else-if 核心文法。
/// 條件、區塊、診斷與允許的所有權行為仍完全由本入口負責。
impl crate::syntax_shared::DeclarationSyntax for Parser<'_> {
    type Type = String;
    type Expression = Term;
    fn declaration_start(&self) -> usize { self.tok().start }
    fn begin_declaration(&mut self) -> Result<bool, Diagnostic> {
        let mutable = self.keyword("var");
        if !mutable && !self.keyword("const") {
            return Err(self.syntax("此階段不接受的宣告"));
        }
        self.cursor += 1;
        Ok(mutable)
    }
    fn declaration_name(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn declaration_colon(&mut self) -> Result<(), Diagnostic> {
        self.take(':', "宣告必須有型別註記")
    }
    fn declaration_type(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn declaration_has_initializer(&self) -> bool { self.symbol('=') }
    fn declaration_equals(&mut self) { self.cursor += 1; }
    fn declaration_expression(&mut self) -> Result<Term, Diagnostic> { self.term() }
    fn validate_declaration_initializer(
        &self, mutable: bool, initializer: &Option<Term>,
    ) -> Result<(), Diagnostic> {
        if !mutable && initializer.is_none() {
            Err(self.syntax("const 宣告必須立即初始化"))
        } else { Ok(()) }
    }
    fn declaration_end(&self, _initializer: &Option<Term>) -> usize {
        self.tokens[self.cursor - 1].end
    }
}

impl crate::syntax_shared::NamedAssignmentSyntax for Parser<'_> {
    fn named_assignment_name(&mut self) -> Result<String, Diagnostic> { self.name() }
}

impl crate::syntax_shared::AssignmentSyntax for Parser<'_> {
    type Expression = Term;
    fn assignment_equals(&mut self) -> Result<(), Diagnostic> {
        self.take('=', "此階段具名語句必須為賦值")
    }
    fn assignment_expression(&mut self) -> Result<Term, Diagnostic> { self.term() }
    fn validate_assignment(&self, _expression: &Term) -> Result<(), Diagnostic> { Ok(()) }
}

impl crate::syntax_shared::ReturnSyntax for Parser<'_> {
    type Expression = Term;
    fn begin_return(&mut self) -> Result<(), Diagnostic> {
        if !self.keyword("return") { return Err(self.syntax("缺少 return")); }
        self.cursor += 1;
        Ok(())
    }
    fn return_expression(&mut self) -> Result<Term, Diagnostic> { self.term() }
    fn validate_return(&self, _expression: &Term) -> Result<(), Diagnostic> { Ok(()) }
}

impl crate::syntax_shared::IfSyntax for Parser<'_> {
    type Condition = Term;
    type Body = Vec<Stmt>;

    fn current_if_start(&self) -> usize { self.tok().start }
    fn begin_if(&mut self) -> Result<(), Diagnostic> {
        if !self.keyword("if") {
            return Err(self.syntax("此階段不接受的運算式或來源形式"));
        }
        self.cursor += 1;
        Ok(())
    }
    fn if_condition(&mut self) -> Result<Term, Diagnostic> {
        self.take('(', "if 條件需要左括號")?;
        let condition = self.term()?;
        self.take(')', "if 條件需要右括號")?;
        Ok(condition)
    }
    fn if_body(&mut self) -> Result<Vec<Stmt>, Diagnostic> { self.body() }
    fn maybe_else(&mut self) -> bool {
        let saved = self.cursor;
        self.lines();
        if self.keyword("else") {
            self.cursor += 1;
            true
        } else {
            self.cursor = saved;
            false
        }
    }
    fn next_is_if(&self) -> bool { self.keyword("if") }
    fn last_end(&self) -> usize { self.tokens[self.cursor - 1].end }
}

/// 私有已核准參數與型別仍由本解析器獨立驗證；只有語法組構順序共用。
impl crate::syntax_shared::FunctionSyntax for Parser<'_> {
    type Params = Vec<Parameter>;
    type ResultType = String;
    type Body = Vec<Stmt>;

    fn begin_function(&mut self) -> Result<usize, Diagnostic> {
        crate::syntax_shared::take_keyword(self.tokens, &mut self.cursor, "fn")
            .ok_or_else(|| self.syntax("函式必須以 fn 宣告"))
    }
    fn function_name(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn function_left_paren(&mut self) -> Result<(), Diagnostic> {
        self.take('(', "函式名稱後需左括號")
    }
    fn function_params(&mut self) -> Result<Vec<Parameter>, Diagnostic> {
        let mut params = Vec::new();
        let mut seen = HashSet::new();
        while !self.symbol(')') {
            let declaration_start = self.tok().start;
            let parameter = self.name()?;
            if !seen.insert(parameter.clone()) {
                return Err(self.syntax("函式參數名稱不得重複"));
            }
            self.take(':', "函式參數需要明確型別")?;
            let ty = self.name()?;
            params.push(Parameter { name: parameter, ty, declaration_start });
            if self.symbol(',') { self.cursor += 1; }
            else if !self.symbol(')') {
                return Err(self.syntax("函式參數之間需要逗號"));
            }
        }
        Ok(params)
    }
    fn function_right_paren(&mut self) -> Result<(), Diagnostic> {
        self.take(')', "函式參數缺少右括號")
    }
    fn function_return_arrow(&mut self) -> Result<(), Diagnostic> {
        crate::syntax_shared::take_return_arrow(self.tokens, &mut self.cursor)
            .map(|_| ()).ok_or_else(|| self.syntax("函式需要回傳型別標註"))
    }
    fn function_result_type(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn function_body(&mut self) -> Result<Vec<Stmt>, Diagnostic> { self.body() }
    fn function_last_end(&self) -> usize { self.tokens[self.cursor - 1].end }
}
