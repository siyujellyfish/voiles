// 同步方法沿用普通函式的持有者、票據與步數。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateMethodInteger {
    Field(String),
    Expression(PrivateScalarReturnExpr),
    Addition(Box<Self>, Box<Self>),
    Subtraction(Box<Self>, Box<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateMethodValue<T> {
    Int(i64),
    Str(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarMethodStep {
    Return(PrivateMethodInteger),
    ReturnMoved(DeclarationId),
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
            TermKind::Name(name) => PrivateScalarReturnExpr::Parameter(DeclarationId(
                self.method.params.iter().find(|p| p.name == *name && p.ty == "int")?.declaration_start)),
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
                    } else {
                        Step::Return(self.integer(&ret.expression)?)
                    }
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
            let returns = matches!(step, Step::Return(_) | Step::ReturnMoved(_));
            body.push(step);
            if returns { break; }
        }
        Some(body)
    }
}

impl PrivateMethodInteger {
    fn evaluate(&self, owner: &str, parameters: &BTreeMap<DeclarationId, i64>,
        runtime: &crate::mixed_struct_runtime::MixedStructRuntime) -> Result<i64, PrivateScalarCallRuntimeError> {
        use crate::mixed_struct_runtime::Observed;
        use PrivateScalarCallRuntimeError::{IntegerOverflow, StateMismatch};
        match self {
            Self::Expression(expr) => expr.evaluate(parameters),
            Self::Field(field) => match runtime.field(owner, field) {
                Ok(Observed::Int(n)) => Ok(n), _ => Err(StateMismatch),
            },
            Self::Addition(left, right) => left.evaluate(owner, parameters, runtime)?
                .checked_add(right.evaluate(owner, parameters, runtime)?).ok_or(IntegerOverflow),
            Self::Subtraction(left, right) => left.evaluate(owner, parameters, runtime)?
                .checked_sub(right.evaluate(owner, parameters, runtime)?).ok_or(IntegerOverflow),
        }
    }
}

impl PrivateScalarSelfRead {
    fn run_body(&self, parameters: &BTreeMap<DeclarationId, i64>,
        runtime: &mut crate::mixed_struct_runtime::MixedStructRuntime, steps: &mut usize, limit: usize)
        -> Result<PrivateMethodValue<DeclarationId>, PrivateScalarCallRuntimeError> {
        use crate::mixed_struct_runtime::{MixedStructRuntime, Observed};
        use PrivateScalarCallRuntimeError::StateMismatch;
        let integer = |expr: &PrivateMethodInteger, runtime: &MixedStructRuntime| {
            expr.evaluate(&self.owner, parameters, runtime)
        };
        let boolean = |expr: &PrivateSelfBoolWriteSource, runtime: &MixedStructRuntime| match expr {
            PrivateSelfBoolWriteSource::Literal(b) => Ok(*b),
            PrivateSelfBoolWriteSource::Parameter(id) => match parameters.get(id) {
                Some(0) => Ok(false), Some(1) => Ok(true), _ => Err(StateMismatch),
            },
            PrivateSelfBoolWriteSource::SelfField(name) => match runtime.field(&self.owner, name) {
                Ok(Observed::Bool(b)) => Ok(b), _ => Err(StateMismatch),
            },
        };
        let mut blocks = vec![self.body.iter()];
        while let Some(block) = blocks.last_mut() {
            let Some(op) = block.next() else { blocks.pop(); continue; };
            if *steps >= limit { return Err(PrivateScalarCallRuntimeError::StepBudget); }
            *steps += 1;
            match op {
                PrivateScalarMethodStep::Return(expr) => return integer(expr, runtime).map(PrivateMethodValue::Int),
                PrivateScalarMethodStep::ReturnMoved(id) => return Ok(PrivateMethodValue::Str(*id)),
                PrivateScalarMethodStep::WriteInt(field, expr) => {
                    let value = integer(expr, runtime)?;
                    runtime.write_int_field(&self.owner, field, value, true).map_err(|_| StateMismatch)?;
                }
                PrivateScalarMethodStep::WriteBool(field, expr) => {
                    let value = boolean(expr, runtime)?;
                    runtime.write_bool_field(&self.owner, field, value, true).map_err(|_| StateMismatch)?;
                }
                PrivateScalarMethodStep::If(condition, yes, no) => {
                    let next = if boolean(condition, runtime)? { yes } else { no };
                    blocks.push(next.iter());
                }
            }
        }
        Err(StateMismatch)
    }
}
