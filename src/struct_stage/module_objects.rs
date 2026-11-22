//! 跨模組結構的型別解析、欄位種子與整筆移交。
use super::*;
use crate::mixed_struct_runtime::ScalarKind;

fn kind(name:&str)->Option<ScalarKind> {
    match name { "str"=>Some(ScalarKind::Str),"int"|"float"=>Some(ScalarKind::Int),"bool"=>Some(ScalarKind::Bool),_=>None }
}

fn seed(unit:&Unit,t:&Term)->Option<ApprovedSeed> {
    match &t.kind {
        TermKind::Group(inner)=>seed(unit,inner),
        TermKind::Scalar(ScalarLiteral::Integer(n))=>Some(ApprovedSeed::Int(integer_i64(n).ok()?)),
        TermKind::Scalar(ScalarLiteral::Boolean(v))=>Some(ApprovedSeed::Bool(*v)),
        TermKind::Literal("float")=>{
            let i=unit.tokens.binary_search_by_key(&t.start,|t|t.start).ok()?;
            let TokenKind::Decimal(text)=&unit.tokens[i].kind else {return None};
            let (c,e)=decimal::parts(text)?;
            Some(ApprovedSeed::Int(decimal::encode(c,e)))
        }
        TermKind::Literal("str")=>{
            let i=unit.tokens.binary_search_by_key(&t.start,|t|t.start).ok()?;
            if let TokenKind::StringLiteral(value)=&unit.tokens[i].kind {Some(ApprovedSeed::Str(value.clone()))} else {None}
        }
        _=>None,
    }
}

fn method_value(t:&Term, owner:&str, def:&StructDecl, method:&Function)->Option<Number> {
    match &t.kind {
        TermKind::Group(t)=>method_value(t,owner,def,method),
        TermKind::Scalar(ScalarLiteral::Integer(n))=>Some(Number::Constant(integer_i64(n).ok()?)),
        TermKind::Name(n) if method.params.iter().any(|p|p.name==*n && p.ty=="int")=>Some(Number::Local(format!("$arg/{n}"))),
        TermKind::FieldRead(receiver,n) if receiver=="self" && def.field(n)?.ty=="int" =>
            Some(Number::Member {owner:owner.into(),field:n.clone(),receiver:None}),
        TermKind::BinaryInt(n)=>Some(Number::Binary(Box::new(method_value(&n.left,owner,def,method)?),n.operator,
            Box::new(method_value(&n.right,owner,def,method)?))),
        _=>None,
    }
}

fn method_steps(unit:&Unit, module:usize, owner:&str, def:&StructDecl, method:&Function)->Option<Vec<MethodStep>> {
    if method.is_async || method.result_ty!="int" {return None;}
    let mut names=HashSet::new();
    if method.params.iter().any(|p|p.ty!="int" || p.name=="self" || !names.insert(&p.name)) {return None;}
    let (last,body)=method.body.statements.split_last()?;
    let mut steps=Vec::new();
    for stmt in body.iter().chain(std::iter::once(last)) {
        let (write,expr)=match &stmt.kind {
            StmtKind::FieldAssign{receiver,field,value} if receiver=="self" && !std::ptr::eq(stmt,last)
                && !def.field(field)?.immutable && def.field(field)?.ty=="int" => (Some(field.clone()),value),
            StmtKind::Return(r) if std::ptr::eq(stmt,last)=>(None,&r.expression),
            _=>return None,
        };
        let start=unit.source.span(stmt.start,stmt.end).start;
        steps.push(MethodStep { write,value:method_value(expr,owner,def,method)?,
            origin:StepOrigin {module,span:(stmt.start,stmt.end)},
            site:format!("{}:{}:{}",unit.path.display(),start.line,start.column) });
    }
    Some(steps)
}

fn decimal_term(t:&Term)->bool {
    match &t.kind {TermKind::Group(inner)=>decimal_term(inner),TermKind::Literal("float")=>true,_=>false}
}

