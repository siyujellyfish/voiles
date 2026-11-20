// 方法持票期間，純值子框架沿用普通函式已驗證的陳述式與宣告身分。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarMethodHelper {
    name: String,
    parameters: Vec<DeclarationId>,
    body: Vec<PrivateScalarBranchAction>,
}

impl PrivateScalarMethodHelper {
    fn checked(function: &Function, verified: &VerifiedPrivateSource) -> Option<Self> {
        use PrivateScalarBranchAction as A;
        use PrivateScalarLocalStep as S;
        if function.params.iter().any(|p| !matches!(p.ty.as_str(), "int" | "bool")) { return None; }
        let (_, steps, returned, _) = PreparedPrivateScalarCall::prepare_function_locals(function, verified).ok()?;
        let mut body = Vec::new();
        for step in steps {
            body.push(match step {
                S::Declare(init) => A::DeclareInt { declaration: init.declaration, value: init.value },
                S::Assign { declaration, value } => A::AssignInt { declaration, value },
                S::DeclareBool { declaration, value } => A::DeclareBool { declaration, value },
                S::AssignBool { declaration, value } => A::AssignBool { declaration, value },
                S::EarlyReturn(branch) => A::NestedIf(Box::new(branch)),
                S::Control(action) => action,
                _ => return None,
            });
        }
        if !matches!(returned, PrivateScalarReturnExpr::Unreachable) { body.push(A::Return(returned)); }
        if !body.iter().all(Self::supported) { return None; }
        Some(Self { name: function.name.clone(), parameters: function.params.iter()
            .map(|p| DeclarationId(p.declaration_start)).collect(), body })
    }

    fn scalar(expr: &PrivateScalarReturnExpr) -> bool {
        use PrivateScalarReturnExpr as E;
        match expr {
            E::Constant(_) | E::Parameter(_) => true,
            E::Addition(a, b) | E::Subtraction(a, b) => Self::scalar(a) && Self::scalar(b),
            _ => false,
        }
    }

    fn boolean(expr: &PrivateScalarBoolExpr) -> bool {
        matches!(expr, PrivateScalarBoolExpr::Constant(_) | PrivateScalarBoolExpr::Bound(_))
    }

    fn branch_supported(branch: &PrivateScalarEarlyIf) -> bool {
        Self::boolean(&branch.condition) && Self::supported(&branch.when_true) && match &branch.when_false {
            None => true,
            Some(PrivateScalarEarlyElse::Branch(action)) => Self::supported(action),
            Some(PrivateScalarEarlyElse::ElseIf(next)) => Self::branch_supported(next),
        }
    }

    fn supported(action: &PrivateScalarBranchAction) -> bool {
        use PrivateScalarBranchAction as A;
        match action {
            A::Sequence(body) | A::Scoped(body) => body.iter().all(Self::supported),
            A::NestedIf(branch) => Self::branch_supported(branch),
            A::ForRange { body, .. } => Self::supported(body),
            A::While { condition, body } => Self::boolean(condition) && Self::supported(body),
            A::Break | A::Continue => true,
            A::DeclareInt { value, .. } | A::AssignInt { value, .. } => Self::scalar(value),
            A::DeclareBool { value, .. } | A::AssignBool { value, .. } => Self::boolean(value),
            A::Return(value) => Self::scalar(value) || matches!(value,
                PrivateScalarReturnExpr::StrValue(PrivateScalarStrReturn::Literal(_))),
            _ => false,
        }
    }
}

struct PrivateHelperFrame {
    values: BTreeMap<DeclarationId, i64>,
    initialized: Vec<DeclarationId>,
    events: Vec<PrivateSharedCallExitEvent>,
    limit: usize,
    steps: usize,
    loop_checks: Vec<bool>,
    loop_limit: usize,
}

impl PrivateHelperFrame {
    fn new(parameters: &BTreeMap<DeclarationId, i64>, limit: usize) -> Self {
        Self { values: parameters.clone(), initialized: Vec::new(), events: Vec::new(), limit, steps: 0, loop_checks: Vec::new(), loop_limit: limit }
    }

    fn tick(&mut self) -> Result<(), PrivateScalarCallRuntimeError> {
        if self.steps >= self.limit { return Err(PrivateScalarCallRuntimeError::StepBudget); }
        self.steps += 1;
        Ok(())
    }

    fn check_loop(&mut self, entered: bool) -> Result<bool, PrivateScalarCallRuntimeError> {
        self.tick()?;
        if self.loop_checks.len() >= self.loop_limit { return Err(PrivateScalarCallRuntimeError::StateMismatch); }
        self.loop_checks.push(entered);
        Ok(entered)
    }

