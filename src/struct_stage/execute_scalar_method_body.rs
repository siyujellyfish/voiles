// 同步方法沿用普通函式的持有者、票據與步數。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateMethodInteger {
    Field(String),
    Expression(PrivateScalarReturnExpr),
    Helper(crate::common_ir::NodeSpan, Box<PrivateSelfApprovedHelper>, Vec<Self>),
    Addition(Box<Self>, Box<Self>),
    Subtraction(Box<Self>, Box<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateMethodValue<T> {
    Int(i64),
    Bool(bool),
    Str(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarMethodStep {
    Return(PrivateMethodInteger),
    ReturnMoved(DeclarationId),
    ReturnBool(PrivateSelfBoolWriteSource),
    Declare(DeclarationId, PrivateMethodInteger),
    Assign(DeclarationId, PrivateMethodInteger),
    WriteInt(String, PrivateMethodInteger),
    WriteBool(String, PrivateSelfBoolWriteSource),
    If(PrivateSelfBoolWriteSource, Vec<Self>, Vec<Self>),
}

struct PrivateScalarMethodCompiler<'a> {
    verified: &'a VerifiedPrivateSource,
    method: &'a Function,
    definition: &'a StructDecl,
    safety: &'a VerifiedMethodSafety,
}

impl PrivateScalarMethodCompiler<'_> {
    fn integer(&self, term: &Term) -> Option<PrivateMethodInteger> {
        let leaf = private_self_checked_ungrouped_return(self.verified, term).ok()?;
        let expr = match &leaf.kind {
            TermKind::FieldRead(owner, field) if owner == "self" && self.definition.field(field)?.ty == "int" =>
                return Some(PrivateMethodInteger::Field(field.clone())),
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =>
                PrivateScalarReturnExpr::Constant(crate::syntax_shared::integer_i64(raw).ok()?),
            TermKind::Name(name) => {
                let id = self.verified.plan.scalar_hir.reference(leaf.node_span())
                    .filter(|fact| fact.resolved.ty == crate::common_ir::ScalarType::Int)
                    .map(|fact| fact.resolved.declaration)
                    .or_else(|| self.method.params.iter().find(|p| p.name == *name && p.ty == "int")
                        .map(|p| DeclarationId(p.declaration_start)))?;
                PrivateScalarReturnExpr::Parameter(id)
            }
            TermKind::Call(_) | TermKind::UnresolvedZeroArgs(_) => {
                let (name, arguments) = match &leaf.kind {
                    TermKind::Call(call) if !call.awaited => (call.name.as_str(), call.arguments.as_slice()),
                    TermKind::UnresolvedZeroArgs(name) if !self.verified.structs.contains_key(name) => (name.as_str(), &[][..]),
                    _ => return None,
                };
                let helper = self.verified.functions.iter().find(|fun| fun.name == name && fun.name != "main")?;
                if helper.params.len() != arguments.len() { return None; }
                let text = std::str::from_utf8(&self.verified.source.bytes).ok()?;
                let target = private_self_checked_int_helper(text, self.verified, helper).ok()?;
                let args = arguments.iter().map(|arg| self.integer(arg)).collect::<Option<Vec<_>>>()?;
                return Some(PrivateMethodInteger::Helper(leaf.node_span(), Box::new(target), args));
            }
            TermKind::BinaryInt(binary) => {
                let fact = self.verified.plan.scalar_hir.arithmetic(leaf.node_span())?;
                if fact.left != binary.left.node_span() || fact.right != binary.right.node_span()
                    || fact.operator != binary.operator { return None; }
                let left = Box::new(self.integer(&binary.left)?);
                let right = Box::new(self.integer(&binary.right)?);
                return match fact.operator {
                    '+' => Some(PrivateMethodInteger::Addition(left, right)),
                    '-' => Some(PrivateMethodInteger::Subtraction(left, right)),
                    _ => None,
                };
            }
            _ => return None,
        };
        Some(PrivateMethodInteger::Expression(expr))
    }

    fn boolean(&self, term: &Term) -> Option<PrivateSelfBoolWriteSource> {
        let leaf = private_self_checked_ungrouped_return(self.verified, term).ok()?;
        match &leaf.kind {
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) =>
                Some(PrivateSelfBoolWriteSource::Literal(*value)),
            TermKind::Name(name) => Some(PrivateSelfBoolWriteSource::Parameter(DeclarationId(
                self.method.params.iter().find(|p| p.name == *name && p.ty == "bool")?.declaration_start))),
            TermKind::FieldRead(owner, field) if owner == "self" && self.definition.field(field)?.ty == "bool" =>
                Some(PrivateSelfBoolWriteSource::SelfField(field.clone())),
            _ => None,
        }
    }

    fn block(&self, statements: &[Stmt]) -> Option<Vec<PrivateScalarMethodStep>> {
        let mut body = Vec::new();
        for statement in statements {
            use PrivateScalarMethodStep as Step;
            let step = match &statement.kind {
                StmtKind::Return(ret) => {
                    if !self.safety.checked_returns.contains(&statement.node_span()) { return None; }
                    if self.method.result_ty == "str" {
                        let leaf = private_self_checked_ungrouped_return(self.verified, &ret.expression).ok()?;
                        let source = PreparedPrivateMoveCall::checked_move_name(leaf, self.verified).ok()?;
                        let param = self.method.params.iter().find(|p| p.name == source && p.ty == "str")?;
                        Step::ReturnMoved(DeclarationId(param.declaration_start))
                    } else if self.method.result_ty == "bool" {
                        Step::ReturnBool(self.boolean(&ret.expression)?)
                    } else {
                        Step::Return(self.integer(&ret.expression)?)
                    }
                }
                StmtKind::Declare(local) => {
                    let id = DeclarationId(statement.start);
                    if !self.verified.plan.declarations.matches(id, "int", local.mutable) { return None; }
                    Step::Declare(id, self.integer(local.initializer.as_ref()?)?)
                }
                StmtKind::Assign(assignment) => {
                    let id = self.verified.plan.scalar_hir.assignment(statement.node_span())?.resolved.declaration;
                    if !self.verified.plan.declarations.matches(id, "int", true) { return None; }
                    Step::Assign(id, self.integer(&assignment.expression)?)
                }
                StmtKind::FieldAssign { receiver, field, value } => {
                    let target = self.definition.field(field)?;
                    if receiver != "self" || target.immutable || self.safety.self_borrow != BorrowKind::ExclusiveWrite {
                        return None;
                    }
                    match target.ty.as_str() {
                        "int" => Step::WriteInt(field.clone(), self.integer(value)?),
                        "bool" => Step::WriteBool(field.clone(), self.boolean(value)?),
                        _ => return None,
                    }
                }
                StmtKind::If { branch, no } => {
                    let other = match no {
                        None => Vec::new(),
                        Some(crate::syntax_shared::ParsedElse::Block(block)) => self.block(&block.statements)?,
                        Some(crate::syntax_shared::ParsedElse::ElseIf(next)) => self.block(std::slice::from_ref(next))?,
                        #[cfg(test)]
                        Some(crate::syntax_shared::ParsedElse::SyntheticTest(_)) => return None,
                    };
                    Step::If(self.boolean(&branch.condition)?, self.block(&branch.body.statements)?, other)
                }
                _ => return None,
            };
            let returns = matches!(step, Step::Return(_) | Step::ReturnMoved(_) | Step::ReturnBool(_));
            body.push(step);
            if returns { break; }
        }
        Some(body)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateMethodHelperExit {
    span: crate::common_ir::NodeSpan,
    locals: BTreeMap<DeclarationId, i64>,
    result: Result<i64, PrivateScalarCallRuntimeError>,
}

struct PrivateMethodExecution<'a> {
    steps: &'a mut usize,
    limit: usize,
    helper_frame_available: bool,
    trace: &'a mut Vec<PrivateSharedCallExitEvent>,
    helpers: &'a mut Vec<PrivateMethodHelperExit>,
    branches: &'a mut Vec<bool>,
    locals: &'a mut Vec<DeclarationId>,
}

impl PrivateMethodExecution<'_> {
    fn step(&mut self) -> Result<(), PrivateScalarCallRuntimeError> {
        if *self.steps >= self.limit { return Err(PrivateScalarCallRuntimeError::StepBudget); }
        *self.steps += 1;
        Ok(())
    }
}

