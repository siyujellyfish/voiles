//! P21 多檔來源。每份來源保留自己的 Lexer、Parser 與診斷座標。
use super::*;
use crate::execution::registry::{OpenSource, RegistryFailure};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) enum ModuleError {
    Runtime(StrRuntimeError),
    RuntimeAt { path: PathBuf, span: crate::SourceSpan, error: StrRuntimeError },
    File(RegistryFailure),
    Source { path: PathBuf, diagnostic: Diagnostic },
    Import { path: PathBuf, span: crate::SourceSpan, cause: RegistryFailure },
    Cycle { path: PathBuf, span: crate::SourceSpan, chain: Vec<PathBuf> },
}

pub(in crate::struct_stage) struct Import {
    module: String,
    member: Option<String>,
    local: String,
    start: usize,
    end: usize,
}

fn word<const PUBLIC:bool>(parser: &LanguageParser<'_,PUBLIC>, word: &str) -> bool {
    matches!(&parser.tok().kind, TokenKind::Name(n) | TokenKind::Keyword(n) if n == word)
}

pub(in crate::struct_stage) fn parse_import<const PUBLIC:bool>(parser: &mut LanguageParser<'_,PUBLIC>) -> Result<Import, Diagnostic> {
    let start = parser.tok().start;
    parser.cursor += 1;
    let first = parser.name()?;
    let (module, member, local) = if word(parser, "from") {
        parser.cursor += 1;
        let module = parser.name()?;
        if !word(parser, "as") { return Err(parser.syntax("具名匯入需要 as 別名")); }
        parser.cursor += 1;
        (module, Some(first), parser.name()?)
    } else {
        (first.clone(), None, first)
    };
    Ok(Import { module, member, local, start, end: parser.tokens[parser.cursor-1].end })
}

/// 名稱的可見性由定義模組與宣告種類決定。
fn accessible(name: &str, same_module: bool) -> bool {
    same_module || !name.starts_with('_')
}

struct Unit {
    path: PathBuf,
    source: Source,
    tokens: Vec<Token>,
    imports: Vec<Import>,
    functions: Vec<Function>,
    structs: HashMap<String,StructDecl>,
    shares: Vec<Declaration>,
    edges: Vec<usize>,
}

impl Unit {
    fn parse(path: &Path, bytes: Vec<u8>) -> Result<Self, Diagnostic> {
        let source = Source::new(bytes);
        if let Some(error) = source.lexical_diagnostics().into_iter().next() { return Err(error); }
        let lexed = lexer::scan(&source);
        if let Some(error) = lexed.diagnostics.into_iter().next() { return Err(error); }
        let mut imports = Vec::new();
        let mut shares = Vec::new();
        let (structs, functions) = LanguageParser::<true> { source: &source, tokens: &lexed.tokens, cursor: 0 }
            .module_items(Some(&mut shares),Some(&mut imports))?;
        Ok(Self { path: path.to_owned(), source, tokens: lexed.tokens, imports, functions, structs, shares, edges: Vec::new() })
    }

    fn declares(&self, name: &str) -> bool {
        self.functions.iter().any(|f| f.name == name) || self.structs.contains_key(name)
            || self.shares.iter().any(|d| d.name == name)
    }

    fn error(&self, start: usize, end: usize, message: &str) -> ModuleError {
        ModuleError::Source { path: self.path.clone(), diagnostic: Diagnostic {
            kind: DiagnosticKind::Static, message: message.into(), span: Some(self.source.span(start,end))
        }}
    }
}

