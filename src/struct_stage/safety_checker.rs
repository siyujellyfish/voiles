// 對應 P16.6-R01 無語意變更之實作拆檔。
impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn find_function(&self, name: &str) -> Option<&Function> {
        self.function_index.get(name).and_then(|index| self.functions.get(*index))
    }
    fn error(&self, text: &str, term: &Term) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: text.into(),
            span: Some(self.source.span(term.start, term.end)),
        }
    }
    fn copy_permission(&self, ty: &str, active: &mut HashSet<String>) -> CopyPermission {
        if let Some(def) = self.structs.get(ty) {
            if !active.insert(ty.to_string()) { return CopyPermission::Undecided; }
            let fields: Vec<_> = def.fields.iter()
                .map(|field| self.copy_permission(&field.ty, active)).collect();
            active.remove(ty);
            match struct_ownership::whole_struct_copy(&fields) {
                Ok(()) => CopyPermission::Allowed,
                Err(crate::struct_ownership::StructOwnershipError::FieldCopyForbidden) =>
                    CopyPermission::Forbidden,
                _ => CopyPermission::Undecided,
            }
        } else {
            (self.ability)(ty)
        }
    }
    fn permission(&self, ty: &str) -> CopyPermission {
        self.copy_permission(ty, &mut HashSet::new())
    }

    /// 已明確禁止複製與仍待規格核准，必須使用不同診斷，不可混為一談。
    fn reject_noncopyable(&self, permission: CopyPermission,
        context: &str, term: &Term) -> Diagnostic {
        let reason = match permission {
            CopyPermission::Forbidden => "Copyable 已明確禁止",
            CopyPermission::Undecided => "Copyable 資格未決",
            CopyPermission::Allowed => "Copyable 檢查出現不一致",
        };
        self.error(&format!("{context}：{reason}，不能隱含複製"), term)
    }
    fn lookup<'b>(vars: &'b Scopes, name: &str) -> Option<&'b Binding> {
        vars.iter().rev().find_map(|scope| scope.get(name))
    }
    fn lookup_mut<'b>(vars: &'b mut Scopes, name: &str) -> Option<&'b mut Binding> {
        for scope in vars.iter_mut().rev() {
            if scope.contains_key(name) {
                return Rc::make_mut(scope).get_mut(name);
            }
        }
        None
    }

    /// 在受限來源中，只有真正包含具名整筆 .move() 的形狀
    /// 才能消耗參數；否則非 Copyable 參數不可被虛構成獨立所有者。
    /// 此檢查不推定一般間接副作用、別名再借用或跨例外逃逸。
    fn term_moves_name(term: &Term, name: &str) -> bool {
        match &term.kind {
            TermKind::Move(source) => source == name,
            TermKind::Construct(_, args) =>
                args.iter().any(|(_, t)| Self::term_moves_name(t, name)),
            TermKind::Call(_, args) | TermKind::MethodCall(_, _, args) =>
                args.iter().any(|t| Self::term_moves_name(t, name)),
            TermKind::BinaryInt {left, right, ..} =>
                Self::term_moves_name(left, name) || Self::term_moves_name(right, name),
            _ => false,
        }
    }

    fn statements_move_name(body: &[Stmt], name: &str) -> bool {
        body.iter().any(|stmt| match &stmt.kind {
            StmtKind::Declare { init, .. } => init.as_ref()
                .is_some_and(|t| Self::term_moves_name(t, name)),
            StmtKind::Assign { value, .. } | StmtKind::FieldAssign { value, .. }
                | StmtKind::Return(value) => Self::term_moves_name(value, name),
            StmtKind::If {condition, yes, no} =>
                Self::term_moves_name(condition, name)
                    || Self::statements_move_name(yes, name)
                    || no.as_ref().is_some_and(|b| Self::statements_move_name(b, name)),
            StmtKind::While {condition, body} =>
                Self::term_moves_name(condition, name)
                    || Self::statements_move_name(body, name),
            StmtKind::ForRange {body, ..} => Self::statements_move_name(body, name),
            StmtKind::Break | StmtKind::Continue => false,
        })
    }

    fn parameter_is_consumed(function: &Function, name: &str) -> bool {
        Self::statements_move_name(&function.body, name)
    }

    /// HC14-A：以實際宣告來源身分查找全部仍有效的非擁有借用。
    /// 不能只以目前作用域或表面名稱判斷，否則遮蔽可繞過來源封鎖。
    fn active_loans(vars: &Scopes, source: usize) -> Vec<BorrowKind> {
        vars.iter().flat_map(|scope| scope.values()).filter_map(|binding| {
            binding.loan.as_ref().and_then(|loan| {
                (loan.active && loan.source == source).then_some(loan.kind)
            })
        }).collect()
    }

    fn check_direct(&self, vars: &Scopes, name: &str, term: &Term)
        -> Result<(), Diagnostic> {
        let owner = Self::lookup(vars, name)
            .ok_or_else(|| self.error("找不到原始來源宣告", term))?;
        if owner.loan.is_none() && !Self::active_loans(vars, owner.declaration_start).is_empty() {
            return Err(self.error("HC14：來源借用仍有效，禁止直接讀寫、複製或明確移動", term));
        }
        Ok(())
    }

    fn authorize_borrow(&self, vars: &Scopes, source: &str, mutable: bool, term: &Term)
        -> Result<(String, LoanBinding), Diagnostic> {
        let owner = Self::lookup(vars, source)
            .ok_or_else(|| self.error("P14 找不到可借用來源", term))?;
        if owner.loan.is_some() {
            return Err(self.error("P14：由借用再次借用尚未有完整別名證明", term));
        }
        owner.value.read().map_err(|e| self.model_error(e, term))?;
        if owner.mutable != mutable {
            return Err(self.error("VA14-01：借用變數的 var／const 權限必須跟隨來源", term));
        }
        let kind = if mutable { BorrowKind::ExclusiveWrite } else { BorrowKind::SharedRead };
        let conflicts = Self::active_loans(vars, owner.declaration_start);
        BorrowSafety::validate_new_borrow(&conflicts, kind)
            .map_err(|_| self.error("HC14：共享／獨占借用與現有別名衝突", term))?;
        Ok((owner.ty.clone(), LoanBinding {
            source: owner.declaration_start, kind, active: true,
        }))
    }

    /// 本批只對真正已解析的語句追蹤別名是否未來仍被使用。
    /// 此處寧可把有遮蔽的同名字串當成仍存活（假陽性），也不能
    /// 漏掉未來引用造成提前解鎖；完整詞彙來源精細度留待 P15 驗收。
    fn term_mentions(term: &Term, name: &str) -> bool {
        match &term.kind {
            TermKind::Name(id) | TermKind::Move(id) | TermKind::Borrow(id)
                => id == name,
            TermKind::FieldRead(id, _) | TermKind::FieldMove(id, _) => id == name,
            TermKind::MethodCall(id, _, args) => id == name
                || args.iter().any(|arg| Self::term_mentions(arg, name)),
            TermKind::Construct(_, arguments) =>
                arguments.iter().any(|(_, value)| Self::term_mentions(value, name)),
            TermKind::Call(_, arguments) =>
                arguments.iter().any(|value| Self::term_mentions(value, name)),
            TermKind::BinaryInt { left, right, .. } =>
                Self::term_mentions(left, name) || Self::term_mentions(right, name),
            TermKind::Literal(_) => false,
        }
    }

    fn statement_mentions(statement: &Stmt, name: &str) -> bool {
        match &statement.kind {
            StmtKind::Declare { init, .. } => init.as_ref()
                .is_some_and(|expr| Self::term_mentions(expr, name)),
            StmtKind::Assign { name: target, value } =>
                target == name || Self::term_mentions(value, name),
            StmtKind::FieldAssign { receiver, value, .. } =>
                receiver == name || Self::term_mentions(value, name),
            StmtKind::If { condition, yes, no } =>
                Self::term_mentions(condition, name)
                || yes.iter().any(|stmt| Self::statement_mentions(stmt, name))
                || no.as_ref().is_some_and(|body|
                    body.iter().any(|stmt| Self::statement_mentions(stmt, name))),
            StmtKind::While { condition, body } =>
                Self::term_mentions(condition, name)
                || body.iter().any(|stmt| Self::statement_mentions(stmt, name)),
            StmtKind::ForRange { body, .. } =>
                body.iter().any(|stmt| Self::statement_mentions(stmt, name)),
            StmtKind::Return(term) => Self::term_mentions(term, name),
            StmtKind::Break | StmtKind::Continue => false,
        }
    }

    /// 依語句來源順序分辨「同名新宣告」與「原借用別名」：
    /// 新宣告的初始化式先在外層環境求值，宣告完成後才遮蔽。
    /// if 分支各有新作用域，不得把分支內遮蔽誤帶到另一分支。
    fn scope_mentions_binding(body: &[Stmt], name: &str, initially_hidden: bool) -> bool {
        let mut hidden = initially_hidden;
        for statement in body {
            match &statement.kind {
                StmtKind::Declare { name: declared, init, .. } => {
                    if !hidden && init.as_ref().is_some_and(|value|
                        Self::term_mentions(value, name)) {
                        return true;
                    }
                    if declared == name { hidden = true; }
                }
                StmtKind::If { condition, yes, no } => {
                    if !hidden && (Self::term_mentions(condition, name)
                        || Self::scope_mentions_binding(yes, name, false)
                        || no.as_ref().is_some_and(|other|
                            Self::scope_mentions_binding(other, name, false))) {
                        return true;
                    }
                }
                StmtKind::While { condition, body } => {
                    if !hidden && (Self::term_mentions(condition, name)
                        || Self::scope_mentions_binding(body, name, false)) {
                        return true;
                    }
                }
                StmtKind::ForRange { iterator, body, .. } => {
                    if !hidden && Self::scope_mentions_binding(
                        body, name, iterator == name) {
                        return true;
                    }
                }
                _ => {
                    if !hidden && Self::statement_mentions(statement, name) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// VA15-01-A：在已知的當前作用域中，以來源順序及宣告遮蔽
    /// 計算最後合法使用點。外層 continuation 尚未有完整的
    /// 跨作用域已解析身分表，因此仍採名稱保守掃描，不能提早歸還。
    fn release_last_uses(vars: &mut Scopes, rest: &[Stmt],
        continuation: &[(usize, Vec<Stmt>)]) {
        let scope_names: Vec<HashSet<String>> = vars.iter()
            .map(|scope| scope.keys().cloned().collect()).collect();
        let mut younger_names = HashSet::new();
        for (depth, scope) in vars.iter_mut().enumerate().rev() {
            for (name, binding) in Rc::make_mut(scope).iter_mut() {
                if let Some(loan) = binding.loan.as_mut() {
                    let needed_here = Self::scope_mentions_binding(
                        rest, name, younger_names.contains(name));
                    // 各後繼分開保留真正的詞彙深度：內層局部遮蔽
                    // 不會延長外層別名；退出內層後重新可見的外層
                    // 借用，仍須保留至外層自己的最後使用點。
                    let needed_later = continuation.iter().any(|(at_depth, following)| {
                        *at_depth >= depth
                            && (*at_depth == depth
                                || !scope_names[depth + 1..=*at_depth].iter()
                                    .any(|known| known.contains(name)))
                            && Self::scope_mentions_binding(following, name, false)
                    });
                    // 函式預設借用參數以其本身宣告身分保存非擁有
                    // 借用票據代理，直到函式作用域真正退出才歸還。
                    // 一般 owner.borrow() 來源不同，仍以最後使用點歸還。
                    let parameter_scope_loan = loan.source == binding.declaration_start;
                    if loan.active && !parameter_scope_loan
                        && !needed_here && !needed_later {
                        loan.active = false;
                    }
                }
            }
            younger_names.extend(scope.keys().cloned());
        }
    }

    /// 這裡僅核准完全唯讀且可靜態證明安全的 self 方法子集。
    /// 根據方法實際操作自動推導共享或獨占借用。
    /// 在此明確受限子集，方法只能修改自身 var 欄位、讀取已核准 Copyable
    /// 的 self 欄位，並於最後回傳可靜態確定型別的值；其他行為保守拒絕。
    /// 只接受無副作用且可複製的布林條件。沒有取得獨立引用值。
    /// 方法中的純值來源共用一套型別／Copyable 檢查，避免合法的
    /// 已核准加減運算式僅因出現在方法內就被錯誤拒絕。
    /// 不處理可移動的擁有者、借用建立、巢狀呼叫或副作用。
    fn method_pure_value_type(&self, owner: &str, method: &Function, term: &Term)
        -> Result<String, Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok((*ty).to_string()),
            TermKind::Name(name) if name == "self" =>
                Err(self.error("S15：方法不可回傳整個 self，涉及其他借用與消耗", term)),
            TermKind::Name(name) => {
                let param = method.params.iter().find(|param| param.name == *name)
                    .ok_or_else(|| self.error("方法值來源參數不存在", term))?;
                let permission = self.permission(&param.ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission, "方法參數值", term));
                }
                Ok(param.ty.clone())
            }
            TermKind::FieldRead(receiver, name) if receiver == "self" => {
                let field = self.structs.get(owner)
                    .and_then(|def| def.field(name))
                    .ok_or_else(|| self.error("方法 self 欄位不存在", term))?;
                let permission = self.permission(&field.ty);
                if permission != CopyPermission::Allowed {
                    let mut error = self.reject_noncopyable(permission, "P13 self 欄位", term);
                    error.message = format!(
                        "{}；不可借用回傳，留待 P15 有效期與逃逸驗證",
                        error.message).into_boxed_str();
                    return Err(error);
                }
                Ok(field.ty.clone())
            }
            TermKind::Call(name, args) => {
                // 一般函式可操作自己的局部來源，但方法目前僅准把
                // Copyable 的獨立引數交給它，且回傳也須已核准 Copyable。
                let function = self.find_function(name)
                    .ok_or_else(|| self.error("方法呼叫不存在的一般函式", term))?;
                if function.params.len() != args.len() {
                    return Err(self.error("方法內函式引數數量不符", term));
                }
                if self.permission(&function.result_ty) != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(
                        self.permission(&function.result_ty), "方法內函式回傳值", term));
                }
                for (value, parameter) in args.iter().zip(&function.params) {
                    if self.permission(&parameter.ty) != CopyPermission::Allowed {
                        return Err(self.reject_noncopyable(
                            self.permission(&parameter.ty), "方法內函式引數", value));
                    }
                    let actual = self.method_pure_value_type(owner, method, value)?;
                    if actual != parameter.ty {
                        return Err(self.error("方法內函式引數型別不一致", value));
                    }
                }
                Ok(function.result_ty.clone())
            }
            TermKind::Construct(name, args) if args.is_empty()
                && !self.structs.contains_key(name) => {
                // helper() 與 Struct() 共用解析分支，只有已存在的
                // 零參數一般函式可以作為純 Copyable 方法值來源。
                let function = self.find_function(name)
                    .ok_or_else(|| self.error("方法內零參數函式不存在", term))?;
                if !function.params.is_empty() {
                    return Err(self.error("方法內零參數函式實際需傳入引數", term));
                }
                let permission = self.permission(&function.result_ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission, "方法內函式回傳值", term));
                }
                Ok(function.result_ty.clone())
            }
            TermKind::BinaryInt { left, operator, right } => {
                let left_ty = self.method_pure_value_type(owner, method, left)?;
                if left_ty != "int" {
                    return Err(self.error("方法整數加減左運算元必須為 int", left));
                }
                let right_ty = self.method_pure_value_type(owner, method, right)?;
                if right_ty != "int" {
                    return Err(self.error("方法整數加減右運算元必須為 int", right));
                }
                debug_assert!(*operator == '+' || *operator == '-');
                Ok("int".to_string())
            }
            _ => Err(self.error("方法值來源尚未證明可安全獨立求值", term)),
        }
    }

    fn method_bool_condition(&self, owner: &str, method: &Function, term: &Term)
        -> Result<(), Diagnostic> {
        let ty = self.method_pure_value_type(owner, method, term)?;
        if ty != "bool" {
            return Err(self.error("方法 if 條件必須為可複製的 bool", term));
        }
        Ok(())
    }

    /// 方法可將合法具名參數的整筆持有權在 return 時明確移交。
    /// 不推定 self 欄位的借用回傳或隱含複製；本階段方法在
    /// 提前／最後 return 前只能使用已驗證的純 Copyable 語句。
    fn method_return_type(&self, owner: &str, method: &Function, term: &Term)
        -> Result<String, Diagnostic> {
        if let TermKind::Move(name) = &term.kind {
            if name == "self" {
                return Err(self.error("S15：方法不可消耗整個 self", term));
            }
            let parameter = method.params.iter().find(|parameter| parameter.name == *name)
                .ok_or_else(|| self.error(
                    "方法僅能明確移動已取得持有權的具名參數", term))?;
            return Ok(parameter.ty.clone());
        }
        // 非 Copyable 結果可以由「不借用 self、不移動既有資源」
        // 的一般函式呼叫直接新建並在 return 時整筆交接。
        // 僅放行已核准 str 擁有值；尚無安全證明的其他資源保留拒絕。
        let ordinary_call: Option<(&str, &[Term])> = match &term.kind {
            TermKind::Call(name, args) => Some((name.as_str(), args.as_slice())),
            TermKind::Construct(name, args) if args.is_empty()
                && !self.structs.contains_key(name) => Some((name.as_str(), &[])),
            _ => None,
        };
        if let Some((name, args)) = ordinary_call {
            if let Some(function) = self.find_function(name) {
                if function.result_ty == "str" {
                    if function.params.len() != args.len() {
                        return Err(self.error("方法資源回傳的函式引數數量不符", term));
                    }
                    for (arg, parameter) in args.iter().zip(&function.params) {
                        let permission = self.permission(&parameter.ty);
                        if permission != CopyPermission::Allowed {
                            return Err(self.reject_noncopyable(
                                permission, "方法資源回傳呼叫引數", arg));
                        }
                        let actual = self.method_pure_value_type(owner, method, arg)?;
                        if actual != parameter.ty {
                            return Err(self.error("方法資源回傳呼叫引數型別不符", arg));
                        }
                    }
                    return Ok("str".to_string());
                }
            }
        }
        self.method_pure_value_type(owner, method, term)
    }

    /// 以所有可能執行路徑的操作推導方法整體借用模式；
    /// 任一路徑寫入都需要在方法入口取得獨占借用。
    fn method_writes(&self, owner: &str, method: &Function, body: &[Stmt])
        -> Result<bool, Diagnostic> {
        let definition = self.structs.get(owner).expect("方法所屬型別已確認");
        let mut writes = false;
        for statement in body {
            match &statement.kind {
                StmtKind::FieldAssign { receiver, field, value } if receiver == "self" => {
                    let target = definition.field(field)
                        .ok_or_else(|| self.error("self 欄位不存在", value))?;
                    if target.immutable {
                        return Err(self.error("S18：const 欄位禁止透過方法寫入", value));
                    }
                    let actual = self.method_pure_value_type(owner, method, value)?;
                    if actual != target.ty {
                        return Err(self.error("方法欄位賦值型別與宣告不一致", value));
                    }
                    // 方法接收者在此子集已完整初始化，欄位寫入必然覆寫舊值。
                    // 非 Copyable 資源的舊值清理順序尚待 P16，必須先保守拒絕。
                    let permission = self.permission(&target.ty);
                    if permission != CopyPermission::Allowed {
                        return Err(self.reject_noncopyable(permission,
                            "P16 尚未核准方法覆寫既有資源的清理責任", value));
                    }
                    writes = true;
                }
                StmtKind::Return(value) => {
                    // P01 已核准可達分支提前 return；方法的每個出口
                    // 均須遵守相同回傳型別與 Copyable／self 借用限制。
                    let actual = self.method_return_type(owner, method, value)?;
                    if actual != method.result_ty {
                        return Err(self.error("方法提前回傳型別與簽章不一致", value));
                    }
                    // 後續同一條路徑已不可達，不得把它當作額外借用操作。
                    return Ok(writes);
                }
                StmtKind::If { condition, yes, no } => {
                    self.method_bool_condition(owner, method, condition)?;
                    let yes_writes = self.method_writes(owner, method, yes)?;
                    let no_writes = if let Some(otherwise) = no {
                        self.method_writes(owner, method, otherwise)?
                    } else { false };
                    writes |= yes_writes || no_writes;
                }
                _ => return Err(Diagnostic {
                    kind: DiagnosticKind::Static,
                    message: "方法仍有未驗證的語句／別名或生命週期語意".into(),
                    span: Some(self.source.span(statement.start, statement.end)),
                }),
            }
        }
        Ok(writes)
    }

    fn check_method(&self, owner: &str, method: &Function)
        -> Result<BorrowKind, Diagnostic> {
        for parameter in &method.params {
            if parameter.name == "self" {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "S15：方法不得宣告遮蔽 self 的參數".into(), span: None });
            }
            if !["int", "bool", "float", "str"].contains(&parameter.ty.as_str())
                && !self.structs.contains_key(&parameter.ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：方法參數使用不存在的型別".into(), span: None });
            }
        }
        self.reject_method_statements(&method.body)?;
        let Some(Stmt { kind: StmtKind::Return(return_value), .. }) = method.body.last() else {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "目前方法必須以單一明確 return 結束".into(), span: None });
        };
        let mode = if self.method_writes(owner, method,
            &method.body[..method.body.len() - 1])? {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        let actual = self.method_return_type(owner, method, return_value)?;
        if actual != method.result_ty {
            return Err(self.error("方法回傳型別與已驗證運算式不一致", return_value));
        }
        Ok(mode)
    }

    /// 方法引數依來源順序在取得 self 借用前求值。
    /// 已核准 Copyable 參數可接受具名整筆 .move()；求值後必須
    /// 再查核接收者仍有效。仍禁止未完成的再借用／巢狀呼叫資源副作用。
    fn method_argument_safe_before_borrow(term: &Term) -> bool {
        match &term.kind {
            TermKind::Literal(_) | TermKind::Name(_) | TermKind::Move(_)
                | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt { left, right, .. } =>
                Self::method_argument_safe_before_borrow(left)
                    && Self::method_argument_safe_before_borrow(right),
            _ => false,
        }
    }

    /// 迴圈條件每次重新求值；不像方法位置引數，
    /// 整筆 .move() 在這裡不能視為可重複使用的安全來源。
    /// D04-A 對來源整數常值的半開遞增區間：僅在端點相差一且
    /// 可確定只迭代一次時消除不必要的反向邊。其他範圍仍保守驗證。
    fn known_range_iterations(lower: &str, upper: &str) -> Option<u8> {
        fn read_positive_int(text: &str) -> Option<u128> {
            let (radix, digits) = if let Some(rest) = text.strip_prefix("0x") {
                (16, rest)
            } else if let Some(rest) = text.strip_prefix("0b") {
                (2, rest)
            } else if let Some(rest) = text.strip_prefix("0o") {
                (8, rest)
            } else {
                (10, text)
            };
            let normalized: String = digits.chars().filter(|ch| *ch != '_').collect();
            u128::from_str_radix(&normalized, radix).ok()
        }
        match (read_positive_int(lower), read_positive_int(upper)) {
            (Some(start), Some(end)) if start == end => Some(0),
            (Some(start), Some(end)) if start.checked_add(1) == Some(end) => Some(1),
            _ => None,
        }
    }

    fn while_condition_nonconsuming(term: &Term) -> bool {
        match &term.kind {
            TermKind::Literal(_) | TermKind::Name(_) | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt { left, right, .. } =>
                Self::while_condition_nonconsuming(left)
                    && Self::while_condition_nonconsuming(right),
            _ => false,
        }
    }

    fn evaluate(&self, term: &Term, vars: &mut Scopes)
        -> Result<(String, bool), Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok(((*ty).to_string(), true)),
            TermKind::BinaryInt { left, operator, right } => {
                // 順序由 P01 左結合決定，不得跳過內層 .move() 狀態更新。
                let (left_ty, left_ready) = self.evaluate(left, vars)?;
                if left_ty != "int" || !left_ready {
                    return Err(self.error("P13：加減左運算元必須為已初始化 int", left));
                }
                let (right_ty, right_ready) = self.evaluate(right, vars)?;
                if right_ty != "int" || !right_ready {
                    return Err(self.error("P13：加減右運算元必須為已初始化 int", right));
                }
                debug_assert!(*operator == '+' || *operator == '-');
                Ok(("int".to_string(), true))
            }
            TermKind::Name(name) => {
                self.check_direct(vars, name, term)?;
                let binding = Self::lookup(vars, name)
                    .ok_or_else(|| self.error("找不到合法的來源宣告", term))?;
                if binding.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用已於最後使用點歸還", term));
                }
                // 非擁有借用僅能對已核准 Copyable 值產生獨立複本，
                // 不得以一般名稱使用把 str 借用轉成新資源擁有者。
                binding.value.duplicate().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::Borrow(_) => Err(self.error(
                "VA14／VA15：.borrow() 僅可直接綁定到合法 var／const；回傳與逃逸保守拒絕",
                term)),
            TermKind::Move(name) => {
                if name == "self" {
                    return Err(self.error("S15：方法不可消耗或移動整個 self", term));
                }
                self.check_direct(vars, name, term)?;
                let binding = Self::lookup_mut(vars, name)
                    .ok_or_else(|| self.error("找不到可移動的具名來源", term))?;
                if binding.loan.is_some() {
                    return Err(self.error("VA14-01：借用別名不是所有者，不能 .move()", term));
                }
                binding.value.move_whole().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::FieldRead(owner, name) => {
                if owner == "self" {
                    return Err(self.error("只有方法內才能讀取 self 欄位", term));
                }
                self.check_direct(vars, owner, term)?;
                let bound = Self::lookup(vars, owner)
                    .ok_or_else(|| self.error("找不到欄位來源宣告", term))?;
                if bound.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用別名已不再有效", term));
                }
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                let field = self.structs.get(&bound.ty)
                    .and_then(|def| def.field(name))
                    .ok_or_else(|| self.error("來源不是具有此欄位的結構", term))?;
                let permission = self.permission(&field.ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission,
                        "欄位讀取", term));
                }
                Ok((field.ty.clone(), true))
            }
            TermKind::MethodCall(receiver, name, args) => {
                if receiver == "self" {
                    return Err(self.error("重入 self 方法的借用安全尚未驗證", term));
                }
                self.check_direct(vars, receiver, term)?;
                let owner_type = {
                    let bound = Self::lookup(vars, receiver)
                        .ok_or_else(|| self.error("找不到方法接收者來源", term))?;
                    bound.value.read().map_err(|err| self.model_error(err, term))?;
                    bound.ty.clone()
                };
                let method = self.structs.get(&owner_type)
                    .and_then(|def| def.method(name))
                    .ok_or_else(|| self.error("找不到結構上的合法方法", term))?;
                let mode = self.check_method(&owner_type, method)?;
                if method.params.len() != args.len() {
                    return Err(self.error("P13：方法引數數量不符", term));
                }
                // P15.6：方法參數也屬函式的預設借用，但 self
                // 在本次呼叫期間有自己的借用票據。先評估所有
                // 參數並保持其借用到方法出口；具名 str 不等同複製。
                let receiver_source = Self::lookup(vars, receiver)
                    .ok_or_else(|| self.error("找不到方法接收者來源", term))?
                    .declaration_start;
                let mut argument_loans = BorrowSafety::new();
                let mut argument_tickets = Vec::new();
                let mut borrowed_names: Vec<String> = Vec::new();
                for (arg, parameter) in args.iter().zip(&method.params) {
                    if !Self::method_argument_safe_before_borrow(arg) {
                        return Err(self.error(
                            "P13：方法引數僅接受已驗證的純值或具名整筆移動", arg));
                    }
                    let is_default_borrow = self.permission(&parameter.ty)
                        != CopyPermission::Allowed
                        && matches!(&arg.kind, TermKind::Name(_))
                        && !Self::parameter_is_consumed(method, &parameter.name);
                    if !is_default_borrow && borrowed_names.iter()
                        .any(|name| Self::term_mentions(arg, name)) {
                        return Err(self.error(
                            "P15.6：方法其餘引數不能使用尚未歸還的借用來源", arg));
                    }
                    if is_default_borrow {
                        let TermKind::Name(name) = &arg.kind else { unreachable!() };
                        let original = Self::lookup(vars, name)
                            .ok_or_else(|| self.error("P15.6：方法引數借用來源不存在", arg))?;
                        if original.loan.is_some() {
                            return Err(self.error(
                                "VB14：方法參數不可在未證明安全時再借用別名", arg));
                        }
                        if original.declaration_start == receiver_source {
                            return Err(self.error(
                                "P15.6：self 與方法參數不能對同一來源建立未證明的重疊借用", arg));
                        }
                        original.value.read().map_err(|err| self.model_error(err, arg))?;
                        if original.ty != parameter.ty {
                            return Err(self.error("P15.6：方法借用參數型別不一致", arg));
                        }
                        let kind = if original.mutable {
                            BorrowKind::ExclusiveWrite
                        } else { BorrowKind::SharedRead };
                        BorrowSafety::validate_new_borrow(
                            &Self::active_loans(vars, original.declaration_start), kind)
                            .map_err(|_| self.error(
                                "HC14：方法參數與既有借用衝突", arg))?;
                        let ticket = argument_loans.acquire(
                            DeclarationId(original.declaration_start), kind)
                            .map_err(|_| self.error(
                                "HC14：同一方法呼叫的參數借用衝突", arg))?;
                        argument_tickets.push(ticket);
                        borrowed_names.push(name.clone());
                        continue;
                    }
                    // 非 Copyable 參數若確實被方法整筆 .move() 消耗，
                    // 普通名稱仍不得通過 evaluate() 偷成第二所有者。
                    let (actual, ready) = self.evaluate(arg, vars)?;
                    if !ready || actual != parameter.ty {
                        return Err(self.error("P13：方法引數型別錯誤或尚未初始化", arg));
                    }
                }
                // 任何參數求值後再次驗證接收者與來源借用狀態。
                self.check_direct(vars, receiver, term)?;
                let bound = Self::lookup(vars, receiver)
                    .ok_or_else(|| self.error("找不到方法接收者來源", term))?;
                if bound.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用別名已不再有效", term));
                }
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                if mode == BorrowKind::ExclusiveWrite && !bound.mutable {
                    return Err(self.error(
                        "S18：const 結構實例不能呼叫會修改 self 欄位的方法", term));
                }
                if let Some(loan) = &bound.loan {
                    if mode == BorrowKind::ExclusiveWrite
                        && loan.kind != BorrowKind::ExclusiveWrite {
                        return Err(self.error("HC14：唯讀借用不可呼叫可寫方法", term));
                    }
                } else {
                    let mut loans = BorrowSafety::new();
                    let loan = loans.acquire(DeclarationId(bound.declaration_start), mode)
                        .map_err(|_| self.error("方法 self 借用與現有借用衝突", term))?;
                    if loans.check_direct_access(DeclarationId(bound.declaration_start)).is_ok() {
                        return Err(self.error("HC14：方法期間不得直接使用被借用來源", term));
                    }
                    loans.finish_scope(&[loan])
                        .map_err(|_| self.error("方法 self 借用未安全歸還", term))?;
                    loans.check_direct_access(DeclarationId(bound.declaration_start))
                        .map_err(|_| self.error("方法後來源借用未解除", term))?;
                }
                // 方法實際完成後才歸還參數，不使用一般借用最後
                // 使用點規則，也不讓未通過的其他控制路徑釋放票據。
                argument_loans.finish_scope(&argument_tickets)
                    .map_err(|_| self.error("P15.6：方法參數作用域歸還失敗", term))?;
                Ok((method.result_ty.clone(), true))
            }
            TermKind::FieldMove(_owner, _field) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：結構欄位不能單獨移動", term))
            }
            TermKind::Call(name, args) => {
                let function = self.find_function(name)
                    .ok_or_else(|| self.error("P13：呼叫不存在的函式", term))?;
                if function.params.len() != args.len() {
                    return Err(self.error("P13：函式引數數量不符", term));
                }
                // P15.6：函式參數預設借用。若被呼叫端確實包含
                // 該參數的整筆 .move()，則呼叫端必須明確移交來源；
                // 不得把非擁有別名假裝成能被消耗的資源持有者。
                // 引數形式先全部預檢，避免驗到後面的錯誤時已
                // 將前面具名資源移走。
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    if self.permission(expected) != CopyPermission::Allowed {
                        match &arg.kind {
                            TermKind::Name(_) if !Self::parameter_is_consumed(
                                function, &parameter.name) => {}
                            TermKind::Literal("str") | TermKind::Move(_)
                                | TermKind::Construct(_, _) | TermKind::Call(_, _) => {}
                            _ => return Err(self.error(
                                "P15.6：非 Copyable 參數只接受可證明的預設借用或明確擁有權轉交", arg)),
                        }
                    }
                }
                // 使用同一套 HC14 借用衝突與票據模型，票據只存活
                // 於函式呼叫期間。不能讓同一呼叫較後的引數提前
                // 移動、覆寫或再次獨占借用仍被傳入的來源。
                let mut call_loans = BorrowSafety::new();
                let mut tickets = Vec::new();
                let mut borrowed_names: Vec<String> = Vec::new();
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    let new_borrow = self.permission(expected) != CopyPermission::Allowed
                        && matches!(&arg.kind, TermKind::Name(_))
                        && !Self::parameter_is_consumed(function, &parameter.name);
                    if !new_borrow && borrowed_names.iter()
                        .any(|name| Self::term_mentions(arg, name)) {
                        return Err(self.error(
                            "P15.6：同一呼叫的後續引數不可使用尚未歸還的借用來源", arg));
                    }
                    if self.permission(expected) != CopyPermission::Allowed {
                        if let TermKind::Name(name) = &arg.kind {
                            if !Self::parameter_is_consumed(function, &parameter.name) {
                                let original = Self::lookup(vars, name)
                                    .ok_or_else(|| self.error("P15.6：函式借用來源不存在", arg))?;
                                if original.loan.is_some() {
                                    return Err(self.error(
                                        "VB14：借用別名不能在缺少重借用證明時傳入函式", arg));
                                }
                                original.value.read().map_err(|err| self.model_error(err, arg))?;
                                if original.ty != *expected {
                                    return Err(self.error("P15.6：函式借用實參型別不符", arg));
                                }
                                let kind = if original.mutable {
                                    BorrowKind::ExclusiveWrite
                                } else { BorrowKind::SharedRead };
                                BorrowSafety::validate_new_borrow(
                                    &Self::active_loans(vars, original.declaration_start), kind)
                                    .map_err(|_| self.error(
                                        "HC14：函式參數借用與仍有效別名衝突", arg))?;
                                let ticket = call_loans.acquire(
                                    DeclarationId(original.declaration_start), kind)
                                    .map_err(|_| self.error(
                                        "HC14：同一呼叫內函式借用來源衝突", arg))?;
                                tickets.push(ticket);
                                borrowed_names.push(name.clone());
                                continue;
                            }
                        }
                    }
                    let (actual, initialized) = self.evaluate(arg, vars)?;
                    if actual != *expected || !initialized {
                        return Err(self.error("P13：函式引數型別不符或尚未初始化", arg));
                    }
                }
                call_loans.finish_scope(&tickets)
                    .map_err(|_| self.error("P15.6：函式退出時借用票據未完整歸還", term))?;
                Ok((function.result_ty.clone(), true))
            },
            TermKind::Construct(name, args) => {
                // 零引數語法與結構建立式共用括號。只有不存在同名結構
                // 且已宣告零參數函式時，才按函式呼叫處理。
                if args.is_empty() && !self.structs.contains_key(name) {
                    if let Some(function) = self.find_function(name) {
                        if !function.params.is_empty() {
                            return Err(self.error("P13：函式引數數量不符", term));
                        }
                        return Ok((function.result_ty.clone(), true));
                    }
                }
                let definition = self.structs.get(name)
                    .ok_or_else(|| self.error("找不到結構型別或不支援一般函式呼叫", term))?;
                // VB16-01-A：嚴格依書寫順序先求值所有具名引數，之後
                // 再依宣告順序求值未被覆蓋的預設值。先檢查整份
                // 計畫再求值，重複／不存在欄位不得先消耗資源來源。
                let fields: Vec<_> = definition.fields.iter().map(|field|
                    crate::struct_eval_plan::FieldInfo {
                        name: field.name.as_str(),
                        has_default: field.default.is_some(),
                        immutable: field.immutable,
                    }).collect();
                let names: Vec<_> = args.iter().map(|(field, _)| field.as_str()).collect();
                let ordered = crate::struct_eval_plan::plan(&fields, &names)
                    .map_err(|error| self.error(&format!(
                        "VB16：結構求值計畫不合法：{error:?}"), term))?;
                for step in &ordered.steps {
                    let expected = definition.fields.iter()
                        .find(|field| field.name == step.field)
                        .expect("計畫中的欄位已預檢存在");
                    let (expr, actual, ready) = match step.origin {
                        crate::struct_eval_plan::Origin::Named => {
                            let expr = args.iter()
                                .find(|(field, _)| *field == step.field)
                                .map(|(_, expression)| expression)
                                .expect("具名引數已預檢存在");
                            let (actual, ready) = self.evaluate(expr, vars)?;
                            (expr, actual, ready)
                        }
                        crate::struct_eval_plan::Origin::Default => {
                            let expr = expected.default.as_ref()
                                .expect("未被覆蓋的預設值已預檢存在");
                            // 欄位預設值在其宣告本身的獨立上下文求值，
                            // 不暗中捕獲呼叫端的區域名稱。
                            let (actual, ready) = self.evaluate(
                                expr, &mut vec![Rc::new(HashMap::new())])?;
                            (expr, actual, ready)
                        }
                    };
                    if actual != expected.ty || !ready {
                        return Err(self.error(
                            "VB16：結構欄位求值型別錯誤或資源尚未初始化",
                            expr));
                    }
                }
                Ok((name.clone(), ordered.missing_mutable_fields.is_empty()))
            }
        }
    }
    fn model_error(&self, err: ModelError, term: &Term) -> Diagnostic {
        let message = match err {
            ModelError::MovedRead => "來源已移動，禁止再次使用",
            ModelError::UninitializedRead => "來源尚未完成初始化，不可使用",
            ModelError::UncertainRead => "存在可達路徑使來源可能無效",
            ModelError::CopyForbidden => "型別 Copyable 已明確禁止，拒絕隱含複製",
            ModelError::CopyUndecided => "型別 Copyable 能力未核准，拒絕預設複製",
            _ => "所有權狀態無法獲得合法證明",
        };
        self.error(message, term)
    }
    /// 只尋找會跳出目前 for／while 的控制轉移；內層迴圈的
    /// break／continue 不屬於外層，不能因此封鎖合法單次移動。
    fn has_current_loop_transfer(body: &[Stmt]) -> bool {
        body.iter().any(|stmt| match &stmt.kind {
            StmtKind::Break | StmtKind::Continue => true,
            StmtKind::If { yes, no, .. } => Self::has_current_loop_transfer(yes)
                || no.as_ref().is_some_and(|other| Self::has_current_loop_transfer(other)),
            StmtKind::ForRange { .. } | StmtKind::While { .. } => false,
            _ => false,
        })
    }

    /// P15.3：對 break／continue 的每條實際轉移路徑確認所有
    /// 外層持有者、部分初始化與借用票據均與迴圈入口一致。
    /// 不把未達不動點的資源變化偽裝成安全跳出或下一輪。
    fn check_loop_exit_stability(&self, vars: &Scopes, entry: &Scopes,
        statement: &Stmt) -> Result<(), Diagnostic> {
        for (depth, scope) in entry.iter().enumerate() {
            for (name, original) in scope.iter() {
                let candidate = vars.get(depth).and_then(|scope| scope.get(name))
                    .ok_or_else(|| Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "P15：跳轉後外層來源宣告遺失".into(),
                        span: Some(self.source.span(statement.start, statement.end)),
                    })?;
                let original_loan = original.loan.as_ref().map(|loan|
                    (loan.source, loan.kind, loan.active));
                let candidate_loan = candidate.loan.as_ref().map(|loan|
                    (loan.source, loan.kind, loan.active));
                if candidate.declaration_start != original.declaration_start
                    || candidate.value != original.value
                    || candidate.pending_fields != original.pending_fields
                    || original_loan != candidate_loan {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: if matches!(&statement.kind, StmtKind::ForRange { .. }) {
                            "P15：單次 for 混合迴圈跳轉與正常路徑時外層所有權或借用狀態不穩定".into()
                        } else {
                            "P15：break／continue 轉移時外層所有權或借用狀態不穩定".into()
                        },
                        span: Some(self.source.span(statement.start, statement.end)),
                    });
                }
            }
        }
        Ok(())
    }

    fn statements(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool) -> Result<FlowState, Diagnostic> {
        self.statements_with_follow(body, vars, expected_return, new_scope, &[], None)
    }

    fn statements_with_follow(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool,
        continuation: &[(usize, Vec<Stmt>)],
        loop_entry: Option<&Scopes>) -> Result<FlowState, Diagnostic> {
        if new_scope { vars.push(Rc::new(HashMap::new())); }
        let result = self.statement_body(body, vars, expected_return, continuation, loop_entry);
        if new_scope { vars.pop(); }
        result
    }

    fn statement_body(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, continuation: &[(usize, Vec<Stmt>)],
        loop_entry: Option<&Scopes>) -> Result<FlowState, Diagnostic> {
        // 區塊入口若某外層借用在本條路徑及其外層後繼均不再使用，
        // 便已過最後使用點，不需要等到下一筆語句才歸還。
        Self::release_last_uses(vars, body, continuation);
        for (position, statement) in body.iter().enumerate() {
            match &statement.kind {
                StmtKind::Declare { name, ty, mutable, init } => {
                    if vars.last().expect("應有詞彙作用域").contains_key(name) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "此受限子集不接受同作用域重複宣告".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    if !["int", "bool", "float", "str"].contains(&ty.as_str())
                        && !self.structs.contains_key(ty) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "宣告使用不存在的型別".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    let mut loan = None;
                    let initialized = if let Some(expr) = init {
                        let (actual, ready) = if let TermKind::Borrow(source) = &expr.kind {
                            let (actual, alias) = self.authorize_borrow(vars, source, *mutable, expr)?;
                            loan = Some(alias);
                            (actual, true)
                        } else {
                            self.evaluate(expr, vars)?
                        };
                        if &actual != ty {
                            return Err(self.error("初始化運算式與宣告型別不一致", expr));
                        }
                        ready
                    } else { false };
                    // 只有具名 struct 建立式可給出精確的缺失欄位集合；
                    // 無初始化的普通 var 宣告不得假設已分配結構實例。
                    let pending_fields = if !initialized {
                        if let Some(Term { kind: TermKind::Construct(actual, args), .. }) = init {
                            self.structs.get(actual).and_then(|definition| {
                                let explicit: HashSet<&str> =
                                    args.iter().map(|(name, _)| name.as_str()).collect();
                                let missing: HashSet<String> = definition.fields.iter()
                                    .filter(|field| field.default.is_none()
                                        && !explicit.contains(field.name.as_str()))
                                    .map(|field| field.name.clone())
                                    .collect();
                                if missing.is_empty() { None } else { Some(missing) }
                            })
                        } else { None }
                    } else { None };
                    let permission = self.permission(ty);
                    let slot = if initialized {
                        ValueSlot::initialized(permission)
                    } else {
                        ValueSlot::uninitialized(permission)
                    };
                    Rc::make_mut(vars.last_mut().expect("應有詞彙作用域")).insert(name.clone(), Binding {
                        declaration_start: statement.start, ty: ty.clone(),
                        mutable: *mutable, value: slot, pending_fields, loan
                    });
                }
                StmtKind::FieldAssign { receiver, field, value } => {
                    // VA16-03-A：已核准的 str 獨立常值可以先取得唯一新資源，
                    // 成功後原子覆寫具名 var 結構的 var str 欄位；
                    // 其他非 Copyable 右值仍維持靜態保守拒絕。
                    self.check_direct(vars, receiver, value)?;
                    let owner = Self::lookup(vars, receiver)
                        .ok_or_else(|| self.error("欄位寫入找不到合法接收者", value))?;
                    if owner.loan.as_ref().is_some_and(|loan| !loan.active) {
                        return Err(self.error("P15：借用別名已不再有效", value));
                    }
                    let first_init = owner.value.state() == ValueState::Uninitialized
                        && owner.pending_fields.as_ref()
                            .is_some_and(|pending| pending.contains(field));
                    if !first_init {
                        owner.value.read().map_err(|err| self.model_error(err, value))?;
                    }
                    if !owner.mutable {
                        return Err(self.error("S18：const 結構實例不可修改欄位", value));
                    }
                    let member = self.structs.get(&owner.ty)
                        .and_then(|def| def.field(field))
                        .ok_or_else(|| self.error("欄位寫入的型別或欄位不存在", value))?;
                    if member.immutable {
                        return Err(self.error("S18：const 欄位不可重新賦值", value));
                    }
                    let expected_type = member.ty.clone();
                    if !first_init {
                        let permission = self.permission(&expected_type);
                        let proven_fresh_string = expected_type == "str"
                            && matches!(&value.kind, TermKind::Literal("str"));
                        if permission != CopyPermission::Allowed && !proven_fresh_string {
                            return Err(self.reject_noncopyable(permission,
                                "P16：非 Copyable 欄位只核准新建 str 常值原子覆寫，其他資源右值仍未證明", value));
                        }
                    }
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != expected_type || !ready {
                        return Err(self.error("欄位賦值型別錯誤或來源尚未完整初始化", value));
                    }
                    // RHS 有可能消耗或重新繫結接收者：重新查核後才更新證明。
                    let owner = Self::lookup_mut(vars, receiver)
                        .ok_or_else(|| self.error("欄位寫入接收者已失效", value))?;
                    if first_init {
                        if owner.value.state() != ValueState::Uninitialized
                            || !owner.pending_fields.as_ref()
                                .is_some_and(|pending| pending.contains(field)) {
                            return Err(self.error("欄位初始化來源身分已失效", value));
                        }
                        let pending = owner.pending_fields.as_mut()
                            .expect("首次初始化必須有待完成欄位證明");
                        pending.remove(field);
                        if pending.is_empty() {
                            owner.pending_fields = None;
                            owner.value.mark_reinitialized();
                        }
                    } else {
                        owner.value.read().map_err(|err| self.model_error(err, value))?;
                    }
                }
                StmtKind::Assign { name, value } => {
                    self.check_direct(vars, name, value)?;
                    let expected = Self::lookup(vars, name).ok_or_else(|| self.error(
                        "無法賦值給尚未宣告的名稱", value))?;
                    if expected.loan.is_some() {
                        return Err(self.error(
                            "VA14-01：不得把非擁有借用綁定重新賦值成持有者", value));
                    }
                    if !expected.mutable { return Err(self.error("const 不可重新賦值", value)); }
                    let ty = expected.ty.clone();
                    // VA16-03-A 已核准非 Copyable 資源原子覆寫。
                    // 僅對已實作真實 P16 單一持有者緩衝區的 str，
                    // 准許獨立新常值與另一具名整筆移動；含資源結構、
                    // 容器與未確認欄位清理仍不得假裝有相同執行期保證。
                    let state = expected.value.state();
                    if self.permission(&ty) != CopyPermission::Allowed
                        && state == ValueState::MayBeUnavailable {
                        return Err(self.error(
                            "VA16-03：合流後資源可能存在或已移動，條件性舊值清理尚未證明",
                            value));
                    }
                    if self.permission(&ty) != CopyPermission::Allowed
                        && !matches!(state, ValueState::Uninitialized | ValueState::Moved) {
                        let proven_str_rhs = ty == "str"
                            && matches!(&value.kind,
                                TermKind::Literal("str") | TermKind::Move(_));
                        if !proven_str_rhs {
                            return Err(self.error(
                                "P16：此型別／右值的既有資源覆寫尚未完成端到端清理證明",
                                value));
                        }
                    }
                    // 自身轉移與寫回的重疊求值次序尚未有獨立安全證據。
                    if matches!(&value.kind, TermKind::Move(source) if source == name) {
                        return Err(self.error(
                            "P13：同一名稱不可在同一次賦值中移動並重綁自身",
                            value));
                    }
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != ty || !ready {
                        return Err(self.error("賦值型別錯誤或來源尚未完成初始化", value));
                    }
                    let destination = Self::lookup_mut(vars, name).expect("已確認來源");
                    destination.value.mark_reinitialized();
                    destination.pending_fields = None;
                }
                StmtKind::Return(term) => {
                    let (actual, ready) = self.evaluate(term, vars)?;
                    if actual != expected_return || !ready {
                        return Err(self.error("函式回傳型別不一致或來源未初始化", term));
                    }
                    return Ok(FlowState::Returned);
                }
                StmtKind::Break | StmtKind::Continue => {
                    let Some(snapshot) = loop_entry else {
                        return Err(Diagnostic {
                            kind: DiagnosticKind::Static,
                            message: "P15：break／continue 只能位於已核准的 for／while 迴圈內".into(),
                            span: Some(self.source.span(statement.start, statement.end)),
                        });
                    };
                    // 迴圈的中斷或反向邊都不能把外層來源變成
                    // 另一個未證明的持有者／別名狀態。嵌套 if 各路
                    // 徑在轉移點個別查核，因此不會把 continue 偷當 return。
                    self.check_loop_exit_stability(vars, snapshot, statement)?;
                    return Ok(FlowState::LoopTransfer);
                }
                StmtKind::ForRange { iterator, iterator_start,
                    lower, upper, body: loop_body } => {
                    // D04-A 半開區間為已核准來源，但正式執行期的
                    // 整數計數器／步幅不屬 P15，本階段只驗證借用
                    // 與反向邊：一次以上迭代皆不能破壞外層狀態。
                    let known_iterations = Self::known_range_iterations(lower, upper);
                    let one_iteration = known_iterations == Some(1);
                    // 端點相同的半開區間沒有任何迭代：不得把未執行
                    // 的迴圈本體當成已消耗或仍持有外層借用的路徑。
                    if known_iterations != Some(0) {
                    let snapshot = vars.clone();
                    let mut simulated = snapshot.clone();
                    // for 迭代名稱每次重新建立，不能在出口沿用前
                    // 一次迭代的借用或將其誤認成外層同名宣告。
                    simulated.push(Rc::new(HashMap::new()));
                    Rc::make_mut(simulated.last_mut().expect("迭代作用域存在")).insert(
                        iterator.clone(),
                        Binding {
                            declaration_start: *iterator_start, ty: "int".into(),
                            mutable: false,
                            value: ValueSlot::initialized(CopyPermission::Allowed),
                            pending_fields: None, loan: None,
                        });
                    let mut after_loop = Vec::with_capacity(continuation.len() + 2);
                    // 已知恰好一次迭代時不存在下一次本體；
                    // 不得將假想反向邊延長借用最後使用點。
                    if !one_iteration {
                        after_loop.push((vars.len(), loop_body.clone()));
                    }
                    after_loop.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_loop.extend_from_slice(continuation);
                    let can_repeat = self.statements_with_follow(
                        loop_body, &mut simulated, expected_return, false, &after_loop,
                        Some(&snapshot))?;
                    simulated.pop();
                    if one_iteration {
                        // break／continue 只退出本迴圈，不是函式 return。
                        // 若入口狀態經轉移驗證保持穩定，後繼仍須檢查。
                        match can_repeat {
                            FlowState::Returned => return Ok(FlowState::Returned),
                            FlowState::LoopTransfer => *vars = snapshot,
                            FlowState::Fallthrough => {
                                // 此三態回傳只記錄「可繼續」的分支狀態。
                                // 若同一輪也存在 break／continue，不能把
                                // 正常分支的重新初始化或移動當成所有出口
                                // 都發生過；跳轉路徑已個別驗證與入口一致，
                                // 正常路徑也必須一致才可安全合流。
                                if Self::has_current_loop_transfer(loop_body) {
                                    self.check_loop_exit_stability(&simulated, &snapshot, statement)?;
                                }
                                *vars = simulated;
                            }
                        }
                    } else if can_repeat == FlowState::Fallthrough {
                        for (depth, scope) in snapshot.iter().enumerate() {
                            for (name, original) in scope.iter() {
                                let candidate = simulated[depth].get(name)
                                    .expect("迴圈不得移除外層宣告");
                                let old_loan = original.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                let new_loan = candidate.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                if candidate.declaration_start != original.declaration_start
                                    || candidate.value != original.value
                                    || candidate.pending_fields != original.pending_fields
                                    || old_loan != new_loan {
                                    return Err(Diagnostic {
                                        kind: DiagnosticKind::Static,
                                        message: "P15：for 反向邊來源狀態不穩定，不可跨迭代沿用借用／所有權".into(),
                                        span: Some(self.source.span(statement.start, statement.end)),
                                    });
                                }
                            }
                        }
                    }
                    } // 有至少一種迭代可能時才檢查迴圈本體
                }
                StmtKind::While { condition, body: loop_body } => {
                    // 條件於每次迭代重新求值；不得讓條件本身移動
                    // 所有者、建立借用或觸發未受控函式副作用。
                    if !Self::while_condition_nonconsuming(condition) {
                        return Err(self.error(
                            "P15：while 條件只能使用無資源副作用的合法純值", condition));
                    }
                    let (actual, ready) = self.evaluate(condition, vars)?;
                    if actual != "bool" || !ready {
                        return Err(self.error("while 條件必須是已初始化 bool", condition));
                    }
                    let snapshot = vars.clone();
                    let mut simulated = snapshot.clone();
                    // 下一次條件檢查及迭代本體都屬未來可達路徑，
                    // 不能在本次迭代提早歸還還會被引用的外層別名。
                    let condition_read = Stmt {
                        kind: StmtKind::Return(condition.clone()),
                        start: condition.start,
                        end: condition.end,
                    };
                    let mut after_loop = Vec::with_capacity(continuation.len() + 3);
                    after_loop.push((vars.len() - 1, vec![condition_read]));
                    after_loop.push((vars.len() - 1, loop_body.clone()));
                    after_loop.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_loop.extend_from_slice(continuation);
                    let can_repeat = self.statements_with_follow(
                        loop_body, &mut simulated, expected_return, true, &after_loop,
                        Some(&snapshot))?;
                    if can_repeat == FlowState::Fallthrough {
                        // 未建立不動點證明前，所有會回到條件的外部來源
                        // 必須保持所有權、部分初始化及借用活性不變。
                        // 迴圈局部宣告會於每次區塊退出後消失。
                        for (depth, scope) in snapshot.iter().enumerate() {
                            for (name, original) in scope.iter() {
                                let candidate = simulated[depth].get(name)
                                    .expect("迴圈不得移除外層宣告");
                                let old_loan = original.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                let new_loan = candidate.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                if candidate.declaration_start != original.declaration_start
                                    || candidate.value != original.value
                                    || candidate.pending_fields != original.pending_fields
                                    || old_loan != new_loan {
                                    return Err(self.error(
                                        "P15：while 反向邊的來源所有權／借用狀態不穩定，需完整不動點證明",
                                        condition));
                                }
                            }
                        }
                    }
                    // while 可執行零次，因此後續只使用入口快照；
                    // 任何可回返狀態已確認與此快照一致。
                }
                StmtKind::If { condition, yes, no } => {
                    let (actual, ready) = self.evaluate(condition, vars)?;
                    if actual != "bool" || !ready {
                        return Err(self.error("if 條件必須是已初始化 bool", condition));
                    }
                    let snapshot = vars.clone();
                    // 每條分支只能參考自己之後真正可達的語句。
                    // 外層後繼若還會使用某個借用別名，任何分支都
                    // 不得在提前歸還後直接使用原來源。
                    let mut after_branch = Vec::with_capacity(continuation.len() + 1);
                    after_branch.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_branch.extend_from_slice(continuation);
                    let mut on_true = snapshot.clone();
                    let true_flow = self.statements_with_follow(
                        yes, &mut on_true, expected_return, true, &after_branch,
                        loop_entry)?;
                    let mut on_false = snapshot.clone();
                    let false_flow = if let Some(otherwise) = no {
                        self.statements_with_follow(
                            otherwise, &mut on_false, expected_return, true, &after_branch,
                            loop_entry)?
                    } else {
                        Self::release_last_uses(&mut on_false, &[], &after_branch);
                        FlowState::Fallthrough
                    };
                    let live_true = true_flow == FlowState::Fallthrough;
                    let live_false = false_flow == FlowState::Fallthrough;
                    if !live_true && !live_false {
                        return Ok(if true_flow == FlowState::Returned
                            && false_flow == FlowState::Returned {
                            FlowState::Returned
                        } else { FlowState::LoopTransfer });
                    }
                    // 僅合流分支進入前可見的真正名稱。分支私有宣告不得洩漏。
                    for (depth, scope) in vars.iter_mut().enumerate() {
                        for (name, original) in Rc::make_mut(scope).iter_mut() {
                            // 真正來源以原始宣告位元組起點及詞彙深度識別。
                            let left = if live_true {
                                let same = on_true[depth].get(name)
                                    .expect("原來源必須存在");
                                if same.declaration_start != original.declaration_start {
                                    return Err(self.error("分支來源身分不一致", condition));
                                }
                                Some(same.value)
                            } else { None };
                            let right = if live_false {
                                let same = on_false[depth].get(name)
                                    .expect("原來源必須存在");
                                if same.declaration_start != original.declaration_start {
                                    return Err(self.error("分支來源身分不一致", condition));
                                }
                                Some(same.value)
                            } else { None };
                            original.value = ValueSlot::join_reachable(&[left, right])
                                .map_err(|err| self.model_error(err, condition))?;
                            // 只有各可達分支的缺失欄位集合完全相同，才能
                            // 保留首次寫入許可；否則失去部分初始化證明。
                            let pending_true = if live_true {
                                Some(&on_true[depth][name].pending_fields)
                            } else { None };
                            let pending_false = if live_false {
                                Some(&on_false[depth][name].pending_fields)
                            } else { None };
                            original.pending_fields = match (pending_true, pending_false) {
                                (Some(left), Some(right)) if left == right => left.clone(),
                                // 兩個可達分支皆持有尚未完整的相同結構，
                                // 但完成欄位的順序不同時，只有「可能已初始化」
                                // 的差異欄位全為 Copyable，才可安全合併缺失聯集：
                                // 後續寫入若在某路徑是覆寫，也不會遺失資源責任。
                                (Some(Some(left)), Some(Some(right)))
                                    if original.value.state() == ValueState::Uninitialized
                                        && left.symmetric_difference(right).all(|field_name| {
                                            self.structs.get(&original.ty)
                                                .and_then(|definition| definition.fields.iter()
                                                    .find(|field| field.name == *field_name))
                                                .is_some_and(|field|
                                                    self.permission(&field.ty) == CopyPermission::Allowed)
                                        }) =>
                                    Some(left.union(right).cloned().collect()),
                                (Some(one), None) | (None, Some(one)) => one.clone(),
                                _ => None,
                            };
                            if let Some(loan) = original.loan.as_mut() {
                                // 僅合流實際繼續執行的分支；若任一可達
                                // 路徑仍有借用，合流後必須維持來源封鎖。
                                let true_active = live_true && on_true[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                let false_active = live_false && on_false[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                loan.active = true_active || false_active;
                            }
                        }
                    }
                }
            }
            // 本區塊剩餘路徑已不再引用的非擁有借用可即時歸還；
            // 不得因來源尚存在就把借用自動延長到作用域結束。
            Self::release_last_uses(vars, &body[position + 1..], continuation);
        }
        Ok(FlowState::Fallthrough)
    }

    fn function(&self, fun: &Function) -> Result<(), Diagnostic> {
        if !matches!(fun.body.last().map(|s| &s.kind), Some(StmtKind::Return(_))) {
            return Err(Diagnostic { kind: DiagnosticKind::Syntax,
                message: "函式最外層最後必須為明確 return".into(),
                span: Some(self.source.span(
                    fun.body.last().map_or(0, |s| s.start),
                    fun.body.last().map_or(0, |s| s.end))) });
        }
        let mut vars = vec![Rc::new(HashMap::new())];
        for parameter in &fun.params {
            let name = &parameter.name;
            let ty = &parameter.ty;
            if !["int", "bool", "float", "str"].contains(&ty.as_str())
                && !self.structs.contains_key(ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式參數使用不存在的型別".into(), span: None });
            }
            Rc::make_mut(&mut vars[0]).insert(name.clone(), Binding {
                declaration_start: parameter.declaration_start,
                ty: ty.clone(), mutable: false,
                value: ValueSlot::initialized(self.permission(ty)),
                pending_fields: None,
                loan: (self.permission(ty) != CopyPermission::Allowed
                    && !Self::parameter_is_consumed(fun, name)).then_some(LoanBinding {
                        // 只在被呼叫端本地使用代理宣告身份，真正來源
                        // 的借用票據由呼叫端持有至呼叫結束。
                        source: parameter.declaration_start,
                        kind: BorrowKind::SharedRead, active: true,
                    }),
            });
        }
        self.statements(&fun.body, &mut vars, &fun.result_ty, false)?;
        Ok(())
    }
    /// 對方法內所有深度的運算式查核明確禁令，不只檢查頂層 return。
    /// 方法借用完整合法性仍待 P14，不能藉此宣告方法可使用。
    fn reject_method_term(&self, term: &Term) -> Result<(), Diagnostic> {
        match &term.kind {
            TermKind::Move(name) if name == "self" => {
                let _ = struct_ownership::check_method_self(SelfAction::ConsumeWhole);
                Err(self.error("S15：方法禁止消耗整個 self", term))
            }
            TermKind::FieldMove(_, _) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：方法不可個別移動結構欄位", term))
            }
            TermKind::Construct(_, args) => {
                for (_, value) in args { self.reject_method_term(value)?; }
                Ok(())
            }
            TermKind::Call(_, args) => {
                // 函式呼叫的合法型別、Copyable 資格與完整參數來源
                // 由 method_pure_value_type 統一查核；仍先檢查 S15／S21。
                for value in args { self.reject_method_term(value)?; }
                Ok(())
            }
            TermKind::BinaryInt { left, right, .. } => {
                self.reject_method_term(left)?;
                self.reject_method_term(right)
            }
            _ => Ok(()),
        }
    }

    fn reject_method_statements(&self, body: &[Stmt]) -> Result<(), Diagnostic> {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Declare { init, .. } => {
                    if let Some(value) = init { self.reject_method_term(value)?; }
                }
                StmtKind::Assign { value, .. } | StmtKind::Return(value)
                    | StmtKind::FieldAssign { value, .. } => {
                    self.reject_method_term(value)?;
                }
                StmtKind::If { condition, yes, no } => {
                    self.reject_method_term(condition)?;
                    self.reject_method_statements(yes)?;
                    if let Some(otherwise) = no {
                        self.reject_method_statements(otherwise)?;
                    }
                }
                StmtKind::While { condition, .. } => {
                    return Err(self.error("P15：方法中的 while 借用重入尚未完成安全證明", condition));
                }
                StmtKind::ForRange { .. } => {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P15：方法中的 for 借用重入尚未完成安全證明".into(),
                        span: Some(self.source.span(stmt.start, stmt.end)) });
                }
                StmtKind::Break | StmtKind::Continue => {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P15：方法內跳出／繼續迭代的 self 有效期尚未驗證".into(),
                        span: Some(self.source.span(stmt.start, stmt.end)) });
                }
            }
        }
        Ok(())
    }

    /// 檢查結構欄位預設建立式是否形成循環。
    /// 未有資源執行期之前，不能把互相建立的預設值誤視為完成初始化。
    fn check_default_constructor_cycles(&self) -> Result<(), Diagnostic> {
        let mut active = HashSet::new();
        let mut checked = HashSet::new();
        for name in self.structs.keys() {
            self.check_default_type(name, &mut active, &mut checked)?;
        }
        Ok(())
    }

    fn check_default_type(&self, name: &str, active: &mut HashSet<String>,
        checked: &mut HashSet<String>) -> Result<(), Diagnostic> {
        if checked.contains(name) { return Ok(()); }
        if !active.insert(name.to_string()) {
            return Err(Diagnostic {
                kind: DiagnosticKind::Static,
                message: "結構預設建構形成循環，無法證明安全初始化".into(),
                span: None,
            });
        }
        if let Some(definition) = self.structs.get(name) {
            for field in &definition.fields {
                if let Some(default) = &field.default {
                    self.check_default_term(default, active, checked)?;
                }
            }
        }
        active.remove(name);
        checked.insert(name.to_string());
        Ok(())
    }

    fn check_default_term(&self, term: &Term, active: &mut HashSet<String>,
        checked: &mut HashSet<String>) -> Result<(), Diagnostic> {
        match &term.kind {
            TermKind::Construct(name, args) => {
                if active.contains(name) {
                    return Err(self.error("結構欄位預設建構形成循環，無法安全完成初始化", term));
                }
                // 明確指定的欄位會覆蓋其預設式；不能把已覆蓋的
                // 預設建構誤判為實際執行的循環。保留現行保守規則：
                // 先檢查型別既有預設依賴，再遍歷具名引數。
                self.check_default_type(name, active, checked)?;
                // 具名欄位實際求值時，也不得在巢狀建立引數中形成循環。
                for (_, value) in args {
                    self.check_default_term(value, active, checked)?;
                }
            }
            TermKind::Call(_, args) => {
                // 呼叫引數內的結構建立式亦屬預設值的巢狀來源，
                // 不得繞過循環建構的靜態檢查。
                for value in args {
                    self.check_default_term(value, active, checked)?;
                }
            }
            TermKind::BinaryInt { left, right, .. } => {
                self.check_default_term(left, active, checked)?;
                self.check_default_term(right, active, checked)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn all(&self, funcs: &[Function]) -> Result<(), Diagnostic> {
        // 一般呼叫以名稱查找簽章；若允許重名，呼叫端可能
        // 檢查第一份簽章而執行其他定義，破壞型別／所有權安全。
        let mut seen = HashSet::new();
        for function in funcs {
            if !seen.insert(function.name.as_str()) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式名稱重複，呼叫簽章不唯一".into(), span: None });
            }
            if function.name == "main" && !function.params.is_empty() {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：main 入口不可宣告參數".into(), span: None });
            }
        }
        // 所有函式與方法簽章須先封閉型別名稱，不能只檢查參數或
        // 等待 return 運算式碰巧揭露不存在的回傳型別。
        for function in funcs {
            if !["int", "bool", "float", "str"].contains(&function.result_ty.as_str())
                && !self.structs.contains_key(&function.result_ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式回傳型別不存在".into(), span: None });
            }
        }
        for definition in self.structs.values() {
            for method in &definition.methods {
                if !["int", "bool", "float", "str"].contains(&method.result_ty.as_str())
                    && !self.structs.contains_key(&method.result_ty) {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P13：方法回傳型別不存在".into(), span: None });
                }
            }
        }
        self.check_default_constructor_cycles()?;
        for (struct_name, definition) in self.structs.iter() {
            for field in &definition.fields {
                if !["int", "bool", "float", "str"].contains(&field.ty.as_str())
                    && !self.structs.contains_key(&field.ty) {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "結構欄位使用不存在的型別".into(),
                        span: None,
                    });
                }
                if let Some(default) = &field.default {
                    let (ty, initialized) =
                        self.evaluate(default, &mut vec![Rc::new(HashMap::new())])?;
                    if ty != field.ty {
                        return Err(self.error("結構預設欄位型別不一致", default));
                    }
                    // S07 容許 var 欄位延後初始化，但不能把「存在預設運算式」
                    // 當成嵌入的結構值已全部初始化。受限模型尚無欄位級
                    // 初始化證明時，必須拒絕不完整的巢狀預設建立式。
                    if !initialized {
                        return Err(self.error(
                            "S07：結構預設值尚含未初始化欄位，不能證明完整初始化",
                            default));
                    }
                }
                // immutable 僅保存 AST，不在此決定後續 S18 可變借用。
                let _ = field.immutable;
            }
            for method in &definition.methods {
                self.check_method(struct_name, method)?;
            }
        }
        if !funcs.iter().any(|fun| fun.name == "main" && fun.result_ty == "int") {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "受限驗證需要 fn main()->int".into(), span: None });
        }
        for fun in funcs { self.function(fun)?; }
        Ok(())
    }
}

