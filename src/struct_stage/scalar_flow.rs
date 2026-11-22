//! 僅以 Copyable 的 int/bool 建構控制流程；資源操作繼續走具所有權證據的路徑。
use super::*;

pub(super) fn needed(f: &Function) -> bool {
    f.result_ty == "bool" || f.params.iter().any(|p| p.ty == "bool")
        || f.body.statements.iter().any(|s| match &s.kind {
            StmtKind::Declare(d) => d.ty == "bool" || stored_type(&d.ty),
            StmtKind::ReadonlyElement(_) => true,
            StmtKind::Try { .. } | StmtKind::If { .. } | StmtKind::While { .. } | StmtKind::ForRange { .. } => true,
            _ => false,
        })
}

#[derive(Clone)]
struct Binding {
    slot: String,
    ty: String,
    mutable: bool,
}

#[derive(Clone)]
struct Presence {
    slot: String,
    candidate: String,
    present_when_true: bool,
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
    move_floor: usize,
    uninitialized: BTreeSet<String>,
    next_slot: usize,
    candidate_sites: HashMap<usize, String>,
    containers: std::cell::RefCell<BTreeSet<String>>,
    views: BTreeSet<String>,
    storage: Vec<Storage>,
    borrowed: Vec<String>,
    flags: std::cell::RefCell<HashMap<String, Presence>>,
    narrowed: std::cell::RefCell<HashMap<String,String>>,
}