impl PrivateMethodInteger {
    fn collect_helpers<'a>(&'a self, order: &mut Vec<(crate::common_ir::NodeSpan, &'a PrivateSelfApprovedHelper)>) {
        match self {
            Self::Helper(span, helper, arguments) => {
                for argument in arguments {
                    argument.collect_helpers(order);
                }
                order.push((*span, helper));
            }
            Self::Addition(left, right) | Self::Subtraction(left, right) => {
                left.collect_helpers(order);
                right.collect_helpers(order);
            }
            Self::Field(_) | Self::Expression(_) => {}
        }
    }

    fn evaluate(&self, owner: &str, parameters: &BTreeMap<DeclarationId, i64>,
        runtime: &crate::mixed_struct_runtime::MixedStructRuntime, work: &mut PrivateMethodExecution) -> Result<i64, PrivateScalarCallRuntimeError> {
        use crate::mixed_struct_runtime::Observed;
        use PrivateScalarCallRuntimeError::{IntegerOverflow, StateMismatch};
        match self {
            Self::Expression(expr) => expr.evaluate(parameters),
            Self::Helper(span, helper, arguments) => {
                let mut locals = BTreeMap::new();
                for (id, arg) in helper.parameters.iter().zip(arguments) {
                    locals.insert(*id, arg.evaluate(owner, parameters, runtime, work)?);
                }
                if !work.helper_frame_available { return Err(PrivateScalarCallRuntimeError::CallBudget); }
                work.step()?;
                work.trace.push(PrivateSharedCallExitEvent::FunctionEntered);
                let result = work.step().and_then(|_| helper.returned.evaluate(&locals));
                work.helpers.push(PrivateMethodHelperExit { span: *span, locals, result });
                work.trace.push(if result.is_ok() { PrivateSharedCallExitEvent::Returned }
                    else { PrivateSharedCallExitEvent::UnwindStarted });
                work.trace.push(PrivateSharedCallExitEvent::FunctionExited { returned: result.is_ok() });
                result
            }
            Self::Field(field) => match runtime.field(owner, field) {
                Ok(Observed::Int(n)) => Ok(n), _ => Err(StateMismatch),
            },
            Self::Addition(left, right) => left.evaluate(owner, parameters, runtime, work)?
                .checked_add(right.evaluate(owner, parameters, runtime, work)?).ok_or(IntegerOverflow),
            Self::Subtraction(left, right) => left.evaluate(owner, parameters, runtime, work)?
                .checked_sub(right.evaluate(owner, parameters, runtime, work)?).ok_or(IntegerOverflow),
        }
    }
}