pub(super) fn check_definition(unit:&Unit,def:&StructDecl)->Result<(),ModuleError> {
    for f in &def.fields {
        let expected=kind(&f.ty);
        if expected.is_none() || (f.immutable && f.default.is_none()) || f.default.as_ref().is_some_and(|v|(f.ty=="float")!=decimal_term(v) || seed(unit,v).map(|s|s.kind())!=expected) {
            return Err(unit.error(f.declaration_span.start,f.declaration_span.end,"結構欄位型別或預設值缺少有效證明"));
        }
    }
    let mut seen=HashSet::new();
    for m in &def.methods {
        if !seen.insert(&m.name) || method_steps(unit,0,"self",def,m).is_none() {
            return Err(unit.error(m.start,m.end,"跨檔方法需要唯一的已檢查整數欄位讀取"));
        }
    }
    Ok(())
}

struct Object { ty:String, id:DeclarationId, mutable:bool, missing:BTreeSet<String>, moved:std::cell::Cell<bool> }

pub(in crate::struct_stage::module_share) struct ObjectScope<'a> {
    graph:&'a ModuleGraph,
    caller:usize,
    targets:&'a BTreeMap<String,String>,
    owners:&'a BTreeMap<String,usize>,
    variables:HashMap<String,Object>,
}
impl<'a> ObjectScope<'a> {
    pub(super) fn new(graph:&'a ModuleGraph,caller:usize,targets:&'a BTreeMap<String,String>,owners:&'a BTreeMap<String,usize>)->Self {
        Self{graph,caller,targets,owners,variables:HashMap::new()}
    }
    pub(super) fn bind(&mut self,name:&str,ty:&str,site:usize,mutable:bool) {
        self.variables.insert(name.to_owned(),Object{ty:ty.to_owned(),id:DeclarationId(site),mutable,missing:BTreeSet::new(),moved:false.into()});
    }
    pub(in crate::struct_stage::module_share) fn has(&self,n:&str)->bool {self.variables.contains_key(n)}
    pub(in crate::struct_stage::module_share) fn function(&self,n:&str)->Option<(&String,usize,&Function)> {
        let target=self.targets.get(n)?;
        let origin=*self.owners.get(target)?;
        let name=target.split_once(':').map_or(target.as_str(),|(_,n)|n);
        let f=self.graph.units[origin].functions.iter().find(|f|f.name==name)?;
        Some((target,origin,f))
    }
    pub(in crate::struct_stage::module_share) fn allows_shared(&self,name:&str,param:&str)->bool {
        let Some((_,origin,f))=self.function(name) else { return false };
        let tokens=&self.graph.units[origin].tokens;
        let start=tokens.partition_point(|t|t.start<f.body.start);
        let end=tokens.partition_point(|t|t.start<f.body.end);
        !tokens[start..end].windows(3).any(|w|matches!(&w[0].kind,TokenKind::Name(n) if n==param)
            && w[1].kind==TokenKind::Symbol('.') && matches!(&w[2].kind,TokenKind::Name(n) if n=="move"))
    }
    pub(in crate::struct_stage::module_share) fn requires_argument(&self,name:&str)->bool {
        self.function(name).is_some_and(|(_,_,f)|!f.params.is_empty())
    }
    pub(in crate::struct_stage::module_share) fn valid_storage(&self,ty:&str)->bool {
        if let Some((_,inner))=element_type(ty) {return self.valid_storage(inner);}
        matches!(ty,"int"|"float"|"str"|"bool") || ty.starts_with("Optional[") || self.graph.resolve_struct(self.caller,ty).is_some()
    }
    pub(in crate::struct_stage::module_share) fn same_type(&self,local:&str,origin:usize,remote:&str)->bool {
        match (element_type(local),element_type(remote)) {
            (Some((k,a)),Some((other,b))) => local.starts_with("list[")==remote.starts_with("list[") &&
                k==other && self.same_type(a,origin,b),
            (None,None) => match (self.graph.resolve_struct(self.caller,local),self.graph.resolve_struct(origin,remote)) {
                (Some((_,a)),Some((_,b)))=>std::ptr::eq(a,b),
                (None,None)=>local==remote,
                _=>false,
            },
            _=>false,
        }
    }
    pub(in crate::struct_stage::module_share) fn record_defaults(&self,ty:&str,constructor:&str)
        -> Option<Vec<(String,Option<containers::Value>)>> {
        let (origin,def)=self.graph.resolve_struct(self.caller,ty)?;
        if !std::ptr::eq(def,self.graph.resolve_struct(self.caller,constructor)?.1) {return None;}
        let mut fields=Vec::new();
        for field in &def.fields {
            let default=match &field.default {
                None=>None,
                Some(t)=>Some(match seed(&self.graph.units[origin],t)? {
                    ApprovedSeed::Int(v) if field.ty=="float"=>containers::Value::Decimal(v),
                    ApprovedSeed::Int(v)=>containers::Value::Integer(v),
                    ApprovedSeed::Bool(v)=>containers::Value::Boolean(v),
                    ApprovedSeed::Str(v)=>containers::Value::Text(v),
                    _=>return None,
                })
            };
            fields.push((field.name.clone(),default));
        }
        Some(fields)
    }
    pub(in crate::struct_stage::module_share) fn named_field(&self,ty:&str,name:&str)->Option<&str> {
        Some(&self.graph.field(self.caller,ty,name)?.ty)
    }
    pub(in crate::struct_stage::module_share) fn construct(&mut self,name:&str,ty:&str,start:usize,mutable:bool,t:&Term,supplied:Vec<(String,MemberValue)>)->Option<Action> {
        let (origin,def)=self.graph.resolve_struct(self.caller,ty)?;
        if let TermKind::Move(from) | TermKind::Name(from) = &t.kind {
            let source = self.variables.get(from)?;
            let (_, original) = self.graph.resolve_struct(self.caller,&source.ty)?;
            if !std::ptr::eq(original,def) || source.moved.get() || !source.missing.is_empty() { return None; }
            let moving = matches!(t.kind, TermKind::Move(_));
            if !moving && def.fields.iter().any(|f| !matches!(f.ty.as_str(),"int" | "bool" | "float")) { return None; }
            if moving { source.moved.set(true); }
            self.bind(name,ty,start,mutable);
            return Some(Action::RelocateObject { source: from.clone(), target: name.into(), moving });
        }
        let qualified;
        let (ctor,args)=match &t.kind {
            TermKind::Construct(n,args)=>(n.as_str(),args.as_slice()),
            TermKind::UnresolvedZeroArgs(n)=>(n.as_str(),&[][..]),
            TermKind::MethodCall(module,n,args) if args.is_empty()=>{
                qualified=format!("{module}.{n}"); (qualified.as_str(),&[][..])
            }
            _=>return None,
        };
        if let Some((target,unit,function))=self.function(ctor) {
            let (_,result)=self.graph.resolve_struct(unit,&function.result_ty)?;
            if !std::ptr::eq(result,def) || function.is_async || !args.is_empty() || !function.params.is_empty() {return None;}
            let target=target.clone();
            self.bind(name,ty,start,mutable);
            return Some(Action::ReceiveObject {target, owner:name.into()});
        }
        let (ctor_origin,ctor_def)=self.graph.resolve_struct(self.caller,ctor)?;
        if ctor_origin!=origin || !std::ptr::eq(ctor_def,def) {return None;}
        let seen:BTreeSet<_>=supplied.iter().map(|(name,_)|name.as_str()).collect();
        if seen.len()!=args.len() {return None;}
        let mut fields=Vec::new();
        let mut missing=BTreeSet::new();
        for f in &def.fields {
            if f.default.is_none() && !seen.contains(f.name.as_str()) {missing.insert(f.name.clone());}
            fields.push(PreparedField{name:f.name.clone(),kind:kind(&f.ty)?,immutable:f.immutable,
                default:f.default.as_ref().and_then(|t|seed(&self.graph.units[origin],t))});
        }
        self.bind(name,ty,start,mutable);
        self.variables.get_mut(name)?.missing=missing;
        Some(Action::Construct(name.to_owned(),fields,supplied))
    }
    pub(in crate::struct_stage::module_share) fn field_type(&self, owner:&str, name:&str, write:bool) -> Option<&str> {
        let object = self.variables.get(owner)?;
        if object.moved.get() || (write && !object.mutable)
            || (!object.missing.is_empty() && !(write && object.missing.contains(name))) { return None; }
        let field = self.graph.field(self.caller,&object.ty,name)?;
        if write && field.immutable { return None; }
        Some(&field.ty)
    }

