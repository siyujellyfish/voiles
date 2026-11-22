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
    storage_depth: usize,
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
    candidate_sites: HashMap<usize, String>,
    containers: BTreeSet<String>,
    views: BTreeSet<String>,
    storage: Vec<String>,
    borrowed: Vec<String>,
    narrowed: std::cell::RefCell<HashMap<String,String>>,
}

pub(super) fn lower(lower: &Lower<'_>, function: &Function, module: usize) -> Result<Body, Diagnostic> {
    let mut builder = Builder { lower, module, result: &function.result_ty, body: Vec::new(),
        scopes: vec![HashMap::new()], loops: Vec::new(), next_slot: 0, candidate_sites: HashMap::new(), containers: BTreeSet::new(), views: BTreeSet::new(), storage: Vec::new(), borrowed: Vec::new(), narrowed: Default::default() };
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
                let ty = self.narrowed.borrow().get(&b.slot).cloned().unwrap_or_else(|| b.ty.clone());
                if !matches!(ty.as_str(), "int" | "bool") { return None; }
                let value = if self.views.contains(&b.slot) { Number::Container(b.slot.clone(), ContainerRead::Scalar) }
                    else { Number::Local(b.slot.clone()) };
                return Some((ty, value));
            }
            TermKind::Scalar(ScalarLiteral::Integer(n)) => ("int", Number::Constant(integer_i64(n).ok()?)),
            TermKind::Scalar(ScalarLiteral::Boolean(b)) => ("bool", Number::Constant(i64::from(*b))),
            TermKind::OptionalCondition(name, OptionalPredicate::IsMissing(equal)) => {
                let binding = self.binding(name)?;
                if !binding.ty.starts_with("Optional[") { return None; }
                let operator = if *equal { "==" } else { "!=" };
                ("bool", Number::Logic(operator, Box::new(Number::Container(binding.slot.clone(), ContainerRead::Tag)),
                    Box::new(Number::Constant(0))))
            }
            TermKind::OptionalCondition(name, OptionalPredicate::Candidate(Some(candidate))) => {
                let binding = self.binding(name)?;
                if self.candidate_sites.get(&term.start)? != &binding.slot { return None; }
                let candidates = binding.ty.strip_prefix("Optional[")?.strip_suffix(']')?;
                if !candidates.split(',').any(|ty| ty == candidate) { return None; }
                let code = match candidate.as_str() {
                    "int" => 1, "bool" => 2, "str" => 3, "float" => 4, _ => return None,
                };
                ("bool", Number::Logic("==", Box::new(Number::Container(binding.slot.clone(), ContainerRead::Tag)),
                    Box::new(Number::Constant(code))))
            }
            TermKind::StringIndex(owner,i,Some(j)) => {
                let binding=self.binding(owner)?;
                let ty=self.narrowed.borrow().get(&binding.slot).cloned().unwrap_or_else(||binding.ty.clone());
                if !self.views.contains(&binding.slot) || ty != "str" { return None; }
                ("int",Number::ViewByte(binding.slot.clone(),self.offset(i)?,self.offset(j)?))
            }
            TermKind::MethodCall(owner,method,args) if self.binding(owner).is_some() =>
                return self.edit(owner,method,args),
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

    fn offset(&self, offset: &StringOffset) -> Option<StringOffset> {
        match offset {
            StringOffset::Constant(i) => Some(StringOffset::Constant(*i)),
            StringOffset::Named(n) => {
                let b=self.binding(n)?;
                (b.ty=="int" && !self.views.contains(&b.slot)).then(||StringOffset::Named(b.slot.clone()))
            }
        }
    }

    fn copy_source(&self, name:&str, expected:&str) -> Option<String> {
        let b=self.binding(name)?;
        let members=|ty:&str| ty.strip_prefix("Optional[").and_then(|s|s.strip_suffix(']'))
            .map(|s|s.split(',').map(str::to_owned).collect::<BTreeSet<_>>());
        let source=members(&b.ty)?;
        let target=members(expected)?;
        (self.views.contains(&b.slot) && !source.contains("str") && source.is_subset(&target)).then(||b.slot.clone())
    }

    fn item(&self, term:&Term, expected:&str) -> Option<ContainerItem> {
        use containers::Value;
        let candidates=expected.strip_prefix("Optional[").and_then(|v|v.strip_suffix(']'));
        if let TermKind::Name(n)=&term.kind {
            if let Some(slot)=self.copy_source(n,expected) { return Some(ContainerItem::Literal(Value::CopyFrom(slot))); }
        }
        let permits=|ty:&str| expected==ty || candidates.is_some_and(|s|s.split(',').any(|t|t==ty));
        match &term.kind {
            TermKind::Literal("none") if candidates.is_some()=>Some(ContainerItem::Literal(Value::Missing)),
            TermKind::Literal("str") if permits("str") => {
                let Text::Literal(s)=self.lower.text(term)? else {return None};
                Some(ContainerItem::Literal(Value::Text(s)))
            }
            _ => {
                let (ty,n)=self.expr(term)?;
                (permits(&ty) && matches!(ty.as_str(),"int"|"bool")).then(||ContainerItem::Scalar(n,ty=="bool"))
            }
        }
    }

    fn edit(&self, name:&str, method:&str, args:&[Term]) -> Option<(String,Number)> {
        let b=self.binding(name)?;
        if !b.mutable || !self.containers.contains(&b.slot) || self.borrowed.contains(&b.slot) {return None;}
        let (_,ty)=element_type(&b.ty)?;
        let (result,edit)=match (method,args) {
            ("append",[value]) if b.ty.starts_with("list[")=>("int",Edit::Append(self.item(value,ty)?)),
            ("pop",[]) if b.ty.starts_with("list[") && matches!(ty,"int"|"bool")=>(ty,Edit::Pop),
            ("set",[key,value]) if b.ty.starts_with("dict{") => {
                let (_,_,key)=self.element(name,key)?;
                ("int",Edit::Set(key,self.item(value,ty)?))
            }
            _=>return None,
        };
        self.narrowed.borrow_mut().clear();
        Some((result.into(),Number::Edit(b.slot.clone(),Box::new(edit))))
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
            if element_type(&binding.ty).is_none() || !(self.containers.contains(&binding.slot) || self.views.contains(&binding.slot)) { return None; }
            return Some(("int".into(), Number::Container(binding.slot.clone(), ContainerRead::Length)));
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
        self.narrowed.borrow_mut().clear();
        Some((f.result_ty.clone(), Number::ScalarCall { target:target.clone(), arguments:values }))
    }

    fn container(&mut self, stmt: &Stmt, declaration: &crate::syntax_shared::ParsedDeclaration<String, Term>) -> Option<()> {
        let initial = declaration.initializer.as_ref()?;
        let value = match &initial.kind {
            TermKind::Container(literal) if literal.ty == declaration.ty => {
                let mut sources = HashMap::new();
                literal.visit_elements(&mut |ty, atom| {
                    if let ContainerAtom::ElementName(n)=atom {
                        sources.insert(n.clone(), self.copy_source(n,ty).ok_or(())?);
                    }
                    Ok::<(),()>(())
                }).ok()?;
                let mut value=literal.clone();
                value.fill_and_count(&mut |atom| {
                    if let ContainerAtom::ElementName(n)=atom { *n=sources[n].clone(); }
                    Ok(())
                }).ok()?;
                ContainerInit::Literal(value)
            }
            TermKind::Move(source) => {
                // 在同一區域內，移動必然與來源宣告沿同一路徑執行。
                // 外層來源可能只在某一條分支移走，亦可能仍有 with 借用。
                let owner = self.scopes.last()?.get(source)?;
                if owner.ty != declaration.ty { return None; }
                let slot = owner.slot.clone();
                if !self.containers.remove(&slot) { return None; }
                ContainerInit::Move(slot)
            }
            _ => return None,
        };
        let slot = format!("@{}", self.next_slot);
        self.next_slot += 1;
        self.containers.insert(slot.clone());
        self.storage.push(slot.clone());
        self.step(stmt, Action::Container(slot.clone(),value));
        self.scopes.last_mut()?.insert(declaration.name.clone(), Binding {
            slot, ty: declaration.ty.clone(), mutable: declaration.mutable,
        });
        Some(())
    }

    fn element(&self, owner: &str, selector: &Term) -> Option<(String, String, ContainerKey)> {
        let binding = self.binding(owner)?;
        if !(self.containers.contains(&binding.slot) || self.views.contains(&binding.slot)) { return None; }
        let (key_type, value_type) = element_type(&binding.ty)?;
        let key = if key_type == "str" {
            let Text::Literal(text) = self.lower.text(selector)? else { return None };
            ContainerKey::Text(text)
        } else { ContainerKey::Index(self.typed(selector,key_type)?) };
        Some((binding.slot.clone(), value_type.into(), key))
    }

    fn refine(&self, condition:&Term, truth:bool) {
        let TermKind::OptionalCondition(name,predicate)=&condition.kind else {return};
        let Some(b)=self.binding(name) else {return};
        let candidate=match predicate {
            OptionalPredicate::Candidate(Some(ty)) if truth=>Some(ty.as_str()),
            OptionalPredicate::IsMissing(equal) if *equal!=truth=>b.ty.strip_prefix("Optional[").and_then(|t|t.strip_suffix(']')).filter(|t|!t.contains(',')),
            _=>None,
        };
        if let Some(ty)=candidate {self.narrowed.borrow_mut().insert(b.slot.clone(),ty.into());}
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
        let storage_depth = self.storage.len();
        let mut closed=false;
        for s in stmts {
            let err=|msg| problem(self.lower.source,(s.start,s.end),msg);
            if closed { return Err(err("控制轉移後不可再有語句")); }
            match &s.kind {
                StmtKind::Declare(d) => {
                    if self.scopes.last().unwrap().contains_key(&d.name) { return Err(err("重複區域宣告")); }
                    if element_type(&d.ty).is_some() {
                        self.container(s,d).ok_or_else(|| err("容器需要具型別常值或有效的整筆移動來源"))?;
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
                    let (owner, ty, key) = self.element(owner, &block.selector).ok_or_else(|| err("此索引與容器元素型別不符"))?;
                    let slot = format!("@{}", self.next_slot);
                    self.next_slot += 1;
                    self.borrowed.push(owner.clone());
                    self.step(s, Action::Select { alias: slot.clone(), owner, key });
                    self.views.insert(slot.clone());
                    self.storage.push(slot.clone());
                    self.scopes.push(HashMap::from([(alias.clone(), Binding { slot:slot.clone(), ty, mutable: false })]));
                    closed = self.block(&block.body.statements, false)?;
                    self.storage.pop();
                    self.borrowed.pop();
                    if !closed { self.step(s,Action::DropContainer(slot)); }
                    self.scopes.pop();
                }
                StmtKind::Evaluate(t) => {
                    let (_,v)=self.expr(t).ok_or_else(||err("容器操作的權限或參數型別不符"))?;
                    self.step(s,Action::Evaluate(v));
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
                    if let TermKind::OptionalCondition(name, OptionalPredicate::IsMissing(equal)) = &branch.condition.kind {
                        let body = if *equal { no.as_ref().map(|arm| arm.as_slice()) }
                            else { Some(branch.body.statements.as_slice()) };
                        let slot = self.binding(name).unwrap().slot.clone();
                        if let Some(statements) = body {
                            visit_candidate_sites(name, statements, |site| {
                                self.candidate_sites.insert(site, slot.clone());
                            });
                        }
                    }
                    let skip=self.step(s,Action::Unless(test,0));
                    let before=self.narrowed.borrow().clone();
                    self.refine(&branch.condition,true);
                    let yes=self.block(&branch.body.statements,true)?;
                    let yes_facts=self.narrowed.replace(before.clone());
                    let end=(!yes).then(||self.step(s,Action::Jump(0)));
                    self.patch(skip,self.body.len());
                    self.refine(&branch.condition,false);
                    let no_closed=if let Some(other)=no { self.block(other.as_slice(),true)? } else {false};
                    let no_facts=self.narrowed.replace(before);
                    self.narrowed.borrow_mut().retain(|slot,ty|
                        (yes || yes_facts.get(slot)==Some(ty)) && (no_closed || no_facts.get(slot)==Some(ty)));
                    closed=yes && no_closed;
                    if let Some(at)=end { self.patch(at,self.body.len()); }
                }
                StmtKind::Break | StmtKind::Continue => {
                    let depth=self.loops.last().ok_or_else(||err("控制轉移需要所在迴圈"))?.storage_depth;
                    for i in (depth..self.storage.len()).rev() {
                        self.step(s,Action::DropContainer(self.storage[i].clone()));
                    }
                    let at=self.step(s,Action::Jump(0));
                    let scope=self.loops.last_mut().ok_or_else(||err("控制轉移需要所在迴圈"))?;
                    if matches!(s.kind,StmtKind::Break) { scope.breaks.push(at); } else { scope.continues.push(at); }
                    closed=true;
                }
                StmtKind::While{condition,body} => {
                    self.narrowed.borrow_mut().clear();
                    let cond=self.typed(condition,"bool").ok_or_else(||err("while 需要 bool 條件"))?;
                    let head=self.step(s,Action::Unless(cond,0));
                    self.loops.push(Loop{storage_depth:self.storage.len(),breaks:vec![head],continues:Vec::new()});
                    self.block(&body.statements,true)?;
                    self.step(s,Action::Jump(head));
                    self.end_loop(head);
                }
                StmtKind::ForRange{iterator,lower,upper,body,..} => {
                    self.narrowed.borrow_mut().clear();
                    let start=integer_i64(lower).map_err(|_|err("區間下界無效"))?;
                    let end=integer_i64(upper).map_err(|_|err("區間上界無效"))?;
                    let slot=format!("@{}",self.next_slot); self.next_slot+=1;
                    self.step(s,Action::Bind(slot.clone(),Number::Constant(start)));
                    let test=Number::Logic("lt",Box::new(Number::Local(slot.clone())),Box::new(Number::Constant(end)));
                    let head=self.step(s,Action::Unless(test,0));
                    self.loops.push(Loop{storage_depth:self.storage.len(),breaks:vec![head],continues:Vec::new()});
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
        while self.storage.len() > storage_depth {
            let slot = self.storage.pop().unwrap();
            if !closed { self.step(stmts.last().unwrap(),Action::DropContainer(slot)); }
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