impl PrivateScalarSelfRead {
    fn selected_steps(&self, branches: &[bool], success: bool) -> Option<Vec<&PrivateScalarMethodStep>> {
        use PrivateScalarMethodStep as Step;
        let mut choices = branches.iter();
        let mut blocks = vec![self.body.iter()];
        let mut steps = Vec::new();
        while let Some(block) = blocks.last_mut() {
            let Some(step) = block.next() else { blocks.pop(); continue; };
            match step {
                Step::If(_, yes, no) => {
                    let Some(choice) = choices.next() else {
                        return (!success).then_some(steps);
                    };
                    blocks.push(if *choice { yes.iter() } else { no.iter() });
                }
                Step::Return(_) | Step::ReturnMoved(_) | Step::ReturnBool(_) => {
                    steps.push(step);
                    break;
                }
                _ => steps.push(step),
            }
        }
        choices.next().is_none().then_some(steps)
    }

    fn matches_exit_trace(&self, trace: &[PrivateSharedCallExitEvent],
        ticket: &crate::borrow_safety::BorrowTicket, args: &[crate::borrow_safety::BorrowTicket], helpers: &[PrivateMethodHelperExit], branches: &[bool], success: bool) -> bool {
        use PrivateSharedCallExitEvent::*;
        if !trace.starts_with(&[LoanAcquired(*ticket), FunctionEntered]) { return false; }
        let mut inside = &trace[2..];
        let mut unwound = false;
        let mut order = Vec::new();
        let Some(steps) = self.selected_steps(branches, success) else { return false; };
        for step in &steps {
            use PrivateScalarMethodStep as Step;
            if let Step::Declare(_, expr) | Step::Assign(_, expr) | Step::Return(expr) | Step::WriteInt(_, expr) = step {
                expr.collect_helpers(&mut order);
            }
        }
        let mut remaining = order.into_iter();
        let mut receipts = helpers.iter();
        let mut locals = Vec::new();
        let mut local_plan = steps.into_iter().filter_map(|op| match op {
            PrivateScalarMethodStep::Declare(id, _) => Some(LocalInitialized(*id)),
            PrivateScalarMethodStep::Assign(id, _) => Some(LocalAssigned(*id)),
            _ => None,
        });
        while let Some(event) = inside.first() {
            if matches!(event, LocalInitialized(_) | LocalAssigned(_)) {
                if unwound || local_plan.next().as_ref() != Some(event) { return false; }
                if let LocalInitialized(id) = event { locals.push(*id); }
                inside = &inside[1..];
                continue;
            }
            if !matches!(event, FunctionEntered) { break; }
            let Some(receipt) = receipts.next() else { return false; };
            let Some((_, target)) = remaining.next().filter(|(span, _)| *span == receipt.span) else { return false; };
            let [FunctionEntered, outcome, FunctionExited { returned }, rest @ ..] = inside else { return false; };
            if unwound || *returned != receipt.result.is_ok()
                || receipt.locals.len() != target.parameters.len()
                || !target.parameters.iter().all(|id| receipt.locals.contains_key(id))
                || receipt.result != Err(PrivateScalarCallRuntimeError::StepBudget)
                    && target.returned.evaluate(&receipt.locals) != receipt.result
                || *outcome != if *returned { Returned } else { UnwindStarted } { return false; }
            unwound = !*returned;
            inside = rest;
        }
        if receipts.next().is_some() || success && (unwound || remaining.next().is_some() || local_plan.next().is_some()) { return false; }
        if !unwound {
            let expected = if success { Returned } else { UnwindStarted };
            let Some((head, rest)) = inside.split_first() else { return false; };
            if *head != expected { return false; }
            inside = rest;
        }
        inside.iter().cloned().eq(locals.into_iter().rev().map(LocalScopeEnded)
            .chain(std::iter::once(LoanReturned(*ticket)))
            .chain(args.iter().rev().copied().map(LoanReturned))
            .chain(std::iter::once(FunctionExited { returned: success })))
    }