    pub(in crate::struct_stage::module_share) fn commit_write(&mut self, owner:&str, field:&str) -> Option<bool> {
        self.field_type(owner,field,true)?;
        Some(self.variables.get_mut(owner)?.missing.remove(field))
    }

    pub(in crate::struct_stage::module_share) fn expression(&self,t:&Term)->Option<Number> {
        let (owner,field)=match &t.kind {
            TermKind::FieldRead(owner,n)=>(owner,n),
            TermKind::Call(c) if !c.awaited=>return self.consume(&c.name,&c.arguments),
            TermKind::MethodCall(module,n,args) if !self.has(module)=>return self.consume(&format!("{module}.{n}"),args),
            _=>return None,
        };
        let object=self.variables.get(owner)?;
        if object.moved.get() || !object.missing.is_empty() || self.graph.field(self.caller,&object.ty,field)?.ty!="int" {return None;}
        Some(Number::Member{owner:owner.clone(),field:field.clone(),receiver:None})
    }
    pub(in crate::struct_stage::module_share) fn method(&self,owner:&str,name:&str,values:Vec<Number>)->Option<MethodCall> {
        let object=self.variables.get(owner)?;
        if object.moved.get() || !object.missing.is_empty() {return None;}
        let (module,def)=self.graph.resolve_struct(self.caller,&object.ty)?;
        let methods=self.graph.methods(self.caller,&object.ty,name);
        let [method]=methods.as_slice() else {return None;};
        if values.len()!=method.params.len() {return None;}
        let steps=method_steps(&self.graph.units[module],module,owner,def,method)?;
        let writes=steps.iter().any(|s|s.write.is_some());
        if writes && !object.mutable {return None;}
        let arguments=method.params.iter().zip(values).map(|(p,n)|(format!("$arg/{}",p.name),n)).collect();
        Some(MethodCall{owner:owner.into(),id:object.id,writes,steps,arguments})
    }
    pub(in crate::struct_stage::module_share) fn return_owner(&self,ty:&str,value:&Term)->Option<String> {
        let TermKind::Move(name)=&value.kind else {return None};
        let object=self.variables.get(name)?;
        let (_,actual)=self.graph.resolve_struct(self.caller,&object.ty)?;
        let (_,expected)=self.graph.resolve_struct(self.caller,ty)?;
        if !std::ptr::eq(actual,expected) || !object.missing.is_empty() || object.moved.replace(true) {return None;}
        Some(name.clone())
    }
    fn consume(&self,name:&str,args:&[Term])->Option<Number> {
        let [arg]=args else{return None;};
        let TermKind::Move(from)=&arg.kind else{return None;};
        let (target,origin,f)=self.function(name)?;
        let [p]=f.params.as_slice() else{return None;};
        if f.is_async || f.result_ty != "int" {return None;}
        let object=self.variables.get(from)?;
        let (_,actual)=self.graph.resolve_struct(self.caller,&object.ty)?;
        let (_,expected)=self.graph.resolve_struct(origin,&p.ty)?;
        if !std::ptr::eq(actual,expected) || !object.missing.is_empty() || object.moved.replace(true){return None;}
        Some(Number::Consume{target:target.clone(),from:from.clone(),parameter:p.name.clone()})
    }
}
