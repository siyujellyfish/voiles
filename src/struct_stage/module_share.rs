//! P20 的模組字串。從私有 Parser 的正式節點建立有型別的直線執行計畫。
use super::*;
#[path = "modules.rs"]
pub(crate) mod modules;
use crate::borrow_safety::BorrowTicket;
use crate::owned_str_runtime::{OwnedStrRuntime, ResourceEvent, StrRuntimeError};
use crate::syntax_shared::{integer_i64, ScalarLiteral};

#[derive(Clone)]
enum Initial { Text(String), Integer(i64) }

pub(super) struct Declaration {
    pub(super) name: String,
    initial: Initial,
    id: DeclarationId,
}

pub(super) fn parse<const PUBLIC:bool>(parser: &mut LanguageParser<'_,PUBLIC>) -> Result<Declaration, Diagnostic> {
    parser.cursor += 1;
    let id = DeclarationId(parser.tok().start);
    let name = parser.name()?;
    parser.take(':', "share 必須明確標註型別")?;
    let ty = parser.tok().clone();
    let kind=parser.name()?;
    if kind != "str" && !(PUBLIC && kind == "int") {
        return Err(problem(
            parser.source,
            (ty.start, ty.end),
            "P20 僅開放模組級 share str",
        ));
    }
    parser.take('=', "share 必須立即初始化")?;
    let negative = kind == "int" && parser.symbol('-');
    if negative { parser.cursor += 1; }
    let initial=match (&parser.tok().kind,kind.as_str()) {
        (TokenKind::StringLiteral(text),"str")=>Initial::Text(text.clone()),
        (TokenKind::Integer(n),"int")=>Initial::Integer((if negative { crate::syntax_shared::negative_integer_i64(n) } else { integer_i64(n) }).map_err(|_|parser.syntax("整數初值超出範圍"))?),
        _ if kind=="str"=>return Err(parser.syntax("模組初始化需要已驗證的字串常值")),
        _=>return Err(parser.syntax("整數 share 需要 int 常值")),
    };
    parser.cursor += 1;
    Ok(Declaration { name, initial, id })
}

pub(super) fn has_module_share(text: &str) -> bool {
    let mut depth = 0usize;
    let source = Source::new(text.as_bytes().to_vec());
    for token in lexer::scan(&source).tokens {
        match token.kind {
            TokenKind::Keyword(word) if depth == 0 && word == "share" => return true,
            TokenKind::Symbol('{') => depth += 1,
            TokenKind::Symbol('}') => depth = depth.saturating_sub(1),
            _ => (),
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ModuleId(usize);

#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
}

impl ModuleId {
    fn permits(self, caller: Self, visible: bool, access: Access) -> bool {
        self == caller || (visible && matches!(access, Access::Read))
    }
}

#[derive(Clone)]
struct Place {
    slot: usize,
    alias: Option<(String, BorrowKind)>,
}

#[path = "public_format.rs"]
mod public_format;
#[path = "scalar_flow.rs"]
mod scalar_flow;
#[path = "public_containers.rs"]
mod containers;

enum ContainerKey { NamedText(String), Text(String), Index(Number) }
enum ContainerItem { Record(Vec<(String,ContainerItem)>), Decimal(Number), Scalar(Number, bool), Literal(containers::Value) }
enum Edit { ReplaceOptional(ContainerItem), Append(ContainerItem), Set(ContainerKey, ContainerItem), Pop }
enum ContainerRead { Field(String), Length, Tag, Scalar }
fn stored_type(ty:&str)->bool { element_type(ty).is_some() || ty.starts_with("Optional[") }
enum CollectionArgument { Optional(ContainerItem), Move(String), Scalar(Number) }
struct CollectionCall { target:String, arguments:Vec<(String,usize,CollectionArgument)> }
impl CollectionCall {
    fn uses<'a>(&'a self,uses:&mut Vec<Use<'a>>) {
        for (_,_,arg) in &self.arguments {
            match arg {
                CollectionArgument::Scalar(n)|CollectionArgument::Optional(ContainerItem::Scalar(n,_)|ContainerItem::Decimal(n))=>n.uses(uses),
                _=>(),
            }
        }
        uses.push(Use::Call(&self.target,false));
    }
}

enum ContainerInit { ReadBytes(StandardCall), Call(CollectionCall), Optional(ContainerItem), Literal(ContainerLiteral), Move(String) }

impl ContainerLiteral {
    fn runtime(&self) -> Option<containers::Value> {
        use containers::{Key, Value};
        fn atom(a: &ContainerAtom) -> Option<Value> {
            Some(match a {
                ContainerAtom::Integer(n) => Value::Integer(*n), ContainerAtom::Boolean(b) => Value::Boolean(*b),
                ContainerAtom::Text(s) => Value::Text(s.clone()), ContainerAtom::Missing => Value::Missing,
                ContainerAtom::Decimal(c,e) => Value::Decimal(decimal::encode(*c,*e)), ContainerAtom::Nested(v) => v.runtime()?,
                ContainerAtom::ElementName(slot) => Value::CopyFrom(slot.clone()),
            })
        }
        match &self.items {
            ContainerItems::List(v) => Some(Value::List(v.iter().map(atom).collect::<Option<_>>()?)),
            ContainerItems::Dictionary(v) => Some(Value::Dict(v.iter().map(|(k,v)| {
                let key = match k { ContainerAtom::Integer(n) => Key::Index(*n), ContainerAtom::Boolean(b) => Key::Index(i64::from(*b)),
                    ContainerAtom::Text(s) => Key::Text(s.clone()), _ => return None };
                Some((key, atom(v)?))
            }).collect::<Option<_>>()?)),
        }
    }
}

#[path = "standard.rs"]
mod standard;
enum StandardArg { Int(Number), Text(LocalText), Read(String) }
struct StandardCall { name:String, args:Vec<StandardArg> }
impl StandardCall {
    fn uses<'a>(&'a self, uses:&mut Vec<Use<'a>>) {
        for arg in &self.args {
            match arg { StandardArg::Int(n)=>n.uses(uses),StandardArg::Text(t)=>t.uses(uses),StandardArg::Read(_)=>() }
        }
    }
}
impl LocalText {
    fn uses<'a>(&'a self, uses:&mut Vec<Use<'a>>) {
        match self { Self::Formatted(parts)=>for part in parts {
            match part { FormatPart::Boolean(n)|FormatPart::Float(n,_)|FormatPart::Decimal(n)|FormatPart::Hex(n)=>n.uses(uses), _=>() }
        }, Self::Standard(call)=>call.uses(uses), Self::Call(call)=>call.uses(uses), _=>() }
    }
}
struct MethodStep {
    boolean: bool,
    write: Option<String>,
    value: Number,
    origin: StepOrigin,
    site: String,
}

struct MethodCall {
    owner:String,
    id:DeclarationId,
    writes:bool,
    arguments:Vec<(String,Number)>,
    steps:Vec<MethodStep>,
}

enum Number {
    GlobalInt(usize),
    FromCollection(CollectionCall),
    Method(MethodCall),
    TextLength(String),
    ParseInt(String),
    Decimal(Box<Number>, char, Box<Number>),
    Standard(StandardCall),
    Container(String, ContainerRead),
    Edit(String, Box<Edit>),
    ViewByte(String,StringOffset,StringOffset),
    Logic(&'static str, Box<Number>, Box<Number>),
    ScalarCall { awaited:bool, target: String, arguments: Vec<(String, usize, Number)> },
    Choice(Box<Number>, Box<Number>, Box<Number>),
    Member { owner:String, field:String, receiver:Option<DeclarationId> },
    TextCall(TextInvocation),
    Consume { target:String, from:String, parameter:String },
    Constant(i64),
    Local(String),
    BoolMember(String, String),
    MemberByte(String, String, StringOffset, StringOffset),
    LocalByte(String, StringOffset, StringOffset),
    Byte(Place, StringOffset, StringOffset),
    Call(String, bool),
    Binary(Box<Number>, char, Box<Number>),
}

impl StringOffset {
    fn value(&self, frame: &Frame) -> Result<i64, StrRuntimeError> {
        match self {
            Self::Constant(i) => Ok(*i),
            Self::Named(name) => frame.locals.get(name).copied().ok_or(StrRuntimeError::MissingOwner),
        }
    }
}

enum Text {
    Literal(String),
    Grapheme(Place, StringOffset),
}

enum FormatPart { Boolean(Number), Float(Number,Option<usize>), Literal(String), Decimal(Number), Hex(Number), Text(String) }

enum LocalText {
    CaughtMessage,
    Formatted(Vec<FormatPart>),
    Standard(StandardCall),
    Call(TextInvocation),
    Literal(String),
    Grapheme(String, StringOffset),
    Move(String),
    Replace(LocalReplacement),
}

struct TextInvocation {
    target: String,
    arguments: Vec<(String, usize, Argument)>,
}

enum Argument {
    SharedText(String),
    Integer(Number),
    Text(LocalText),
}

// 引數從左至右求值，已建立的共享借用會持續到整次呼叫結束。
fn writes_argument(term:&Term,shared:&BTreeSet<String>)->bool {
    match &term.kind {
        TermKind::Move(n)|TermKind::StringReplace(n,..)=>shared.contains(n),
        TermKind::Group(t)|TermKind::Not(t)=>writes_argument(t,shared),
        TermKind::Logic(_,a,b)=>writes_argument(a,shared)||writes_argument(b,shared),
        TermKind::BinaryInt(n)=>writes_argument(&n.left,shared)||writes_argument(&n.right,shared),
        TermKind::Construct(_,args)=>args.iter().any(|(_,t)|writes_argument(t,shared)),
        TermKind::Call(c)=>c.arguments.iter().any(|t|writes_argument(t,shared)),
        TermKind::MethodCall(_,_,args)|TermKind::AwaitMethodCall(_,_,args)=>args.iter().any(|t|writes_argument(t,shared)),
        _=>false,
    }
}

impl TextInvocation {
    fn uses<'a>(&'a self, uses: &mut Vec<Use<'a>>) {
        for (_, _, arg) in &self.arguments {
            match arg {
                Argument::SharedText(_) => (),
                Argument::Integer(n) => n.uses(uses),
                Argument::Text(t) => t.uses(uses),
            }
        }
        uses.push(Use::Call(&self.target, false));
    }
}

struct LocalReplacement {
    owner: String,
    index: StringOffset,
    rhs: GraphemeReplacement,
}

impl LocalReplacement {
    fn perform(&self, state: &mut State, frame: &Frame) -> Result<crate::owned_str_runtime::OwnedUtf8, StrRuntimeError> {
        let text = match &self.rhs {
            GraphemeReplacement::Literal(text) => std::borrow::Cow::Borrowed(text.as_str()),
            GraphemeReplacement::Named(source) =>
                std::borrow::Cow::Owned(state.heap.read(&format!("@{source}"))?.to_owned()),
        };
        state.heap.replace_grapheme(&format!("@{}", self.owner), self.index.value(frame)?, &text)
            .map_err(|_| StrRuntimeError::EvaluationFailed)
    }
}

enum Replacement {
    Literal(String),
    Named(Place),
}

enum MemberValue {
    Integer(Number),
    Boolean(Number),
    Text(String),
}