    fn run_body(&self, mut parameters: BTreeMap<DeclarationId, i64>,
        runtime: &mut crate::mixed_struct_runtime::MixedStructRuntime, work: &mut PrivateMethodExecution)
        -> Result<PrivateMethodValue<DeclarationId>, PrivateScalarCallRuntimeError> {
        use crate::mixed_struct_runtime::{MixedStructRuntime, Observed};
        use PrivateScalarCallRuntimeError::StateMismatch;
        let boolean = |expr: &PrivateSelfBoolWriteSource, runtime: &MixedStructRuntime, values: &BTreeMap<DeclarationId, i64>| match expr {
            PrivateSelfBoolWriteSource::Literal(b) => Ok(*b),
            PrivateSelfBoolWriteSource::Parameter(id) => match values.get(id) {
                Some(0) => Ok(false), Some(1) => Ok(true), _ => Err(StateMismatch),
            },
            PrivateSelfBoolWriteSource::SelfField(name) => match runtime.field(&self.owner, name) {
                Ok(Observed::Bool(b)) => Ok(b), _ => Err(StateMismatch),
            },
        };
        let mut blocks = vec![self.body.iter()];
        while let Some(block) = blocks.last_mut() {
            let Some(op) = block.next() else { blocks.pop(); continue; };
            work.step()?;
            match op {
                PrivateScalarMethodStep::Return(expr) => return expr.evaluate(&self.owner, &parameters, runtime, work).map(PrivateMethodValue::Int),
                PrivateScalarMethodStep::ReturnBool(expr) => return boolean(expr, runtime, &parameters).map(PrivateMethodValue::Bool),
                PrivateScalarMethodStep::ReturnMoved(id) => return Ok(PrivateMethodValue::Str(*id)),
                PrivateScalarMethodStep::Declare(id, expr) => {
                    let value = expr.evaluate(&self.owner, &parameters, runtime, work)?;
                    if parameters.insert(*id, value).is_some() { return Err(StateMismatch); }
                    work.locals.push(*id);
                    work.trace.push(PrivateSharedCallExitEvent::LocalInitialized(*id));
                }
                PrivateScalarMethodStep::Assign(id, expr) => {
                    let value = expr.evaluate(&self.owner, &parameters, runtime, work)?;
                    *parameters.get_mut(id).ok_or(StateMismatch)? = value;
                    work.trace.push(PrivateSharedCallExitEvent::LocalAssigned(*id));
                }
                PrivateScalarMethodStep::WriteInt(field, expr) => {
                    let value = expr.evaluate(&self.owner, &parameters, runtime, work)?;
                    runtime.write_int_field(&self.owner, field, value, true).map_err(|_| StateMismatch)?;
                }
                PrivateScalarMethodStep::WriteBool(field, expr) => {
                    let value = boolean(expr, runtime, &parameters)?;
                    runtime.write_bool_field(&self.owner, field, value, true).map_err(|_| StateMismatch)?;
                }
                PrivateScalarMethodStep::If(condition, yes, no) => {
                    let selected = boolean(condition, runtime, &parameters)?;
                    work.branches.push(selected);
                    let next = if selected { yes } else { no };
                    blocks.push(next.iter());
                }
            }
        }
        Err(StateMismatch)
    }
}
