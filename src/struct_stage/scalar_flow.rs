//! 僅以 Copyable 的 int/bool 建構控制流程；資源操作繼續走具所有權證據的路徑。
use super::*;

pub(super) fn needed(f: &Function) -> bool {
    f.result_ty == "bool" || f.params.iter().any(|p| p.ty == "bool")
        || f.body.statements.iter().any(|s| match &s.kind {
            StmtKind::Declare(d) => d.ty == "bool",
            StmtKind::If { .. } | StmtKind::While { .. } | StmtKind::ForRange { .. } => true,
            _ => false,
        })
}

#[derive(Clone)]
struct Binding {
    slot: String,
    ty: String,
    mutable: bool,
}

struct Loop {
    breaks: Vec<usize>,
    continues: Vec<usize>,
}

struct Builder<'a,'b> {
    lower: &'a Lower<'b>,
    module: usize,
    result: &'a str,
    body: Body,
    scopes: Vec<HashMap<String, Binding>>,
    loops: Vec<Loop>,
    next_slot: usize,
    containers: HashMap<String, Option<ContainerLiteral>>,
}

pub(super) fn lower(lower: &Lower<'_>, function: &Function, module: usize) -> Result<Body, Diagnostic> {
    let mut builder = Builder { lower, module, result: &function.result_ty, body: Vec::new(),
        scopes: vec![HashMap::new()], loops: Vec::new(), next_slot: 0, containers: HashMap::new() };
    for p in &function.params {
        if !matches!(p.ty.as_str(), "int" | "bool") {
            return Err(problem(lower.source,(function.start,function.end),"此控制流程尚未驗證持有者參數"));
        }
        builder.scopes[0].insert(p.name.clone(), Binding { slot:p.name.clone(), ty:p.ty.clone(), mutable:false });
    }
    if !builder.block(&function.body.statements, false)? {
        return Err(problem(lower.source,(function.start,function.end),"函式並非所有路徑皆明確返回"));
    }
    Ok(builder.body)
}

impl Builder<'_, '_> {
    fn binding(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn expr(&self, term: &Term) -> Option<(String, Number)> {
        let (ty, value) = match &term.kind {
            TermKind::Group(t) => return self.expr(t),
            TermKind::Name(n) => {
                let b = self.binding(n)?;
                if element_type(&b.ty).is_some() { return None; }
                return Some((b.ty.clone(), Number::Local(b.slot.clone())));
            }
            TermKind::Scalar(ScalarLiteral::Integer(n)) => ("int", Number::Constant(integer_i64(n).ok()?)),
            TermKind::Scalar(ScalarLiteral::Boolean(b)) => ("bool", Number::Constant(i64::from(*b))),
            TermKind::BinaryInt(n) => ("int", Number::Binary(Box::new(self.typed(&n.left,"int")?),
                n.operator, Box::new(self.typed(&n.right,"int")?))),
            TermKind::Logic(op,a,b) => {
                let (ty,left) = self.expr(a)?;
                if *op != "!=" && ty != "bool" { return None; }
                ("bool", Number::Logic(op, Box::new(left), Box::new(self.typed(b,&ty)?)))
            }
            TermKind::UnresolvedZeroArgs(n) => return self.call(n,std::iter::empty()),
            TermKind::Call(c) if !c.awaited => return self.call(&c.name,c.arguments.iter().map(|v|(None,v))),
            TermKind::Construct(n,args) => return self.call(n,args.iter().map(|(n,v)|(Some(n.as_str()),v))),
            TermKind::MethodCall(m,n,args) => return self.call(&format!("{m}.{n}"),args.iter().map(|v|(None,v))),
            _ => return None,
        };
        Some((ty.to_owned(),value))
    }

    fn typed(&self, t: &Term, expected: &str) -> Option<Number> {
        self.expr(t).and_then(|(ty,n)| (ty==expected).then_some(n))
    }

    fn call<'t>(&self,name:&str,args: impl Iterator<Item=(Option<&'t str>, &'t Term)>) -> Option<(String,Number)> {
        if self.binding(name.split('.').next()?).is_some() { return None; }
        let objects = self.lower.objects.as_ref()?;
        if name == "len" && objects.function(name).is_none() && !self.lower.globals.contains_key(name) {
            let mut args = args;
            let (None, term) = args.next()? else { return None };
            let TermKind::Name(owner) = &term.kind else { return None };
            if args.next().is_some() { return None; }
            let binding = self.binding(owner)?;
            let value = self.containers.get(&binding.slot)?.as_ref()?;
            let size = match &value.items {
                ContainerItems::List(values) => values.len(),
                ContainerItems::Dictionary(values) => values.len(),
            };
            return Some(("int".into(), Number::Constant(size.try_into().ok()?)));
        }
        let (target,_,f) = objects.function(name)?;
        if f.is_async || !matches!(f.result_ty.as_str(), "int" | "bool") { return None; }
        let mut values = Vec::new();
        let mut slots = BTreeSet::new();
        for (index,(label,arg)) in args.enumerate() {
            let slot = match label { Some(n) => f.params.iter().position(|p| p.name==n)?, None => index };
            let param = f.params.get(slot)?;
            if !slots.insert(slot) { return None; }
            values.push((param.name.clone(),slot,self.typed(arg,&param.ty)?));
        }
        if values.len()!=f.params.len() { return None; }
        Some((f.result_ty.clone(), Number::ScalarCall { target:target.clone(), arguments:values }))
    }