enum Action {
    StoreInt(usize,Number),
    BorrowLocal { alias:String, owner:String,kind:BorrowKind },
    ReturnCollection(String),
    ReturnOptional(ContainerItem),
    ReturnObject(String),
    ReceiveObject {target:String,owner:String},
    Evaluate(Number),
    Container(String, ContainerInit),
    Select { alias: String, owner: String, key: ContainerKey },
    DropStorage(Vec<Storage>),
    RelocateObject { source: String, target: String, moving: bool },
    WriteMember { owner: String, field: String, value: MemberValue, initializing: bool },
    ReturnText(LocalText),
    ReplaceLocal(LocalReplacement),
    StoreText { target: String, declaration: Option<bool>, value: LocalText },
    Jump(usize),
    Unless(Number, usize),
    Construct(String,Vec<PreparedField>,Vec<(String,MemberValue)>),
    Bind(String, Number),
    Loan(String, Place, BorrowKind),
    Write(Place, Text),
    Replace(Place, StringOffset, Replacement),
    Return(Number),
}

#[derive(Clone, Copy)]
struct StepOrigin {
    module: usize,
    span: (usize, usize),
}

#[derive(Clone)]
enum Storage { Container(String), Text(String) }
impl Storage {
    fn cleanup(slots:&[Storage],frame:&mut Frame,heap:&mut OwnedStrRuntime) {
        let mut texts=BTreeSet::new();
        for slot in slots {
            match slot {Self::Container(n)=>frame.containers.drop_slot(n),Self::Text(n)=>{texts.insert(format!("@{n}"));}}
        }
        heap.leave_bindings(&texts);
    }
}
struct Handler { category:String, target:usize, cleanup:Vec<Storage> }

struct Step {
    text_access:Vec<String>,
    expire:Vec<String>,
    active_view:bool,
    handlers:Vec<Handler>,
    module: usize,
    action: Action,
    span: (usize, usize),
}
type Body = Vec<Step>;

pub(crate) struct Program {
    globals: Vec<Declaration>,
    bodies: BTreeMap<String, Body>,
    // 整個已驗證模組限於所屬執行緒。
    _local: std::marker::PhantomData<Rc<()>>,
}

fn problem(source: &Source, span: (usize, usize), message: &str) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::Static,
        message: message.into(),
        span: Some(source.span(span.0, span.1)),
    }
}

struct TextBinding {
    borrowed:Option<String>,
    mutable: bool,
    live: std::cell::Cell<bool>,
}
impl TextBinding {
    fn new(mutable: bool) -> Self { Self { borrowed:None, mutable, live: true.into() } }
}

struct Lower<'a> {
    text_access:std::cell::RefCell<Vec<String>>,
    objects: Option<modules::objects::ObjectScope<'a>>,
    source: &'a Source,
    tokens: &'a [Token],
    globals: &'a BTreeMap<String, usize>,
    integer_slots: BTreeSet<usize>,
    functions: &'a BTreeMap<String, bool>,
    linked_names: Option<&'a BTreeMap<String,String>>,
    readonly_slots: BTreeSet<usize>,
    locals: HashMap<String, (bool,String)>,
    strings: HashMap<String, TextBinding>,
    aliases: HashMap<String, Place>,
}