pub(super) fn lower(lower: &Lower<'_>, function: &Function, module: usize) -> Result<Body, Diagnostic> {
    let mut builder = Builder { lower, module, result: &function.result_ty, body: Vec::new(),
        scopes: vec![HashMap::new()], loops: Vec::new(), move_floor: 0, uninitialized: BTreeSet::new(), next_slot: 0, candidate_sites: HashMap::new(), containers: Default::default(), views: BTreeSet::new(), storage: Vec::new(), borrowed: Vec::new(), flags: Default::default(), narrowed: Default::default() };
    for p in &function.params {
        if stored_type(&p.ty) {
            builder.containers.borrow_mut().insert(p.name.clone());
        } else if !matches!(p.ty.as_str(), "int" | "bool" | "float" | "str") {
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

    fn readable(&self, name: &str) -> Option<&Binding> {
        self.binding(name).filter(|binding| !self.uninitialized.contains(&binding.slot))
    }

    // 只有仍可走到合流處的路徑會限制後面的讀取。
    fn join_initialization(&mut self, first: BTreeSet<String>, first_closed: bool, second_closed: bool) {
        if second_closed { self.uninitialized.clear(); }
        if !first_closed { self.uninitialized.extend(first); }
    }

    fn expr(&self, term: &Term) -> Option<(String, Number)> {
        let (ty, value) = match &term.kind {
            TermKind::Group(t) => return self.expr(t),
            TermKind::Literal("float") => {
                let index = self.lower.tokens.binary_search_by_key(&term.start, |t| t.start).ok()?;
                let TokenKind::Decimal(text) = &self.lower.tokens[index].kind else { return None };
                let (c,e) = decimal::parts(text)?;
                ("float", Number::Constant(decimal::encode(c,e)))
            }
            TermKind::FieldRead(owner,field) if self.binding(owner).is_none() =>
                ("int",Number::GlobalInt(self.lower.integer_slot(&format!("{owner}.{field}"))?)),
            TermKind::Name(n) if self.binding(n).is_none()=>("int",Number::GlobalInt(self.lower.integer_slot(n)?)),
            TermKind::FieldRead(owner,field) => {
                let b=self.readable(owner)?;
                if !self.views.contains(&b.slot) {return None;}
                let ty=self.lower.objects.as_ref()?.named_field(&b.ty,field)?;
                if !matches!(ty,"int"|"bool"|"float") {return None;}
                return Some((ty.to_owned(),Number::Container(b.slot.clone(),ContainerRead::Field(field.clone()))));
            }
            TermKind::Name(n) => {
                let b = self.readable(n)?;
                let ty = self.narrowed.borrow().get(&b.slot).cloned().unwrap_or_else(|| b.ty.clone());
                if !matches!(ty.as_str(), "int" | "bool" | "float") { return None; }
                let value = if self.views.contains(&b.slot) || b.ty.starts_with("Optional[") { Number::Container(b.slot.clone(), ContainerRead::Scalar) }
                    else { Number::Local(b.slot.clone()) };
                return Some((ty, value));
            }
            TermKind::Scalar(ScalarLiteral::Integer(n)) => ("int", Number::Constant(integer_i64(n).ok()?)),
            TermKind::Scalar(ScalarLiteral::Boolean(b)) => ("bool", Number::Constant(i64::from(*b))),
            TermKind::OptionalCondition(name, OptionalPredicate::IsMissing(equal)) => {
                let binding = self.binding(name)?;
                if !binding.ty.starts_with("Optional[") || !(self.views.contains(&binding.slot) || self.containers.borrow().contains(&binding.slot)) { return None; }
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
                if ty != "str" { return None; }
                let (i,j)=(self.offset(i)?,self.offset(j)?);
                ("int",if self.views.contains(&binding.slot) || binding.ty.starts_with("Optional[") {Number::ViewByte(binding.slot.clone(),i,j)}
                    else {Number::LocalByte(binding.slot.clone(),i,j)})
            }
            TermKind::MethodCall(owner,method,args) if owner=="float" && self.binding(owner).is_none() &&
                self.lower.float_special(owner,method,args).is_some()=>("float",self.lower.float_special(owner,method,args)?),
            TermKind::MethodCall(owner,method,args) if method=="to_int" && args.is_empty()
                && self.binding(owner).is_some_and(|b|b.ty=="str" && !self.views.contains(&b.slot)) =>
                ("int", Number::ParseInt(self.binding(owner)?.slot.clone())),
            TermKind::MethodCall(owner,method,args) if self.binding(owner).is_some() =>
                return self.edit(owner,method,args),
            TermKind::BinaryInt(n) if unary_minus(n,self.lower.tokens) => {
                let (ty,rhs)=self.expr(&n.right)?;
                let zero=Box::new(Number::Constant(0));
                let rhs=Box::new(rhs);
                match ty.as_str() {
                    "float"=>("float",Number::Decimal(zero,'~',rhs)),
                    "int"=>("int",Number::Binary(zero,'-',rhs)),
                    _=>return None,
                }
            }
            TermKind::BinaryInt(n) => {
                let (ty, left) = self.expr(&n.left)?;
                let right = Box::new(self.typed(&n.right,&ty)?);
                let left = Box::new(left);
                match ty.as_str() {
                    "float" if matches!(n.operator, '+' | '-' | '*' | '/') => ("float", Number::Decimal(left,n.operator,right)),
                    "int" => ("int", Number::Binary(left,n.operator,right)),
                    _ => return None,
                }
            }
            TermKind::Not(inner) => ("bool",Number::Logic("==",Box::new(self.typed(inner,"bool")?),Box::new(Number::Constant(0)))),
            TermKind::Logic(op,a,b) => {
                let (ty,left) = self.expr(a)?;
                if !public_format::logic_types(op, &ty) { return None; }
                ("bool", Number::Logic(if ty=="float" {"decimal!="} else {op}, Box::new(left), Box::new(self.typed(b,&ty)?)))
            }
            TermKind::UnresolvedZeroArgs(n) => return self.call(n,false,std::iter::empty()),
            TermKind::Call(c) => return self.call(&c.name,c.awaited,c.arguments.iter().map(|v|(None,v))),
            TermKind::Construct(n,args) => return self.call(n,false,args.iter().map(|(n,v)|(Some(n.as_str()),v))),
            TermKind::MethodCall(m,n,args) => return self.call(&format!("{m}.{n}"),false,args.iter().map(|v|(None,v))),
            _ => return None,
        };
        Some((ty.to_owned(),value))
    }

    fn offset(&self, offset: &StringOffset) -> Option<StringOffset> {
        match offset {
            StringOffset::Constant(i) => Some(StringOffset::Constant(*i)),
            StringOffset::Named(n) => {
                let b=self.readable(n)?;
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
        ((self.views.contains(&b.slot) || self.containers.borrow().contains(&b.slot)) && !source.contains("str") && source.is_subset(&target)).then(||b.slot.clone())
    }

    fn item(&self, term:&Term, expected:&str) -> Option<ContainerItem> {
        use containers::Value;
        let constructor = match &term.kind {
            TermKind::Construct(n,args) => Some((n.as_str(),args.as_slice())),
            TermKind::UnresolvedZeroArgs(n) => Some((n.as_str(),&[][..])),
            _ => None,
        };
        if let Some((ctor,args))=constructor {
            let objects=self.lower.objects.as_ref()?;
            if let Some(defaults)=objects.record_defaults(expected,ctor) {
                let mut provided=BTreeSet::new();
                let mut fields=Vec::new();
                for (field,arg) in args {
                    if !provided.insert(field.as_str()) {return None;}
                    fields.push((field.clone(),self.item(arg,objects.named_field(expected,field)?)?));
                }
                for (field,default) in defaults {
                    if !provided.contains(field.as_str()) { fields.push((field,ContainerItem::Literal(default?))); }
                }
                return Some(ContainerItem::Record(fields));
            }
        }
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
                if !permits(&ty) {return None;}
                match ty.as_str() { "float"=>Some(ContainerItem::Decimal(n)), "int"|"bool"=>Some(ContainerItem::Scalar(n,ty=="bool")), _=>None }
            }
        }
    }

    fn edit(&self, name:&str, method:&str, args:&[Term]) -> Option<(String,Number)> {
        let b=self.binding(name)?;
        if !b.mutable || !self.containers.borrow().contains(&b.slot) || self.borrowed.contains(&b.slot) {return None;}
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
        self.invalidate();
        Some((result.into(),Number::Edit(b.slot.clone(),Box::new(edit))))
    }

    fn typed(&self, t: &Term, expected: &str) -> Option<Number> {
        self.expr(t).and_then(|(ty,n)| (ty==expected).then_some(n))
    }

    fn standard_call<'t>(&self,name:&str,args:impl Iterator<Item=(Option<&'t str>,&'t Term)>,expected:&str)->Option<StandardCall> {
        if self.binding(name).is_some() || self.lower.objects.as_ref()?.function(name).is_some() || self.lower.globals.contains_key(name) {return None;}
        let (params,result)=standard::signature(name)?;
        if result!=expected {return None;}
        let mut values=Vec::new();
        for (i,(label,t)) in args.enumerate() {
            if label.is_some() {return None;}
            let ty=params.get(i).copied().or_else(||standard::variadic_parameter(name))?;
            values.push(if ty=="int" { StandardArg::Int(self.typed(t,"int")?) }
                else if let TermKind::Name(name)=&t.kind {
                    let b=self.binding(name)?;
                    if b.ty!="str" || self.views.contains(&b.slot) {return None;}
                    StandardArg::Read(b.slot.clone())
                } else { StandardArg::Text(self.text(t)?) });
        }
        if values.len()<params.len() {return None;}
        self.invalidate();
        Some(StandardCall {name:name.into(),args:values})
    }

    fn formatted(&self,t:&Term)->Option<LocalText> {
        public_format::format_parts(self.lower.tokens,t,|n|{
            let b=self.binding(n)?;
            (b.ty=="str" && !self.views.contains(&b.slot)).then(||b.slot.clone())
        },|term|self.expr(term)).map(LocalText::Formatted)
    }

    fn replacement(&self,name:&str,index:&StringOffset,rhs:&GraphemeReplacement)->Option<LocalReplacement> {
        let b=self.binding(name)?;
        if b.ty!="str" || !b.mutable || self.views.contains(&b.slot) {return None;}
        let rhs=match rhs {
            GraphemeReplacement::Literal(s) if crate::owned_str_runtime::single_grapheme(s)=>rhs.clone(),
            GraphemeReplacement::Named(n)=>{
                let source=self.binding(n)?;
                if source.ty!="str" || self.views.contains(&source.slot) {return None;}
                GraphemeReplacement::Named(source.slot.clone())
            }
            _=>return None,
        };
        Some(LocalReplacement{owner:b.slot.clone(),index:self.offset(index)?,rhs})
    }
    fn text_argument(&self,callee:&str,param:&Parameter,term:&Term)->Option<Argument> {
        if let TermKind::Name(owner)=&term.kind {
            let binding=self.readable(owner)?;
            if binding.ty!="str" || self.views.contains(&binding.slot) ||
                !self.lower.objects.as_ref()?.allows_shared(callee,&param.name) {return None;}
            return Some(Argument::SharedText(binding.slot.clone()));
        }
        Some(Argument::Text(self.text(term)?))
    }

    fn text_function<'t>(&self, name: &str, args: impl Iterator<Item=(Option<&'t str>, &'t Term)>) -> Option<LocalText> {
        if self.binding(name.split('.').next()?).is_some() { return None; }
        let (target, _, function) = self.lower.objects.as_ref()?.function(name)?;
        if function.is_async || function.result_ty != "str" { return None; }
        let mut arguments = Vec::new();
        let mut supplied = BTreeSet::new();
        let mut shared=BTreeSet::new();
        for (position, (label, term)) in args.enumerate() {
            if writes_argument(term,&shared) {return None;}
            let slot = match label {
                Some(n) => function.params.iter().position(|param| param.name == n)?,
                None => position,
            };
            let param = function.params.get(slot)?;
            if !supplied.insert(slot) { return None; }
            let value = match param.ty.as_str() {
                "str" => self.text_argument(name,param,term)?,
                ty @ ("int" | "bool" | "float") => Argument::Integer(self.typed(term, ty)?),
                _ => return None,
            };
            if let (Argument::SharedText(_),TermKind::Name(n))=(&value,&term.kind) {shared.insert(n.clone());}
            arguments.push((param.name.clone(), slot, value));
        }
        if supplied.len() != function.params.len() { return None; }
        self.invalidate();
        Some(LocalText::Call(TextInvocation { target: target.clone(), arguments }))
    }
    fn text(&self,t:&Term)->Option<LocalText> {
        match &t.kind {
            TermKind::Group(inner)=>self.text(inner),
            TermKind::Literal("formatted")=>self.formatted(t),
            TermKind::StringReplace(n,i,rhs)=>Some(LocalText::Replace(self.replacement(n,i,rhs)?)),
            TermKind::UnresolvedZeroArgs(n) => self.text_function(n, std::iter::empty()),
            TermKind::Call(c) if !c.awaited => {
                let args = || c.arguments.iter().map(|t| (None, t));
                if self.lower.objects.as_ref()?.function(&c.name).is_some() {
                    self.text_function(&c.name, args())
                } else {
                    self.standard_call(&c.name, args(), "str").map(LocalText::Standard)
                }
            }
            TermKind::Construct(n,args) => self.text_function(n,args.iter().map(|(k,t)|(Some(k.as_str()),t))),
            TermKind::MethodCall(m,n,args) => self.text_function(&format!("{m}.{n}"),args.iter().map(|t|(None,t))),
            TermKind::StringIndex(n,i,None) => {
                let b=self.binding(n)?;
                if b.ty!="str" || self.views.contains(&b.slot) {return None;}
                Some(LocalText::Grapheme(b.slot.clone(),self.offset(i)?))
            }
            _=>match self.lower.text(t)? {Text::Literal(s)=>Some(LocalText::Literal(s)), _=>None},
        }
    }

    fn call<'t>(&self,name:&str,awaited:bool,args: impl Iterator<Item=(Option<&'t str>, &'t Term)>) -> Option<(String,Number)> {
        if self.binding(name.split('.').next()?).is_some() { return None; }
        let objects = self.lower.objects.as_ref()?;
        if !awaited && name == "len" && objects.function(name).is_none() && !self.lower.globals.contains_key(name) {
            let mut args = args;
            let (None, term) = args.next()? else { return None };
            let TermKind::Name(owner) = &term.kind else { return None };
            if args.next().is_some() { return None; }
            let binding = self.binding(owner)?;
            if binding.ty=="str" && !self.views.contains(&binding.slot) {
                return Some(("int".into(),Number::TextLength(binding.slot.clone())));
            }
            if element_type(&binding.ty).is_none() || !(self.containers.borrow().contains(&binding.slot) || self.views.contains(&binding.slot)) { return None; }
            self.invalidate();
            return Some(("int".into(), Number::Container(binding.slot.clone(), ContainerRead::Length)));
        }
        if !awaited && objects.function(name).is_none() && !self.lower.globals.contains_key(name) {
            let call=self.standard_call(name,args,"int")?;
            return Some(("int".into(),Number::Standard(call)));
        }
        let (target,_,f) = objects.function(name)?;
        if f.is_async!=awaited || !matches!(f.result_ty.as_str(), "int" | "bool" | "float" | "none") { return None; }
        if !awaited && f.params.iter().any(|p|stored_type(&p.ty)) {
            let call=self.collection_call(name,args,&f.result_ty)?;
            return Some((f.result_ty.clone(),Number::FromCollection(call)));
        }
        let takes_text = f.params.iter().any(|param| param.ty == "str");
        let mut text_arguments = Vec::new();
        let mut values = Vec::new();
        let mut slots = BTreeSet::new();
        let mut shared=BTreeSet::new();
        for (index,(label,arg)) in args.enumerate() {
            if writes_argument(arg,&shared) {return None;}
            let slot = match label { Some(n) => f.params.iter().position(|p| p.name==n)?, None => index };
            let param = f.params.get(slot)?;
            if !slots.insert(slot) { return None; }
            if takes_text {
                let value = match param.ty.as_str() {
                    "str" => self.text_argument(name,param,arg)?,
                    ty @ ("int" | "bool" | "float") => Argument::Integer(self.typed(arg, ty)?),
                    _ => return None,
                };
                if let (Argument::SharedText(_),TermKind::Name(n))=(&value,&arg.kind) {shared.insert(n.clone());}
                text_arguments.push((param.name.clone(), slot, value));
            } else {
                values.push((param.name.clone(),slot,self.typed(arg,&param.ty)?));
            }
        }
        if slots.len()!=f.params.len() { return None; }
        self.invalidate();
        if takes_text {
            return Some((f.result_ty.clone(), Number::TextCall(TextInvocation {
                target: target.clone(), arguments: text_arguments,
            })));
        }
        Some((f.result_ty.clone(), Number::ScalarCall { awaited, target:target.clone(), arguments:values }))
    }

    fn collection_call<'t>(&self,name:&str,args:impl Iterator<Item=(Option<&'t str>,&'t Term)>,expected:&str)->Option<CollectionCall> {
        if self.binding(name.split('.').next()?).is_some() {return None;}
        let objects=self.lower.objects.as_ref()?;
        let (target,origin,f)=objects.function(name)?;
        if f.is_async || !objects.same_type(expected,origin,&f.result_ty) {return None;}
        let mut arguments=Vec::new();
        let mut seen=BTreeSet::new();
        for (i,(label,t)) in args.enumerate() {
            let index=match label { Some(n)=>f.params.iter().position(|p|p.name==n)?, None=>i };
            let p=f.params.get(index)?;
            if !seen.insert(index) {return None;}
            let arg=if p.ty.starts_with("Optional[") && !matches!(t.kind,TermKind::Move(_)) {
                CollectionArgument::Optional(self.item(t,&p.ty)?)
            } else if stored_type(&p.ty) {
                let TermKind::Move(from)=&t.kind else {return None};
                let b=self.scopes.last()?.get(from)?;
                if !objects.same_type(&b.ty,origin,&p.ty) || self.borrowed.contains(&b.slot) || !self.containers.borrow_mut().remove(&b.slot) {return None;}
                CollectionArgument::Move(b.slot.clone())
            } else {CollectionArgument::Scalar(self.typed(t,&p.ty)?)};
            arguments.push((p.name.clone(),index,arg));
        }
        if seen.len()!=f.params.len() {return None;}
        self.invalidate();
        Some(CollectionCall{target:target.clone(),arguments})
    }

    fn container(&mut self, stmt: &Stmt, declaration: &crate::syntax_shared::ParsedDeclaration<String, Term>) -> Option<()> {
        if !self.lower.objects.as_ref()?.valid_storage(&declaration.ty) { return None; }
        let initial = declaration.initializer.as_ref()?;
        let returns_storage=|name:&str|self.lower.objects.as_ref().and_then(|o|o.function(name)).is_some_and(|(_,_,f)|stored_type(&f.result_ty));
        let value = match &initial.kind {
            TermKind::UnresolvedZeroArgs(n) if returns_storage(n)=>ContainerInit::Call(self.collection_call(n,std::iter::empty(),&declaration.ty)?),
            TermKind::Call(c) if !c.awaited && returns_storage(&c.name)=>ContainerInit::Call(self.collection_call(&c.name,c.arguments.iter().map(|t|(None,t)),&declaration.ty)?),
            TermKind::Construct(n,args) if returns_storage(n)=>ContainerInit::Call(self.collection_call(n,args.iter().map(|(k,t)|(Some(k.as_str()),t)),&declaration.ty)?),
            TermKind::MethodCall(m,n,args) if returns_storage(&format!("{m}.{n}"))=>ContainerInit::Call(self.collection_call(&format!("{m}.{n}"),args.iter().map(|t|(None,t)),&declaration.ty)?),
            TermKind::Call(c) if !c.awaited && c.name == "read_bytes" && declaration.ty == "list[int]" =>
                ContainerInit::ReadBytes(self.standard_call(&c.name,c.arguments.iter().map(|v|(None,v)),&declaration.ty)?),
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
                let owner = self.scopes[self.move_floor..].iter().rev()
                    .find_map(|scope| scope.get(source))?;
                if owner.ty != declaration.ty || self.borrowed.contains(&owner.slot) { return None; }
                let slot = owner.slot.clone();
                if !self.containers.borrow_mut().remove(&slot) { return None; }
                ContainerInit::Move(slot)
            }
            _ if declaration.ty.starts_with("Optional[")=>ContainerInit::Optional(self.item(initial,&declaration.ty)?),
            _ => return None,
        };
        let slot = format!("@{}", self.next_slot);
        self.next_slot += 1;
        self.containers.borrow_mut().insert(slot.clone());
        self.storage.push(Storage::Container(slot.clone()));
        self.step(stmt, Action::Container(slot.clone(),value));
        self.scopes.last_mut()?.insert(declaration.name.clone(), Binding {
            slot, ty: declaration.ty.clone(), mutable: declaration.mutable,
        });
        Some(())
    }

    fn element(&self, owner: &str, selector: &Term) -> Option<(String, String, ContainerKey)> {
        let binding = self.binding(owner)?;
        if !(self.containers.borrow().contains(&binding.slot) || self.views.contains(&binding.slot)) { return None; }
        let (key_type, value_type) = element_type(&binding.ty)?;
        let key = if key_type == "str" {
            if let TermKind::Name(n)=&selector.kind {
                let source=self.binding(n)?;
                if source.ty!="str" || self.views.contains(&source.slot) {return None;}
                ContainerKey::NamedText(source.slot.clone())
            } else {
                let Text::Literal(text) = self.lower.text(selector)? else { return None };
                ContainerKey::Text(text)
            }
        } else { ContainerKey::Index(self.typed(selector,key_type)?) };
        Some((binding.slot.clone(), value_type.into(), key))
    }

    fn invalidate(&self) {
        self.flags.borrow_mut().clear();
        self.narrowed.borrow_mut().clear();
    }

    fn presence(&self, term: &Term, flags: bool) -> Option<Presence> {
        match &term.kind {
            TermKind::Group(t) => self.presence(t, flags),
            TermKind::Not(t) => {
                let mut fact=self.presence(t,flags)?;
                fact.present_when_true=!fact.present_when_true;
                Some(fact)
            }
            TermKind::Name(n) if flags => self.flags.borrow().get(&self.binding(n)?.slot).cloned(),
            TermKind::OptionalCondition(n,OptionalPredicate::IsMissing(equal)) => {
                let b=self.binding(n)?;
                let ty=b.ty.strip_prefix("Optional[")?.strip_suffix(']')?;
                if ty.contains(',') {return None;}
                Some(Presence { slot:b.slot.clone(), candidate:ty.into(), present_when_true:!*equal })
            }
            _=>None,
        }
    }

    fn refine(&self, condition:&Term, truth:bool) {
        if let Some(f)=self.presence(condition,true) {
            if f.present_when_true==truth {self.narrowed.borrow_mut().insert(f.slot,f.candidate);}
        } else if let TermKind::OptionalCondition(n,OptionalPredicate::Candidate(Some(ty)))=&condition.kind {
            if truth {
                if let Some(b)=self.binding(n) {self.narrowed.borrow_mut().insert(b.slot.clone(),ty.clone());}
            }
        }
    }

    fn step(&mut self, s:&Stmt, action:Action) -> usize {
        let at=self.body.len();
        self.body.push(Step{text_access:vec![],expire:vec![],active_view:!self.borrowed.is_empty(),handlers:vec![],module:self.module,span:(s.start,s.end),action});
        at
    }
    fn patch(&mut self, at:usize, to:usize) {
        match &mut self.body[at].action {
            Action::Unless(_,v) | Action::Jump(v) => *v=to,
            _ => unreachable!(),
        }
    }
    fn block(&mut self, stmts:&[Stmt], nested:bool) -> Result<bool, Diagnostic> {
        // 迴圈、例外與元素借用各自建立移動邊界；if 的兩臂則在既有邊界內合流。
        let floor = if nested { self.scopes.len() } else { self.scopes.len() - 1 };
        let previous = std::mem::replace(&mut self.move_floor, floor);
        let result = self.branch_block(stmts, nested);
        self.move_floor = previous;
        result
    }
    fn branch_block(&mut self, stmts:&[Stmt], nested:bool) -> Result<bool, Diagnostic> {
        if nested { self.scopes.push(HashMap::new()); }
        let storage_depth = self.storage.len();
        let mut closed=false;
        for s in stmts {
            let err=|msg| problem(self.lower.source,(s.start,s.end),msg);
            if closed { return Err(err("控制轉移後不可再有語句")); }
            match &s.kind {
                StmtKind::Declare(d) => {
                    if self.scopes.last().unwrap().contains_key(&d.name) { return Err(err("重複區域宣告")); }
                    if d.ty=="str" {
                        let value=d.initializer.as_ref().and_then(|t|if !d.mutable && matches!(t.kind,TermKind::Literal("template")) {
                            self.formatted(t)
                        } else {self.text(t)}).ok_or_else(||err("需要可獨立持有的字串"))?;
                        let slot=format!("@{}",self.next_slot);
                        self.next_slot+=1;
                        self.scopes.last_mut().unwrap().insert(d.name.clone(),Binding{slot:slot.clone(),ty:d.ty.clone(),mutable:d.mutable});
                        self.storage.push(Storage::Text(slot.clone()));
                        self.step(s,Action::StoreText{target:slot,declaration:Some(d.mutable),value});
                        continue;
                    }
                    if stored_type(&d.ty) {
                        self.container(s,d).ok_or_else(|| err("容器需要具型別常值或有效的整筆移動來源"))?;
                        continue;
                    }
                    let slot=format!("@{}",self.next_slot); self.next_slot+=1;
                    let value = match &d.initializer {
                        Some(t) => self.typed(t, &d.ty).ok_or_else(||err("區域純量初值型別不符"))?,
                        None if d.mutable && matches!(d.ty.as_str(), "int" | "bool" | "float") => {
                            self.uninitialized.insert(slot.clone());
                            self.scopes.last_mut().unwrap().insert(d.name.clone(), Binding {
                                slot, ty: d.ty.clone(), mutable: true,
                            });
                            continue;
                        }
                        None => return Err(err("此宣告需要初值")),
                    };
                    if d.ty=="bool" {
                        if let Some(fact)=d.initializer.as_ref().and_then(|t|self.presence(t,false)) {
                            self.flags.borrow_mut().insert(slot.clone(),fact);
                        }
                    }
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
                    self.storage.push(Storage::Container(slot.clone()));
                    self.scopes.push(HashMap::from([(alias.clone(), Binding { slot:slot.clone(), ty, mutable: false })]));
                    closed = self.block(&block.body.statements, false)?;
                    self.storage.pop();
                    self.borrowed.pop();
                    self.flags.borrow_mut().clear();
                    if !closed { self.step(s,Action::DropStorage(vec![Storage::Container(slot)])); }
                    self.scopes.pop();
                }
                StmtKind::GraphemeWrite(t)=>{
                    let TermKind::StringReplace(n,i,rhs)=&t.kind else {return Err(err("需要字素寫入"))};
                    let change=self.replacement(n,i,rhs).ok_or_else(||err("字素寫入的接收者或右值無效"))?;
                    self.step(s,Action::ReplaceLocal(change));
                }
                StmtKind::ProcedureCall(call) => {
                    let value = self.typed(call, "none").ok_or_else(|| err("獨立函式呼叫需回傳 none"))?;
                    self.step(s, Action::Evaluate(value));
                }
                StmtKind::Evaluate(t) => {
                    let (_,v)=self.expr(t).ok_or_else(||err("容器操作的權限或參數型別不符"))?;
                    self.step(s,Action::Evaluate(v));
                }
                StmtKind::Assign(a) if self.binding(&a.name).is_none() && self.lower.integer_slot(&a.name).is_some() => {
                    let slot=self.lower.integer_slot(&a.name).unwrap();
                    if self.lower.readonly_slots.contains(&slot) {return Err(err("匯入的 share 不可寫入"));}
                    let value=self.typed(&a.expression,"int").ok_or_else(||err("share 賦值需要 int"))?;
                    self.step(s,Action::StoreInt(slot,value));
                }
                StmtKind::Assign(a) => {
                    let b=self.binding(&a.name).filter(|b|b.mutable).ok_or_else(||err("缺少可寫區域純量"))?;
                    if b.ty.starts_with("Optional[") {
                        let value=self.item(&a.expression,&b.ty).ok_or_else(||err("Optional 賦值型別不符"))?;
                        let slot=b.slot.clone();
                        self.invalidate();
                        self.containers.borrow_mut().insert(slot.clone());
                        self.step(s,Action::Evaluate(Number::Edit(slot,Box::new(Edit::ReplaceOptional(value)))));
                        continue;
                    }
                    if b.ty=="str" && !self.views.contains(&b.slot) {
                        let value=self.text(&a.expression).ok_or_else(||err("需要字串右值"))?;
                        self.step(s,Action::StoreText{target:b.slot.clone(),declaration:None,value});
                        continue;
                    }
                    let v=self.typed(&a.expression,&b.ty).ok_or_else(||err("賦值型別不符"))?;
                    self.flags.borrow_mut().remove(&b.slot);
                    let slot = b.slot.clone();
                    self.uninitialized.remove(&slot);
                    self.step(s,Action::Bind(slot,v));
                }
                StmtKind::Try { body, handler, category, binding } => {
                    self.invalidate();
                    let start = self.body.len();
                    let entry = self.uninitialized.clone();
                    let normal_returns = self.block(body,true)?;
                    let normal = std::mem::replace(&mut self.uninitialized, entry);
                    let end = self.body.len();
                    let jump = (!normal_returns).then(||self.step(s,Action::Jump(0)));
                    let target = self.body.len();
                    let mut cleanup = Vec::new();
                    for step in &self.body[start..end] {
                        match &step.action {
                            Action::Container(slot,_) | Action::Select { alias:slot,.. } => cleanup.push(Storage::Container(slot.clone())),
                            Action::StoreText {target,declaration:Some(_),..}=>cleanup.push(Storage::Text(target.clone())),
                            _ => (),
                        }
                    }
                    cleanup.reverse();
                    for step in &mut self.body[start..end] {
                        step.handlers.push(Handler { category:category.clone(),target,cleanup:cleanup.clone() });
                    }
                    self.invalidate();
                    self.scopes.push(HashMap::new());
                    let message=binding.as_ref().map(|name| {
                        let slot=format!("@{}",self.next_slot);
                        self.next_slot+=1;
                        self.scopes.last_mut().unwrap().insert(name.clone(),Binding{slot:slot.clone(),ty:"str".into(),mutable:false});
                        self.step(s,Action::StoreText{target:slot.clone(),declaration:Some(false),value:LocalText::CaughtMessage});
                        self.storage.push(Storage::Text(slot.clone()));
                        slot
                    });
                    let handled_returns = self.block(handler,false)?;
                    if let Some(slot)=message {
                        self.storage.pop();
                        if !handled_returns { self.step(s,Action::DropStorage(vec![Storage::Text(slot)])); }
                    }
                    self.scopes.pop();
                    self.join_initialization(normal, normal_returns, handled_returns);
                    if let Some(jump) = jump { self.patch(jump,self.body.len()); }
                    closed = normal_returns && handled_returns;
                    self.invalidate();
                }
                StmtKind::ReturnAdd(name,term) if self.binding(name).is_none() && self.lower.integer_slot(name).is_some() => {
                    let slot=self.lower.integer_slot(name).unwrap();
                    if self.result!="int" || self.lower.readonly_slots.contains(&slot) {return Err(err("share 加法賦值的權限或返回型別不符"));}
                    let rhs=self.typed(term,"int").ok_or_else(||err("share 加法賦值需要 int"))?;
                    self.step(s,Action::StoreInt(slot,Number::Binary(Box::new(Number::GlobalInt(slot)),'+',Box::new(rhs))));
                    self.step(s,Action::Return(Number::GlobalInt(slot))); closed=true;
                }
                StmtKind::ReturnAdd(name, term) => {
                    let binding = self.readable(name).filter(|b| b.mutable && b.ty == self.result)
                        .ok_or_else(|| err("回傳加法賦值需要可讀寫且與返回型別相同的變數"))?;
                    let slot = binding.slot.clone();
                    let lhs = Box::new(Number::Local(slot.clone()));
                    let rhs = Box::new(self.typed(term, self.result).ok_or_else(|| err("加法賦值型別不符"))?);
                    let value = match self.result {
                        "int" => Number::Binary(lhs, '+', rhs),
                        "float" => Number::Decimal(lhs, '+', rhs),
                        _ => return Err(err("加法賦值需要數值")),
                    };
                    self.step(s, Action::Bind(slot.clone(), value));
                    self.step(s, Action::Return(Number::Local(slot)));
                    closed = true;
                }
                StmtKind::Return(r) if self.result == "none" && matches!(r.expression.kind, TermKind::Literal("none")) => {
                    self.step(s, Action::Return(Number::Constant(0)));
                    closed = true;
                }
                StmtKind::Return(r) if self.result == "str" => {
                    let text = self.text(&r.expression).ok_or_else(||err("返回需要獨立持有的字串"))?;
                    self.step(s, Action::ReturnText(text));
                    closed = true;
                }
                StmtKind::Return(r) if self.result.starts_with("Optional[") && !matches!(r.expression.kind,TermKind::Move(_)) => {
                    let value=self.item(&r.expression,self.result).ok_or_else(||err("Optional 返回值型別不符"))?;
                    self.step(s,Action::ReturnOptional(value)); closed=true;
                }
                StmtKind::Return(r) if stored_type(self.result) => {
                    let TermKind::Move(from)=&r.expression.kind else {return Err(err("容器返回需要明確移動"));};
                    let b=self.binding(from).ok_or_else(||err("未知的容器"))?;
                    if b.ty!=self.result || self.borrowed.contains(&b.slot) || !self.containers.borrow().contains(&b.slot) {return Err(err("容器返回型別或所有權不符"));}
                    self.step(s,Action::ReturnCollection(b.slot.clone())); closed=true;
                }
                StmtKind::Return(r) => {
                    let v=self.typed(&r.expression,self.result).ok_or_else(||err("回傳型別不符"))?;
                    self.step(s,Action::Return(v)); closed=true;
                }
                StmtKind::Match {value,arms} => {
                    let (ty,subject)=self.expr(value).filter(|(ty,_)|matches!(ty.as_str(),"int"|"bool"))
                        .ok_or_else(||err("match 需要整數或布林值"))?;
                    let name=format!("@match/{}",s.start);
                    self.step(s,Action::Bind(name.clone(),subject));
                    self.scopes.push(HashMap::from([(name.clone(),Binding{slot:name.clone(),ty:ty.clone(),mutable:false})]));
                    let mut seen=BTreeSet::new();
                    let mut tests=Vec::new();
                    for (pattern,block) in arms {
                        let test=if let Some(p)=pattern {
                            let key=match (&p.kind,ty.as_str()) {
                                (TermKind::Scalar(ScalarLiteral::Integer(n)),"int")=>integer_i64(n).ok(),
                                (TermKind::BinaryInt(_),"int") => {
                                    let tokens = self.lower.tokens;
                                    let first = tokens.partition_point(|token| token.start < p.start);
                                    match tokens.get(first..first + 2) {
                                        Some([minus, literal]) if minus.kind == TokenKind::Symbol('-') && literal.end == p.end =>
                                            match &literal.kind {
                                                TokenKind::Integer(raw) => crate::syntax_shared::negative_integer_i64(raw).ok(),
                                                _ => None,
                                            },
                                        _ => None,
                                    }
                                },
                                (TermKind::Scalar(ScalarLiteral::Boolean(b)),"bool")=>Some(i64::from(*b)),
                                _=>None,
                            }.ok_or_else(||err("case 需要與比對值同型的常值"))?;
                            if !seen.insert(key) {return Err(err("case 常值重複"));}
                            let local=Term{kind:TermKind::Name(name.clone()),start:value.start,end:value.end};
                            Some(if ty=="int" {
                                Term{kind:TermKind::Logic("==",Box::new(local),Box::new(p.clone())),start:p.start,end:p.end}
                            } else if key==1 {local} else {
                                Term{kind:TermKind::Not(Box::new(local)),start:p.start,end:p.end}
                            })
                        } else {None};
                        if ty=="bool" && seen.len()==2 {
                            if tests.len()+1!=arms.len() {return Err(err("所有布林值都已有分支"));}
                            tests.push((None,block));
                        } else {tests.push((test,block));}
                    }
                    let mut tail=None;
                    for (condition,block) in tests.into_iter().rev() {
                        tail=Some(if let Some(condition)=condition {
                            let branch=crate::syntax_shared::ParsedIfArm{start:condition.start,
                                branch:crate::common_ir::ConditionalBranch::new(condition,block.clone())};
                            crate::syntax_shared::ParsedElse::ElseIf(Box::new(Stmt{start:s.start,end:block.end,
                                kind:StmtKind::If{branch,no:tail}}))
                        } else {crate::syntax_shared::ParsedElse::Block(block.clone())});
                    }
                    closed=self.branch_block(tail.as_ref().unwrap().as_slice(),true)?;
                    self.scopes.pop();
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
                    let entry_flags=self.flags.borrow().clone();
                    let before=self.narrowed.borrow().clone();
                    self.refine(&branch.condition,true);
                    let entry_owners=self.containers.borrow().clone();
                    let entry = self.uninitialized.clone();
                    let yes=self.branch_block(&branch.body.statements,true)?;
                    let yes_pending = std::mem::replace(&mut self.uninitialized, entry);
                    let yes_owners=self.containers.replace(entry_owners.clone());
                    let yes_facts=self.narrowed.replace(before.clone());
                    let end=(!yes).then(||self.step(s,Action::Jump(0)));
                    self.patch(skip,self.body.len());
                    self.flags.replace(entry_flags);
                    self.refine(&branch.condition,false);
                    let no_closed=if let Some(other)=no { self.branch_block(other.as_slice(),true)? } else {false};
                    self.join_initialization(yes_pending, yes, no_closed);
                    let no_owners=self.containers.replace(entry_owners);
                    self.containers.borrow_mut().retain(|slot|
                        (yes || yes_owners.contains(slot)) && (no_closed || no_owners.contains(slot)));
                    let no_facts=self.narrowed.replace(before);
                    self.narrowed.borrow_mut().retain(|slot,ty|
                        (yes || yes_facts.get(slot)==Some(ty)) && (no_closed || no_facts.get(slot)==Some(ty)));
                    self.flags.borrow_mut().clear();
                    closed=yes && no_closed;
                    if let Some(at)=end { self.patch(at,self.body.len()); }
                }
                StmtKind::Break | StmtKind::Continue => {
                    let depth=self.loops.last().ok_or_else(||err("控制轉移需要所在迴圈"))?.storage_depth;
                    if depth<self.storage.len() {self.step(s,Action::DropStorage(self.storage[depth..].to_vec()));}
                    let at=self.step(s,Action::Jump(0));
                    let scope=self.loops.last_mut().ok_or_else(||err("控制轉移需要所在迴圈"))?;
                    if matches!(s.kind,StmtKind::Break) { scope.breaks.push(at); } else { scope.continues.push(at); }
                    closed=true;
                }
                StmtKind::While{condition,body} => {
                    self.invalidate();
                    let cond=self.typed(condition,"bool").ok_or_else(||err("while 需要 bool 條件"))?;
                    let head=self.step(s,Action::Unless(cond,0));
                    self.loops.push(Loop{storage_depth:self.storage.len(),breaks:vec![head],continues:Vec::new()});
                    let entry = self.uninitialized.clone();
                    self.block(&body.statements,true)?;
                    self.uninitialized = entry;
                    self.step(s,Action::Jump(head));
                    self.end_loop(head);
                }
                StmtKind::ForRange{iterator,lower,upper,body,..} => {
                    self.invalidate();
                    let endpoint = |bound: &RangeBound| match bound {
                        RangeBound::Literal(_) => bound.constant().ok().map(Number::Constant),
                        RangeBound::Expression(t) => self.typed(t, "int"),
                    };
                    let start=endpoint(lower).ok_or_else(||err("區間下界需要 int"))?;
                    let end=endpoint(upper).ok_or_else(||err("區間上界需要 int"))?;
                    let slot=format!("@{}",self.next_slot); self.next_slot+=1;
                    self.step(s,Action::Bind(slot.clone(),start));
                    let limit=format!("@{}",self.next_slot); self.next_slot+=1;
                    self.step(s,Action::Bind(limit.clone(),end));
                    let test=Number::Logic("lt",Box::new(Number::Local(slot.clone())),Box::new(Number::Local(limit)));
                    let head=self.step(s,Action::Unless(test,0));
                    self.loops.push(Loop{storage_depth:self.storage.len(),breaks:vec![head],continues:Vec::new()});
                    self.scopes.push(HashMap::from([(iterator.clone(),Binding{slot:slot.clone(),ty:"int".into(),mutable:false})]));
                    let entry = self.uninitialized.clone();
                    self.block(&body.statements,false)?;
                    self.uninitialized = entry;
                    self.scopes.pop();
                    let increment=self.step(s,Action::Bind(slot.clone(),Number::Binary(Box::new(Number::Local(slot)),'+',Box::new(Number::Constant(1)))));
                    self.step(s,Action::Jump(head));
                    self.end_loop(increment);
                }
                _ => return Err(err("控制流程中的資源操作尚待驗證")),
            }
        }
        let leaving=self.storage.split_off(storage_depth);
        if !closed && !leaving.is_empty() {self.step(stmts.last().unwrap(),Action::DropStorage(leaving));}
        if nested {
            for binding in self.scopes.pop().unwrap().values() {
                self.uninitialized.remove(&binding.slot);
            }
        }
        Ok(closed)
    }
    fn end_loop(&mut self, next:usize) {
        let scope=self.loops.pop().unwrap();
        for at in scope.breaks { self.patch(at,self.body.len()); }
        for at in scope.continues { self.patch(at,next); }
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;

    fn parsed_main(text: &str) -> Function {
        let source = Source::new(text.as_bytes().to_vec());
        let scanned = lexer::scan(&source);
        assert!(scanned.diagnostics.is_empty());
        let (_, functions) = LanguageParser::<true> {
            source: &source, tokens: &scanned.tokens, cursor: 0,
        }.module_items(None, None).unwrap();
        functions.into_iter().find(|f| f.name == "main").unwrap()
    }

    #[test]
    fn 公開清單宣告與唯讀元素必須走容器降階() {
        for text in [
            "fn main()->int{\nvar xs:list[int] = [1]\nreturn len(xs)\n}",
            "fn main()->int{\nvar xs:list[int] = [1]\nwith xs[0] as item {\nreturn item\n}\n}",
        ] {
            assert!(needed(&parsed_main(text)), "{text}");
        }
    }
}
