// 對應 P16.6-R01 無語意變更之實作拆檔。
/// A2/F02：只有這個真正 Checker 名稱決策能將 name()
/// 裁決為已核准 struct 或零參數 fn；缺名維持舊拒絕路徑。
/// G3：公開與私有仍保持原解析模式；私有最後使用點與參數整筆
/// .move() 只准消費同一詞彙已解析 DeclarationId use-def 結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedUseKind {
    All,
    WholeMove,
}

enum ZeroArgResolution<'a> {
    Structure,
    Function(&'a Function),
    Missing,
}

impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn find_function(&self, name: &str) -> Option<&Function> {
        self.function_index.get(name).and_then(|index| self.functions.get(*index))
    }
    /// 原有同名 struct 優先與函式前向宣告都只由此處判定，
    /// Parser 不得猜名稱屬於類型或函式；所有方法和一般式共用。
    fn resolve_empty_call(&self, name: &str) -> ZeroArgResolution<'_> {
        if self.structs.contains_key(name) {
            ZeroArgResolution::Structure
        } else if let Some(function) = self.find_function(name) {
            ZeroArgResolution::Function(function)
        } else {
            ZeroArgResolution::Missing
        }
    }
    /// F04：真正私有 Checker 所有函式、方法、欄位與區塊宣告
    /// 共用唯一型別名稱存在性裁決；不改任何既有錯誤優先順序。
    /// A4/F04-05：共通 int／bool 型別名稱只由 ScalarType::from_name
    /// 唯一決定；float、str、struct 專屬私有核准能力仍保留原判斷。
    fn known_type(&self, ty: &str) -> bool {
        crate::common_ir::ScalarType::from_name(ty).is_some()
            || matches!(ty, "float" | "str")
            || self.structs.contains_key(ty)
    }

    /// F04：已核准 int/bool 使用與公開 P10 唯一共同返回裁決。
    /// float、str、struct 只維持原本的精確型別字串一致性和 ready
    /// 條件；其獨立所有權、借用及回傳資源證據仍由其他安全檢查證明。
    fn checked_return_type(actual: &str, expected: &str, ready: bool) -> bool {
        match crate::common_ir::ScalarType::from_name(expected) {
            Some(scalar) => crate::common_ir::scalar_return_is_ready(
                crate::common_ir::ScalarType::from_name(actual), scalar, ready,
            ),
            None => ready && actual == expected,
        }
    }
    /// F11：真正呼叫的全部真實票據已與 Checker TypedDeclaration 核對
    /// 並完成批次歸還後，才將不含動態票據序號的借用責任交付安全計畫。
    /// 相同 AST 在分支或迴圈重訪時只能提出相同身分及權限；不覆寫
    /// 已存在的首次證據，衝突狀態不可由後續路徑自動恢復。
    fn record_verified_call_borrows(&self, term: &Term, facts: &[BorrowFact]) {
        let plan = VerifiedCallBorrowPlan {
            span: term.node_span(), facts: facts.to_vec(),
        };
        let mut recorded = self.recorded_call_borrows.borrow_mut();
        if let Some(previous) = recorded.get(&term.start) {
            if previous != &plan {
                self.recorded_call_borrow_conflicts.borrow_mut().insert(term.start);
            }
        } else {
            recorded.insert(term.start, plan);
        }
    }

    /// 只在 Checker 的真實語意與路徑驗證成功的控制轉移原位記錄，
    /// 重複造訪同一來源可接受相同事實，但禁止矛盾出口覆寫。
    fn record_checked_control_edge(
        &self, span: crate::common_ir::NodeSpan, flow: FlowState,
    ) {
        let fact = VerifiedControlEdge { span, flow };
        let mut edges = self.recorded_control_edges.borrow_mut();
        if let Some(previous) = edges.get(&span.start) {
            if previous != &fact {
                self.recorded_control_conflicts.borrow_mut().insert(span.start);
            }
        } else {
            edges.insert(span.start, fact);
        }
    }

    /// G4：保留同一 Checker 所檢查的逐路狀態選擇，而不是用一份
    /// 任意拼接的「真實執行」事件或第二本可寫所有權表替代 Scopes。
    fn record_checked_control_snapshot(
        &self, vars: &Scopes, span: crate::common_ir::NodeSpan, flow: FlowState,
    ) {
        // 純靜態狀態與後續可能的清理責任採唯一唯讀來源正規化。
        // 不簽真票據，不進行資源釋放，不為未執行分支產生動態事件。
        let snapshot = VerifiedExitSnapshot::from_checked_scopes(vars, span, flow);
        let mut recorded = self.recorded_exit_snapshots.borrow_mut();
        let alternatives = recorded.entry(span.start).or_default();
        if !alternatives.contains(&snapshot) {
            alternatives.push(snapshot);
        }
    }

    fn record_declaration(&self, id: DeclarationId, ty: &str, mutable: bool) {
        let fact = crate::common_ir::TypedDeclaration::new(
            id, ty.to_owned(), mutable,
        );
        let mut existing = self.recorded_declarations.borrow_mut();
        if let Some(earlier) = existing.get(&id) {
            // 同一 AST 宣告在不同路徑被反覆造訪可重用同一證據；
            // 如有型別／var-const 權限矛盾，保留第一次真實記錄
            // 並持久標記衝突，禁止 release 建置繞過 debug_assert。
            if earlier != &fact {
                self.recorded_declaration_conflicts.borrow_mut().insert(id);
            }
        } else {
            existing.insert(id, fact);
        }
    }
    /// F01/F04：這是由真正 Checker 已確認型別名稱的宣告
    /// 交付的來源註記，和 Binding/借用來源表完全隔離。
    fn record_checked_member(
        &self, role: crate::common_ir::NodeRole,
        declaration_span: crate::common_ir::NodeSpan,
        name: &str, name_span: crate::common_ir::NodeSpan,
        ty: &str, type_span: crate::common_ir::NodeSpan,
        mutable: bool,
    ) {
        let source = match std::str::from_utf8(&self.source.bytes) {
            Ok(source) => source,
            Err(_) => {
                self.recorded_member_conflicts.borrow_mut()
                    .insert(declaration_span.start);
                return;
            }
        };
        let Some(fact) = crate::common_ir::VerifiedTypedMember::checked_private(
            source, role, declaration_span, name, name_span,
            ty, type_span, mutable,
        ) else {
            self.recorded_member_conflicts.borrow_mut()
                .insert(declaration_span.start);
            return;
        };
        let mut recorded = self.recorded_member_types.borrow_mut();
        if let Some(previous) = recorded.get(&fact.identity) {
            if previous != &fact {
                self.recorded_member_conflicts.borrow_mut()
                    .insert(declaration_span.start);
            }
        } else {
            recorded.insert(fact.identity, fact);
        }
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
        crate::common_ir::lookup_lexical(vars.iter().map(|scope| scope.as_ref()), name)
    }
    fn lookup_mut<'b>(vars: &'b mut Scopes, name: &str) -> Option<&'b mut Binding> {
        for scope in vars.iter_mut().rev() {
            if scope.contains_key(name) {
                return Rc::make_mut(scope).get_mut(name);
            }
        }
        None
    }

    /// 參數只在「真正指向此 Parameter DeclarationId」的已解析
    /// WholeMove use-def 存在時才視為消耗。與最後使用點的活性分析
    /// 共用同一份詞彙解析器，不為同名遮蔽重建第二套規則。
    fn parameter_is_consumed(function: &Function, name: &str) -> bool {
        let Some(parameter) = function.params.iter().find(|p| p.name == name) else {
            return false;
        };
        let source = DeclarationId(parameter.declaration_start);
        let mut visible: HashMap<String, DeclarationId> = function.params.iter()
            .map(|p| (p.name.clone(), DeclarationId(p.declaration_start))).collect();
        let mut consumed = BTreeSet::new();
        Self::scan_resolved_block_uses(
            &function.body, &mut visible, &mut consumed, ResolvedUseKind::WholeMove);
        consumed.contains(&source)
    }

    /// HC14-A：以實際宣告來源身分查找全部仍有效的非擁有借用。
    /// 不能只以目前作用域或表面名稱判斷，否則遮蔽可繞過來源封鎖。
    /// 合流後不許把不同可達分支的活躍借用狀態組成 HC14 不合法組合。
    /// 此處是對既有 LoanBinding 的保守驗證，尚未取代實際 BorrowTicket 身分表。
    fn active_loan_snapshot(vars: &Scopes) -> StaticBorrowSnapshot {
        StaticBorrowSnapshot::from_aliases(vars.iter()
            .flat_map(|scope| scope.values())
            .filter_map(|binding| binding.loan.as_ref().map(|loan| {
                (binding.declaration_start, loan.fact, loan.active)
            })))
    }

    fn active_loan_state_is_consistent(vars: &Scopes) -> bool {
        Self::active_loan_snapshot(vars).validate().is_ok()
    }

    /// F08/B05：先完整檢查合流候選，再一次性替換全部作用域。
    /// 檢查失敗時保留呼叫端的原狀態，沒有任何半完成的值／借用提交。
    fn commit_reachable_safety_join(
        vars: &mut Scopes, proposed: Scopes,
    ) -> Result<(), crate::borrow_safety::BorrowSafetyError> {
        Self::active_loan_snapshot(&proposed).validate()?;
        *vars = proposed;
        Ok(())
    }

    /// R03.12：宣告時直接借用、方法預設借用及一般函式預設借用
    /// 一律透過相同的 DeclarationId／活躍借用快照／HC14 衝突規則。
    /// 診斷內容仍由三種已核准的呼叫情境各自指定。
    fn check_borrow_conflicts(
        &self, vars: &Scopes, source: DeclarationId, kind: BorrowKind,
        term: &Term, diagnostic: &str,
    ) -> Result<(), Diagnostic> {
        Self::active_loan_snapshot(vars).validate_acquire(source, kind)
            .map_err(|_| self.error(diagnostic, term))
    }

    /// F04/F08：正式私有詞彙 Binding 與 Checker 成功登記的 HIR
    /// 必須對同一個 DeclarationId、型別、var/const 權限達成一致，
    /// 才能以 TypedUse 對借用或擁有權執行後續安全判斷。
    fn checked_binding_use<'b>(&self, binding: &'b Binding, term: &Term)
        -> Result<crate::common_ir::TypedUse<&'b str>, Diagnostic> {
        let use_at = binding.semantic_use_at(term.node_span());
        let valid = self.recorded_declarations.borrow()
            .get(&use_at.resolved.declaration)
            .is_some_and(|decl| decl.owns(use_at.resolved.declaration)
                && decl.ty == use_at.resolved.ty
                && decl.mutable == binding.mutable);
        if !valid {
            return Err(self.error(
                "P16：詞彙持有者與 Checker 具型別宣告身分或變更權限不一致",
                term,
            ));
        }
        Ok(use_at)
    }

    fn check_direct(&self, vars: &Scopes, name: &str, term: &Term)
        -> Result<(), Diagnostic> {
        let owner = Self::lookup(vars, name)
            .ok_or_else(|| self.error("找不到原始來源宣告", term))?;
        // 所有入口先驗證同一筆 Checker TypedDeclaration，包括非擁有
        // 借用別名。先前 loan.is_some() 的捷徑會跳過身份／型別核對，
        // 使欄位讀取、方法接收者等路徑另有一套未受驗證的權威。
        let reference = self.checked_binding_use(owner, term)?;
        if let Some(loan) = &owner.loan {
            // 僅驗證借用來源身分／型別／權限；實際最後使用點仍由
            // 既有 evaluate 與 CFG 入口查核，不藉此重新啟用失效別名。
            let source = vars.iter().rev()
                .flat_map(|scope| scope.values())
                .find(|binding| binding.declaration_start == loan.fact.source)
                .ok_or_else(|| self.error(
                    "P16：借用別名不存在對應的原始來源身分", term))?;
            let intended = if owner.mutable {
                BorrowKind::ExclusiveWrite
            } else {
                BorrowKind::SharedRead
            };
            // 來源 Binding 本身也必須由同一份 Checker 已提交的
            // TypedDeclaration 授權；不能只看一筆可以被篡改的詞彙型別。
            let verified_source = self.checked_binding_use(source, term)?;
            if verified_source.resolved.declaration != loan.fact.source
                || verified_source.resolved.ty != reference.resolved.ty
                || source.mutable != owner.mutable
                || loan.fact.kind != intended {
                return Err(self.error(
                    "P16：借用別名的來源型別或共享／獨占權限不一致", term));
            }
            // 只接受全域相容的活躍靜態借用集合，避免某條別名捷徑
            // 在其他來源已有矛盾票據時繼續放行直接存取。
            Self::active_loan_snapshot(vars).validate().map_err(|_| self.error(
                "HC14：活躍借用快照包含衝突的共享／獨占權限", term,
            ))?;
            return Ok(());
        }
        // 擁有者的讀取、寫入、複製與 move 使用相同 HC14-A 快照。
        Self::active_loan_snapshot(vars).validate_direct(reference.resolved.declaration)
            .map_err(|_| self.error(
                "HC14：來源借用仍有效，禁止直接讀寫、複製或明確移動", term))?;
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
        let reference = self.checked_binding_use(owner, term)?;
        self.check_borrow_conflicts(
            vars, reference.resolved.declaration, kind, term,
            "HC14：共享／獨占借用與現有別名衝突",
        )?;
        Ok((reference.resolved.ty.to_string(), LoanBinding {
            fact: BorrowFact::new(reference.resolved.declaration, kind), active: true,
        }))
    }

    /// 本批只對真正已解析的語句追蹤別名是否未來仍被使用。
    /// 此處寧可把有遮蔽的同名字串當成仍存活（假陽性），也不能
    /// 漏掉未來引用造成提前解鎖；完整詞彙來源精細度留待 P15 驗收。
    fn term_mentions(term: &Term, name: &str) -> bool {
        match &term.kind {
            TermKind::Group(inner) => Self::term_mentions(inner, name),
            TermKind::Name(id) | TermKind::Move(id) | TermKind::Borrow(id)
                => id == name,
            TermKind::FieldRead(id, _) | TermKind::FieldMove(id, _) => id == name,
            TermKind::MethodCall(id, _, args) => id == name
                || args.iter().any(|arg| Self::term_mentions(arg, name)),
            TermKind::Construct(_, arguments) =>
                arguments.iter().any(|(_, value)| Self::term_mentions(value, name)),
            TermKind::Call(call) =>
                call.arguments.iter().any(|value| Self::term_mentions(value, name)),
            TermKind::BinaryInt(binary) =>
                Self::term_mentions(&binary.left, name) || Self::term_mentions(&binary.right, name),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::UnresolvedZeroArgs(_) => false,
        }
    }

    /// V7／E5：直接 await 的相依表達式必須由原 AST 遞迴取得，
    /// 不以文字搜尋「await」猜測來源。用於防止同一同步呼叫
    /// 較早引數的借用跨越較後引數真正執行的等待安全點。
    fn term_contains_direct_await(term: &Term) -> bool {
        match &term.kind {
            TermKind::Call(call) => call.awaited
                || call.arguments.iter().any(Self::term_contains_direct_await),
            TermKind::Group(inner) => Self::term_contains_direct_await(inner),
            TermKind::BinaryInt(binary) =>
                Self::term_contains_direct_await(&binary.left)
                    || Self::term_contains_direct_await(&binary.right),
            TermKind::Construct(_, fields) => fields.iter()
                .any(|(_, value)| Self::term_contains_direct_await(value)),
            TermKind::MethodCall(_, _, args) =>
                args.iter().any(Self::term_contains_direct_await),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::Name(_)
            | TermKind::UnresolvedZeroArgs(_) | TermKind::Move(_)
            | TermKind::Borrow(_) | TermKind::FieldMove(_, _)
            | TermKind::FieldRead(_, _) => false,
        }
    }

    /// 真正 Lexer/Parser 提供 AST 節點，唯讀詞彙入口提供已驗證
    /// DeclarationId；兩種用途都必須用這份共同 use-def 掃描器。
    /// All 保留後繼活性，WholeMove 只認列整筆所有權消耗。
    fn record_named_future_use(
        visible: &HashMap<String, DeclarationId>,
        used: &mut BTreeSet<DeclarationId>, name: &str,
    ) {
        if let Some(id) = visible.get(name) {
            used.insert(*id);
        }
    }

    fn scan_resolved_term_uses(
        term: &Term, visible: &HashMap<String, DeclarationId>,
        used: &mut BTreeSet<DeclarationId>, kind: ResolvedUseKind,
    ) {
        match &term.kind {
            TermKind::Group(inner) =>
                Self::scan_resolved_term_uses(inner, visible, used, kind),
            TermKind::Move(name) =>
                Self::record_named_future_use(visible, used, name),
            TermKind::Name(name) | TermKind::Borrow(name)
                | TermKind::FieldRead(name, _) | TermKind::FieldMove(name, _)
                if kind == ResolvedUseKind::All =>
                Self::record_named_future_use(visible, used, name),
            TermKind::MethodCall(receiver, _, arguments) => {
                if kind == ResolvedUseKind::All {
                    Self::record_named_future_use(visible, used, receiver);
                }
                for value in arguments {
                    Self::scan_resolved_term_uses(value, visible, used, kind);
                }
            }
            TermKind::Construct(_, fields) => {
                for (_, value) in fields {
                    Self::scan_resolved_term_uses(value, visible, used, kind);
                }
            }
            TermKind::Call(call) => {
                for value in &call.arguments {
                    Self::scan_resolved_term_uses(value, visible, used, kind);
                }
            }
            TermKind::BinaryInt(binary) => {
                Self::scan_resolved_term_uses(&binary.left, visible, used, kind);
                Self::scan_resolved_term_uses(&binary.right, visible, used, kind);
            }
            TermKind::Scalar(_) | TermKind::Literal(_)
            | TermKind::UnresolvedZeroArgs(_) | TermKind::Name(_)
            | TermKind::Borrow(_) | TermKind::FieldRead(_, _)
            | TermKind::FieldMove(_, _) => {}
        }
    }

    /// F10：正式 CFG 分支合流與未來活性掃描共用同一種退出分類。
    /// 兩側只要都不再正常繼續，混合的 return／break／continue
    /// 仍屬必定離開當前序列，但其實際控制流類別不能被猜成 return。
    fn combine_non_fallthrough(left: FlowState, right: FlowState) -> FlowState {
        debug_assert!(left != FlowState::Fallthrough && right != FlowState::Fallthrough);
        if left == right { left } else { FlowState::LoopTransfer }
    }

    fn future_stmt_exit(statement: &Stmt) -> Option<FlowState> {
        match &statement.kind {
            StmtKind::Return(_) => Some(FlowState::Returned),
            StmtKind::Break => Some(FlowState::Break),
            StmtKind::Continue => Some(FlowState::Continue),
            StmtKind::If { branch, no: Some(otherwise) } => {
                let left = Self::future_block_exit(&branch.body)?;
                let right = Self::future_block_exit(otherwise)?;
                Some(Self::combine_non_fallthrough(left, right))
            }
            // 不對可能零次執行的迴圈或無 else 的條件猜測終止。
            _ => None,
        }
    }

    fn future_block_exit(body: &[Stmt]) -> Option<FlowState> {
        body.iter().find_map(Self::future_stmt_exit)
    }

    fn future_stmt_must_exit(statement: &Stmt) -> bool {
        Self::future_stmt_exit(statement).is_some()
    }

    fn future_block_must_exit(body: &[Stmt]) -> bool {
        Self::future_block_exit(body).is_some()
    }

    /// 對原 AST 的唯一詞彙 DeclarationId use-def 掃描；
    /// 只追蹤入口已可見的真來源，RHS 在宣告遮蔽前先求值。
    /// 分支／迴圈有獨立的詞彙視圖，不產生真 ResourceId／票據。
    fn scan_resolved_block_uses(
        body: &[Stmt], visible: &mut HashMap<String, DeclarationId>,
        used: &mut BTreeSet<DeclarationId>, kind: ResolvedUseKind,
    ) {
        for statement in body {
            match &statement.kind {
                StmtKind::Declare(declaration) => {
                    if let Some(term) = &declaration.initializer {
                        Self::scan_resolved_term_uses(term, visible, used, kind);
                    }
                    // 只為入口已有宣告追蹤活性；新同名局部不可能
                    // 代表原借用來源，故必須自入口可見表移除。
                    visible.remove(&declaration.name);
                }
                StmtKind::Assign(assignment) => {
                    if kind == ResolvedUseKind::All {
                        Self::record_named_future_use(visible, used, &assignment.name);
                    }
                    Self::scan_resolved_term_uses(
                        &assignment.expression, visible, used, kind);
                }
                StmtKind::FieldAssign { receiver, value, .. } => {
                    if kind == ResolvedUseKind::All {
                        Self::record_named_future_use(visible, used, receiver);
                    }
                    Self::scan_resolved_term_uses(value, visible, used, kind);
                }
                StmtKind::If { branch, no } => {
                    Self::scan_resolved_term_uses(&branch.condition, visible, used, kind);
                    Self::scan_resolved_block_uses(
                        &branch.body, &mut visible.clone(), used, kind);
                    if let Some(otherwise) = no {
                        Self::scan_resolved_block_uses(
                            otherwise, &mut visible.clone(), used, kind);
                    }
                }
                StmtKind::While { condition, body } => {
                    Self::scan_resolved_term_uses(condition, visible, used, kind);
                    Self::scan_resolved_block_uses(
                        body, &mut visible.clone(), used, kind);
                }
                StmtKind::ForRange { iterator, lower, upper, body, .. } => {
                    if Self::known_range_iterations(lower, upper) != Some(0) {
                        let mut nested = visible.clone();
                        nested.remove(iterator);
                        Self::scan_resolved_block_uses(body, &mut nested, used, kind);
                    }
                }
                StmtKind::Return(term) =>
                    Self::scan_resolved_term_uses(term, visible, used, kind),
                StmtKind::Break | StmtKind::Continue => {}
            }
            if Self::future_stmt_must_exit(statement) {
                break;
            }
        }
    }

    /// 後繼引用與參數消耗只差使用種類，不可各維護一套名稱規則。
    fn scan_future_block_uses(
        body: &[Stmt], visible: &mut HashMap<String, DeclarationId>,
        used: &mut BTreeSet<DeclarationId>,
    ) {
        Self::scan_resolved_block_uses(body, visible, used, ResolvedUseKind::All);
    }

    fn visible_future_sources(vars: &Scopes, depth: usize)
        -> HashMap<String, DeclarationId> {
        let mut result = HashMap::new();
        for scope in vars.iter().take(depth + 1) {
            for (name, binding) in scope.iter() {
                result.insert(name.clone(), binding.declaration_start);
            }
        }
        result
    }

    /// F07：每個詞法後繼只解析一次所有受影響的 DeclarationId；
    /// 取代每個借用別名對每個後繼重掃 AST 名稱／遮蔽位置的重複權威。
    fn release_last_uses(vars: &mut Scopes, rest: &[Stmt],
        continuation: &[(usize, Vec<Stmt>)]) {
        if !vars.iter().any(|scope| scope.values().any(|binding| {
            binding.loan.as_ref().is_some_and(|loan|
                loan.active && loan.fact.source != binding.declaration_start)
        })) {
            return;
        }
        // F07/B06：借用活性是唯一 SafetyState 的一部分。
        // 原始全域快照若已不一致，不可透過局部提早歸還把錯誤
        // 狀態「修好」，使後續 Checker 錯把非法狀態視為已驗證。
        if Self::active_loan_snapshot(vars).validate().is_err() {
            return;
        }
        let mut needed_here = BTreeSet::new();
        Self::scan_future_block_uses(rest,
            &mut Self::visible_future_sources(vars, vars.len() - 1),
            &mut needed_here);
        let needed_later: Vec<(usize, BTreeSet<DeclarationId>)> =
            continuation.iter().map(|(depth, following)| {
                let mut used = BTreeSet::new();
                Self::scan_future_block_uses(following,
                    &mut Self::visible_future_sources(vars, *depth),
                    &mut used);
                (*depth, used)
            }).collect();

        // 所有歸還判定都在同一份不可變後繼 use-def 集合上進行。
        // 只修改 COW 候選，完整核對活躍借用後才整筆提交。
        let mut proposed = vars.clone();
        for (depth, scope) in proposed.iter_mut().enumerate() {
            let has_releasable = scope.values().any(|binding|
                binding.loan.as_ref().is_some_and(|loan|
                    loan.active && loan.fact.source != binding.declaration_start));
            if !has_releasable {
                continue;
            }
            for binding in Rc::make_mut(scope).values_mut() {
                let declaration_start = binding.declaration_start;
                if let Some(loan) = binding.loan.as_mut() {
                    // 參數及 self 的原有效期仍須由其函式出口負責。
                    let parameter_scope_loan =
                        loan.fact.source == declaration_start;
                    let future_use = needed_here.contains(&declaration_start)
                        || needed_later.iter().any(|(at_depth, used)|
                            *at_depth >= depth
                                && used.contains(&declaration_start));
                    if loan.active && !parameter_scope_loan && !future_use {
                        // 是否仍合法結束借用由唯一 SafetyState 轉移判定；
                        // 先停止此次局部 loan 借用，再提交原有 Binding。
                        let _ = loan;
                        binding.expire_alias_checked();
                    }
                }
            }
        }
        // 靜態有效期歸還不是執行期真票據操作；這裡不觸碰 tracker。
        if Self::active_loan_snapshot(&proposed).validate().is_ok() {
            *vars = proposed;
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
            TermKind::Group(inner) => self.method_pure_value_type(owner, method, inner),
            TermKind::Scalar(atom) => Ok(atom.type_name().to_owned()),
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
            TermKind::Call(call) => {
                let name = &call.name;
                let args = &call.arguments;
                // 一般函式可操作自己的局部來源，但方法目前僅准把
                // Copyable 的獨立引數交給它，且回傳也須已核准 Copyable。
                let function = self.find_function(name)
                    .ok_or_else(|| self.error("方法呼叫不存在的一般函式", term))?;
                if call.awaited {
                    return Err(self.error(
                        "V7：同步 self 方法本體的 await 尚無完整借用／工作框架證據", term,
                    ));
                }
                if function.is_async {
                    return Err(self.error("V7：async 普通函式不可當作同步方法值", term));
                }
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
            TermKind::UnresolvedZeroArgs(name) => {
                // 方法也使用單一完整符號決議；同名 struct 仍不可冒充
                // 可獨立求值的零參數函式回傳值。
                let function = match self.resolve_empty_call(name) {
                    ZeroArgResolution::Function(function) => function,
                    ZeroArgResolution::Structure => return Err(self.error(
                        "方法值來源尚未證明可安全獨立求值", term)),
                    ZeroArgResolution::Missing => return Err(self.error(
                        "方法內零參數函式不存在", term)),
                };
                if function.is_async {
                    return Err(self.error("V7：未經 await 的 async 零參數函式不可求值", term));
                }
                if !function.params.is_empty() {
                    return Err(self.error("方法內零參數函式實際需傳入引數", term));
                }
                let permission = self.permission(&function.result_ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission, "方法內函式回傳值", term));
                }
                Ok(function.result_ty.clone())
            }
            TermKind::BinaryInt(binary) => {
                let left = &binary.left;
                let right = &binary.right;
                let operator = &binary.operator;
                let left_ty = self.method_pure_value_type(owner, method, left)?;
                // 方法專用安全 Checker 和一般 Checker 使用 F04 同一份
                // 已核准 int/初始化判定，方法 Copyable/self/逃逸規則仍
                // 已先由 method_pure_value_type 獨立完成。
                if !crate::common_ir::arithmetic_operand_is_ready(
                    crate::common_ir::ScalarType::from_name(&left_ty), true,
                ) {
                    return Err(self.error("方法整數加減左運算元必須為 int", left));
                }
                let right_ty = self.method_pure_value_type(owner, method, right)?;
                if !crate::common_ir::arithmetic_operand_is_ready(
                    crate::common_ir::ScalarType::from_name(&right_ty), true,
                ) {
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
        if !crate::common_ir::boolean_condition_is_ready(
            crate::common_ir::ScalarType::from_name(&ty),true,
        ) {
            return Err(self.error("方法 if 條件必須為可複製的 bool", term));
        }
        Ok(())
    }

    /// 方法可將合法具名參數的整筆持有權在 return 時明確移交。
    /// 不推定 self 欄位的借用回傳或隱含複製；本階段方法在
    /// 提前／最後 return 前只能使用已驗證的純 Copyable 語句。
    fn method_return_type(&self, owner: &str, method: &Function, term: &Term)
        -> Result<String, Diagnostic> {
        if let TermKind::Group(inner) = &term.kind {
            return self.method_return_type(owner, method, inner);
        }
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
            TermKind::Call(call) => Some((call.name.as_str(), call.arguments.as_slice())),
            TermKind::UnresolvedZeroArgs(name)
                if !matches!(self.resolve_empty_call(name), ZeroArgResolution::Structure) =>
                Some((name.as_str(), &[])),
            _ => None,
        };
        if let Some((name, args)) = ordinary_call {
            if let Some(function) = self.find_function(name) {
                if function.is_async {
                    return Err(self.error("V7：async 函式不可作同步方法資源回傳", term));
                }
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

    /// B4／M3：僅在方法真 Return 出口，為同一原 Checker 已核准的
    /// 無副作用純 int helper 生成原 ScalarHIR。這不是另一份型別決議：
    /// 所有形參、Call、Lexer 和 checked expression 都經原 evaluate，
    /// 真方法 int 形參只可由原 Parser DeclarationId／Checker Scopes
    /// 讀取；bool／str、self、未初始化局部及資源仍不可偷渡。
    fn method_branch_checked_pure_helper_return(
        &self, method: &Function, term: &Term,
    ) -> Result<(), Diagnostic> {
        fn approved_int_tree(term: &Term, method: &Function) -> bool {
            match &term.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(_),
                ) => true,
                TermKind::Name(name) => method.params.iter().any(|p|
                    p.ty == "int" && p.name == *name && p.name != "self"),
                TermKind::Group(inner) => approved_int_tree(inner, method),
                TermKind::BinaryInt(binary) =>
                    matches!(binary.operator, '+' | '-')
                        && approved_int_tree(&binary.left, method)
                        && approved_int_tree(&binary.right, method),
                _ => false,
            }
        }
        fn pure_helper_body(term: &Term, parameters: &[&str]) -> bool {
            match &term.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(_),
                ) => true,
                TermKind::Name(name) => parameters.contains(&name.as_str()),
                TermKind::Group(inner) => pure_helper_body(inner, parameters),
                TermKind::BinaryInt(binary) =>
                    matches!(binary.operator, '+' | '-')
                        && pure_helper_body(&binary.left, parameters)
                        && pure_helper_body(&binary.right, parameters),
                _ => false,
            }
        }
        // F02 正式 Parser：零參數 name() 先保持未決節點，必須
        // 經原 Checker ZeroArgResolution 唯一決定普通函式，
        // 不得用字串名稱猜測或讓同名 struct 冒充 helper。
        let (name, args, awaited): (&str, &[Term], bool) = match &term.kind {
            TermKind::Call(call) => (
                call.name.as_str(), call.arguments.as_slice(), call.awaited,
            ),
            TermKind::UnresolvedZeroArgs(name)
                if matches!(self.resolve_empty_call(name),
                    ZeroArgResolution::Function(_)) =>
                (name.as_str(), &[], false),
            _ => return Ok(()),
        };
        let Some(helper) = self.find_function(name) else { return Ok(()); };
        if awaited || helper.is_async || helper.name == "main"
            || helper.result_ty != "int" || helper.params.len() != args.len()
            || helper.body.statements.len() != 1
            || helper.params.iter().any(|p| p.ty != "int" || p.name == "self")
            || helper.params.iter().enumerate().any(|(i, p)|
                helper.params[..i].iter().any(|q| q.name == p.name))
            || !args.iter().all(|arg| approved_int_tree(arg, method)) {
            return Ok(());
        }
        let names = helper.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>();
        let StmtKind::Return(returned) = &helper.body.statements[0].kind else {
            return Ok(());
        };
        if !pure_helper_body(&returned.expression, &names) {
            return Ok(());
        }
        // 與 method_checked_int_locals 共用**同一**原 Checker
        // Scopes／Binding／DeclarationId 構建契約，讓方法引數的
        // HIR 使用真正形參身分，而非按名稱或 prepare 的預知值造假。
        // bool 只作獨立條件框架，不能當成 int 實參。
        let mut vars = vec![Rc::new(HashMap::new())];
        for parameter in &method.params {
            if parameter.ty != "int" { continue; }
            if parameter.name == "self"
                || vars[0].contains_key(&parameter.name) {
                return Err(self.error(
                    "純 helper int 實參的原方法形參重複或非法", term,
                ));
            }
            let id = DeclarationId(parameter.declaration_start);
            Rc::make_mut(&mut vars[0]).insert(parameter.name.clone(),
                Binding::new(
                    id, "int".to_string(), false,
                    ValueSlot::initialized(CopyPermission::Allowed), None, None,
                ));
            self.record_declaration(id, "int", false);
        }
        let (ty, ready) = self.evaluate(term, &mut vars)?;
        if ty != "int" || !ready {
            return Err(self.error(
                "M3 提前 return 的純 int helper 尚未完成 Checker 型別／可用性證明",
                term,
            ));
        }
        Ok(())
    }

    /// 以所有可能執行路徑的操作推導方法整體借用模式；
    /// 任一路徑寫入都需要在方法入口取得獨占借用。
    fn method_writes(&self, owner: &str, method: &Function, body: &[Stmt],
        checked_returns: &mut BTreeSet<crate::common_ir::NodeSpan>,
    ) -> Result<bool, Diagnostic> {
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
                    if !Self::checked_return_type(&actual, &method.result_ty, true) {
                        return Err(self.error("方法提前回傳型別與簽章不一致", value));
                    }
                    // 僅在已核准提前 Return 葉的純 literal-int helper
                    // 產生原 Checker ScalarHIR；未選執行葉仍不在此求值。
                    self.method_branch_checked_pure_helper_return(method, &value.expression)?;
                    // 真正核對返回類型與自借用責任後才認領此靜態
                    // 控制點；後續同一路徑不可達，不會憑 AST 假補。
                    checked_returns.insert(statement.node_span());
                    return Ok(writes);
                }
                StmtKind::If { branch, no } => {
                    let condition = &branch.condition;
                    let yes = &branch.body;
                    self.method_bool_condition(owner, method, condition)?;
                    let yes_writes = self.method_writes(
                        owner, method, yes, checked_returns)?;
                    let no_writes = if let Some(otherwise) = no {
                        self.method_writes(owner, method, otherwise, checked_returns)?
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

    /// 方法內純值局部的最小安全證明：借用 self 始終只讀，
    /// 不含任何可移動／可借用資源、不設控制分支或巢狀函式，
    /// 將原參數與合法局部置入**同一 Checker Scopes／SafetyState**
    /// 後使用 evaluate() 的正式 ScalarHIR 記錄，而非從 AST
    /// 外形直接賦予方法來源執行權限。
    fn method_checked_int_locals(
        &self, method: &Function,
        returned: &crate::syntax_shared::ParsedReturn<Term>,
    ) -> Result<(), Diagnostic> {
        // B4：原 Checker 唯一普通函式索引可證的同步純 int helper，
        // 依正式簽章接任意數量 int 形參、單 Return、純加減；名稱只
        // 能是該唯一形參。任意 Borrow／Move／Call／Self 欄位均拒絕。
        // 每個呼叫及其引數仍由同一 Checker evaluate 真正驗證 TypedHIR。
        fn pure_helper_int_tree(term: &Term, parameters: &[&str]) -> bool {
            match &term.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(_),
                ) => true,
                TermKind::Name(name) => parameters.contains(&name.as_str()),
                TermKind::Group(inner) => pure_helper_int_tree(inner, parameters),
                TermKind::BinaryInt(binary) =>
                    matches!(binary.operator, '+' | '-')
                        && pure_helper_int_tree(&binary.left, parameters)
                        && pure_helper_int_tree(&binary.right, parameters),
                _ => false,
            }
        }
        let helper_arity = |name: &str| self.find_function(name).and_then(|helper| {
            if helper.is_async || helper.name == "main"
                || helper.result_ty != "int" || helper.body.statements.len() != 1 {
                return None;
            }
            // 原 Checker 已驗證一般函式參數，但 B4 再拒絕未核准
            // 的型別、遮蔽與重複名稱。順序是正式 Parser 形參順序。
            if helper.params.iter().any(|param|
                param.ty != "int" || param.name == "self")
                || helper.params.iter().enumerate().any(|(index, param)|
                    helper.params[..index].iter().any(|earlier|
                        earlier.name == param.name)) {
                return None;
            }
            let parameters = helper.params.iter().map(|param|
                param.name.as_str()
            ).collect::<Vec<_>>();
            match &helper.body.statements[0].kind {
                StmtKind::Return(returned)
                    if pure_helper_int_tree(&returned.expression, &parameters) =>
                    Some(helper.params.len()),
                _ => None,
            }
        });
        fn pure_int(
            term: &Term, helper_arity: &dyn Fn(&str) -> Option<usize>,
            allow_direct_helper: bool,
        ) -> bool {
            match &term.kind {
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                | TermKind::Name(_) => true,
                TermKind::Group(inner) =>
                    pure_int(inner, helper_arity, allow_direct_helper),
                TermKind::BinaryInt(binary) =>
                    matches!(binary.operator, '+' | '-')
                    && pure_int(&binary.left, helper_arity, false)
                    && pure_int(&binary.right, helper_arity, false),
                TermKind::UnresolvedZeroArgs(name) if allow_direct_helper =>
                    helper_arity(name) == Some(0),
                TermKind::Call(call) if allow_direct_helper =>
                    !call.awaited && helper_arity(&call.name) == Some(call.arguments.len())
                        && call.arguments.iter().all(|argument|
                            pure_int(argument, helper_arity, false)),
                _ => false,
            }
        }
        let mut vars = vec![Rc::new(HashMap::new())];
        for parameter in &method.params {
            if parameter.ty != "int" || parameter.name == "self"
                || vars[0].contains_key(&parameter.name) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "方法 int 局部子集需要不重複的原 int 形式參數".into(),
                    span: Some(self.source.span(method.start, method.end)),
                });
            }
            let id = DeclarationId(parameter.declaration_start);
            Rc::make_mut(&mut vars[0]).insert(parameter.name.clone(),
                Binding::new(
                    id, "int".to_string(), false,
                    ValueSlot::initialized(CopyPermission::Allowed), None, None,
                ));
            self.record_declaration(id, "int", false);
        }
        let mut after_assignment = false;
        for statement in method.body.iter().take(method.body.len() - 1) {
            match &statement.kind {
                StmtKind::Declare(local) => {
                    // 這一批只接通「宣告群→重賦值群→return」的
                    // 直線來源順序，避免用整段重排偷改執行期語意。
                    if after_assignment || local.ty != "int" || local.name == "self"
                        || vars[0].contains_key(&local.name) {
                        return Err(Diagnostic {
                            kind: DiagnosticKind::Static,
                            message: "方法 int 局部只接受先宣告後賦值的唯一 Copyable int 來源".into(),
                            span: Some(self.source.span(statement.start, statement.end)),
                        });
                    }
                    let init = local.initializer.as_ref().ok_or(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "方法 int 局部必須有已核准的初始化來源".into(),
                        span: Some(self.source.span(statement.start, statement.end)),
                    })?;
                    if !pure_int(init, &helper_arity, true) {
                        return Err(self.error(
                            "方法 int 局部不得含未驗證的 Borrow／Move／函式呼叫／self 欄位",
                            init,
                        ));
                    }
                    let (ty, ready) = self.evaluate(init, &mut vars)?;
                    if ty != "int" || !ready {
                        return Err(self.error(
                            "方法 int 局部的初始化未完成原 Checker int 安全驗證",
                            init,
                        ));
                    }
                    let id = DeclarationId(statement.start);
                    Rc::make_mut(&mut vars[0]).insert(local.name.clone(),
                        Binding::new(id, "int".to_string(), local.mutable,
                            ValueSlot::initialized(CopyPermission::Allowed), None, None));
                    self.record_declaration(id, "int", local.mutable);
                }
                StmtKind::Assign(assignment) => {
                    after_assignment = true;
                    let target = Self::lookup(&vars, &assignment.name).ok_or(
                        self.error("方法賦值目標不存在", &assignment.expression),
                    )?;
                    // 參數的 var 權限永遠是 false；const／借用別名也
                    // 不因這條純值局部執行器而取得可寫權。
                    if !target.mutable || target.ty != "int"
                        || target.loan.is_some()
                        || target.value.state() != ValueState::Available
                        // 與已核准的局部初始化相同：只有完整 RHS
                        // 直接呼叫原 Checker 已證的零參數純 int helper 可通過。
                        // BinaryInt 子樹仍由 pure_int(..., false) 禁止嵌入呼叫。
                        || !pure_int(&assignment.expression, &helper_arity, true) {
                        return Err(self.error(
                            "方法局部賦值僅接受已初始化的 var int 與純整數算式",
                            &assignment.expression,
                        ));
                    }
                    let id = target.declaration_start;
                    let (actual, ready) = self.evaluate(&assignment.expression, &mut vars)?;
                    if actual != "int" || !ready {
                        return Err(self.error(
                            "方法 var int 重賦值缺少原 Checker 型別及初始化證明",
                            &assignment.expression,
                        ));
                    }
                    self.recorded_scalar_hir.borrow_mut().record_assignment(
                        crate::common_ir::TypedUse::new(
                            statement.node_span(),
                            crate::common_ir::ResolvedUse::new(
                                id, crate::common_ir::ScalarType::Int,
                            ),
                        ),
                    );
                }
                _ => return Err(Diagnostic {
                    kind: DiagnosticKind::Static,
                    message: "方法純 int 子集仍拒絕未驗證控制流程或欄位移動".into(),
                    span: Some(self.source.span(statement.start, statement.end)),
                }),
            }
        }
        // B4／M3：原已核准純 int helper 現可位於最後的直接
        // Return 退出邊；BinaryInt 子樹仍禁止 Call，必須真正到達
        // 退出邊時才動態呼叫、先求參數後進普通函式框架。
        if !pure_int(&returned.expression, &helper_arity, true) {
            return Err(self.error(
                "方法局部返回僅接通原 Checker 的純 int 算術或直接 helper",
                &returned.expression,
            ));
        }
        let (ty, ready) = self.evaluate(&returned.expression, &mut vars)?;
        if ty != "int" || !ready || method.result_ty != "int" {
            return Err(self.error(
                "方法局部返回未完成 int 型別與來源有效期驗證",
                &returned.expression,
            ));
        }
        Ok(())
    }

    /// G4 原有唯一寫入權威：所有方法簽章與來源安全證據
    /// 都必須由此一處完成 VerifiedMethodSafety 的衝突比對。
    /// R04 純 int 局部不可另開一份可寫來源。
    fn record_checked_method_safety(
        &self, method: &Function, mode: BorrowKind,
        checked_returns: BTreeSet<crate::common_ir::NodeSpan>,
    ) {
        let fact = VerifiedMethodSafety {
            span: crate::common_ir::NodeSpan::new(method.start, method.end),
            self_borrow: mode, checked_returns,
        };
        let mut evidence = self.recorded_method_safety.borrow_mut();
        if let Some(previous) = evidence.get(&method.start) {
            if previous != &fact {
                self.recorded_method_safety_conflicts.borrow_mut()
                    .insert(method.start);
            }
        } else {
            evidence.insert(method.start, fact);
        }
    }

    fn check_method(&self, owner: &str, method: &Function)
        -> Result<BorrowKind, Diagnostic> {
        for parameter in &method.params {
            if parameter.name == "self" {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "S15：方法不得宣告遮蔽 self 的參數".into(), span: None });
            }
            if !self.known_type(&parameter.ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：方法參數使用不存在的型別".into(), span: None });
            }
            self.record_checked_member(
                crate::common_ir::NodeRole::Parameter,
                parameter.declaration_span,
                &parameter.name, parameter.name_span,
                &parameter.ty, parameter.type_span, false,
            );
        }
        self.reject_method_statements(&method.body)?;
        let Some(Stmt { kind: StmtKind::Return(return_value), .. }) = method.body.last() else {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "目前方法必須以單一明確 return 結束".into(), span: None });
        };
        // R04：原方法內出現已解析局部宣告時，只採用
        // 純 Copyable int 線性子集。一般方法仍維持原本更嚴格的
        // method_writes／method_return_type 與自借用安全界線。
        if method.body.iter().take(method.body.len() - 1).any(|stmt|
            matches!(&stmt.kind, StmtKind::Declare(_))) {
            self.method_checked_int_locals(method, return_value)?;
            let mut checked_returns = BTreeSet::new();
            let last_span = method.body.last().expect(
                "已驗證方法最後有真 return",
            ).node_span();
            checked_returns.insert(last_span);
            self.record_checked_method_safety(
                method, BorrowKind::SharedRead, checked_returns,
            );
            return Ok(BorrowKind::SharedRead);
        }
        let mut checked_returns = BTreeSet::new();
        let mode = if self.method_writes(owner, method,
            &method.body[..method.body.len() - 1], &mut checked_returns)? {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        let actual = self.method_return_type(owner, method, return_value)?;
        if !Self::checked_return_type(&actual, &method.result_ty, true) {
            return Err(self.error("方法回傳型別與已驗證運算式不一致", return_value));
        }
        // B4／M3：沒有 Copyable int 局部的最後直接 Return，
        // 與提前返回葉一樣只在原 Checker 已核准的純
        // literal-int helper 上生成原 ScalarHIR，不另設型別器。
        self.method_branch_checked_pure_helper_return(method, &return_value.expression)?;
        checked_returns.insert(method.body.last()
            .expect("已驗證方法最後必有 return").node_span());
        self.record_checked_method_safety(method, mode, checked_returns);
        Ok(mode)
    }

    /// D18-02-A：只為「選擇多個同名方法候選」取得靜態型別，
    /// 不執行 .move()、讀取資源、簽發票據或改變任何 Scopes。
    /// 無法唯一判定的表達式必拒絕，不能退回舊的最後一個同名方法。
    fn method_argument_type_without_effect(
        &self, term: &Term, vars: &Scopes,
    ) -> Option<String> {
        match &term.kind {
            TermKind::Scalar(literal) => Some(literal.type_name().to_owned()),
            TermKind::Literal(ty) => Some((*ty).to_owned()),
            TermKind::Group(inner) =>
                self.method_argument_type_without_effect(inner, vars),
            TermKind::Name(name) | TermKind::Move(name) =>
                Self::lookup(vars, name).map(|binding| binding.ty.clone()),
            TermKind::BinaryInt(binary) => {
                let left = self.method_argument_type_without_effect(&binary.left, vars)?;
                let right = self.method_argument_type_without_effect(&binary.right, vars)?;
                (left == "int" && right == "int").then(|| "int".to_owned())
            }
            TermKind::FieldRead(receiver, field) => {
                let source = Self::lookup(vars, receiver)?;
                let definition = self.structs.get(&source.ty)?;
                definition.field(field).map(|found| found.ty.clone())
            }
            _ => None,
        }
    }

    /// 方法引數依來源順序在取得 self 借用前求值。
    /// 已核准 Copyable 參數可接受具名整筆 .move()；求值後必須
    /// 再查核接收者仍有效。仍禁止未完成的再借用／巢狀呼叫資源副作用。
    fn method_argument_safe_before_borrow(term: &Term) -> bool {
        match &term.kind {
            TermKind::Group(inner) =>
                Self::method_argument_safe_before_borrow(inner),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::Name(_) | TermKind::Move(_)
                | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt(binary) =>
                Self::method_argument_safe_before_borrow(&binary.left)
                    && Self::method_argument_safe_before_borrow(&binary.right),
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
            TermKind::Group(inner) => Self::while_condition_nonconsuming(inner),
            TermKind::Scalar(_) | TermKind::Literal(_) | TermKind::Name(_) | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt(binary) =>
                Self::while_condition_nonconsuming(&binary.left)
                    && Self::while_condition_nonconsuming(&binary.right),
            _ => false,
        }
    }

    /// 正式私有求值成功後，直接以與公開 P10 相同的 HIR 型別
    /// 記錄原運算式的來源半開跨度；失敗不交付任何新的合法證據。
    fn evaluate(&self, term: &Term, vars: &mut Scopes)
        -> Result<(String, bool), Diagnostic> {
        let result = self.evaluate_inner(term, vars)?;
        if result.1 {
            if let Some(ty) = crate::common_ir::ScalarType::from_name(&result.0) {
                self.recorded_scalar_hir.borrow_mut()
                    .record_expression(term.node_span(), ty);
            }
        }
        Ok(result)
    }

    fn evaluate_inner(&self, term: &Term, vars: &mut Scopes)
        -> Result<(String, bool), Diagnostic> {
        match &term.kind {
            TermKind::Group(inner) => self.evaluate(inner, vars),
            TermKind::Scalar(atom) => Ok((atom.type_name().to_owned(), true)),
            TermKind::Literal(ty) => Ok(((*ty).to_string(), true)),
            TermKind::BinaryInt(binary) => {
                let left = &binary.left;
                let right = &binary.right;
                let operator = &binary.operator;
                // 順序由 P01 左結合決定，不得跳過內層 .move() 狀態更新。
                let (left_ty, left_ready) = self.evaluate(left, vars)?;
                if !crate::common_ir::arithmetic_operand_is_ready(
                    crate::common_ir::ScalarType::from_name(&left_ty), left_ready,
                ) {
                    return Err(self.error("P13：加減左運算元必須為已初始化 int", left));
                }
                let (right_ty, right_ready) = self.evaluate(right, vars)?;
                if !crate::common_ir::arithmetic_operand_is_ready(
                    crate::common_ir::ScalarType::from_name(&right_ty), right_ready,
                ) {
                    return Err(self.error("P13：加減右運算元必須為已初始化 int", right));
                }
                debug_assert!(*operator == '+' || *operator == '-');
                self.recorded_scalar_hir.borrow_mut().record_arithmetic(
                    crate::common_ir::VerifiedScalarArithmetic {
                        span: term.node_span(),
                        left: left.node_span(),
                        right: right.node_span(),
                        operator: *operator,
                    });
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
                let reference = self.checked_binding_use(binding, term)?;
                if let Some(scalar) = crate::common_ir::ScalarType::from_name(
                    reference.resolved.ty
                ) {
                    self.recorded_scalar_hir.borrow_mut().record_reference(
                        crate::common_ir::TypedUse::new(
                            term.node_span(),
                            crate::common_ir::ResolvedUse::new(
                                reference.resolved.declaration, scalar,
                            ),
                        ),
                    );
                }
                Ok((reference.resolved.ty.to_owned(), true))
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
                // 必須在移動改變來源狀態之前固定已解析型別及宣告。
                let verified_type = self.checked_binding_use(binding, term)?
                    .resolved.ty.to_owned();
                binding.move_whole().map_err(|err| self.model_error(err, term))?;
                Ok((verified_type, true))
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
                let definition = self.structs.get(&owner_type)
                    .ok_or_else(|| self.error("找不到結構上的合法方法", term))?;
                let candidates: Vec<&Function> = definition.methods_named(name).collect();
                let method = match candidates.as_slice() {
                    [] => return Err(self.error("找不到結構上的合法方法", term)),
                    [only] => *only,
                    // S20／D18-02-A：原 Parser 允許同名不同參數簽章，
                    // 但同名多載只能由靜態型別與順序找出唯一候選。
                    // 不依回傳型別區分，也不在執行期依值搜尋。
                    _ => {
                        let static_types = args.iter().map(|arg|
                            self.method_argument_type_without_effect(arg, vars)
                        ).collect::<Option<Vec<_>>>()
                            .ok_or_else(|| self.error(
                                "S20：方法多載引數缺少可安全判定的靜態型別", term,
                            ))?;
                        let matching: Vec<&Function> = candidates.iter().copied()
                            .filter(|method| {
                                method.params.len() == static_types.len()
                                    && method.params.iter().zip(&static_types)
                                        .all(|(param, ty)| param.ty == *ty)
                            }).collect();
                        match matching.as_slice() {
                            [unique] => *unique,
                            [] => return Err(self.error(
                                "S20：找不到參數簽章完全相符的同名方法", term,
                            )),
                            _ => return Err(self.error(
                                "S20：方法多載簽章無法唯一解析", term,
                            )),
                        }
                    }
                };
                // E1-A：未經直接 await 的同步表達式不得執行
                // async 方法本體或提前取得其回傳值；後續專用
                // await 來源另行接通，不繞過既有 BorrowFact 安全。
                if method.is_async {
                    return Err(self.error(
                        "V7：async 方法需要已核准的直接 await 執行路徑", term,
                    ));
                }
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
                // self 與預設借用參數在同一次方法呼叫必須共享
                // 一個真實 tracker，靜態身分由 Checker 詞彙 HIR 產生。
                let mut argument_loans = BorrowSafety::new();
                let mut argument_tickets = Vec::new();
                let mut argument_facts = Vec::new();
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
                        let source = self.checked_binding_use(original, arg)?
                            .resolved.declaration;
                        self.check_borrow_conflicts(
                            vars, source, kind, arg,
                            "HC14：方法參數與既有借用衝突",
                        )?;
                        let ticket = argument_loans.acquire(
                            source, kind)
                            .map_err(|_| self.error(
                                "HC14：同一方法呼叫的參數借用衝突", arg))?;
                        argument_tickets.push(ticket);
                        argument_facts.push(BorrowFact::new(source, kind));
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
                        && loan.fact.kind != BorrowKind::ExclusiveWrite {
                        return Err(self.error("HC14：唯讀借用不可呼叫可寫方法", term));
                    }
                } else {
                    let source = self.checked_binding_use(bound, term)?
                        .resolved.declaration;
                    let loan = argument_loans.acquire(source, mode)
                        .map_err(|_| self.error("方法 self 借用與現有借用衝突", term))?;
                    argument_tickets.push(loan);
                    argument_facts.push(BorrowFact::new(source, mode));
                    if argument_loans.check_direct_access(source).is_ok() {
                        return Err(self.error("HC14：方法期間不得直接使用被借用來源", term));
                    }
                }
                // 以 Checker 真正已解析的詞彙來源與同一 tracker 所簽發
                // 的 self／參數票據嚴格核對後原子釋放；既有借用別名
                // 屬外層有效期，不在此次方法呼叫額外簽發虛構票據。
                argument_loans.finish_scope_verified(&argument_tickets, &argument_facts)
                    .map_err(|_| self.error("P15.6：方法參數作用域歸還失敗", term))?;
                if bound.loan.is_none() {
                    argument_loans.check_direct_access(bound.declaration_start)
                        .map_err(|_| self.error("方法後來源借用未解除", term))?;
                }
                self.record_verified_call_borrows(term, &argument_facts);
                Ok((method.result_ty.clone(), true))
            }
            TermKind::FieldMove(_owner, _field) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：結構欄位不能單獨移動", term))
            }
            TermKind::Call(call) => {
                let name = &call.name;
                let args = &call.arguments;
                let function = self.find_function(name)
                    .ok_or_else(|| self.error("P13：呼叫不存在的函式", term))?;
                if function.is_async && !call.awaited {
                    return Err(self.error(
                        "V7：async 函式不得以同步呼叫冒稱已執行，須有直接 await", term,
                    ));
                }
                if call.awaited && !function.is_async {
                    return Err(self.error(
                        "V7：只有原 Checker 已核准的 async fn 可被直接 await", term,
                    ));
                }
                if function.params.len() != args.len() {
                    return Err(self.error("P13：函式引數數量不符", term));
                }
                // VB15-01-A／V7 E5：以原 Checker 詞彙 SafetyState
                // 的活躍 BorrowFact 為唯一權威。已於最後使用點
                // 正式失效的別名不在快照內，不能阻止合法 await。
                // 若仍有任何有效借用，或快照本身不一致，等待前
                // 必須保守拒絕，不讓真執行器跨越此安全邊界。
                if call.awaited {
                    let live = Self::active_loan_snapshot(vars);
                    if live.validate().is_err() || !live.facts().is_empty() {
                        return Err(self.error(
                            "VB15-01-A：有效借用不得跨直接 await 安全邊界", term,
                        ));
                    }
                }
                // 原非 Copyable 具名實參按 P15.6 建立預設借用：
                // 此票據在子函式退出前始終活躍，不可於直接 await
                // 啟動時借出，亦不可先借給同步函式較早的實參、
                // 再在後續實參的表達式中進入任一直接 await。
                // 檢查在所有引數求值／移動之前完成，拒絕不造成
                // 任何部分已提交的來源移動或票據變更。
                let borrowed_argument = |index: usize| {
                    let arg = &args[index];
                    let parameter = &function.params[index];
                    self.permission(&parameter.ty) != CopyPermission::Allowed
                        && matches!(&arg.kind, TermKind::Name(_))
                        && !Self::parameter_is_consumed(function, &parameter.name)
                };
                if call.awaited && (0..args.len()).any(|i| borrowed_argument(i)) {
                    return Err(self.error(
                        "VB15-01-A：直接 await 不能持有預設借用的非 Copyable 參數",
                        term,
                    ));
                }
                if (0..args.len()).any(|i| borrowed_argument(i)
                    && args[i + 1..].iter()
                        .any(Self::term_contains_direct_await)) {
                    return Err(self.error(
                        "VB15-01-A：前序函式參數借用未歸還，不得在後續實參 await",
                        term,
                    ));
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
                                | TermKind::Construct(_, _) | TermKind::Call(_)
                                | TermKind::UnresolvedZeroArgs(_) => {}
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
                let mut call_facts = Vec::new();
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
                                let source = self.checked_binding_use(original, arg)?
                                    .resolved.declaration;
                                self.check_borrow_conflicts(
                                    vars, source, kind, arg,
                                    "HC14：函式參數借用與仍有效別名衝突",
                                )?;
                                let ticket = call_loans.acquire(
                                    source, kind)
                                    .map_err(|_| self.error(
                                        "HC14：同一呼叫內函式借用來源衝突", arg))?;
                                tickets.push(ticket);
                                call_facts.push(BorrowFact::new(source, kind));
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
                call_loans.finish_scope_verified(&tickets, &call_facts)
                    .map_err(|_| self.error("P15.6：函式退出時借用票據未完整歸還", term))?;
                self.record_verified_call_borrows(term, &call_facts);
                Ok((function.result_ty.clone(), true))
            },
            TermKind::UnresolvedZeroArgs(name) => {
                // 唯一完整符號分類：同名 struct 優先，否則只能
                // 是已宣告的零參數函式；沒有獨立可寫的 Parser 判斷。
                match self.resolve_empty_call(name) {
                    ZeroArgResolution::Structure =>
                        self.evaluate_struct_constructor(name, &[], term, vars),
                    ZeroArgResolution::Function(function) if !function.params.is_empty() =>
                        Err(self.error("P13：函式引數數量不符", term)),
                    ZeroArgResolution::Function(function) if function.is_async =>
                        Err(self.error("V7：async 零參數函式須由直接 await 驅動", term)),
                    ZeroArgResolution::Function(function) =>
                        Ok((function.result_ty.clone(), true)),
                    ZeroArgResolution::Missing =>
                        Err(self.error("找不到結構型別或不支援一般函式呼叫", term)),
                }
            }
            TermKind::Construct(name, args) =>
                self.evaluate_struct_constructor(name, args, term, vars),
        }
    }
    /// 只對 Parser 已確認的具名 struct 欄位建立式，或同一唯一
    /// 零引數分類器確認為 struct 的 name() 執行既有 VB16 求值計畫。
    /// 安全預檢、引數求值次序、預設值與 Copyable 規則不變。
    fn evaluate_struct_constructor(
        &self, name: &str, args: &[(String, Term)], term: &Term,
        vars: &mut Scopes,
    ) -> Result<(String, bool), Diagnostic> {
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
            Ok((name.to_string(), ordered.missing_mutable_fields.is_empty()))
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
            StmtKind::If { branch, no } => Self::has_current_loop_transfer(&branch.body)
                || no.as_ref().is_some_and(|other| Self::has_current_loop_transfer(other)),
            StmtKind::ForRange { .. } | StmtKind::While { .. } => false,
            _ => false,
        })
    }

    /// F10/B07：唯一可寫 SafetyState 已能以同一份來源身份
    /// 實際驗證所有外層宣告在跳轉、for 及 while 反向邊的穩定性。
    /// 此處只讀，不改任何分支／執行期資源事件，不重新建立狀態帳簿。
    fn checked_outer_state_stability(
        vars: &Scopes, entry: &Scopes,
    ) -> Result<(), OuterSafetyDiff> {
        for (depth, scope) in entry.iter().enumerate() {
            for (name, original) in scope.iter() {
                let candidate = vars.get(depth).and_then(|scope| scope.get(name))
                    .ok_or(OuterSafetyDiff::MissingBinding)?;
                if !original.same_safety_state(candidate) {
                    return Err(OuterSafetyDiff::ChangedState);
                }
            }
        }
        Ok(())
    }

    /// 保留 P15.3 已凍結的「宣告遺失」與「安全狀態漂移」
    /// 兩種精確繁體中文診斷，不因收斂共同核對器而改變優先序。
    fn check_loop_exit_stability(&self, vars: &Scopes, entry: &Scopes,
        statement: &Stmt) -> Result<(), Diagnostic> {
        Self::checked_outer_state_stability(vars, entry).map_err(|difference| {
            let message = match difference {
                OuterSafetyDiff::MissingBinding => "P15：跳轉後外層來源宣告遺失",
                OuterSafetyDiff::ChangedState if matches!(&statement.kind, StmtKind::ForRange { .. }) =>
                    "P15：單次 for 混合迴圈跳轉與正常路徑時外層所有權或借用狀態不穩定",
                OuterSafetyDiff::ChangedState =>
                    "P15：break／continue 轉移時外層所有權或借用狀態不穩定",
            };
            Diagnostic {
                kind: DiagnosticKind::Static,
                message: message.into(),
                span: Some(self.source.span(statement.start, statement.end)),
            }
        })
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
                StmtKind::Declare(declaration) => {
                    let name = &declaration.name;
                    let ty = &declaration.ty;
                    let mutable = &declaration.mutable;
                    let init = &declaration.initializer;
                    if vars.last().expect("應有詞彙作用域").contains_key(name) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "此受限子集不接受同作用域重複宣告".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    if !self.known_type(ty) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "宣告使用不存在的型別".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    // 直線函式本體的真實解析身分須在 RHS move 令來源失效前取得。
                    // 巢狀 if／for／while 一律不得記成「已執行」的直線資源移交。
                    let direct_move_source = if continuation.is_empty() && loop_entry.is_none() {
                        match init.as_ref().map(|expr| &expr.kind) {
                            Some(TermKind::Move(name)) =>
                                Self::lookup(vars, name).map(|binding| binding.declaration_start),
                            _ => None,
                        }
                    } else { None };
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
                        let struct_args: Option<(&str, &[(String, Term)])> = match
                            init.as_ref().map(|expr| &expr.kind) {
                            Some(TermKind::Construct(actual, args)) =>
                                Some((actual, args.as_slice())),
                            Some(TermKind::UnresolvedZeroArgs(actual))
                                if matches!(self.resolve_empty_call(actual),
                                    ZeroArgResolution::Structure) =>
                                Some((actual, &[])),
                            _ => None,
                        };
                        if let Some((actual, args)) = struct_args {
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
                    // 僅當同一原 Checker 已完成 borrow 權限檢查及 RHS 型別
                    // 核對時封存真正直線借用來源；不是動態票據或事件。
                    let direct_borrow_fact =
                        if continuation.is_empty() && loop_entry.is_none() {
                            loan.as_ref().map(|alias| alias.fact)
                        } else { None };
                    Rc::make_mut(vars.last_mut().expect("應有詞彙作用域")).insert(name.clone(), Binding::new(
        DeclarationId(statement.start),
        ty.clone(),
        *mutable,
        slot,
        pending_fields,
        loan,
    ));
                    self.record_declaration(DeclarationId(statement.start), ty, *mutable);
                    if let Some(fact) = direct_borrow_fact {
                        self.recorded_direct_borrows.borrow_mut()
                            .insert(DeclarationId(statement.start), fact);
                    }
                    // 僅成功完成 RHS 型別、借用與所有權驗證且寫入目標之後，
                    // 才允許 Checker 輸出此具名整筆轉移證據。
                    if let Some(source) = direct_move_source {
                        self.recorded_direct_moves.borrow_mut()
                            .insert((source, DeclarationId(statement.start)));
                    }
                }
                StmtKind::FieldAssign { receiver, field, value } => {
                    // VA16-03-A：已核准的 str 獨立常值可以先取得唯一新資源，
                    // 成功後原子覆寫具名 var 結構的 var str 欄位；
                    // 其他非 Copyable 右值仍維持靜態保守拒絕。
                    self.check_direct(vars, receiver, value)?;
                    let owner = Self::lookup(vars, receiver)
                        .ok_or_else(|| self.error("欄位寫入找不到合法接收者", value))?;
                    let field_owner_id = owner.declaration_start;
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
                        if !owner.commit_first_field(field) {
                            return Err(self.error("欄位初始化來源身分已失效", value));
                        }
                    } else {
                        owner.value.read().map_err(|err| self.model_error(err, value))?;
                    }
                    // 只在真正完整提交欄位寫入後紀錄。若同一詞彙位置位於
                    // 不確定分支／迴圈，不冒充一個已執行的直線欄位資源事件。
                    if continuation.is_empty() && loop_entry.is_none() {
                        self.recorded_field_writes.borrow_mut().insert((
                            field_owner_id, statement.start, field.clone(),
                        ));
                    }
                }
                StmtKind::Assign(assignment) => {
                    let name = &assignment.name;
                    let value = &assignment.expression;
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
                    destination.commit_rebind();
                    // 只在 RHS 型別、初始化與真正寫入全部成功後簽發
                    // P10 同一種類的 typed assignment 證據；不為非純值
                    // str/struct 資源覆寫假造 ScalarType 型別授權。
                    if let Some(scalar) = crate::common_ir::ScalarType::from_name(&ty) {
                        let declared = destination.semantic_declaration();
                        let verified = crate::common_ir::TypedUse::new(
                            statement.node_span(),
                            crate::common_ir::ResolvedUse::new(
                                declared.declaration, scalar,
                            ),
                        );
                        self.recorded_scalar_hir.borrow_mut().record_assignment(verified);
                    }
                }
                StmtKind::Return(term) => {
                    // 提前取得真正詞彙來源；evaluate(move) 將讓原來源失效。
                    // 不得僅從 return AST 形狀對外授權資源交接。
                    let moved_from = if let TermKind::Move(name) = &term.kind {
                        Self::lookup(vars, name).map(|binding| binding.declaration_start)
                    } else { None };
                    let (actual, ready) = self.evaluate(term, vars)?;
                    if !Self::checked_return_type(&actual, expected_return, ready) {
                        return Err(self.error("函式回傳型別不一致或來源未初始化", term));
                    }
                    // 靜態分支／迴圈的 return 可能可達，但不能假裝已執行。
                    // F12 專用直線資源橋接只接受沒有後繼分支與反向邊的
                    // Checker 返回證據；不能從任意分支推論唯一執行期事件。
                    if continuation.is_empty() && loop_entry.is_none() {
                        if let Some(source) = moved_from {
                            self.recorded_named_returns.borrow_mut()
                                .insert((source, term.start));
                        }
                    }
                    self.record_checked_control_edge(
                        statement.node_span(), FlowState::Returned,
                    );
                    self.record_checked_control_snapshot(
                        vars, statement.node_span(), FlowState::Returned,
                    );
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
                    let transfer = match &statement.kind {
                        StmtKind::Break => FlowState::Break,
                        StmtKind::Continue => FlowState::Continue,
                        _ => unreachable!("只可能為已核准的 break／continue"),
                    };
                    self.record_checked_control_edge(statement.node_span(), transfer);
                    self.record_checked_control_snapshot(vars, statement.node_span(), transfer);
                    return Ok(transfer);
                }
                StmtKind::ForRange { iterator, iterator_start,
                    lower, upper, body: loop_body } => {
                    // 靜態迭代宣告即使區間為零次，仍只有一個詞彙來源身分。
                    self.record_declaration(DeclarationId(*iterator_start), "int", false);
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
                        Binding::new(
        DeclarationId(*iterator_start),
        "int".into(),
        false,
        ValueSlot::initialized(CopyPermission::Allowed),
        None,
        None,
    ));
                    let mut after_loop = Vec::with_capacity(continuation.len() + 2);
                    // 已知恰好一次迭代時不存在下一次本體；
                    // 不得將假想反向邊延長借用最後使用點。
                    if !one_iteration {
                        after_loop.push((vars.len(), loop_body.statements.clone()));
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
                            FlowState::Break | FlowState::Continue
                                | FlowState::LoopTransfer => *vars = snapshot,
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
                        if Self::checked_outer_state_stability(&simulated, &snapshot).is_err() {
                            return Err(Diagnostic {
                                kind: DiagnosticKind::Static,
                                message: "P15：for 反向邊來源狀態不穩定，不可跨迭代沿用借用／所有權".into(),
                                span: Some(self.source.span(statement.start, statement.end)),
                            });
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
                    if !crate::common_ir::boolean_condition_is_ready(
                        crate::common_ir::ScalarType::from_name(&actual),ready,
                    ) {
                        return Err(self.error("while 條件必須是已初始化 bool", condition));
                    }
                    if self.recorded_scalar_hir.borrow()
                        .expression(condition.node_span())
                        .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Bool) {
                        return Err(self.error(
                            "P16：控制流程條件缺少 Checker 已驗證 bool 運算式 HIR",
                            condition,
                        ));
                    }
                    let snapshot = vars.clone();
                    let mut simulated = snapshot.clone();
                    // 下一次條件檢查及迭代本體都屬未來可達路徑，
                    // 不能在本次迭代提早歸還還會被引用的外層別名。
                    let condition_read = Stmt {
                        kind: StmtKind::Return(
                            crate::syntax_shared::ParsedReturn::new(condition.clone())),
                        start: condition.start,
                        end: condition.end,
                    };
                    let mut after_loop = Vec::with_capacity(continuation.len() + 3);
                    after_loop.push((vars.len() - 1, vec![condition_read]));
                    after_loop.push((vars.len() - 1, loop_body.statements.clone()));
                    after_loop.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_loop.extend_from_slice(continuation);
                    let can_repeat = self.statements_with_follow(
                        loop_body, &mut simulated, expected_return, true, &after_loop,
                        Some(&snapshot))?;
                    if can_repeat == FlowState::Fallthrough {
                        // for／while 的反向邊使用同一個真 SafetyState 唯讀
                        // 契約；沒有不動點證據時仍維持原保守拒絕。
                        if Self::checked_outer_state_stability(&simulated, &snapshot).is_err() {
                            return Err(self.error(
                                "P15：while 反向邊的來源所有權／借用狀態不穩定，需完整不動點證明",
                                condition));
                        }
                    }
                    // while 可執行零次，因此後續只使用入口快照；
                    // 任何可回返狀態已確認與此快照一致。
                }
                StmtKind::If { branch, no } => {
                    let condition = &branch.condition;
                    let yes = &branch.body;
                    let (actual, ready) = self.evaluate(condition, vars)?;
                    if !crate::common_ir::boolean_condition_is_ready(
                        crate::common_ir::ScalarType::from_name(&actual),ready,
                    ) {
                        return Err(self.error("if 條件必須是已初始化 bool", condition));
                    }
                    if self.recorded_scalar_hir.borrow()
                        .expression(condition.node_span())
                        .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Bool) {
                        return Err(self.error(
                            "P16：控制流程條件缺少 Checker 已驗證 bool 運算式 HIR",
                            condition,
                        ));
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
                        return Ok(Self::combine_non_fallthrough(true_flow, false_flow));
                    }
                    // 從原作用域複製**候選**；中途任何 ValueSlot 或來源
                    // 證據驗證失敗都不得讓原 vars 留下半完成的合流。
                    let mut proposed = snapshot.clone();
                    // 僅合流分支進入前可見的真正名稱。分支私有宣告不得洩漏。
                    for (depth, scope) in proposed.iter_mut().enumerate() {
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
                            let joined_value = ValueSlot::join_reachable(&[left, right])
                                .map_err(|err| self.model_error(err, condition))?;
                            // 只有各可達分支的缺失欄位集合完全相同，才能
                            // 保留首次寫入許可；否則失去部分初始化證明。
                            let pending_true = if live_true {
                                Some(&on_true[depth][name].pending_fields)
                            } else { None };
                            let pending_false = if live_false {
                                Some(&on_false[depth][name].pending_fields)
                            } else { None };
                            let joined_fields = match (pending_true, pending_false) {
                                (Some(left), Some(right)) if left == right => left.clone(),
                                // 兩個可達分支皆持有尚未完整的相同結構，
                                // 但完成欄位的順序不同時，只有「可能已初始化」
                                // 的差異欄位全為 Copyable，才可安全合併缺失聯集：
                                // 後續寫入若在某路徑是覆寫，也不會遺失資源責任。
                                (Some(Some(left)), Some(Some(right)))
                                    if joined_value.state() == ValueState::Uninitialized
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
                            // 只有一份仍有入口別名的來源必須合流活性。
                            // 之前不修改任何 ValueSlot、欄位及借用寫入欄位。
                            let joined_loan = original.loan.as_ref().map(|_| {
                                let true_active = live_true && on_true[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                let false_active = live_false && on_false[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                true_active || false_active
                            });
                            if !original.commit_reachable_join(
                                joined_value, joined_fields, joined_loan,
                            ) {
                                return Err(self.error(
                                    "HC14：分支合流缺少原始借用有效期來源身分", condition));
                            }
                        }
                    }
                    // 唯一可寫原狀態直到整筆合流驗證完畢才一起提交；
                    // 失敗時不會留下已改 value、但尚未改 loan 的污染狀態。
                    Self::commit_reachable_safety_join(vars, proposed).map_err(|_| {
                        self.error("HC14：分支合流後共享／獨占借用狀態無法一致證明", condition)
                    })?;
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
            if !self.known_type(ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式參數使用不存在的型別".into(), span: None });
            }
            self.record_checked_member(
                crate::common_ir::NodeRole::Parameter,
                parameter.declaration_span,
                name, parameter.name_span,
                ty, parameter.type_span, false,
            );
            Rc::make_mut(&mut vars[0]).insert(name.clone(), Binding::new(
        DeclarationId(parameter.declaration_start),
        ty.clone(),
        false,
        ValueSlot::initialized(self.permission(ty)),
        None,
        (self.permission(ty) != CopyPermission::Allowed
                    && !Self::parameter_is_consumed(fun, name)).then_some(LoanBinding {
                        // 呼叫端真正 BorrowTicket 仍由其追蹤器維護；
                        // 被呼叫端的靜態代理只保存相同的來源／權限契約。
                        fact: BorrowFact::new(
                            DeclarationId(parameter.declaration_start),
                            BorrowKind::SharedRead,
                        ), active: true,
                    }),
    ));
            self.record_declaration(DeclarationId(parameter.declaration_start), ty, false);
        }
        let flow = self.statements(&fun.body, &mut vars, &fun.result_ty, false)?;
        // 只有整份函式經 Checker 成功之後才能交付真 CFG 退出事實。
        // 此處不執行 return，更不建立真 ResourceEvent。
        let fact = VerifiedFunctionExit {
            span: crate::common_ir::NodeSpan::new(fun.start, fun.end), flow,
        };
        if self.recorded_function_exits.borrow_mut()
            .insert(fun.start, fact).is_some() {
            return Err(Diagnostic {
                kind: DiagnosticKind::Static,
                message: "P16：同一函式來源跨度不能重複認領安全退出證據".into(),
                span: None,
            });
        }
        Ok(())
    }
    /// 對方法內所有深度的運算式查核明確禁令，不只檢查頂層 return。
    /// 方法借用完整合法性仍待 P14，不能藉此宣告方法可使用。
    fn reject_method_term(&self, term: &Term) -> Result<(), Diagnostic> {
        match &term.kind {
            TermKind::Group(inner) => self.reject_method_term(inner),
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
            TermKind::Call(call) => {
                // 函式呼叫的合法型別、Copyable 資格與完整參數來源
                // 由 method_pure_value_type 統一查核；仍先檢查 S15／S21。
                for value in &call.arguments { self.reject_method_term(value)?; }
                Ok(())
            }
            TermKind::BinaryInt(binary) => {
                self.reject_method_term(&binary.left)?;
                self.reject_method_term(&binary.right)
            }
            _ => Ok(()),
        }
    }

    fn reject_method_statements(&self, body: &[Stmt]) -> Result<(), Diagnostic> {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Declare(declaration) => {
                    let init = &declaration.initializer;
                    if let Some(value) = init { self.reject_method_term(value)?; }
                }
                StmtKind::Assign(assignment) => {
                    self.reject_method_term(&assignment.expression)?;
                }
                StmtKind::Return(value) => {
                    self.reject_method_term(&value.expression)?;
                }
                StmtKind::FieldAssign { value, .. } => {
                    self.reject_method_term(value)?;
                }
                StmtKind::If { branch, no } => {
                    let condition = &branch.condition;
                    let yes = &branch.body;
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
            TermKind::Group(inner) => self.check_default_term(inner, active, checked)?,
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
            TermKind::UnresolvedZeroArgs(name)
                if matches!(self.resolve_empty_call(name), ZeroArgResolution::Structure) => {
                if active.contains(name) {
                    return Err(self.error("結構欄位預設建構形成循環，無法安全完成初始化", term));
                }
                self.check_default_type(name, active, checked)?;
            }
            TermKind::Call(call) => {
                // 呼叫引數內的結構建立式亦屬預設值的巢狀來源，
                // 不得繞過循環建構的靜態檢查。
                for value in &call.arguments {
                    self.check_default_term(value, active, checked)?;
                }
            }
            TermKind::BinaryInt(binary) => {
                self.check_default_term(&binary.left, active, checked)?;
                self.check_default_term(&binary.right, active, checked)?;
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
            if function.name == "main" && function.is_async {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "V7：非同步 main 入口尚未有正式驅動契約".into(),
                    span: Some(self.source.span(function.start, function.end)) });
            }
            if function.name == "main" && !function.params.is_empty() {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：main 入口不可宣告參數".into(), span: None });
            }
        }
        // 所有函式與方法簽章須先封閉型別名稱，不能只檢查參數或
        // 等待 return 運算式碰巧揭露不存在的回傳型別。
        for function in funcs {
            if !self.known_type(&function.result_ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式回傳型別不存在".into(), span: None });
            }
        }
        for definition in self.structs.values() {
            // S20／D18-02-A：完整型別序列才構成方法多載簽章；
            // 不把參數名稱或返回型別當作消歧條件，也不能因
            // 第一個同名方法合法便放過第二個重複宣告。
            let mut signatures = HashSet::new();
            for method in &definition.methods {
                let signature = (method.name.as_str(), method.params.iter()
                    .map(|parameter| parameter.ty.as_str()).collect::<Vec<_>>());
                if !signatures.insert(signature) {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "S20：方法名稱與完整參數型別簽章重複".into(),
                        span: Some(self.source.span(method.start, method.end)),
                    });
                }
                if !self.known_type(&method.result_ty) {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P13：方法回傳型別不存在".into(), span: None });
                }
            }
        }
        self.check_default_constructor_cycles()?;
        for (struct_name, definition) in self.structs.iter() {
            for field in &definition.fields {
                if !self.known_type(&field.ty) {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "結構欄位使用不存在的型別".into(),
                        span: None,
                    });
                }
                self.record_checked_member(
                    crate::common_ir::NodeRole::Field,
                    field.declaration_span,
                    &field.name, field.name_span,
                    &field.ty, field.type_span, !field.immutable,
                );
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