impl Lower<'_> {
    fn text_owner(&self, name:&str, write:bool, must_be_live:bool)->Option<String> {
        let binding=self.strings.get(name)?;
        if write && !binding.mutable || must_be_live && !binding.live.get() {return None;}
        self.text_access.borrow_mut().push(name.into());
        Some(binding.borrowed.as_deref().unwrap_or(name).into())
    }

    fn integer_slot(&self,name:&str)->Option<usize> {
        let slot=*self.globals.get(name)?;
        (self.integer_slots.contains(&slot) && !self.shadowed(name)).then_some(slot)
    }

    fn place(&self, name: &str) -> Option<Place> {
        if self.objects.as_ref().is_some_and(|s| s.has(name.split('.').next().unwrap_or(name))) { return None; }
        if self.strings.contains_key(name.split('.').next()?) || self.locals.contains_key(name) || name.split_once('.').is_some_and(|(prefix,_)| self.locals.contains_key(prefix) || self.aliases.contains_key(prefix)) {
            return None;
        }
        self.aliases.get(name).cloned().or_else(|| {
            self.globals
                .get(name).filter(|s|!self.integer_slots.contains(s))
                .map(|&slot| Place { slot, alias: None })
        })
    }

    fn shadowed(&self, name: &str) -> bool {
        let root = name.split_once('.').map_or(name, |(root, _)| root);
        self.strings.contains_key(root) || self.locals.contains_key(root) || self.aliases.contains_key(root)
            || self.objects.as_ref().is_some_and(|scope| scope.has(root))
    }

    fn standard_call<'t>(&self,name:&str,args:impl Iterator<Item=(Option<&'t str>, &'t Term)>, result:&str) -> Option<StandardCall> {
        if self.objects.is_none() || self.shadowed(name) || self.functions.contains_key(name) || self.globals.contains_key(name) {return None;}
        let (params,ty)=standard::signature(name)?;
        if result!=ty {return None;}
        let mut values=Vec::new();
        for (i,(label,t)) in args.enumerate() {
            if label.is_some() {return None;}
            let expected=params.get(i).copied().or_else(||standard::variadic_parameter(name))?;
            values.push(if expected=="int" { StandardArg::Int(self.number(t)?) }
                else if let TermKind::Name(n)=&t.kind {
                    StandardArg::Read(self.text_owner(n,false,true)?)
                } else {StandardArg::Text(self.local_text(t,"")?)});
        }
        if values.len()<params.len() {return None;}
        Some(StandardCall{name:name.into(),args:values})
    }

    fn call_target(&self, name: &str, awaited: bool) -> Option<Number> {
        if !awaited && standard::signature(name).is_some() && !self.functions.contains_key(name) {return self.standard_call(name,std::iter::empty(),"int").map(Number::Standard);}
        if self.objects.as_ref().and_then(|s| s.function(name)).is_some_and(|(_,_,f)| f.result_ty != "int") { return None; }
        if self.objects.as_ref().is_some_and(|s| s.requires_argument(name)) { return None; }
        if self.shadowed(name)
            || self.functions.get(name) != Some(&awaited) { return None; }
        let target = match self.linked_names { Some(names) => names.get(name)?.clone(), None => name.to_owned() };
        Some(Number::Call(target,awaited))
    }

    fn scalar_call<'t>(&self, name: &str, arguments: impl Iterator<Item = (Option<&'t str>, &'t Term)>) -> Option<Number> {
        if self.shadowed(name) { return None; }
        if standard::signature(name).is_some() && !self.functions.contains_key(name) {
            return self.standard_call(name,arguments,"int").map(Number::Standard);
        }
        let (target, _, function) = self.objects.as_ref()?.function(name)?;
        if function.params.iter().any(|p| p.ty == "str") {
            return self.invocation(name, arguments, "int").map(Number::TextCall);
        }
        if function.result_ty != "int" || function.is_async || function.params.is_empty() || function.params.iter().any(|p| p.ty != "int") {
            return None;
        }
        let mut values = Vec::new();
        let mut supplied = BTreeSet::new();
        for (position, (label, term)) in arguments.enumerate() {
            let slot = match label {
                Some(label) => function.params.iter().position(|p| p.name == label)?,
                None => position,
            };
            let param = function.params.get(slot)?;
            if !supplied.insert(slot) { return None; }
            values.push((param.name.clone(), slot, self.number(term)?));
        }
        if values.len() != function.params.len() { return None; }
        Some(Number::ScalarCall { awaited:false, target: target.clone(), arguments: values })
    }

    fn offset(&self, offset: &StringOffset) -> Option<StringOffset> {
        match offset {
            StringOffset::Named(name) if !self.locals.get(name).is_some_and(|(_,ty)|ty=="int") => None,
            _ => Some(offset.clone()),
        }
    }

    fn replacement(&self, owner: &str, index: &StringOffset, rhs: &GraphemeReplacement) -> Option<LocalReplacement> {
        let owner=self.text_owner(owner,true,true)?;
        let rhs=match rhs {
            GraphemeReplacement::Literal(s) if crate::owned_str_runtime::single_grapheme(s)=>rhs.clone(),
            GraphemeReplacement::Named(n)=>GraphemeReplacement::Named(self.text_owner(n,false,true)?),
            _=>return None,
        };
        Some(LocalReplacement { owner, index:self.offset(index)?, rhs })
    }

    fn invocation<'t>(&self, name: &str, args: impl Iterator<Item=(Option<&'t str>, &'t Term)>, result: &str)
        -> Option<TextInvocation> {
        if self.shadowed(name) { return None; }
        let (target, _, function) = self.objects.as_ref()?.function(name)?;
        if function.result_ty != result || function.is_async { return None; }
        let mut arguments = Vec::new();
        let mut slots = BTreeSet::new();
        let mut shared=BTreeSet::new();
        for (index, (label, term)) in args.enumerate() {
            if writes_argument(term,&shared) {return None;}
            let slot = match label { Some(n) => function.params.iter().position(|p| p.name == n)?, None => index };
            let p = function.params.get(slot)?;
            if !slots.insert(slot) { return None; }
            let value = match p.ty.as_str() {
                "str" => if let TermKind::Name(owner) = &term.kind {
                    if !self.objects.as_ref()?.allows_shared(name,&p.name) || self.strings.get(owner)?.borrowed.is_some() { return None; }
                    shared.insert(owner.clone());
                    Argument::SharedText(self.text_owner(owner,false,true)?)
                } else { Argument::Text(self.local_text(term, "")?) },
                ty @ ("int" | "bool" | "float") => Argument::Integer(self.scalar_value(ty,term)?),
                _ => return None,
            };
            arguments.push((p.name.clone(),slot,value));
        }
        if arguments.len() != function.params.len() || arguments.iter().any(|(_,_,a)| match a {
            Argument::SharedText(n) => !self.strings.get(n).is_some_and(|b| b.live.get()),
            _ => false,
        }) { return None; }
        Some(TextInvocation { target: target.clone(), arguments })
    }

    fn text_call<'t>(&self, name: &str, args: impl Iterator<Item=(Option<&'t str>, &'t Term)>) -> Option<LocalText> {
        if standard::signature(name).is_some() && !self.functions.contains_key(name) {
            return self.standard_call(name,args,"str").map(LocalText::Standard);
        }
        self.invocation(name,args,"str").map(LocalText::Call)
    }

    fn format_parts(&self,term:&Term)->Option<Vec<FormatPart>> {
        public_format::format_parts(self.tokens,term,|name|self.text_owner(name,false,true),|t|{
            if let TermKind::Name(n)=&t.kind {
                let ty=self.locals.get(n)?.1.clone();
                return Some((ty,Number::Local(n.clone())));
            }
            Some(("int".into(),self.number(t)?))
        })
    }

    fn local_text(&self, term: &Term, target: &str) -> Option<LocalText> {
        match &term.kind {
            TermKind::Literal("formatted") => self.format_parts(term).map(LocalText::Formatted),
            TermKind::Group(inner) => self.local_text(inner, target),
            TermKind::UnresolvedZeroArgs(name) => self.text_call(name, std::iter::empty()),
            TermKind::Call(c) if !c.awaited => self.text_call(&c.name, c.arguments.iter().map(|t|(None,t))),
            TermKind::Construct(name,args) => self.text_call(name,args.iter().map(|(label,t)|(Some(label.as_str()),t))),
            TermKind::MethodCall(module,name,args) if !self.shadowed(module) =>
                self.text_call(&format!("{module}.{name}"),args.iter().map(|t|(None,t))),
            TermKind::Move(source) if source != target => {
                self.text_owner(source,false,true)?;
                if self.strings[source].borrowed.is_some() || !self.strings[source].live.replace(false) { return None; }
                Some(LocalText::Move(source.clone()))
            }
            TermKind::StringReplace(owner, index, rhs) => Some(LocalText::Replace(self.replacement(owner, index, rhs)?)),
            TermKind::StringIndex(owner, index, None) if self.strings.get(owner)?.live.get() =>
                Some(LocalText::Grapheme(self.text_owner(owner,false,true)?, self.offset(index)?)),
            TermKind::LiteralIndex(text, index, None) => Some(LocalText::Literal(
                crate::owned_str_runtime::literal_grapheme(text, *index)?.to_owned())),
            _ => match self.text(term)? {
                Text::Literal(s) => Some(LocalText::Literal(s)),
                _ => None,
            },
        }
    }

    fn float_special(&self,owner:&str,method:&str,args:&[Term])->Option<Number> {
        if owner!="float" || !args.is_empty() || self.shadowed(owner) ||
            self.objects.as_ref()?.names_module_item(owner) {return None;}
        decimal::special(method).map(Number::Constant)
    }

    fn scalar_method(&self,owner:&str,name:&str,args:&[Term],expected:&str)->Option<Number> {
        let objects=self.objects.as_ref()?;
        if !objects.has(owner) {return None;}
        let mut values=Vec::new();
        for arg in args {
            let value=if let Some(n)=self.boolean(arg) {("bool",n)}
                else if let Some(n)=self.floating(arg) {("float",n)}
                else {("int",self.number(arg)?)};
            values.push(value);
        }
        objects.method(owner,name,values,expected).map(Number::Method)
    }

    fn floating(&self, term:&Term)->Option<Number> {
        match &term.kind {
            TermKind::Group(t) => self.floating(t),
            TermKind::MethodCall(owner,name,args) => self.scalar_method(owner,name,args,"float").or_else(||self.float_special(owner,name,args)),
            TermKind::FieldRead(owner,field) if self.objects.as_ref()?.field_type(owner,field,false)?=="float" =>
                Some(Number::Member {owner:owner.clone(),field:field.clone(),receiver:None}),
            TermKind::Name(n) if self.locals.get(n)?.1 == "float" => Some(Number::Local(n.clone())),
            TermKind::Literal("float") => {
                let index=self.tokens.binary_search_by_key(&term.start,|t|t.start).ok()?;
                let TokenKind::Decimal(s)=&self.tokens[index].kind else {return None};
                let (c,e)=decimal::parts(s)?;
                Some(Number::Constant(decimal::encode(c,e)))
            }
            TermKind::BinaryInt(n) if unary_minus(n,self.tokens) => Some(Number::Decimal(Box::new(Number::Constant(0)),'~',Box::new(self.floating(&n.right)?))),
            TermKind::BinaryInt(n) if matches!(n.operator, '+' | '-' | '*' | '/') => Some(Number::Decimal(Box::new(self.floating(&n.left)?),
                n.operator,Box::new(self.floating(&n.right)?))),
            _ => None,
        }
    }
    fn scalar_value(&self,ty:&str,term:&Term)->Option<Number> {
        match ty { "bool"=>self.boolean(term), "float"=>self.floating(term), "int"=>self.number(term), _=>None }
    }

    fn number(&self, term: &Term) -> Option<Number> {
        if let TermKind::Call(call)=&term.kind {
            if !call.awaited && call.name=="len" && !self.shadowed("len") && !self.functions.contains_key("len") && !self.globals.contains_key("len") {
                if let [Term {kind:TermKind::Name(name),..}]=call.arguments.as_slice() {
                    if self.strings.get(name).is_some_and(|b|b.live.get()) {return Some(Number::TextLength(self.text_owner(name,false,true)?));}
                }
            }
        }
        if let TermKind::MethodCall(name,method,args)=&term.kind {
            if method=="to_int" && args.is_empty() && self.strings.get(name).is_some_and(|b|b.live.get()) {
                return Some(Number::ParseInt(self.text_owner(name,false,true)?));
            }
        }
        if let Some(objects) = &self.objects {
            let callee = match &term.kind {
                TermKind::Call(c) => Some(&c.name),
                TermKind::MethodCall(module,_,_) if !objects.has(module) => Some(module),
                _ => None,
            };
            if callee.is_some_and(|n| self.shadowed(n)) {
                return None;
            }
            if let TermKind::MethodCall(owner,method,arguments)=&term.kind {
                if objects.has(owner) {
                    return self.scalar_method(owner,method,arguments,"int");
                }
            }
            if let Some(value) = objects.expression(term) { return Some(value); }
            if matches!(&term.kind,TermKind::MethodCall(owner,_,_) if objects.has(owner)) { return None; }
        }
        match &term.kind {
            TermKind::Group(inner) => self.number(inner),
            TermKind::Scalar(ScalarLiteral::Integer(value)) => {
                Some(Number::Constant(integer_i64(value).ok()?))
            }
            TermKind::StringIndex(n, i, Some(j)) if n.split_once('.').is_some_and(|(owner,_)| self.objects.as_ref().is_some_and(|objects| objects.has(owner))) => {
                let (owner, field) = n.split_once('.')?;
                if self.objects.as_ref()?.field_type(owner,field,false)? != "str" { return None; }
                Some(Number::MemberByte(owner.into(),field.into(),self.offset(i)?,self.offset(j)?))
            }
            TermKind::StringIndex(n, i, Some(j)) if self.strings.contains_key(n) =>
                Some(Number::LocalByte(self.text_owner(n,false,true)?, self.offset(i)?, self.offset(j)?)),
            TermKind::LiteralIndex(s, i, Some(j)) =>
                Some(Number::Constant(crate::owned_str_runtime::literal_byte(s, *i, *j)?)),
            TermKind::StringIndex(n, i, Some(j)) =>
                Some(Number::Byte(self.place(n)?, self.offset(i)?, self.offset(j)?)),
            TermKind::Name(n) if self.integer_slot(n).is_some()=>Some(Number::GlobalInt(self.integer_slot(n)?)),
            TermKind::FieldRead(module,member) if self.integer_slot(&format!("{module}.{member}")).is_some()=>
                Some(Number::GlobalInt(self.integer_slot(&format!("{module}.{member}"))?)),
            TermKind::Name(n) if self.locals.get(n).is_some_and(|(_,ty)|ty=="int") => Some(Number::Local(n.clone())),
            TermKind::UnresolvedZeroArgs(n) => self.call_target(n,false),
            TermKind::Call(call) if call.arguments.is_empty() => self.call_target(&call.name,call.awaited),
            TermKind::MethodCall(module,member,args) if args.is_empty() && self.linked_names.is_some() =>
                self.call_target(&format!("{module}.{member}"),false),
            TermKind::Call(c) if !c.awaited => self.scalar_call(&c.name, c.arguments.iter().map(|a| (None, a))),
            TermKind::Construct(name, args) => self.scalar_call(name, args.iter().map(|(n, a)| (Some(n.as_str()), a))),
            TermKind::MethodCall(module, member, args) => self.scalar_call(&format!("{module}.{member}"), args.iter().map(|a| (None, a))),
            TermKind::BinaryInt(node) => Some(Number::Binary(
                Box::new(self.number(&node.left)?),
                node.operator,
                Box::new(self.number(&node.right)?),
            )),
            _ => None,
        }
    }

    fn boolean(&self, term: &Term) -> Option<Number> {
        match &term.kind {
            TermKind::Group(t) => self.boolean(t),
            TermKind::MethodCall(owner,name,args) => self.scalar_method(owner,name,args,"bool"),
            TermKind::Name(n) if self.locals.get(n).is_some_and(|(_,ty)|ty=="bool") => Some(Number::Local(n.clone())),
            TermKind::Not(t) => Some(Number::Logic("==",Box::new(self.boolean(t)?),Box::new(Number::Constant(0)))) ,
            TermKind::Scalar(ScalarLiteral::Boolean(b)) => Some(Number::Constant(i64::from(*b))),
            TermKind::FieldRead(owner, field) if self.objects.as_ref()?.field_type(owner, field, false)? == "bool" =>
                Some(Number::BoolMember(owner.clone(),field.clone())),
            TermKind::Logic("!=",a,b) if self.floating(a).is_some() =>
                Some(Number::Logic("decimal!=",Box::new(self.floating(a)?),Box::new(self.floating(b)?))),
            TermKind::Logic(op @ ("==" | "<" | "<=" | ">" | ">="), a, b) =>
                Some(Number::Logic(op,Box::new(self.number(a)?),Box::new(self.number(b)?))),
            TermKind::Logic(op @ ("and" | "or" | "!="), a, b) =>
                Some(Number::Logic(op,Box::new(self.boolean(a)?),Box::new(self.boolean(b)?))),
            _ => None,
        }
    }

    fn scalar_leaf(&self, statements: &[Stmt]) -> Option<Number> {
        let [statement] = statements else { return None };
        let StmtKind::Return(value) = &statement.kind else { return None };
        // 此分支子集只讀取 int，兩條路徑都須獨立通過型別檢查。
        fn pure(term: &Term) -> bool {
            match &term.kind {
                TermKind::Group(inner) => pure(inner),
                TermKind::BinaryInt(op) => pure(&op.left) && pure(&op.right),
                TermKind::Name(_) | TermKind::FieldRead(_, _) | TermKind::Scalar(ScalarLiteral::Integer(_)) => true,
                _ => false,
            }
        }
        if !pure(&value.expression) { return None; }
        self.number(&value.expression)
    }

    fn text(&self, term: &Term) -> Option<Text> {
        match &term.kind {
            TermKind::Group(inner) => self.text(inner),
            TermKind::Literal("str") => {
                let pos = self
                    .tokens
                    .binary_search_by_key(&term.start, |t| t.start)
                    .ok()?;
                match &self.tokens[pos].kind {
                    TokenKind::StringLiteral(value) => Some(Text::Literal(value.clone())),
                    _ => None,
                }
            }
            TermKind::StringIndex(n, i, None) => {
                Some(Text::Grapheme(self.place(n)?, self.offset(i)?))
            }
            _ => None,
        }
    }

    fn body(&mut self, function: &Function, module: usize) -> Result<Body, Diagnostic> {
        let mut steps = Vec::new();
        for stmt in &function.body.statements {
            let span = (stmt.start, stmt.end);
            let source = self.source;
            let err = |msg: &str| problem(source, span, msg);
            if matches!(
                steps.last(),
                Some(Step {
                    action: Action::Return(_) | Action::ReturnText(_) | Action::ReturnObject(_),
                    ..
                })
            ) {
                return Err(err("return 後不可再有語句"));
            }
            let action = match &stmt.kind {
                StmtKind::Declare(d) => {
                    if self.strings.contains_key(&d.name) || self.locals.contains_key(&d.name) || self.aliases.contains_key(&d.name) || self.objects.as_ref().is_some_and(|s|s.has(&d.name)) {
                        return Err(err("重複區域宣告"));
                    }
                    let init = d
                        .initializer
                        .as_ref()
                        .ok_or_else(|| err("此執行路徑需要初值"))?;
                    if d.ty == "str" && !matches!(init.kind, TermKind::Borrow(_)) {
                        let value = if !d.mutable && matches!(init.kind,TermKind::Literal("template")) {
                            self.format_parts(init).map(LocalText::Formatted)
                        } else {self.local_text(init,&d.name)}.ok_or_else(||err("字串需要獨立初值，樣板需要 const"))?;
                        self.strings.insert(d.name.clone(), TextBinding::new(d.mutable));
                        Action::StoreText { target: d.name.clone(), declaration: Some(d.mutable), value }
                    } else if d.ty=="str" && matches!(&init.kind,TermKind::Borrow(n) if self.strings.contains_key(n)) {
                        let TermKind::Borrow(n)=&init.kind else {unreachable!()};
                        let source=&self.strings[n];
                        if source.borrowed.is_some() || !source.live.get() || d.mutable && !source.mutable {
                            return Err(err("字串借用需要有效且具相應權限的持有者"));
                        }
                        let mut alias=TextBinding::new(d.mutable);
                        alias.borrowed=Some(n.clone());
                        self.strings.insert(d.name.clone(),alias);
                        Action::BorrowLocal { alias:d.name.clone(),owner:n.clone(),
                            kind:if d.mutable {BorrowKind::ExclusiveWrite} else {BorrowKind::SharedRead} }
                    } else if d.ty == "str" {
                        let TermKind::Borrow(n) = &init.kind else {
                            return Err(err("需要明確借用來源"));
                        };
                        let place = self
                            .place(n)
                            .filter(|p| p.alias.is_none())
                            .ok_or_else(|| err("需要模組持有者"))?;
                        if d.mutable && self.readonly_slots.contains(&place.slot) { return Err(err("其他模組的共享字串不可獨占借用")); }
                        let kind = if d.mutable {
                            BorrowKind::ExclusiveWrite
                        } else {
                            BorrowKind::SharedRead
                        };
                        self.aliases.insert(
                            d.name.clone(),
                            Place {
                                slot: place.slot,
                                alias: Some((d.name.clone(), kind)),
                            },
                        );
                        Action::Loan(d.name.clone(), place, kind)
                    } else if matches!(d.ty.as_str(),"int"|"float"|"bool") {
                        let value = self
                            .scalar_value(&d.ty,init)
                            .ok_or_else(|| err("需要 int 初值或已驗證的同步函式呼叫"))?;
                        self.locals.insert(d.name.clone(), (d.mutable,d.ty.clone()));
                        Action::Bind(d.name.clone(), value)
                    } else {
                        if self.objects.is_none() {
                            return Err(err("此模組執行路徑尚未支援該區域型別"));
                        }
                        let root = match &init.kind {
                            TermKind::Name(_) | TermKind::Move(_) => None,
                            TermKind::Construct(name, _) | TermKind::UnresolvedZeroArgs(name)
                                | TermKind::MethodCall(name, _, _) => Some(name),
                            _ => return Err(err("結構需要建構式初值")),
                        };
                        if root.is_some_and(|n| self.shadowed(n)) {
                            return Err(err("建構式名稱已被區域綁定遮蔽"));
                        }
                        let mut named=Vec::new();
                        if let TermKind::Construct(_,args)=&init.kind {
                            for (field,t) in args {
                                let ty=self.objects.as_ref().and_then(|o|o.named_field(&d.ty,field)).ok_or_else(||err("找不到可見的建構欄位"))?;
                                let value=match ty {
                                    "float"=>MemberValue::Integer(self.floating(t).ok_or_else(||err("需要 decimal64 建構值"))?),
                                    "int"=>MemberValue::Integer(self.number(t).ok_or_else(||err("需要整數建構值"))?),
                                    "bool"=>MemberValue::Boolean(self.boolean(t).ok_or_else(||err("需要布林建構值"))?),
                                    "str"=>match self.text(t) {Some(Text::Literal(v))=>MemberValue::Text(v),_=>return Err(err("需要獨立字串建構值"))},
                                    _=>return Err(err("不支援的建構欄位型別")),
                                };
                                named.push((field.clone(),value));
                            }
                        }
                        self.objects.as_mut().and_then(|scope| scope.construct(&d.name, &d.ty, d.start, d.mutable, init,named))
                            .ok_or_else(|| err("結構型別、具名欄位或建構值無效"))?
                    }
                }
                StmtKind::Assign(a) if self.strings.contains_key(&a.name) => {
                    if !self.strings[&a.name].mutable { return Err(err("const 不可重賦值")); }
                    let value = self.local_text(&a.expression, &a.name).ok_or_else(|| err("需要獨立字串值"))?;
                    self.strings[&a.name].live.set(true);
                    Action::StoreText { target:self.text_owner(&a.name,true,false).ok_or_else(||err("缺少可寫字串"))?, declaration: None, value }
                }
                StmtKind::Assign(a) if self.locals.contains_key(&a.name) => {
                    if !self.locals[&a.name].0 {
                        return Err(err("const 不可重賦值"));
                    }
                    Action::Bind(
                        a.name.clone(),
                        self.scalar_value(&self.locals[&a.name].1,&a.expression).ok_or_else(|| err("純量賦值型別不符"))?,
                    )
                }
                StmtKind::Assign(a) => {
                    let target = self.place(&a.name).ok_or_else(|| err("缺少可寫宣告"))?;
                    if self.readonly_slots.contains(&target.slot) || matches!(target.alias, Some((_, BorrowKind::SharedRead))) {
                        return Err(err("唯讀借用不可寫入"));
                    }
                    Action::Write(
                        target,
                        self.text(&a.expression)
                            .ok_or_else(|| err("需要獨立的字串值"))?,
                    )
                }
                StmtKind::FieldAssign { receiver, field, value } => {
                    let ty = self.objects.as_ref().and_then(|o| o.field_type(receiver,field,true))
                        .ok_or_else(|| err("需要可見、可寫且仍由此變數持有的欄位"))?;
                    let rhs = match (ty, &value.kind) {
                        ("float",_)=>MemberValue::Integer(self.floating(value).ok_or_else(||err("需要 decimal64 欄位值"))?),
                        ("int", _) => MemberValue::Integer(self.number(value).ok_or_else(||err("需要 int 欄位值"))?),
                        ("bool", _) => MemberValue::Boolean(self.boolean(value).ok_or_else(||err("需要 bool 欄位值"))?),
                        ("str", _) => match self.text(value) {
                            Some(Text::Literal(s)) => MemberValue::Text(s),
                            _ => return Err(err("需要獨立字串欄位值")),
                        },
                        _ => return Err(err("欄位賦值型別不符")),
                    };
                    let initializing=self.objects.as_mut().and_then(|o|o.commit_write(receiver,field))
                        .ok_or_else(|| err("求值後接收者已經移動"))?;
                    Action::WriteMember { owner: receiver.clone(), field: field.clone(), value: rhs, initializing }
                }
                StmtKind::Evaluate(term) => Action::Evaluate(self.number(term).ok_or_else(||err("此呼叫需要相符的標準庫或函式簽章"))?),
                StmtKind::GraphemeWrite(t) => {
                    let TermKind::StringReplace(
                        name,
                        i,
                        replacement,
                    ) = &t.kind
                    else {
                        return Err(err("字素覆寫需要已驗證的索引與替換值"));
                    };
                    if self.strings.contains_key(name) {
                        let change = self.replacement(name, i, replacement)
                            .ok_or_else(|| err("區域字串覆寫需要可寫持有者、整數索引及單一字素"))?;
                        steps.push(Step { text_access:self.text_access.take(),expire:Vec::new(),active_view:false,handlers:vec![], module, span, action: Action::ReplaceLocal(change) });
                        continue;
                    }
                    let rhs = match replacement {
                        GraphemeReplacement::Literal(text) => {
                            if !crate::owned_str_runtime::single_grapheme(text) {
                                return Err(err("替換值必須為單一字素"));
                            }
                            Replacement::Literal(text.clone())
                        }
                        GraphemeReplacement::Named(name) =>
                            Replacement::Named(self.place(name).ok_or_else(|| err("替換值需要字串"))?),
                    };
                    let p = self.place(name).ok_or_else(|| err("字串未宣告"))?;
                    if self.readonly_slots.contains(&p.slot) || matches!(p.alias, Some((_, BorrowKind::SharedRead))) {
                        return Err(err("唯讀借用不可寫入"));
                    }
                    Action::Replace(p, self.offset(i).ok_or_else(|| err("字素索引需要 int"))?, rhs)
                }
                StmtKind::If { branch, no: Some(other) } if function.result_ty == "int" => {
                    let test = self.boolean(&branch.condition).ok_or_else(||err("此分支需要 bool 條件"))?;
                    let yes = self.scalar_leaf(&branch.body.statements);
                    let no = self.scalar_leaf(other.as_slice());
                    let (Some(yes), Some(no)) = (yes, no) else {
                        return Err(err("此分支兩側均需明確回傳純 int 運算式"));
                    };
                    Action::Return(Number::Choice(Box::new(test), Box::new(yes), Box::new(no)))
                }
                StmtKind::Return(r) if !matches!(function.result_ty.as_str(),"int"|"str"|"bool") =>
                    Action::ReturnObject(self.objects.as_ref().and_then(|o|o.return_owner(&function.result_ty,&r.expression))
                        .ok_or_else(||err("結構返回需要完整且同型的具名移動"))?),
                StmtKind::Return(r) if function.result_ty == "str" => Action::ReturnText(
                    self.local_text(&r.expression, "").ok_or_else(|| err("需要獨立的 str 返回值"))?),
                StmtKind::Return(r) => Action::Return(
                    self.number(&r.expression)
                        .ok_or_else(|| err("需要已驗證的 int 返回值"))?,
                ),
                _ => return Err(err("此模組語句缺少共享存取證明")),
            };
            steps.push(Step { text_access:self.text_access.take(),expire:Vec::new(),active_view:false,handlers:vec![], action, module, span });
        }
        if !matches!(
            steps.last(),
            Some(Step {
                action: Action::Return(_) | Action::ReturnText(_) | Action::ReturnObject(_),
                ..
            })
        ) {
            return Err(problem(
                self.source,
                (function.start, function.end),
                "函式必須明確返回 int",
            ));
        }
        verify_local_text_loans(&mut steps,self.source)?;
        Ok(steps)
    }
}

