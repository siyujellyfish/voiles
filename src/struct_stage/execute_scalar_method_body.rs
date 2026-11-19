// 同步方法沿用普通函式的持有者、票據與步數。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateMethodInteger {
    Field(String),
    Expression(PrivateScalarReturnExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarMethodStep {
    Return(PrivateMethodInteger),
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
            TermKind::BinaryInt(_) => {
                let bindings = self.method.params.iter().filter(|p| p.ty == "int")
                    .map(|p| (p.name.clone(), DeclarationId(p.declaration_start))).collect();
                let expr = PrivateScalarReturnExpr::from_checked_bindings(term, &bindings, self.verified).ok()?;
                if !private_self_approved_int_call_arithmetic(&expr, &BTreeMap::new()) { return None; }
                expr
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
                    Step::Return(self.integer(&ret.expression)?)
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
            let returns = matches!(step, Step::Return(_));
            body.push(step);
            if returns { break; }
        }
        Some(body)
    }
}

impl PrivateScalarSelfRead {
    fn run_body(&self, parameters: &BTreeMap<DeclarationId, i64>,
        runtime: &mut crate::mixed_struct_runtime::MixedStructRuntime, steps: &mut usize, limit: usize)
        -> Result<i64, PrivateScalarCallRuntimeError> {
        use crate::mixed_struct_runtime::{MixedStructRuntime, Observed};
        use PrivateScalarCallRuntimeError::StateMismatch;
        let integer = |expr: &PrivateMethodInteger, runtime: &MixedStructRuntime| match expr {
            PrivateMethodInteger::Expression(expr) => expr.evaluate(parameters),
            PrivateMethodInteger::Field(name) => match runtime.field(&self.owner, name) {
                Ok(Observed::Int(n)) => Ok(n), _ => Err(StateMismatch),
            },
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
                PrivateScalarMethodStep::Return(expr) => return integer(expr, runtime),
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