    fn container(&mut self, declaration: &crate::syntax_shared::ParsedDeclaration<String, Term>) -> Option<()> {
        let initial = declaration.initializer.as_ref()?;
        let value = match &initial.kind {
            TermKind::Container(literal) if literal.ty == declaration.ty => {
                literal.visit_elements(&mut |_, atom| match atom {
                    ContainerAtom::ElementName(_) => Err(()),
                    _ => Ok(()),
                }).ok()?;
                literal.clone()
            }
            TermKind::Move(source) => {
                // 在同一區域內，移動必然與來源宣告沿同一路徑執行。
                // 外層來源可能只在某一條分支移走，亦可能仍有 with 借用。
                let owner = self.scopes.last()?.get(source)?;
                if owner.ty != declaration.ty { return None; }
                let slot = owner.slot.clone();
                self.containers.get_mut(&slot)?.take()?
            }
            _ => return None,
        };
        let slot = format!("@{}", self.next_slot);
        self.next_slot += 1;
        self.containers.insert(slot.clone(), Some(value));
        self.scopes.last_mut()?.insert(declaration.name.clone(), Binding {
            slot, ty: declaration.ty.clone(), mutable: false,
        });
        Some(())
    }

    fn element(&self, owner: &str, key: &Term) -> Option<(String, Number)> {
        fn scalar(atom: &ContainerAtom) -> Option<i64> {
            match atom {
                ContainerAtom::Integer(value) => Some(*value),
                ContainerAtom::Boolean(value) => Some(i64::from(*value)),
                _ => None,
            }
        }
        let binding = self.binding(owner)?;
        let value = self.containers.get(&binding.slot)?.as_ref()?;
        let (key_type, value_type) = element_type(&value.ty)?;
        if !matches!(value_type, "int" | "bool") { return None; }
        let mut entries = BTreeMap::new();
        let index = if key_type == "str" {
            if !matches!(key.kind, TermKind::Literal("str")) { return None; }
            let at = self.lower.tokens.binary_search_by_key(&key.start, |token| token.start).ok()?;
            let TokenKind::StringLiteral(text) = &self.lower.tokens[at].kind else { return None };
            if let Some(atom) = value.lookup(&ContainerAtom::Text(text.clone())) {
                entries.insert(0, scalar(atom)?);
            }
            Number::Constant(0)
        } else {
            match &value.items {
                ContainerItems::List(items) => {
                    for (i, atom) in items.iter().enumerate() { entries.insert(i.try_into().ok()?, scalar(atom)?); }
                }
                ContainerItems::Dictionary(items) => {
                    for (k, v) in items { entries.insert(scalar(k)?, scalar(v)?); }
                }
            }
            self.typed(key, key_type)?
        };
        Some((value_type.into(), Number::Lookup(Box::new(index), entries)))
    }