struct Snapshot {
    unit: usize,
    canonical: PathBuf,
    physical: Option<(u64,u64)>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum FileKey { Inode(u64,u64), Canonical(PathBuf) }

pub(crate) struct ModuleGraph {
    units: Vec<Unit>,
    snapshots: BTreeMap<PathBuf,Snapshot>,
    order: Vec<usize>,
}

impl ModuleGraph {
    fn load(&mut self, path: &Path, identities: &mut BTreeMap<FileKey,usize>) -> Result<usize,ModuleError> {
        if let Some(s) = self.snapshots.get(path) { return Ok(s.unit); }
        let mut opened = OpenSource::open(path).map_err(ModuleError::File)?;
        let key = match opened.physical {
            Some((dev,ino)) => FileKey::Inode(dev,ino),
            None => FileKey::Canonical(opened.canonical.clone()),
        };
        let unit = if let Some(&id) = identities.get(&key) { id } else {
            let bytes = opened.bytes(path).map_err(ModuleError::File)?;
            let unit = Unit::parse(path,bytes).map_err(|diagnostic| ModuleError::Source { path:path.to_owned(), diagnostic })?;
            let id = self.units.len();
            self.units.push(unit);
            identities.insert(key,id);
            id
        };
        self.snapshots.insert(path.to_owned(),Snapshot { unit, canonical:opened.canonical,physical:opened.physical });
        Ok(unit)
    }

    pub(crate) fn prepare(entry: &Path) -> Result<Self,ModuleError> {
        let root = entry.parent().unwrap_or(Path::new(""));
        let mut graph = Self { units: Vec::new(), snapshots:BTreeMap::new(),order:Vec::new() };
        let mut identities = BTreeMap::new();
        let first = graph.load(entry,&mut identities)?;
        let mut stack = vec![(first,0)];
        let mut active = BTreeMap::from([(first,0)]);
        let mut done = BTreeSet::new();
        while let Some((id,next)) = stack.last_mut() {
            let id = *id;
            let unit = &graph.units[id];
            let Some(import) = unit.imports.get(*next) else {
                graph.order.push(id);
                done.insert(id);
                active.remove(&id);
                stack.pop();
                continue;
            };
            *next += 1;
            let path = root.join(format!("{}.voil",import.module));
            let span = unit.source.span(import.start,import.end);
            let child = graph.load(&path,&mut identities).map_err(|e| match e {
                ModuleError::File(cause) => ModuleError::Import {path:graph.units[id].path.clone(),span,cause},
                other => other,
            })?;
            graph.units[id].edges.push(child);
            if let Some(&start) = active.get(&child) {
                let chain = stack[start..].iter().map(|(i,_)| graph.units[*i].path.clone())
                    .chain(std::iter::once(path)).collect();
                return Err(ModuleError::Cycle { path:graph.units[id].path.clone(),span,chain });
            }
            if !done.contains(&child) {
                active.insert(child,stack.len());
                stack.push((child,0));
            }
        }
        for unit in &graph.units {
            let mut names = HashSet::new();
            for (name,site) in unit.structs.iter().map(|(n,s)|(n.as_str(),s.source_span.start))
                .chain(unit.functions.iter().map(|f|(f.name.as_str(),f.start)))
                .chain(unit.shares.iter().map(|d|(d.name.as_str(),d.id.0))) {
                if !names.insert(name) { return Err(unit.error(site,site+name.len(),"模組宣告名稱重複")); }
            }
            for (i,import) in unit.imports.iter().enumerate() {
                if !names.insert(import.local.as_str()) {
                    return Err(unit.error(import.start,import.end,"匯入名稱已宣告"));
                }
                if let Some(n) = &import.member {
                    if !accessible(n,false) || !graph.units[unit.edges[i]].declares(n) {
                        return Err(unit.error(import.start,import.end,"找不到可匯入的公開宣告"));
                    }
                }
            }
        }
        graph.check_unchanged()?;
        Ok(graph)
    }

    /// 解析匯入型別時保留定義模組；欄位、方法仍須分別檢查可見性。
    fn resolve_struct(&self,caller:usize, name:&str) -> Option<(usize,&StructDecl)> {
        let unit = &self.units[caller];
        if let Some(def) = unit.structs.get(name) { return Some((caller,def)); }
        for (item,&origin) in unit.imports.iter().zip(&unit.edges) {
            let member = match (&item.member,name.split_once('.')) {
                (Some(n),None) if item.local==name => n.as_str(),
                (None,Some((module,n))) if module == item.local => n,
                _ => continue,
            };
            if accessible(member,caller==origin) {
                return self.units[origin].structs.get(member).map(|s|(origin,s));
            }
        }
        None
    }