fn verify_local_text_loans(steps:&mut [Step],source:&Source)->Result<(),Diagnostic> {
    let mut last=HashMap::new();
    for (i,step) in steps.iter().enumerate() {
        for name in &step.text_access {last.insert(name.clone(),i);}
    }
    let mut active:BTreeMap<String,(String,BorrowKind,usize)>=BTreeMap::new();
    for (i,step) in steps.iter_mut().enumerate() {
        active.retain(|alias,(_,_,end)|{
            if *end<i {step.expire.push(alias.clone());false} else {true}
        });
        let conflict=||problem(source,step.span,"區域字串仍有有效借用");
        if step.text_access.iter().any(|n|active.values().any(|(owner,_,_)|owner==n)) {return Err(conflict());}
        step.active_view|=!active.is_empty();
        if let Action::BorrowLocal{alias,owner,kind}=&step.action {
            if active.values().any(|(n,k,_)|n==owner && (*k==BorrowKind::ExclusiveWrite || *kind==BorrowKind::ExclusiveWrite)) {return Err(conflict());}
            active.insert(alias.clone(),(owner.clone(),*kind,last.get(alias).copied().unwrap_or(i)));
        }
    }
    Ok(())
}

impl Program {
    pub(crate) fn prepare(text: &str) -> Result<Self, Diagnostic> {
        let source = Source::new(text.as_bytes().to_vec());
        if let Some(error) = source.lexical_diagnostics().into_iter().next() {
            return Err(error);
        }
        let lexed = lexer::scan(&source);
        if let Some(error) = lexed.diagnostics.into_iter().next() {
            return Err(error);
        }
        let mut globals = Vec::new();
        let (structs, functions) = Parser {
            source: &source,
            tokens: &lexed.tokens,
            cursor: 0,
        }
        .file_items(Some(&mut globals))?;
        let names: BTreeMap<_, _> = functions
            .iter()
            .map(|f| (f.name.clone(), f.is_async))
            .collect();
        let mut slots = BTreeMap::new();
        for (i, g) in globals.iter().enumerate() {
            if slots.insert(g.name.clone(), i).is_some()
                || names.contains_key(&g.name)
                || structs.contains_key(&g.name)
            {
                return Err(problem(
                    &source,
                    (g.id.0, g.id.0 + g.name.len()),
                    "重複模組宣告",
                ));
            }
        }
        if !structs.is_empty() || names.get("main") != Some(&false) {
            return Err(problem(
                &source,
                (0, 0),
                "共享模組需要已支援的函式及 main 入口",
            ));
        }
        let mut bodies = BTreeMap::new();
        for f in functions {
            if f.result_ty != "int" || !f.params.is_empty() {
                return Err(problem(
                    &source,
                    (f.start, f.end),
                    "共享模組尚無此簽章或非同步安全證明",
                ));
            }
            let mut lower = Lower { integer_slots:Default::default(),text_access:Default::default(),
                source: &source,
                tokens: &lexed.tokens,
                objects: None, linked_names: None, readonly_slots: BTreeSet::new(),
                globals: &slots,
                functions: &names,
                locals: HashMap::new(),
                strings: HashMap::new(), aliases: HashMap::new(),
            };
            bodies.insert(f.name.clone(), lower.body(&f, 0)?);
        }
        let plan = Self {
            globals,
            bodies,
            _local: Default::default(),
        };
        plan.verify_loans(&source)?;
        Ok(plan)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Effect {
    Direct,
    Shared,
    Exclusive,
}

#[derive(Clone, Copy)]
enum Use<'a> {
    Direct(&'a Place),
    Loan(&'a str, &'a Place, BorrowKind),
    Call(&'a str, bool),
}

impl ContainerItem {
    fn uses<'a>(&'a self,list:&mut Vec<Use<'a>>) {
        match self {
            Self::Record(fields)=>for (_,value) in fields {value.uses(list);},
            Self::Scalar(n,_)|Self::Decimal(n)=>n.uses(list),
            Self::Literal(_)=>(),
        }
    }
}

impl Number {
    fn uses<'a>(&'a self, list: &mut Vec<Use<'a>>) {
        match self {

            Self::FromCollection(call)=>call.uses(list),
            Self::Method(call)=>for (_,n) in &call.arguments { n.uses(list); },
            Self::Edit(_, edit) => {
                if let Edit::Set(ContainerKey::Index(n),_) = edit.as_ref() { n.uses(list); }
                if let Edit::Set(_,item) | Edit::Append(item) | Edit::ReplaceOptional(item)=edit.as_ref() {
                    item.uses(list);
                }
            }
            Self::Logic(_,a,b) => { a.uses(list); b.uses(list); }
            Self::ScalarCall { target, arguments, awaited } => {
                for (_, _, value) in arguments { value.uses(list); }
                list.push(Use::Call(target, *awaited));
            }
            Self::Choice(test, yes, no) => { test.uses(list); yes.uses(list); no.uses(list); }
            Self::Byte(p, ..) => list.push(Use::Direct(p)),
            Self::Decimal(left, _, right) | Self::Binary(left, _, right) => {
                left.uses(list);
                right.uses(list);
            }
            Self::Standard(call) => call.uses(list),
            Self::TextCall(call) => call.uses(list),
            Self::Consume{target,..} => list.push(Use::Call(target,false)),
            Self::Call(name, waited) => list.push(Use::Call(name, *waited)),
            _ => (),
        }
    }
}

impl Action {
    fn uses(&self) -> Vec<Use<'_>> {
        let mut uses = Vec::new();
        match self {
            Self::Select { key: ContainerKey::Index(n), .. } => n.uses(&mut uses),
            Self::Container(_,ContainerInit::ReadBytes(call)) => call.uses(&mut uses),
            Self::Container(_,ContainerInit::Call(call))=>call.uses(&mut uses),
            Self::ReturnOptional(ContainerItem::Scalar(n,_)|ContainerItem::Decimal(n))=>n.uses(&mut uses),
            Self::ReturnOptional(_)|Self::ReturnCollection(_)|Self::BorrowLocal{..}=>(),
            Self::Container(_,ContainerInit::Optional(ContainerItem::Scalar(n,_)|ContainerItem::Decimal(n)))=>n.uses(&mut uses),
            Self::Select { .. } | Self::Container(..) | Self::DropStorage(_) => (),
            Self::ReturnObject(_) => (),
            Self::ReceiveObject {target,..} => uses.push(Use::Call(target,false)),
            Self::ReturnText(value) | Self::StoreText { value, .. } => {
                value.uses(&mut uses);
            }
            Self::WriteMember { value: MemberValue::Integer(n) | MemberValue::Boolean(n), .. } => n.uses(&mut uses),
            Self::WriteMember { .. } | Self::RelocateObject { .. } | Self::Jump(_) | Self::ReplaceLocal(_) => (),
            Self::StoreInt(_,value) | Self::Evaluate(value) | Self::Unless(value, _) | Self::Bind(_, value) | Self::Return(value) => value.uses(&mut uses),
            Self::Loan(alias, p, kind) => uses.push(Use::Loan(alias, p, *kind)),
            Self::Construct(_,_,args) => for (_,value) in args {
                if let MemberValue::Integer(n)|MemberValue::Boolean(n)=value { n.uses(&mut uses); }
            },
            Self::Replace(p, _, rhs) => {
                if let Replacement::Named(source) = rhs { uses.push(Use::Direct(source)); }
                uses.push(Use::Direct(p));
            }
            Self::Write(p, value) => {
                if let Text::Grapheme(rhs, _) = value {
                    uses.push(Use::Direct(rhs));
                }
                uses.push(Use::Direct(p));
            }
        }
        uses
    }
}

