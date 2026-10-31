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
        if self.symbol('}') { return Ok(()); }
        if !matches!(self.tok().kind, TokenKind::Newline) {
            return Err(self.syntax("語句之間必須換行"));
        }
        self.lines();
        Ok(())
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
        crate::syntax_shared::fold_additive(
            left,
            || {
                let Some(op) = crate::syntax_shared::additive_operator(
                    self.tokens, self.cursor) else { return Ok(None) };
                self.cursor += 1;
                Ok(Some((op, self.atom()?)))
            },
            |left, operator, right| Term {
                start: left.start, end: right.end,
                kind: TermKind::BinaryInt {
                    left: Box::new(left), operator, right: Box::new(right),
                },
            },
        )
    }

    fn atom(&mut self) -> Result<Term, Diagnostic> {
        let start = self.tok().start;
        let (kind, mut end) = match self.tok().kind.clone() {
            TokenKind::Integer(text) => {
                if !Self::valid_integer(&text) {
                    return Err(self.syntax("整數字面值無效或超出有號 64 位元範圍"));
                }
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("int"), end)
            }
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
            TokenKind::Keyword(s) if s == "true" || s == "false" => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("bool"), end)
            }
            TokenKind::Symbol('(') => {
                self.cursor += 1;
                let mut expression = self.term()?;
                self.take(')', "括號運算式缺少右括號")?;
                expression.start = start;
                expression.end = self.tokens[self.cursor - 1].end;
                return Ok(expression);
            }
            TokenKind::Name(s) => {
                let end = self.tok().end; self.cursor += 1;
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
                        let mut args = Vec::new();
                        while !self.symbol(')') {
                            args.push(self.term()?);
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("函式位置引數之間需要逗號"));
                            }
                        }
                        let end = self.tok().end; self.cursor += 1;
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
                        let mut args = Vec::new();
                        while !self.symbol(')') {
                            args.push(self.term()?);
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("方法位置引數之間需要逗號"));
                            }
                        }
                        self.take(')', "方法呼叫缺少右括號")?;
                        (TermKind::MethodCall(s, field_or_move, args),
                            self.tokens[self.cursor - 1].end)
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
        self.cursor += 1; // if
        self.take('(', "if 條件需要左括號")?;
        let condition = self.term()?;
        self.take(')', "if 條件需要右括號")?;
        let yes = self.body()?;
        let saved = self.cursor;
        self.lines();
        let no = if self.keyword("else") {
            self.cursor += 1;
            if self.keyword("if") {
                let start = self.tok().start;
                let kind = self.if_statement_kind()?;
                let end = self.tokens[self.cursor - 1].end;
                Some(vec![Stmt { kind, start, end }])
            } else {
                Some(self.body()?)
            }
        } else {
            self.cursor = saved;
            None
        };
        Ok(StmtKind::If { condition, yes, no })
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.tok().start;
        let kind = if self.keyword("var") || self.keyword("const") {
            let mutable = self.keyword("var");
            self.cursor += 1;
            let name = self.name()?;
            self.take(':', "宣告必須有型別註記")?;
            let ty = self.name()?;
            let init = if self.symbol('=') {
                self.cursor += 1;
                Some(self.term()?)
            } else { None };
            if !mutable && init.is_none() {
                return Err(self.syntax("const 宣告必須立即初始化"));
            }
            StmtKind::Declare { name, ty, mutable, init }
        } else if self.keyword("if") {
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
        } else if self.keyword("return") {
            self.cursor += 1;
            StmtKind::Return(self.term()?)
        } else if self.keyword("break") {
            self.cursor += 1;
            StmtKind::Break
        } else if self.keyword("continue") {
            self.cursor += 1;
            StmtKind::Continue
        } else {
            let name = self.name()?;
            if self.symbol('.') {
                self.cursor += 1;
                let field = self.name()?;
                self.take('=', "欄位賦值需要等號")?;
                let value = self.term()?;
                StmtKind::FieldAssign { receiver: name, field, value }
            } else {
                self.take('=', "此階段具名語句必須為賦值")?;
                let value = self.term()?;
                StmtKind::Assign { name, value }
            }
        };
        let end = self.tokens[self.cursor - 1].end;
        self.end_statement()?;
        Ok(Stmt { kind, start, end })
    }
    fn body(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.take('{', "函式需要左大括號")?;
        self.lines();
        let mut body = Vec::new();
        while !self.symbol('}') {
            if self.eof() { return Err(self.syntax("函式缺少右大括號")); }
            body.push(self.statement()?);
        }
        self.cursor += 1;
        if body.is_empty() {
            return Err(self.syntax("此階段函式必須具有明確 return"));
        }
        Ok(body)
    }
    fn function(&mut self) -> Result<Function, Diagnostic> {
        if !self.keyword("fn") { return Err(self.syntax("函式必須以 fn 宣告")); }
        self.cursor += 1;
        let name = self.name()?;
        self.take('(', "函式名稱後需左括號")?;
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
            else if !self.symbol(')') { return Err(self.syntax("函式參數之間需要逗號")); }
        }
        self.take(')', "函式參數缺少右括號")?;
        if !matches!(self.tok().kind, TokenKind::Arrow) {
            return Err(self.syntax("函式需要回傳型別標註"));
        }
        self.cursor += 1;
        let result_ty = self.name()?;
        let body = self.body()?;
        Ok(Function { name, params, result_ty, body })
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
                let immutable = self.keyword("const");
                self.cursor += 1;
                let field = self.name()?;
                if !names.insert(field.clone()) {
                    return Err(self.syntax("同一結構不可重複欄位名稱"));
                }
                self.take(':', "結構欄位必須標註型別")?;
                let ty = self.name()?;
                let default = if self.symbol('=') {
                    self.cursor += 1;
                    Some(self.term()?)
                } else { None };
                if immutable && default.is_none() {
                    return Err(self.syntax("const 結構欄位必須具有宣告時預設值"));
                }
                fields.push(Field { name: field, ty, immutable, default });
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