    fn field(&self, caller:usize, ty:&str,name:&str) -> Option<&Field> {
        let (origin,def) = self.resolve_struct(caller,ty)?;
        if !accessible(name,caller==origin) { return None; }
        def.field(name)
    }

    fn methods(&self, caller:usize,ty:&str,name:&str) -> Vec<&Function> {
        let Some((origin,def)) = self.resolve_struct(caller,ty) else { return Vec::new(); };
        if !accessible(name,caller==origin) { return Vec::new(); }
        def.method_index.get(name).into_iter().flatten().map(|&index| &def.methods[index]).collect()
    }

    fn check_unchanged(&self) -> Result<(),ModuleError> {
        for (path,s) in &self.snapshots {
            let mut opened = OpenSource::open(path).map_err(ModuleError::File)?;
            if opened.canonical != s.canonical || opened.physical != s.physical
                || opened.bytes(path).map_err(ModuleError::File)? != self.units[s.unit].source.bytes {
                return Err(ModuleError::File(RegistryFailure::SourceChanged { path:path.clone() }));
            }
        }
        Ok(())
    }
}

#[path = "module_objects.rs"]
pub(super) mod objects;
use objects::ObjectScope;

/// 每個節點各自解析；跨檔呼叫最後指向定義模組的函式。
pub(crate) struct LinkedProgram {
    graph: ModuleGraph,
    plan: Program,
}

#[path = "native.rs"]
mod native;
#[path = "bootstrap.rs"]
mod bootstrap;

impl LinkedProgram {
    pub(crate) fn prepare(entry: &Path) -> Result<Self,ModuleError> {
        let graph = ModuleGraph::prepare(entry)?;
        let mut globals = Vec::new();
        let mut slots = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let mut owners = BTreeMap::new();
        let key = |id:usize,n:&str| if id==0 && n=="main" { String::from("main") } else { format!("{id}:{n}") };
        for &id in &graph.order {
            let u = &graph.units[id];
            for d in &u.shares {
                let name = key(id,&d.name);
                let slot = globals.len();
                if slots.insert(name.clone(),slot).is_some() || u.functions.iter().any(|f| f.name==d.name) || u.structs.contains_key(&d.name) {
                    return Err(u.error(d.id.0,d.id.0+d.name.len(),"模組宣告重複"));
                }
                globals.push(Declaration { name, initial:d.initial.clone(), id:DeclarationId(slot) });
            }
        }
        for (id,u) in graph.units.iter().enumerate() {
            for f in &u.functions { owners.insert(key(id,&f.name),id); }
        }
        for (id,u) in graph.units.iter().enumerate() {
            let mut definitions: Vec<_> = u.structs.values().collect();
            definitions.sort_unstable_by_key(|def| def.source_span.start);
            for def in definitions {
                objects::check_definition(u,def)?;
            }
            let mut names = BTreeMap::new();
            let mut targets = BTreeMap::new();
            let mut visible = BTreeMap::new();
            let mut readonly = BTreeSet::new();
            let mut expose = |origin:usize,member:&str,local:String| {
                let target = key(origin,member);
                if let Some(&slot) = slots.get(&target) {
                    visible.insert(local,slot);
                    if origin != id { readonly.insert(slot); }
                } else if let Some(f) = graph.units[origin].functions.iter().find(|f| f.name==member) {
                    names.insert(local.clone(),f.is_async);
                    targets.insert(local,target);
                }
            };
            for d in &u.shares { expose(id,&d.name,d.name.clone()); }
            for f in &u.functions { expose(id,&f.name,f.name.clone()); }
            for (imp,&origin) in u.imports.iter().zip(&u.edges) {
                if let Some(n) = &imp.member { expose(origin,n,imp.local.clone()); }
                else {
                    for n in graph.units[origin].functions.iter().map(|f|&f.name)
                        .chain(graph.units[origin].shares.iter().map(|s|&s.name)).filter(|n|accessible(n,false)) {
                        expose(origin,n,format!("{}.{}",imp.local,n));
                    }
                }
            }
            for f in &u.functions {
                if f.result_ty == "str" && f.params.iter().any(|p| !matches!(p.ty.as_str(), "int" | "str" | "bool" | "float")) {
                    return Err(u.error(f.start,f.end,"字串返回簽章需要純量或 str 形參"));
                }
                let collection_signature = stored_type(&f.result_ty) || f.params.iter().any(|p|stored_type(&p.ty));
                let object_result = !f.is_async && graph.resolve_struct(id,&f.result_ty).is_some();
                if (collection_signature && f.is_async) || (!object_result && !stored_type(&f.result_ty) && !matches!(f.result_ty.as_str(), "bool" | "int" | "str" | "float" | "none")) || (matches!(f.result_ty.as_str(), "str" | "none") && f.is_async) || (id==0 && f.name=="main" && (f.is_async || f.result_ty != "int" || !f.params.is_empty())) {
                    return Err(u.error(f.start,f.end,"跨檔直線執行需要已驗證的 int 函式簽章"));
                }
                let mut objects = ObjectScope::new(&graph,id,&targets,&owners);
                if std::iter::once(&f.result_ty).chain(f.params.iter().map(|p|&p.ty))
                    .any(|ty|stored_type(ty) && !objects.valid_storage(ty)) {
                    return Err(u.error(f.start,f.end,"容器簽章引用不可見或未宣告的元素型別"));
                }
                let mut locals = HashMap::new();
                let mut strings = HashMap::new();
                if f.params.iter().all(|p| stored_type(&p.ty) || matches!(p.ty.as_str(), "int" | "bool" | "float")) {
                    for p in &f.params {
                        if locals.insert(p.name.clone(), (false,p.ty.clone())).is_some() {
                            return Err(u.error(f.start, f.end, "函式參數重複"));
                        }
                    }
                } else if !f.is_async && matches!(f.result_ty.as_str(),"int" | "str" | "none")
                    && f.params.iter().all(|p| matches!(p.ty.as_str(),"int" | "str" | "bool" | "float")) {
                    for p in &f.params {
                        if strings.contains_key(&p.name) || locals.contains_key(&p.name) {
                            return Err(u.error(f.start,f.end,"函式參數重複"));
                        }
                        if p.ty == "str" { strings.insert(p.name.clone(), TextBinding::new(false)); }
                        else { locals.insert(p.name.clone(), (false,p.ty.clone())); }
                    }
                } else if let [p] = f.params.as_slice() {
                    if !f.is_async && graph.resolve_struct(id,&p.ty).is_some() {
                        objects.bind(&p.name,&p.ty,p.declaration_start,false);
                    } else {
                        return Err(u.error(f.start,f.end,"跨檔持有者參數需要同步函式與可見結構型別"));
                    }
                } else {
                    return Err(u.error(f.start, f.end, "混合持有者參數尚未支援"));
                }
                let mut lower = Lower { integer_slots:globals.iter().enumerate().filter_map(|(i,g)|matches!(g.initial,Initial::Integer(_)).then_some(i)).collect(), text_access:Default::default(),objects: Some(objects), source:&u.source,tokens:&u.tokens,globals:&visible,functions:&names,
                    linked_names:Some(&targets),readonly_slots:readonly.clone(),locals,strings,aliases:HashMap::new() };
                let typed_signature = matches!(f.result_ty.as_str(), "bool" | "float" | "none") || f.params.iter().any(|p| matches!(p.ty.as_str(), "bool" | "float"));
                let body = if f.result_ty == "str" {
                    lower.body(f,id).or_else(|_| scalar_flow::lower(&lower,f,id))
                } else if object_result {
                    lower.body(f,id)
                } else if typed_signature || collection_signature {
                    scalar_flow::lower(&lower, f, id)
                } else {
                    let pure = scalar_flow::lower(&lower, f, id);
                    if scalar_flow::needed(f) {
                        pure.or_else(|flow_error| lower.body(f, id).map_err(|_| flow_error))
                    } else {
                        lower.body(f, id).or_else(|original| pure.map_err(|_| original))
                    }
                }.map_err(|diagnostic|ModuleError::Source{path:u.path.clone(),diagnostic})?;
                let name = key(id,&f.name);

                bodies.insert(name,body);
            }
        }
        if !bodies.contains_key("main") { return Err(graph.units[0].error(0,0,"入口模組需要 fn main()->int")); }
        let plan = Program { globals,bodies,_local:Default::default() };
        plan.verify_linked_loans(|n| &graph.units[owners[n]].source)
            .map_err(|(n,diagnostic)|ModuleError::Source{path:graph.units[owners[&n]].path.clone(),diagnostic})?;
        Ok(Self{graph,plan})
    }