    fn step(&mut self, s:&Stmt, action:Action) -> usize {
        let at=self.body.len();
        self.body.push(Step{module:self.module,span:(s.start,s.end),action});
        at
    }
    fn patch(&mut self, at:usize, to:usize) {
        match &mut self.body[at].action {
            Action::Unless(_,v) | Action::Jump(v) => *v=to,
            _ => unreachable!(),
        }
    }
    fn block(&mut self, stmts:&[Stmt], nested:bool) -> Result<bool, Diagnostic> {
        if nested { self.scopes.push(HashMap::new()); }
        let mut closed=false;
        for s in stmts {
            let err=|msg| problem(self.lower.source,(s.start,s.end),msg);
            if closed { return Err(err("控制轉移後不可再有語句")); }
            match &s.kind {
                StmtKind::Declare(d) => {
                    if self.scopes.last().unwrap().contains_key(&d.name) { return Err(err("重複區域宣告")); }
                    if element_type(&d.ty).is_some() {
                        self.container(d).ok_or_else(|| err("容器需要具型別常值或有效的整筆移動來源"))?;
                        continue;
                    }
                    let value=d.initializer.as_ref().and_then(|t|self.typed(t,&d.ty)).ok_or_else(||err("區域純量需要型別相符的初值"))?;
                    let slot=format!("@{}",self.next_slot); self.next_slot+=1;
                    self.scopes.last_mut().unwrap().insert(d.name.clone(),Binding{slot:slot.clone(),ty:d.ty.clone(),mutable:d.mutable});
                    self.step(s,Action::Bind(slot,value));
                }
                StmtKind::ReadonlyElement(block) => {
                    let (TokenKind::Name(owner), TokenKind::Name(alias)) = (&block.owner.kind, &block.alias.kind)
                        else { return Err(err("容器或元素名稱無效")); };
                    let (ty, read) = self.element(owner, &block.selector).ok_or_else(|| err("此索引與容器元素型別不符"))?;
                    let slot = format!("@{}", self.next_slot);
                    self.next_slot += 1;
                    self.step(s, Action::Bind(slot.clone(), read));
                    self.scopes.push(HashMap::from([(alias.clone(), Binding { slot, ty, mutable: false })]));
                    closed = self.block(&block.body.statements, false)?;
                    self.scopes.pop();
                }
                StmtKind::Assign(a) => {
                    let b=self.binding(&a.name).filter(|b|b.mutable).ok_or_else(||err("缺少可寫區域純量"))?;
                    let v=self.typed(&a.expression,&b.ty).ok_or_else(||err("賦值型別不符"))?;
                    self.step(s,Action::Bind(b.slot.clone(),v));
                }
                StmtKind::Return(r) => {
                    let v=self.typed(&r.expression,self.result).ok_or_else(||err("回傳型別不符"))?;
                    self.step(s,Action::Return(v)); closed=true;
                }
                StmtKind::If{branch,no} => {
                    let test=self.typed(&branch.condition,"bool").ok_or_else(||err("分支需要 bool 條件"))?;
                    let skip=self.step(s,Action::Unless(test,0));
                    let yes=self.block(&branch.body.statements,true)?;
                    let end=(!yes).then(||self.step(s,Action::Jump(0)));
                    self.patch(skip,self.body.len());
                    if let Some(other)=no { closed=self.block(other.as_slice(),true)? && yes; }
                    if let Some(at)=end { self.patch(at,self.body.len()); }
                }
                StmtKind::Break | StmtKind::Continue => {
                    let at=self.step(s,Action::Jump(0));
                    let scope=self.loops.last_mut().ok_or_else(||err("控制轉移需要所在迴圈"))?;
                    if matches!(s.kind,StmtKind::Break) { scope.breaks.push(at); } else { scope.continues.push(at); }
                    closed=true;
                }
                StmtKind::While{condition,body} => {
                    let cond=self.typed(condition,"bool").ok_or_else(||err("while 需要 bool 條件"))?;
                    let head=self.step(s,Action::Unless(cond,0));
                    self.loops.push(Loop{breaks:vec![head],continues:Vec::new()});
                    self.block(&body.statements,true)?;
                    self.step(s,Action::Jump(head));
                    self.end_loop(head);
                }
                StmtKind::ForRange{iterator,lower,upper,body,..} => {
                    let start=integer_i64(lower).map_err(|_|err("區間下界無效"))?;
                    let end=integer_i64(upper).map_err(|_|err("區間上界無效"))?;
                    let slot=format!("@{}",self.next_slot); self.next_slot+=1;
                    self.step(s,Action::Bind(slot.clone(),Number::Constant(start)));
                    let test=Number::Logic("lt",Box::new(Number::Local(slot.clone())),Box::new(Number::Constant(end)));
                    let head=self.step(s,Action::Unless(test,0));
                    self.loops.push(Loop{breaks:vec![head],continues:Vec::new()});
                    self.scopes.push(HashMap::from([(iterator.clone(),Binding{slot:slot.clone(),ty:"int".into(),mutable:false})]));
                    self.block(&body.statements,false)?;
                    self.scopes.pop();
                    let increment=self.step(s,Action::Bind(slot.clone(),Number::Binary(Box::new(Number::Local(slot)),'+',Box::new(Number::Constant(1)))));
                    self.step(s,Action::Jump(head));
                    self.end_loop(increment);
                }
                _ => return Err(err("控制流程中的資源操作尚待驗證")),
            }
        }
        if nested { self.scopes.pop(); }
        Ok(closed)
    }
    fn end_loop(&mut self, next:usize) {
        let scope=self.loops.pop().unwrap();
        for at in scope.breaks { self.patch(at,self.body.len()); }
        for at in scope.continues { self.patch(at,next); }
    }
}
