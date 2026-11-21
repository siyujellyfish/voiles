//! 跨模組結構的型別解析、欄位種子與整筆移交。
use super::*;
use crate::mixed_struct_runtime::ScalarKind;

fn kind(name:&str)->Option<ScalarKind> {
    match name { "str"=>Some(ScalarKind::Str),"int"=>Some(ScalarKind::Int),"bool"=>Some(ScalarKind::Bool),_=>None }
}

fn seed(unit:&Unit,t:&Term)->Option<ApprovedSeed> {
    match &t.kind {
        TermKind::Group(inner)=>seed(unit,inner),
        TermKind::Scalar(ScalarLiteral::Integer(n))=>Some(ApprovedSeed::Int(integer_i64(n).ok()?)),
        TermKind::Scalar(ScalarLiteral::Boolean(v))=>Some(ApprovedSeed::Bool(*v)),
        TermKind::Literal("str")=>{
            let i=unit.tokens.binary_search_by_key(&t.start,|t|t.start).ok()?;
            if let TokenKind::StringLiteral(value)=&unit.tokens[i].kind {Some(ApprovedSeed::Str(value.clone()))} else {None}
        }
        _=>None,
    }
}

// 這個執行入口目前接受以 self 讀取整數欄位的同步方法。
fn returned_field(method:&Function)->Option<&str> {
    if method.result_ty!="int" || method.is_async || !method.params.is_empty() {return None;}
    let [stmt]=method.body.statements.as_slice() else {return None;};
    let StmtKind::Return(ret)=&stmt.kind else {return None;};
    match &ret.expression.kind {
        TermKind::FieldRead(owner,field) if owner=="self"=>Some(field),
        _=>None,
    }
}

pub(super) fn check_definition(unit:&Unit,def:&StructDecl)->Result<(),ModuleError> {
    for f in &def.fields {
        let expected=kind(&f.ty);
        if expected.is_none() || (f.immutable && f.default.is_none()) || f.default.as_ref().is_some_and(|v|seed(unit,v).map(|s|s.kind())!=expected) {
            return Err(unit.error(f.declaration_span.start,f.declaration_span.end,"結構欄位型別或預設值缺少有效證明"));
        }
    }
    let mut seen=HashSet::new();
    for m in &def.methods {
        if !seen.insert(&m.name) || returned_field(m).and_then(|n|def.field(n)).is_none_or(|f|f.ty!="int") {
            return Err(unit.error(m.start,m.end,"跨檔方法需要唯一的已檢查整數欄位讀取"));
        }
    }
    Ok(())
}

struct Object { ty:String, id:DeclarationId, moved:std::cell::Cell<bool> }

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
    pub(super) fn bind(&mut self,name:&str,ty:&str,site:usize) {
        self.variables.insert(name.to_owned(),Object{ty:ty.to_owned(),id:DeclarationId(site),moved:false.into()});
    }
    pub(in crate::struct_stage::module_share) fn has(&self,n:&str)->bool {self.variables.contains_key(n)}
    fn function(&self,n:&str)->Option<(&String,usize,&Function)> {
        let target=self.targets.get(n)?;
        let origin=*self.owners.get(target)?;
        let name=target.split_once(':').map_or(target.as_str(),|(_,n)|n);
        let f=self.graph.units[origin].functions.iter().find(|f|f.name==name)?;
        Some((target,origin,f))
    }
    pub(in crate::struct_stage::module_share) fn requires_argument(&self,name:&str)->bool {
        self.function(name).is_some_and(|(_,_,f)|!f.params.is_empty())
    }
    pub(in crate::struct_stage::module_share) fn construct(&mut self,name:&str,ty:&str,start:usize,t:&Term)->Option<Action> {
        let (origin,def)=self.graph.resolve_struct(self.caller,ty)?;
        let qualified;
        let (ctor,args)=match &t.kind {
            TermKind::Construct(n,args)=>(n.as_str(),args.as_slice()),
            TermKind::UnresolvedZeroArgs(n)=>(n.as_str(),&[][..]),
            TermKind::MethodCall(module,n,args) if args.is_empty()=>{
                qualified=format!("{module}.{n}"); (qualified.as_str(),&[][..])
            }
            _=>return None,
        };
        let (ctor_origin,ctor_def)=self.graph.resolve_struct(self.caller,ctor)?;
        if ctor_origin!=origin || !std::ptr::eq(ctor_def,def) {return None;}
        let mut supplied=Vec::new();
        let mut seen=HashSet::new();
        for (n,v) in args {
            let field=self.graph.field(self.caller,ty,n)?;
            let value=seed(&self.graph.units[self.caller],v)?;
            if !seen.insert(n.as_str()) || Some(value.kind())!=kind(&field.ty) {return None;}
            supplied.push(PreparedNamedField{name:n.clone(),seed:value});
        }
        let mut fields=Vec::new();
        for f in &def.fields {
            if f.default.is_none() && !seen.contains(f.name.as_str()) {return None;}
            fields.push(PreparedField{name:f.name.clone(),kind:kind(&f.ty)?,immutable:f.immutable,
                default:f.default.as_ref().and_then(|t|seed(&self.graph.units[origin],t))});
        }
        self.bind(name,ty,start);
        Some(Action::Construct(name.to_owned(),fields,supplied))
    }
    pub(in crate::struct_stage::module_share) fn expression(&self,t:&Term)->Option<Number> {
        let (owner,field,method)=match &t.kind {
            TermKind::FieldRead(owner,n)=>(owner,n,false),
            TermKind::MethodCall(owner,n,args) if args.is_empty() && self.has(owner)=>(owner,n,true),
            TermKind::Call(c) if !c.awaited=>return self.consume(&c.name,&c.arguments),
            TermKind::MethodCall(module,n,args) if !self.has(module)=>return self.consume(&format!("{module}.{n}"),args),
            _=>return None,
        };
        let object=self.variables.get(owner)?;
        if object.moved.get(){return None;}
        let selected=if method {
            let methods=self.graph.methods(self.caller,&object.ty,field);
            let [m]=methods.as_slice() else{return None;};
            returned_field(m)?.to_owned()
        } else {
            let f=self.graph.field(self.caller,&object.ty,field)?;
            if f.ty!="int"{return None;}
            field.clone()
        };
        Some(Number::Member{owner:owner.clone(),field:selected,receiver:method.then_some(object.id)})
    }
    fn consume(&self,name:&str,args:&[Term])->Option<Number> {
        let [arg]=args else{return None;};
        let TermKind::Move(from)=&arg.kind else{return None;};
        let (target,origin,f)=self.function(name)?;
        let [p]=f.params.as_slice() else{return None;};
        if f.is_async{return None;}
        let object=self.variables.get(from)?;
        let (_,actual)=self.graph.resolve_struct(self.caller,&object.ty)?;
        let (_,expected)=self.graph.resolve_struct(origin,&p.ty)?;
        if !std::ptr::eq(actual,expected) || object.moved.replace(true){return None;}
        Some(Number::Consume{target:target.clone(),from:from.clone(),parameter:p.name.clone()})
    }
}