    /// 提供同一份已通過跨模組借用檢查的計畫；消費者不重新讀取或解析來源。
    fn checked_plan(&self) -> Result<&Program, ModuleError> {
        self.graph.check_unchanged()?;
        Ok(&self.plan)
    }

    pub(crate) fn execute(&self) -> Result<(Result<i64,ModuleError>,Vec<ResourceEvent>),ModuleError> {
        self.execute_with_args(&[])
    }

    pub(crate) fn execute_with_args(&self,args:&[std::ffi::OsString]) -> Result<(Result<i64,ModuleError>,Vec<ResourceEvent>),ModuleError> {
        self.checked_plan()?.execute_with_origin(args,|error, origin| match origin {
            Some(step) => {
                let unit = &self.graph.units[step.module];
                ModuleError::RuntimeAt { path: unit.path.clone(),
                    span: unit.source.span(step.span.0, step.span.1), error }
            }
            None => ModuleError::Runtime(error),
        }).map_err(ModuleError::Runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize,Ordering};

    struct Files(PathBuf);
    impl Files {
        fn new(files: &[(&str,&str)]) -> Self {
            static ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!("p21-modules-{}-{}",std::process::id(),ID.fetch_add(1,Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            for (name,text) in files { fs::write(path.join(name),text.as_bytes()).unwrap(); }
            Self(path)
        }
        fn entry(&self) -> PathBuf { self.0.join("main.voil") }
    }
    #[test]
    fn shared_plan_preserves_linked_steps_and_rejects_stale_dependencies() {
        let files = Files::new(&[
            ("main.voil", "import dep\nfn main()->int{ return dep.value() + 1 }"),
            ("dep.voil", "fn value()->int{ return 41 }"),
        ]);
        let program = LinkedProgram::prepare(&files.entry()).unwrap();
        assert_eq!(program.checked_plan().unwrap().bodies.len(), 2);
        assert_eq!(program.execute().unwrap().0.unwrap(), 42);
        fs::write(files.0.join("dep.voil"), b"fn value()->int{ return 0 }").unwrap();
        assert!(program.checked_plan().is_err());
        assert!(program.execute().is_err());
    }

    #[test]
    fn invalid_sources_never_publish_a_plan() {
        for source in [
            "fn main()->int{ return true }",
            "share msg:str = \"A\"\nfn main()->int{\nconst alias:str = msg.borrow()\nmsg = \"B\"\nreturn alias[0][0]\n}",
        ] {
            let files = Files::new(&[("main.voil", source)]);
            assert!(matches!(LinkedProgram::prepare(&files.entry()), Err(ModuleError::Source { .. })));
        }
    }

    impl Drop for Files { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn diamond_calls_initialize_one_shared_owner() {
        let files = Files::new(&[
            ("main.voil","import left\nimport get from right as value\nfn main()->int{\nconst ignored:int = left.set()\nreturn value()\n}"),
            ("left.voil","import core\nfn set()->int{\nreturn core.update()\n}"),
            ("right.voil","import core\nfn get()->int{\nreturn core.message[0][0]\n}"),
            ("core.voil","share message:str = \"A\"\nfn update()->int{\nmessage = \"Z\"\nreturn 0\n}")
        ]);
        let p = LinkedProgram::prepare(&files.entry()).unwrap();
        let order:Vec<_> = p.graph.order.iter().map(|&id|p.graph.units[id].path.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(order,["core.voil","left.voil","right.voil","main.voil"]);
        let (v,events) = p.execute().unwrap();
        assert_eq!(v.unwrap(),90);
        use ResourceEvent::*;
        assert_eq!(events,[Created(1),Created(2),Released(1),Released(2)]);
    }

    #[test]
    fn imported_alias_cannot_write_or_hold_loan_over_remote_write() {
        for body in ["s = \"B\"\nreturn 0","var a:str = s.borrow()\nreturn a[0][0]",
                     "const a:str = s.borrow()\nconst k:int = c.set()\nreturn a[0][0]"] {
            let main = format!("import s from c as s\nimport c\nfn main()->int{{\n{body}\n}}");
            let files = Files::new(&[("main.voil",&main),("c.voil","share s:str = \"A\"\nfn set()->int{\ns = \"B\"\nreturn 0\n}")]);
            assert!(matches!(LinkedProgram::prepare(&files.entry()),Err(ModuleError::Source{path,..}) if path==files.entry()));
        }
    }

    #[test]
    fn cycle_missing_file_and_private_import_preserve_paths() {
        let files = Files::new(&[("main.voil","import one"),("one.voil","import two"),("two.voil","import one")]);
        assert!(matches!(ModuleGraph::prepare(&files.entry()),Err(ModuleError::Cycle{chain,..})
            if chain == ["one.voil","two.voil","one.voil"].map(|p|files.0.join(p))));
        fs::write(files.0.join("two.voil"),b"import absent").unwrap();
        assert!(matches!(ModuleGraph::prepare(&files.entry()),Err(ModuleError::Import{path,span,cause:RegistryFailure::Io{path:missing,..}})
            if path==files.0.join("two.voil") && missing==files.0.join("absent.voil") && span.start.line==1));
        fs::write(files.entry(),b"import _hidden from one as local").unwrap();
        fs::write(files.0.join("one.voil"),b"fn _hidden()->int{\nreturn 3\n}").unwrap();
        assert!(matches!(ModuleGraph::prepare(&files.entry()),Err(ModuleError::Source{path,..}) if path==files.entry()));
    }

    #[test]
    fn public_type_keeps_member_visibility_in_its_defining_module() {
        let files = Files::new(&[("main.voil","import model\nimport Box from model as Named"),
            ("model.voil","struct Box{\nvar _secret:int = 1\nvar count:int = 3\nfn _peek()->int{\nreturn self._secret\n}\nfn read()->int{\nreturn self.count\n}\n}\nstruct _Inside{\nvar value:int\n}")]);
        let graph = ModuleGraph::prepare(&files.entry()).unwrap();
        let model = graph.units[0].edges[0];
        for ty in ["model.Box","Named"] {
            assert!(graph.field(0,ty,"count").is_some());
            assert!(graph.field(0,ty,"_secret").is_none());
            assert_eq!(graph.methods(0,ty,"read").len(),1);
            assert!(graph.methods(0,ty,"_peek").is_empty());
        }
        assert!(graph.resolve_struct(0,"model._Inside").is_none());
        assert!(graph.resolve_struct(model,"_Inside").is_some());
        assert!(graph.field(model,"Box","_secret").is_some());
        assert_eq!(graph.methods(model,"Box","_peek").len(),1);
    }

    const MODEL:&str = "struct Box{\nvar _code:int = 7\nvar count:int = 3\nvar text:str = \"甲\"\nfn read()->int{\nreturn self._code\n}\nfn _hidden()->int{\nreturn self.count\n}\n}";

    #[test]
    fn returned_text_and_failed_argument_evaluation_balance_the_original_resources() {
        use ResourceEvent::*;
        let make = "fn pass(text:str)->str{ return text.move() }";
        let take = "fn take(a:str,n:int)->int{ return a[n][0] }";
        for (body, success, events) in [
            ("return take(pass(\"*\"),0)", true, vec![Created(1),Moved(1),Returned(1),Moved(1),Released(1)]),
            ("return take(pass(\"*\"),9223372036854775807+1)", false, vec![Created(1),Moved(1),Returned(1),Released(1)]),
        ] {
            let main = format!("{make}\n{take}\nfn main()->int{{ {body} }}");
            let files = Files::new(&[("main.voil",&main)]);
            let (actual, trace) = LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
            assert_eq!(trace, events);
            assert_eq!(actual.ok(), success.then_some(42));
        }
    }

    #[test]
    fn shared_text_parameters_reuse_one_resource() {
        use ResourceEvent::*;
        let files=Files::new(&[("main.voil","fn read(a:str,b:str)->int{ return a[0][0]+b[0][0] }
fn main()->int{ var s:str = \"*\"
const n:int = read(s,s)
return n - s[0][0] }")]);
        let (value,trace)=LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
        assert_eq!(value.ok(),Some(42));
        assert_eq!(trace,vec![Created(1),Released(1)]);
    }

    #[test]
    fn moved_text_reaches_imported_function_and_unwinds_once() {
        use ResourceEvent::*;
        for index in [0, 2] {
            let consumer = format!("fn read(s:str)->int{{ return s[{index}][0] }}");
            let files = Files::new(&[
                ("main.voil", "import consumer\nfn main()->int{\nvar s:str = \"*\"\nvar t:str = s.move()\nreturn consumer.read(t.move())\n}"),
                ("consumer.voil", &consumer),
            ]);
            let (result, events) = LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
            assert_eq!(events, [Created(1), Moved(1), Moved(1), Released(1)]);
            if index == 0 { assert_eq!(result.unwrap(), 42); }
            else { assert!(matches!(result, Err(ModuleError::RuntimeAt { path, .. }) if path == files.0.join("consumer.voil"))); }
        }
    }

    #[test]
    fn returned_struct_keeps_original_field_resource_across_modules() {
        use ResourceEvent::*;
        let files=Files::new(&[
            ("main.voil","import Box from factory as Item\nimport factory\nfn main()->int{\nvar received:Item = factory.make()\nreturn received.s[0][0]\n}"),
            ("factory.voil","struct Box{\nvar s:str = \"*\"\n}\nfn make()->Box{\nvar result:Box = Box()\nreturn result.move()\n}"),
        ]);
        let (value,events)=LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
        assert_eq!(value.unwrap(),42);
        assert_eq!(events,[Created(1),Returned(1),Released(1)]);
    }

    #[test]
    fn moved_struct_reaches_imported_method_and_cleans_original_resource() {
        let files = Files::new(&[
            ("main.voil","import Box from model as Item\nimport consumer\nfn main()->int{\nvar item:Item = Item(count=4)\nreturn consumer.take(item.move())\n}"),
            ("model.voil",MODEL),
            ("consumer.voil","import Box from model as Arg\nfn take(item:Arg)->int{\nreturn item.read()+item.count\n}")
        ]);
        let p=LinkedProgram::prepare(&files.entry()).unwrap();
        let (v,events)=p.execute().unwrap();
        assert_eq!(v.unwrap(),11);
        use ResourceEvent::*;
        assert_eq!(events,[Created(1),Moved(1),Released(1)]);
        fs::write(files.0.join("consumer.voil"),b"import Box from model as Arg\nfn take(item:Arg)->int{\nreturn item.read()+9223372036854775807\n}").unwrap();
        let (v,events)=LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
        assert!(matches!(v, Err(ModuleError::RuntimeAt { error: StrRuntimeError::EvaluationFailed, path, span })
            if path == files.0.join("consumer.voil") && span.start.line == 3));
        assert_eq!(events,[Created(1),Moved(1),Released(1)]);
    }

    #[test]
    fn qualified_constructor_overrides_public_fields_without_exposing_private_fields() {
        let files=Files::new(&[("main.voil","import model\nfn main()->int{\nvar item:model.Box = model.Box(count=9)\nreturn item.read()+item.count\n}"),
            ("model.voil",MODEL)]);
        let (v,events)=LinkedProgram::prepare(&files.entry()).unwrap().execute().unwrap();
        assert_eq!(v.unwrap(),16);
        assert!(crate::owned_str_runtime::event_trace_balanced(&events));
        fs::write(files.entry(),b"import model\nfn main()->int{\nvar item:model.Box = model.Box(_code=99)\nreturn item.read()\n}").unwrap();
        assert!(matches!(LinkedProgram::prepare(&files.entry()),Err(ModuleError::Source{path,..}) if path==files.entry()));
    }

    #[test]
    fn checked_calls_reject_private_members_wrong_nominal_type_and_use_after_move() {
        for body in ["return item._code", "return item._hidden()", "return take(item.move())+item.count",
                     "return take(item.move())+take(item.move())", "return take()","return take(item)"] {
            let main=format!("import Box from model as B\nimport take from usebox as take\nfn main()->int{{\nvar item:B = B()\n{body}\n}}");
            let files=Files::new(&[("main.voil",&main),("model.voil",MODEL),
                ("usebox.voil","import Box from model as B\nfn take(v:B)->int{\nreturn v.read()\n}")]);
            assert!(matches!(LinkedProgram::prepare(&files.entry()),Err(ModuleError::Source{path,..}) if path==files.entry()),"{body}");
        }
        let files=Files::new(&[("main.voil","import Box from first as A\nimport take from second as f\nfn main()->int{\nvar a:A = A()\nreturn f(a.move())\n}"),
            ("first.voil",MODEL),("second.voil",&format!("{MODEL}\nfn take(x:Box)->int{{\nreturn x.read()\n}}"))]);
        assert!(matches!(LinkedProgram::prepare(&files.entry()),Err(ModuleError::Source{..})));
    }

    #[test]
    fn missing_duplicate_and_lexical_diagnostics_preserve_importing_file() {
        for import in ["import absent from a as x","import f from a as x\nimport f from a as x"] {
            let files=Files::new(&[("main.voil",import),("a.voil","fn f()->int{\nreturn 3\n}")]);
            assert!(matches!(ModuleGraph::prepare(&files.entry()),Err(ModuleError::Source{path,diagnostic})
                if path==files.entry() && diagnostic.span.is_some()));
        }
        let files=Files::new(&[("main.voil","import a"),("a.voil","")]);
        fs::write(files.0.join("a.voil"),b"fn f()->int{\nreturn \xff\n}").unwrap();
        assert!(matches!(ModuleGraph::prepare(&files.entry()),Err(ModuleError::Source{path,diagnostic})
            if path==files.0.join("a.voil") && diagnostic.kind==DiagnosticKind::Lexical));
    }

    #[test]
    fn changed_dependency_rejected_before_any_initialization() {
        let files = Files::new(&[("main.voil","import a\nfn main()->int{\nreturn a.value()\n}"),
            ("a.voil","fn value()->int{\nreturn 4\n}")]);
        let p = LinkedProgram::prepare(&files.entry()).unwrap();
        fs::write(files.0.join("a.voil"),b"fn value()->int{\nreturn 5\n}").unwrap();
        assert!(matches!(p.execute(),Err(ModuleError::File(RegistryFailure::SourceChanged{path})) if path==files.0.join("a.voil")));
    }
}