impl Program {
    fn verify_loans(&self, source: &Source) -> Result<(), Diagnostic> {
        self.verify_linked_loans(|_| source).map_err(|(_,diagnostic)|diagnostic)
    }

    fn verify_linked_loans<'a>(&self, source_for: impl Fn(&str) -> &'a Source) -> Result<(),(String,Diagnostic)> {
        // 將呼叫者仍持有的借用與所有可達函式的來源存取合併。
        let mut effects = BTreeMap::new();
        for (name, body) in &self.bodies {
            let mut slots = BTreeSet::new();
            let mut awaits = false;
            let mut calls = Vec::new();
            for use_site in body.iter().flat_map(|s| s.action.uses()) {
                match use_site {
                    Use::Direct(p) if p.alias.is_none() => {
                        slots.insert((p.slot, Effect::Direct));
                    }
                    Use::Direct(_) => (),
                    Use::Loan(_, p, k) => {
                        slots.insert((
                            p.slot,
                            if k == BorrowKind::SharedRead {
                                Effect::Shared
                            } else {
                                Effect::Exclusive
                            },
                        ));
                    }
                    Use::Call(name, waited) => {
                        calls.push(name);
                        awaits |= waited;
                    }
                }
            }
            effects.insert(name.as_str(), (slots, calls, awaits));
        }
        loop {
            let mut changed = false;
            let previous = effects.clone();
            for (slots, calls, awaits) in effects.values_mut() {
                let len = slots.len();
                for called in calls {
                    slots.extend(&previous[called].0);
                    if previous[called].2 && !*awaits {
                        *awaits = true;
                        changed = true;
                    }
                }
                changed |= slots.len() != len;
            }
            if !changed {
                break;
            }
        }
        for (name,body) in &self.bodies {
            let source = source_for(name);
            let check = || -> Result<(),Diagnostic> {
            let uses: Vec<_> = body
                .iter()
                .flat_map(|s| s.action.uses().into_iter().map(move |u| (u, s.span,s.active_view)))
                .collect();
            let mut last = HashMap::new();
            for (i, (site, _, _)) in uses.iter().enumerate() {
                if let Use::Direct(Place {
                    alias: Some((name, _)),
                    ..
                }) = site
                {
                    last.insert(name.as_str(), i);
                }
            }
            let mut active = HashMap::<&str, SafetyState>::new();
            for (i, (site, span,active_view)) in uses.into_iter().enumerate() {
                let conflict = || problem(source, span, "模組持有者仍有有效借用");
                for (name, state) in &mut active {
                    if last.get(name).is_none_or(|end| *end < i) {
                        state
                            .transition(StaticTransition::ExpireAliasChecked)
                            .map_err(|_| conflict())?;
                    }
                }
                let snapshot = StaticBorrowSnapshot::from_active(
                    active
                        .values()
                        .filter_map(|s| s.loan.as_ref().filter(|l| l.active).map(|l| l.fact)),
                );
                match site {
                    Use::Loan(name, p, kind) => {
                        let id = self.globals[p.slot].id;
                        snapshot
                            .validate_acquire(id, kind)
                            .map_err(|_| conflict())?;
                        active.insert(
                            name,
                            SafetyState {
                                value: ValueSlot::initialized(CopyPermission::Forbidden),
                                pending_fields: None,
                                loan: Some(LoanBinding {
                                    fact: BorrowFact::new(id, kind),
                                    active: true,
                                }),
                            },
                        );
                    }
                    Use::Direct(p) => {
                        let id = self.globals[p.slot].id;
                        match &p.alias {
                            Some((name, k))
                                if active
                                    .get(name.as_str())
                                    .and_then(|s| s.loan.as_ref())
                                    .filter(|l| l.active)
                                    .map(|l| l.fact)
                                    != Some(BorrowFact::new(id, *k)) =>
                            {
                                return Err(conflict())
                            }
                            None if snapshot.has_active(id) => return Err(conflict()),
                            _ => (),
                        }
                    }
                    Use::Call(n, waited) => {
                        if (waited || effects[n].2) && (active_view || !snapshot.facts().is_empty()) {
                            return Err(problem(
                                source,
                                span,
                                "VB15-01-A：有效借用不得跨直接 await",
                            ));
                        }
                        for &(slot, access) in &effects[n].0 {
                            let id = self.globals[slot].id;
                            match access {
                                Effect::Direct if snapshot.has_active(id) => return Err(conflict()),
                                Effect::Shared => snapshot
                                    .validate_acquire(id, BorrowKind::SharedRead)
                                    .map_err(|_| conflict())?,
                                Effect::Exclusive => snapshot
                                    .validate_acquire(id, BorrowKind::ExclusiveWrite)
                                    .map_err(|_| conflict())?,
                                _ => (),
                            }
                        }
                    }
                }
            }
            Ok(())
            };
            check().map_err(|e|(name.clone(),e))?;
        }
        Ok(())
    }

    pub(crate) fn execute(
        &self,
    ) -> Result<(Result<i64, StrRuntimeError>, Vec<ResourceEvent>), StrRuntimeError> {
        self.execute_with_origin(&[],|error, _| error)
    }

    fn execute_with_origin<E>(
        &self,
        args:&[std::ffi::OsString],
        locate: impl FnOnce(StrRuntimeError, Option<StepOrigin>) -> E,
    ) -> Result<(Result<i64, E>, Vec<ResourceEvent>), StrRuntimeError> {
        let mut state = self.instantiate(ModuleId(0))?;
        state.args=args.to_vec();
        let result = self.call("main", &mut state, 0);
        state
            .loans
            .verify_static_snapshot(std::iter::empty())
            .map_err(|_| StrRuntimeError::OutstandingResource)?;
        Ok((result.map_err(|e| locate(e, state.current_step)), state.heap.finish()?))
    }

    fn instantiate(&self, owner: ModuleId) -> Result<State<'_>, StrRuntimeError> {
        let mut state = State {
            integers:self.globals.iter().map(|g|match g.initial {Initial::Integer(n)=>n, _=>0}).collect(),
            args:Vec::new(),
            owner,
            mixed: crate::mixed_struct_runtime::MixedStructRuntime::new(),
            heap: OwnedStrRuntime::new(),
            current_step: None,
            steps: 0,
            loans: BorrowSafety::new(),
            declarations: &self.globals,
            _local: Default::default(),
        };
        for g in &self.globals {
            let Initial::Text(text)=&g.initial else {continue};
            if let Err(error) = state.heap.declare(&g.name, true, text) {
                state.heap.finish()?;
                return Err(error);
            }
        }
        Ok(state)
    }

    fn call(&self, name: &str, state: &mut State, depth: usize) -> Result<i64, StrRuntimeError> {
        self.call_with_owner(name,state,depth,None, HashMap::new())?.integer(&mut state.heap)
    }

    fn call_with_owner(&self, name: &str, state: &mut State, depth: usize, transfer:Option<Transfer<'_>>, locals: HashMap<String, i64>) -> Result<CallResult, StrRuntimeError> {
        let budget = crate::execution::ExecutionBudget::default();
        if depth >= budget.max_call_frames {
            return Err(StrRuntimeError::ExecutionBudgetExhausted);
        }
        let caller_step = state.current_step;
        let body = &self.bodies[name];
        let mut frame = Frame { locals, ..Frame::default() };
        for site in body.iter().flat_map(|s| s.action.uses()) {
            if let Use::Direct(Place {
                alias: Some((name, _)),
                ..
            }) = site
            {
                *frame.remaining.entry(name.clone()).or_default() += 1;
            }
        }
        let object = match transfer {
            Some(Transfer::Text(args, temps)) => {
                state.heap.enter_with_moved_arguments(args, temps)?;
                None
            }
            Some(Transfer::Collections(values))=>{state.heap.enter_scope(); frame.containers=values; None}
            other => { state.heap.enter_scope(); other }
        };
        state.mixed.enter_scope();
        let result = (|| {
            if let Some(Transfer::Object(from,to)) = object {
                state.mixed.move_whole_with_shared_ledger(&mut state.heap,from,to).map_err(|_|StrRuntimeError::EvaluationFailed)?;
            }
            self.run_body(body,&mut frame,state,depth,budget.max_steps)
        })();
        for ticket in frame.text_tickets.into_values() {
            frame.text_loans.release(ticket).map_err(|_|StrRuntimeError::OutstandingResource)?;
        }
        state.mixed.leave_scope_with_shared_ledger(&mut state.heap).map_err(|_|StrRuntimeError::OutstandingResource)?;
        state.heap.leave_scope()?;
        for ticket in frame.tickets.into_values() {
            state
                .loans
                .release(ticket)
                .map_err(|_| StrRuntimeError::OutstandingResource)?;
        }
        if result.is_ok() {
            state.current_step = caller_step;
        }
        result
    }

    fn invoke_text(&self, call: &TextInvocation, frame: &mut Frame, state: &mut State, depth: usize)
        -> Result<CallResult, StrRuntimeError> {
        use crate::owned_str_runtime::StrArgumentSource;
        enum Source { Shared(String), Moved(String), Temporary(u64) }
        let mut temps = Vec::new();
        let result = (|| {
            let mut integers = HashMap::new();
            let mut sources = Vec::new();
            for (name, _, arg) in &call.arguments {
                match arg {
                    Argument::Integer(n) => { integers.insert(name.clone(), self.eval(n,frame,state,depth)?); }
                    Argument::SharedText(owner) =>
                        sources.push((Source::Shared(format!("@{owner}")), format!("@{name}"))),
                    Argument::Text(LocalText::Move(owner)) =>
                        sources.push((Source::Moved(format!("@{owner}")), format!("@{name}"))),
                    Argument::Text(t) => {
                        let value = self.detached_text(t,frame,state,depth)?;
                        sources.push((Source::Temporary(value.resource_id()), format!("@{name}")));
                        temps.push(value);
                    }
                }
            }
            let args: Vec<_> = sources.iter().map(|(source,name)| {
                let from = match source { Source::Shared(n) => StrArgumentSource::Shared(n), Source::Moved(n) => StrArgumentSource::Named(n), Source::Temporary(id) => StrArgumentSource::Temporary(*id) };
                (from,name.as_str())
            }).collect();
            self.call_with_owner(&call.target, state, depth+1, Some(Transfer::Text(&args,&mut temps)), integers)
        })();
        for value in temps.into_iter().rev() { state.heap.release_detached(value); }
        result
    }

    fn standard_value(&self,call:&StandardCall,frame:&mut Frame,state:&mut State,depth:usize) -> Result<standard::Value,StrRuntimeError> {
        let mut args=Vec::new();
        for arg in &call.args {
            args.push(match arg {
                StandardArg::Int(n)=>standard::Value::Int(self.eval(n,frame,state,depth)?),
                StandardArg::Read(name)=>standard::Value::Text(state.heap.read(&format!("@{name}"))?.into()),
                StandardArg::Text(t)=>{
                    let value=self.detached_text(t,frame,state,depth)?;
                    let text=value.text().to_owned(); state.heap.release_detached(value);
                    standard::Value::Text(text)
                }
            });
        }
        standard::invoke(&call.name,&args,&state.args).ok_or(StrRuntimeError::EvaluationFailed)
    }

    fn detached_text(&self, value: &LocalText, frame: &mut Frame, state: &mut State, depth: usize)
        -> Result<crate::owned_str_runtime::OwnedUtf8, StrRuntimeError> {
        match value {
            LocalText::CaughtMessage => state.heap.create_detached(frame.caught_message.take().ok_or(StrRuntimeError::EvaluationFailed)?),
            LocalText::Standard(call) => match self.standard_value(call,frame,state,depth)? {
                standard::Value::Text(s)=>state.heap.create_detached(&s), _=>Err(StrRuntimeError::EvaluationFailed),
            },
            LocalText::Formatted(parts) => {
                let mut output = String::new();
                for part in parts {
                    match part {
                        FormatPart::Boolean(n) => output.push_str(if self.eval(n,frame,state,depth)? != 0 { "true" } else { "false" }),
                        FormatPart::Float(n,p) => output.push_str(&decimal::display(self.eval(n,frame,state,depth)?,*p)),
                        FormatPart::Literal(s) => output.push_str(s),
                        FormatPart::Text(n) => output.push_str(state.heap.read(&format!("@{n}"))?),
                        FormatPart::Decimal(n) | FormatPart::Hex(n) => {
                            let number = self.eval(n,frame,state,depth)?;
                            if matches!(part, FormatPart::Hex(_)) { output.push_str(&format!("{number:x}")); }
                            else { output.push_str(&number.to_string()); }
                        }
                    }
                }
                state.heap.create_detached(&output)
            }
            LocalText::Literal(text) => state.heap.create_detached(text),
            LocalText::Move(source) => state.heap.return_named(&format!("@{source}")),
            LocalText::Grapheme(source, index) => state.heap.read_grapheme(&format!("@{source}"), index.value(frame)?)
                .map_err(|_| StrRuntimeError::EvaluationFailed),
            LocalText::Replace(change) => change.perform(state, frame),
            LocalText::Call(call) => {
                match self.invoke_text(call, frame, state, depth)? {
                    CallResult::Text(text) => Ok(text),
                    other => { other.integer(&mut state.heap)?; Err(StrRuntimeError::EvaluationFailed) },
                }
            }
        }
    }

    fn run_body(
        &self,
        body: &Body,
        frame: &mut Frame,
        state: &mut State,
        depth: usize,
        max_steps: usize,
    ) -> Result<CallResult, StrRuntimeError> {
        let mut pc = 0;
        while let Some(step) = body.get(pc) {
            pc += 1;
            state.current_step = Some(StepOrigin { module: step.module, span: step.span });
            state.steps += 1;
            if state.steps > max_steps {
                return Err(StrRuntimeError::ExecutionBudgetExhausted);
            }
            for alias in &step.expire {
                let ticket=frame.text_tickets.remove(alias).ok_or(StrRuntimeError::OutstandingResource)?;
                frame.text_loans.release(ticket).map_err(|_|StrRuntimeError::OutstandingResource)?;
            }
            let outcome = (|| -> Result<Option<CallResult>, StrRuntimeError> {
            match &step.action {
                Action::Container(slot, init) => match init {
                    ContainerInit::ReadBytes(call) => {
                        let standard::Value::Bytes(bytes) = self.standard_value(call,frame,state,depth)? else {
                            return Err(StrRuntimeError::EvaluationFailed);
                        };
                        frame.containers.put(slot,containers::Value::from_bytes(bytes)).ok_or(StrRuntimeError::EvaluationFailed)?;
                    }
                    ContainerInit::Call(call)=>{
                        let result=self.invoke_collection(call,frame,state,depth)?;
                        let CallResult::Collection(value)=result else {return Err(StrRuntimeError::EvaluationFailed)};
                        frame.containers.put(slot,value).ok_or(StrRuntimeError::EvaluationFailed)?;
                    }
                    ContainerInit::Optional(item)=>{let value=self.container_item(item,frame,state,depth)?; frame.containers.put(slot,value).ok_or(StrRuntimeError::EvaluationFailed)?;}
                    ContainerInit::Literal(literal) => frame.containers.put(slot,literal.runtime().ok_or(StrRuntimeError::EvaluationFailed)?).ok_or(StrRuntimeError::EvaluationFailed)?,
                    ContainerInit::Move(from) => frame.containers.relocate(from,slot).ok_or(StrRuntimeError::EvaluationFailed)?,
                },
                Action::DropStorage(slots) => Storage::cleanup(slots,frame,&mut state.heap),
                Action::Select { alias, owner, key } => {
                    let key = match key {
                        ContainerKey::Index(n) => containers::Key::Index(self.eval(n,frame,state,depth)?),
                        ContainerKey::NamedText(n)=>containers::Key::Text(state.heap.read(&format!("@{n}"))?.into()),
                        ContainerKey::Text(s) => containers::Key::Text(s.clone()),
                    };
                    frame.containers.select(alias,owner,key).ok_or(StrRuntimeError::EvaluationFailed)?;
                }
                Action::Evaluate(value) => { self.eval(value,frame,state,depth)?; }
                Action::Jump(target) => pc = *target,
                Action::Unless(test,target) => if self.eval(test, frame, state, depth)? == 0 { pc = *target; },
                Action::ReceiveObject {target,owner} => {
                    let result=self.call_with_owner(target,state,depth+1,None,HashMap::new())?;
                    let CallResult::Object(object)=result else {return Err(StrRuntimeError::EvaluationFailed)};
                    if let Err((_,object))=state.mixed.accept_return(owner,object) {
                        crate::mixed_struct_runtime::MixedStructRuntime::discard_with_shared_ledger(&mut state.heap,object);
                        return Err(StrRuntimeError::EvaluationFailed);
                    }
                }
                Action::BorrowLocal{alias,owner,kind}=>{
                    let resource=state.heap.resource_id(&format!("@{owner}"))?;
                    let id=DeclarationId(usize::try_from(resource).map_err(|_|StrRuntimeError::ExhaustedIdentity)?);
                    let ticket=frame.text_loans.acquire(id,*kind).map_err(|_|StrRuntimeError::OutstandingResource)?;
                    frame.text_tickets.insert(alias.clone(),ticket);
                }
                Action::ReturnOptional(item)=>{
                    let value=self.container_item(item,frame,state,depth)?;
                    let value=frame.containers.materialize(value).ok_or(StrRuntimeError::EvaluationFailed)?;
                    return Ok(Some(CallResult::Collection(value)));
                }
                Action::ReturnCollection(slot)=>return frame.containers.take(slot).map(|v|Some(CallResult::Collection(v))).ok_or(StrRuntimeError::EvaluationFailed),
                Action::ReturnObject(name) => return state.mixed.return_with_shared_ledger(&mut state.heap,name)
                    .map(|object|Some(CallResult::Object(object))).map_err(|_|StrRuntimeError::EvaluationFailed),
                Action::RelocateObject { source, target, moving } => {
                    let result = if *moving { state.mixed.move_whole_with_shared_ledger(&mut state.heap, source,target) }
                        else { state.mixed.copy_whole(source,target) };
                    result.map_err(|_| StrRuntimeError::EvaluationFailed)?;
                }
                Action::WriteMember { owner, field, value, initializing } => {
                    use crate::mixed_struct_runtime::SeedValue;
                    let seed = match value {
                        MemberValue::Integer(n) => SeedValue::Int(self.eval(n,frame,state,depth)?),
                        MemberValue::Boolean(n) => SeedValue::Bool(self.eval(n,frame,state,depth)? != 0),
                        MemberValue::Text(s) => SeedValue::Str(s),
                    };
                    let result = if *initializing {
                        state.mixed.initialize_field_with_shared_ledger(&mut state.heap,owner,field,seed)
                    } else { match seed {
                        SeedValue::Int(n) => state.mixed.write_int_field(owner,field,n,true),
                        SeedValue::Bool(b) => state.mixed.write_bool_field(owner,field,b,true),
                        SeedValue::Str(s) => state.mixed.write_str_field_with_shared_ledger(&mut state.heap,owner,field,s,true),
                        _ => unreachable!(),
                    }};
                    result.map_err(|_| StrRuntimeError::EvaluationFailed)?;
                }
                Action::Construct(n,fields,named) => {
                    use crate::mixed_struct_runtime::{MixedFieldSpec,NamedMixed};
                    let schema:Vec<_> = fields.iter().map(|f|MixedFieldSpec{name:&f.name,kind:f.kind,
                        default:f.default.as_ref().map(ApprovedSeed::runtime),immutable:f.immutable}).collect();
                    let mut args=Vec::new();
                    for (field,value) in named {
                        use crate::mixed_struct_runtime::SeedValue;
                        let seed=match value {
                            MemberValue::Integer(n)=>SeedValue::Int(self.eval(n,frame,state,depth)?),
                            MemberValue::Boolean(n)=>SeedValue::Bool(self.eval(n,frame,state,depth)?!=0),
                            MemberValue::Text(s)=>SeedValue::Str(s),
                        };
                        args.push(NamedMixed{field,value:seed});
                    }
                    state.mixed.construct_with_shared_ledger(&mut state.heap,n,&schema,&args,None)
                        .map_err(|_|StrRuntimeError::EvaluationFailed)?;
                }
                Action::StoreInt(slot,rhs)=>{let value=self.eval(rhs,frame,state,depth)?; state.integers[*slot]=value;}
                Action::Bind(n, rhs) => {
                    let value = self.eval(rhs, frame, state, depth)?;
                    frame.locals.insert(n.clone(), value);
                }
                Action::ReplaceLocal(change) => {
                    let value = change.perform(state, frame)?;
                    state.heap.release_detached(value);
                }
                Action::StoreText { target, declaration, value } => {
                    let name = format!("@{target}");
                    match value {
                        LocalText::Literal(text) => match declaration {
                            Some(mutable) => state.heap.declare(&name, *mutable, text)?,
                            None => state.heap.assign(&name, text)?,
                        },
                        LocalText::Move(source) => match declaration {
                            Some(mutable) => state.heap.move_named_as(&format!("@{source}"), &name, *mutable)?,
                            None => state.heap.replace_from(&format!("@{source}"), &name)?,
                        },
                        _ => {
                            let value = self.detached_text(value, frame, state, depth)?;
                            let accepted = match declaration {
                                Some(mutable) => state.heap.accept_return_as(&name, value, *mutable),
                                None => state.heap.replace_return_as(&name, value),
                            };
                            if let Err((error, value)) = accepted {
                                state.heap.release_detached(value);
                                return Err(error);
                            }
                        }
                    }
                }
                Action::Loan(alias, p, kind) => {
                    let ticket = state
                        .loans
                        .acquire(self.globals[p.slot].id, *kind)
                        .map_err(|_| StrRuntimeError::OutstandingResource)?;
                    if frame.remaining.contains_key(alias) {
                        frame.tickets.insert(alias.clone(), ticket);
                    } else {
                        state
                            .loans
                            .release(ticket)
                            .map_err(|_| StrRuntimeError::OutstandingResource)?;
                    }
                }
                Action::Return(rhs) => return self.eval(rhs, frame, state, depth).map(|n|Some(CallResult::Integer(n))),
                Action::ReturnText(rhs) => {
                    let text = self.detached_text(rhs, frame, state, depth)?;
                    if !matches!(rhs, LocalText::Move(_)) { state.heap.mark_detached_return(&text); }
                    return Ok(Some(CallResult::Text(text)));
                }
                Action::Replace(p, i, rhs) => {
                    let text = match rhs {
                        Replacement::Literal(s) => std::borrow::Cow::Borrowed(s.as_str()),
                        Replacement::Named(source) => {
                            frame.authorize(&mut state.loans, source, self.globals[source.slot].id,
                                BorrowKind::SharedRead)?;
                            std::borrow::Cow::Owned(state.heap.read(&self.globals[source.slot].name)?.to_owned())
                        }
                    };
                    frame.authorize(
                        &mut state.loans,
                        p,
                        self.globals[p.slot].id,
                        BorrowKind::ExclusiveWrite,
                    )?;
                    let result = state
                        .heap
                        .replace_grapheme(&self.globals[p.slot].name, i.value(&frame)?, &text)
                        .map_err(|_| StrRuntimeError::EvaluationFailed)?;
                    state.heap.release_detached(result);
                }
                Action::Write(p, rhs) => {
                    let target = &self.globals[p.slot].name;
                    match rhs {
                        Text::Literal(value) => {
                            frame.authorize(
                                &mut state.loans,
                                p,
                                self.globals[p.slot].id,
                                BorrowKind::ExclusiveWrite,
                            )?;
                            state.write(state.owner, target, value)?;
                        }
                        Text::Grapheme(from, i) => {
                            frame.authorize(
                                &mut state.loans,
                                from,
                                self.globals[from.slot].id,
                                BorrowKind::SharedRead,
                            )?;
                            let rhs = state
                                .heap
                                .read_grapheme(&self.globals[from.slot].name, i.value(&frame)?)
                                .map_err(|_| StrRuntimeError::EvaluationFailed)?;
                            let result = frame
                                .authorize(
                                    &mut state.loans,
                                    p,
                                    self.globals[p.slot].id,
                                    BorrowKind::ExclusiveWrite,
                                )
                                .and_then(|()| state.write(state.owner, target, rhs.text()));
                            state.heap.release_detached(rhs);
                            result?;
                        }
                    }
                }
            }
                Ok(None)
            })();
            match outcome {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => (),
                Err(error) => {
                    let category = match error {
                        StrRuntimeError::EvaluationFailed => "EvaluationError",
                        StrRuntimeError::ConversionFailed => "TypeConversationError",
                        _ => return Err(error),
                    };
                    let Some(handler) = step.handlers.iter().find(|h| h.category == category) else { return Err(error) };
                    Storage::cleanup(&handler.cleanup,frame,&mut state.heap);
                    frame.caught_message=Some(crate::entry::runtime_message(error));
                    pc = handler.target;
                }
            }
        }
        Err(StrRuntimeError::IncompleteReturn)
    }

    fn invoke_collection(&self,call:&CollectionCall,frame:&mut Frame,state:&mut State,depth:usize)->Result<CallResult,StrRuntimeError> {
        let mut locals=HashMap::new();
        let mut owned=containers::Store::default();
        for (param,_,arg) in &call.arguments {
            if let CollectionArgument::Optional(item)=arg {
                let value=self.container_item(item,frame,state,depth)?;
                let value=frame.containers.materialize(value).ok_or(StrRuntimeError::EvaluationFailed)?;
                owned.put(param,value).ok_or(StrRuntimeError::EvaluationFailed)?;
            } else if let CollectionArgument::Scalar(n)=arg {
                locals.insert(param.clone(),self.eval(n,frame,state,depth)?);
            }
        }
        for (param,_,arg) in &call.arguments {
            if let CollectionArgument::Move(slot)=arg {
                let value=frame.containers.take(slot).ok_or(StrRuntimeError::MovedSource)?;
                owned.put(param,value).ok_or(StrRuntimeError::EvaluationFailed)?;
            }
        }
        self.call_with_owner(&call.target,state,depth+1,Some(Transfer::Collections(owned)),locals)
    }
    fn container_item(&self,item:&ContainerItem,frame:&mut Frame,state:&mut State,depth:usize)->Result<containers::Value,StrRuntimeError> {
        Ok(match item {
            ContainerItem::Record(fields)=>{
                let mut values=BTreeMap::new();
                for (name,field) in fields {
                    values.insert(containers::Key::Text(name.clone()),self.container_item(field,frame,state,depth)?);
                }
                containers::Value::Dict(values)
            }
            ContainerItem::Decimal(n)=>containers::Value::Decimal(self.eval(n,frame,state,depth)?),
            ContainerItem::Scalar(n,boolean)=>{
                let v=self.eval(n,frame,state,depth)?;
                if *boolean {containers::Value::Boolean(v!=0)} else {containers::Value::Integer(v)}
            }
            ContainerItem::Literal(v)=>v.clone(),
        })
    }

    fn eval(
        &self,
        n: &Number,
        frame: &mut Frame,
        state: &mut State,
        depth: usize,
    ) -> Result<i64, StrRuntimeError> {
        match n {
            Number::FromCollection(call)=>self.invoke_collection(call,frame,state,depth)?.integer(&mut state.heap),
            Number::Method(call)=>{
                let mut inner=Frame::default();
                for (name,n) in &call.arguments {
                    inner.locals.insert(name.clone(),self.eval(n,frame,state,depth)?);
                }
                let mode=if call.writes {BorrowKind::ExclusiveWrite} else {BorrowKind::SharedRead};
                let ticket=frame.object_loans.acquire(call.id,mode).map_err(|_|StrRuntimeError::OutstandingResource)?;
                let outer=state.current_step;
                let result=(|| {
                    let budget=crate::execution::ExecutionBudget::default();
                    if depth+1>=budget.max_call_frames {return Err(StrRuntimeError::ExecutionBudgetExhausted);}
                    for step in &call.steps {
                        state.current_step=Some(step.origin);
                        state.steps+=1;
                        if state.steps>budget.max_steps {return Err(StrRuntimeError::ExecutionBudgetExhausted);}
                        let n=self.eval(&step.value,&mut inner,state,depth+1)?;
                        if let Some(field)=&step.write {
                            (if step.boolean {state.mixed.write_bool_field(&call.owner,field,n!=0,true)}
                             else {state.mixed.write_int_field(&call.owner,field,n,true)}).map_err(|_|StrRuntimeError::EvaluationFailed)?;
                        } else {return Ok(n);}
                    }
                    Err(StrRuntimeError::IncompleteReturn)
                })();
                frame.object_loans.release(ticket).map_err(|_|StrRuntimeError::OutstandingResource)?;
                if result.is_ok() {state.current_step=outer;}
                result
            }
            Number::Member { owner,field,receiver } => {
                let ticket = receiver.map(|id|frame.object_loans.acquire(id,BorrowKind::SharedRead)).transpose()
                    .map_err(|_|StrRuntimeError::OutstandingResource)?;
                let result = match state.mixed.field(owner,field) {
                    Ok(crate::mixed_struct_runtime::Observed::Int(n)) => Ok(n),
                    _ => Err(StrRuntimeError::EvaluationFailed),
                };
                if let Some(ticket) = ticket { frame.object_loans.release(ticket).map_err(|_|StrRuntimeError::OutstandingResource)?; }
                result
            }
            Number::Standard(call) => match self.standard_value(call,frame,state,depth)? {
                standard::Value::Int(n)=>Ok(n), _=>Err(StrRuntimeError::EvaluationFailed),
            },
            Number::TextCall(call) => self.invoke_text(call,frame,state,depth)?.integer(&mut state.heap),
            Number::Consume {target,from,parameter} => self.call_with_owner(target,state,depth+1,Some(Transfer::Object(from,parameter)), HashMap::new())?.integer(&mut state.heap),
            Number::ViewByte(slot,i,j) => {
                let Some(containers::Value::Text(text)) = frame.containers.get(slot) else { return Err(StrRuntimeError::EvaluationFailed) };
                crate::owned_str_runtime::literal_byte(text,i.value(frame)?,j.value(frame)?).ok_or(StrRuntimeError::EvaluationFailed)
            }
            Number::Edit(slot, edit) => {
                match edit.as_ref() {
                    Edit::ReplaceOptional(value)=>{let v=self.container_item(value,frame,state,depth)?; frame.containers.put(slot,v).map(|_|0)}
                    Edit::Append(value) => {
                        let v=self.container_item(value,frame,state,depth)?;
                        frame.containers.append(slot,v)
                    }
                    Edit::Set(key,value) => {
                        let k=match key { ContainerKey::NamedText(n)=>containers::Key::Text(state.heap.read(&format!("@{n}"))?.into()),
                            ContainerKey::Text(s)=>containers::Key::Text(s.clone()),
                            ContainerKey::Index(n)=>containers::Key::Index(self.eval(n,frame,state,depth)?) };
                        let v=self.container_item(value,frame,state,depth)?;
                        frame.containers.set(slot,k,v)
                    }
                    Edit::Pop => frame.containers.pop(slot),
                }.ok_or(StrRuntimeError::EvaluationFailed)
            }
            Number::Container(slot, read) => {
                let value = frame.containers.get(slot).ok_or(StrRuntimeError::EvaluationFailed)?;
                match read { ContainerRead::Field(field) => value.field(field).and_then(containers::Value::scalar), ContainerRead::Length => value.length(), ContainerRead::Tag => value.tag(),
                    ContainerRead::Scalar => value.scalar() }.ok_or(StrRuntimeError::EvaluationFailed)
            }
            Number::Choice(test, yes, no) => {
                let selected = if self.eval(test, frame, state, depth)? != 0 { yes } else { no };
                self.eval(selected, frame, state, depth)
            }
            Number::BoolMember(owner, field) => match state.mixed.field(owner,field) {
                Ok(crate::mixed_struct_runtime::Observed::Bool(b)) => Ok(i64::from(b)),
                _ => Err(StrRuntimeError::EvaluationFailed),
            },
            Number::ScalarCall { target, arguments, awaited } => {
                let mut bindings = HashMap::new();
                for (name, _, value) in arguments {
                    bindings.insert(name.clone(), self.eval(value, frame, state, depth)?);
                }
                if *awaited { state.loans.verify_static_snapshot(std::iter::empty()).map_err(|_|StrRuntimeError::OutstandingResource)?; }
                self.call_with_owner(target, state, depth + 1, None, bindings)?.integer(&mut state.heap)
            }
            Number::Logic(op,a,b) => {
                let left = self.eval(a, frame, state, depth)?;
                let result = match *op {
                    "decimal!=" => decimal::unequal(left,self.eval(b,frame,state,depth)?),
                    "and" => left != 0 && self.eval(b, frame, state, depth)? != 0,
                    "or" => left != 0 || self.eval(b, frame, state, depth)? != 0,
                    "==" => left == self.eval(b, frame, state, depth)?,
                    "<=" => left <= self.eval(b, frame, state, depth)?,
                    ">" => left > self.eval(b, frame, state, depth)?,
                    ">=" => left >= self.eval(b, frame, state, depth)?,
                    "<" | "lt" => left < self.eval(b, frame, state, depth)?,
                    _ => left != self.eval(b, frame, state, depth)?,
                };
                Ok(i64::from(result))
            }
            Number::GlobalInt(slot)=>Ok(state.integers[*slot]),
            Number::Constant(v) => Ok(*v),
            Number::Local(n) => frame
                .locals
                .get(n)
                .copied()
                .ok_or(StrRuntimeError::MissingOwner),
            Number::Call(n, awaited) => {
                if *awaited {
                    state
                        .loans
                        .verify_static_snapshot(std::iter::empty())
                        .map_err(|_| StrRuntimeError::OutstandingResource)?;
                }
                self.call(n, state, depth + 1)
            }
            Number::MemberByte(owner, field, i, j) => {
                match state.mixed.field(owner,field) {
                    Ok(crate::mixed_struct_runtime::Observed::Str(text)) =>
                        crate::owned_str_runtime::literal_byte(text,i.value(frame)?,j.value(frame)?)
                            .ok_or(StrRuntimeError::EvaluationFailed),
                    _ => Err(StrRuntimeError::EvaluationFailed),
                }
            }
            Number::LocalByte(owner, i, j) => state.heap.read_grapheme_byte(&format!("@{owner}"),
                i.value(frame)?, j.value(frame)?).map_err(|_| StrRuntimeError::EvaluationFailed),
            Number::Byte(p, i, j) => {
                frame.authorize(
                    &mut state.loans,
                    p,
                    self.globals[p.slot].id,
                    BorrowKind::SharedRead,
                )?;
                state.byte(state.owner, true, &self.globals[p.slot].name, i.value(frame)?, j.value(frame)?)
            }
            Number::TextLength(n)=>state.heap.grapheme_count(&format!("@{n}")),
            Number::ParseInt(n) => state.heap.read(&format!("@{n}"))?.parse::<i64>()
                .map_err(|_|StrRuntimeError::ConversionFailed),
            Number::Decimal(left, op, right) => {
                let a = self.eval(left, frame, state, depth)?;
                let b = self.eval(right, frame, state, depth)?;
                decimal::arithmetic(a,*op,b).ok_or(StrRuntimeError::EvaluationFailed)
            }
            Number::Binary(left, op, right) => {
                let a = self.eval(left, frame, state, depth)?;
                let b = self.eval(right, frame, state, depth)?;
                integer::arithmetic(a, *op, b)
                .ok_or(StrRuntimeError::EvaluationFailed)
            }
        }
    }
}