    fn boolean(&self, expr: &PrivateScalarBoolExpr) -> Result<bool, PrivateScalarCallRuntimeError> {
        match expr {
            PrivateScalarBoolExpr::Constant(value) => Ok(*value),
            PrivateScalarBoolExpr::Bound(id) => match self.values.get(id) {
                Some(0) => Ok(false), Some(1) => Ok(true), _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
            },
            _ => Err(PrivateScalarCallRuntimeError::StateMismatch),
        }
    }

    fn leave(&mut self, depth: usize) {
        for id in self.initialized.drain(depth..).rev() {
            self.values.remove(&id);
            self.events.push(PrivateSharedCallExitEvent::LocalScopeEnded(id));
        }
    }

    fn finish(&mut self, success: bool) {
        self.events.push(if success { PrivateSharedCallExitEvent::Returned } else { PrivateSharedCallExitEvent::UnwindStarted });
        self.leave(0);
    }

    fn run<'a>(&mut self, helper: &'a PrivateScalarMethodHelper)
        -> Result<PrivateMethodValue<&'a str>, PrivateScalarCallRuntimeError> {
        use PrivateScalarBranchAction as A;
        use PrivateScalarCallRuntimeError::StateMismatch;
        enum Work<'a> {
            Action(&'a A), If(&'a PrivateScalarEarlyIf), Leave(usize),
            While(&'a PrivateScalarBoolExpr, &'a A), For(DeclarationId, i64, i64, &'a A),
        }
        let mut work = helper.body.iter().rev().map(Work::Action).collect::<Vec<_>>();
        while let Some(next) = work.pop() {
            let action = match next {
                Work::Leave(depth) => { self.leave(depth); continue; }
                Work::If(branch) => {
                    self.tick()?;
                    if self.boolean(&branch.condition)? {
                        work.push(Work::Action(&branch.when_true));
                    } else if let Some(other) = &branch.when_false {
                        work.push(match other {
                            PrivateScalarEarlyElse::Branch(action) => Work::Action(action),
                            PrivateScalarEarlyElse::ElseIf(next) => Work::If(next),
                        });
                    }
                    continue;
                }
                Work::While(condition, body) => {
                    if self.check_loop(self.boolean(condition)?)? {
                        work.push(Work::While(condition, body));
                        work.push(Work::Action(body));
                    }
                    continue;
                }
                Work::For(id, next, end, body) => {
                    if self.check_loop(next < end)? {
                        let depth = self.initialized.len();
                        if self.values.insert(id, next).is_some() { return Err(StateMismatch); }
                        self.initialized.push(id);
                        self.events.push(PrivateSharedCallExitEvent::LocalInitialized(id));
                        work.push(Work::For(id, next + 1, end, body));
                        work.push(Work::Leave(depth));
                        work.push(Work::Action(body));
                    }
                    continue;
                }
                Work::Action(action) => action,
            };
            match action {
                A::Sequence(body) | A::Scoped(body) => {
                    if matches!(action, A::Scoped(_)) { work.push(Work::Leave(self.initialized.len())); }
                    work.extend(body.iter().rev().map(Work::Action));
                }
                A::NestedIf(branch) => work.push(Work::If(branch)),
                A::ForRange { iterator, lower, upper, body } => work.push(Work::For(*iterator, *lower, *upper, body)),
                A::While { condition, body } => work.push(Work::While(condition, body)),
                A::Break | A::Continue => {
                    self.tick()?;
                    loop {
                        match work.pop().ok_or(StateMismatch)? {
                            next @ (Work::For(..) | Work::While(..)) => {
                                if matches!(action, A::Continue) { work.push(next); }
                                break;
                            }
                            Work::Leave(depth) => self.leave(depth),
                            _ => {}
                        }
                    }
                }
                A::Return(expr) => {
                    self.tick()?;
                    return match expr {
                        PrivateScalarReturnExpr::StrValue(PrivateScalarStrReturn::Literal(text)) => Ok(PrivateMethodValue::Str(text)),
                        _ => expr.evaluate(&self.values).map(PrivateMethodValue::Int),
                    };
                }
                _ => {
                    self.tick()?;
                    let (id, n) = match action {
                        A::DeclareInt { declaration, value } | A::AssignInt { declaration, value } =>
                            (*declaration, value.evaluate(&self.values)?),
                        A::DeclareBool { declaration, value } | A::AssignBool { declaration, value } =>
                            (*declaration, i64::from(self.boolean(value)?)),
                        _ => return Err(StateMismatch),
                    };
                    if matches!(action, A::DeclareInt { .. } | A::DeclareBool { .. }) {
                        if self.values.insert(id, n).is_some() { return Err(StateMismatch); }
                        self.initialized.push(id);
                        self.events.push(PrivateSharedCallExitEvent::LocalInitialized(id));
                    } else {
                        *self.values.get_mut(&id).ok_or(StateMismatch)? = n;
                        self.events.push(PrivateSharedCallExitEvent::LocalAssigned(id));
                    }
                }
            }
        }
        Err(StateMismatch)
    }
}