enum CallResult {
    Collection(containers::Value),
    Object(crate::mixed_struct_runtime::ReturnedMixed),
    Integer(i64),
    Text(crate::owned_str_runtime::OwnedUtf8),
}

impl CallResult {
    fn integer(self, heap: &mut OwnedStrRuntime) -> Result<i64, StrRuntimeError> {
        match self {
            Self::Integer(value) => Ok(value),
            Self::Collection(_)=>Err(StrRuntimeError::EvaluationFailed),
            Self::Object(value) => {
                crate::mixed_struct_runtime::MixedStructRuntime::discard_with_shared_ledger(heap,value);
                Err(StrRuntimeError::EvaluationFailed)
            }
            Self::Text(value) => {
                heap.release_detached(value);
                Err(StrRuntimeError::EvaluationFailed)
            }
        }
    }
}

enum Transfer<'a> {
    Collections(containers::Store),
    Text(&'a [(crate::owned_str_runtime::StrArgumentSource<'a>, &'a str)], &'a mut Vec<crate::owned_str_runtime::OwnedUtf8>),
    Object(&'a str, &'a str),
}

#[derive(Default)]
struct Frame {
    caught_message: Option<&'static str>,
    containers: containers::Store,
    text_loans:BorrowSafety,
    text_tickets:HashMap<String,BorrowTicket>,
    object_loans: BorrowSafety,
    locals: HashMap<String, i64>,
    tickets: HashMap<String, BorrowTicket>,
    remaining: HashMap<String, usize>,
}
impl Frame {
    fn authorize(
        &mut self,
        loans: &mut BorrowSafety,
        place: &Place,
        id: DeclarationId,
        mode: BorrowKind,
    ) -> Result<(), StrRuntimeError> {
        let Some((alias, _)) = &place.alias else {
            return loans
                .check_direct_access(id)
                .map_err(|_| StrRuntimeError::OutstandingResource);
        };
        let ticket = *self
            .tickets
            .get(alias)
            .ok_or(StrRuntimeError::OutstandingResource)?;
        if loans.check_ticket_access(ticket, mode) != Ok(id) {
            return Err(StrRuntimeError::OutstandingResource);
        }
        let remaining = self
            .remaining
            .get_mut(alias)
            .ok_or(StrRuntimeError::OutstandingResource)?;
        *remaining = remaining
            .checked_sub(1)
            .ok_or(StrRuntimeError::OutstandingResource)?;
        if *remaining == 0 {
            loans
                .release(ticket)
                .map_err(|_| StrRuntimeError::OutstandingResource)?;
            self.tickets.remove(alias);
        }
        Ok(())
    }
}

struct State<'a> {
    integers: Vec<i64>,
    args:Vec<std::ffi::OsString>,
    mixed: crate::mixed_struct_runtime::MixedStructRuntime,
    declarations: &'a [Declaration],
    _local: std::marker::PhantomData<Rc<()>>,
    loans: BorrowSafety,
    owner: ModuleId,
    heap: OwnedStrRuntime,
    current_step: Option<StepOrigin>,
    steps: usize,
}
impl State<'_> {
    fn byte(
        &self,
        caller: ModuleId,
        visible: bool,
        n: &str,
        i: i64,
        j: i64,
    ) -> Result<i64, StrRuntimeError> {
        if !self.owner.permits(caller, visible, Access::Read) {
            return Err(StrRuntimeError::MissingOwner);
        }
        if self.owner != caller {
            let declared = self
                .declarations
                .iter()
                .find(|g| g.name == n)
                .ok_or(StrRuntimeError::MissingOwner)?;
            self.loans
                .check_direct_access(declared.id)
                .map_err(|_| StrRuntimeError::OutstandingResource)?;
        }
        self.heap
            .read_grapheme_byte(n, i, j)
            .map_err(|_| StrRuntimeError::EvaluationFailed)
    }
    fn write(&mut self, caller: ModuleId, n: &str, value: &str) -> Result<(), StrRuntimeError> {
        if !self.owner.permits(caller, true, Access::Write) {
            return Err(StrRuntimeError::ImmutableBinding);
        }
        self.heap.assign(n, value)
    }
}

#[cfg(test)]
#[path = "module_share_tests.rs"]
mod tests;
