//! P20 的模組字串。從私有 Parser 的正式節點建立有型別的直線執行計畫。
use super::*;
#[path = "modules.rs"]
pub(crate) mod modules;
use crate::borrow_safety::BorrowTicket;
use crate::owned_str_runtime::{OwnedStrRuntime, ResourceEvent, StrRuntimeError};
use crate::syntax_shared::{integer_i64, ScalarLiteral};

pub(super) struct Declaration {
    pub(super) name: String,
    initial: String,
    id: DeclarationId,
}

pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Declaration, Diagnostic> {
    parser.cursor += 1;
    let id = DeclarationId(parser.tok().start);
    let name = parser.name()?;
    parser.take(':', "share 必須明確標註型別")?;
    let ty = parser.tok().clone();
    if parser.name()? != "str" {
        return Err(problem(
            parser.source,
            (ty.start, ty.end),
            "P20 僅開放模組級 share str",
        ));
    }
    parser.take('=', "share 必須立即初始化")?;
    let TokenKind::StringLiteral(initial) = parser.tok().kind.clone() else {
        return Err(parser.syntax("模組初始化需要已驗證的字串常值"));
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

enum Number {
    Constant(i64),
    Local(String),
    Byte(Place, i64, i64),
    Call(String, bool),
    Binary(Box<Number>, char, Box<Number>),
}

enum Text {
    Literal(String),
    Grapheme(Place, i64),
}

enum Action {
    Bind(String, Number),
    Loan(String, Place, BorrowKind),
    Write(Place, Text),
    Replace(Place, i64, String),
    Return(Number),
}

struct Step {
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

struct Lower<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    globals: &'a BTreeMap<String, usize>,
    functions: &'a BTreeMap<String, bool>,
    linked_names: Option<&'a BTreeMap<String,String>>,
    readonly_slots: BTreeSet<usize>,
    locals: HashMap<String, bool>,
    aliases: HashMap<String, Place>,
}

impl Lower<'_> {
    fn place(&self, name: &str) -> Option<Place> {
        if self.locals.contains_key(name) || name.split_once('.').is_some_and(|(prefix,_)| self.locals.contains_key(prefix) || self.aliases.contains_key(prefix)) {
            return None;
        }
        self.aliases.get(name).cloned().or_else(|| {
            self.globals
                .get(name)
                .map(|&slot| Place { slot, alias: None })
        })
    }

    fn call_target(&self, name: &str, awaited: bool) -> Option<Number> {
        let receiver = name.split('.').next()?;
        if self.locals.contains_key(receiver) || self.aliases.contains_key(receiver)
            || self.functions.get(name) != Some(&awaited) { return None; }
        let target = match self.linked_names { Some(names) => names.get(name)?.clone(), None => name.to_owned() };
        Some(Number::Call(target,awaited))
    }

    fn number(&self, term: &Term) -> Option<Number> {
        match &term.kind {
            TermKind::Group(inner) => self.number(inner),
            TermKind::Scalar(ScalarLiteral::Integer(value)) => {
                Some(Number::Constant(integer_i64(value).ok()?))
            }
            TermKind::StringIndex(
                n,
                StringOffset::Constant(i),
                Some(StringOffset::Constant(j)),
            ) => Some(Number::Byte(self.place(n)?, *i, *j)),
            TermKind::Name(n) if self.locals.contains_key(n) => Some(Number::Local(n.clone())),
            TermKind::UnresolvedZeroArgs(n) => self.call_target(n,false),
            TermKind::Call(call) if call.arguments.is_empty() => self.call_target(&call.name,call.awaited),
            TermKind::MethodCall(module,member,args) if args.is_empty() && self.linked_names.is_some() =>
                self.call_target(&format!("{module}.{member}"),false),
            TermKind::BinaryInt(node) => Some(Number::Binary(
                Box::new(self.number(&node.left)?),
                node.operator,
                Box::new(self.number(&node.right)?),
            )),
            _ => None,
        }
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
            TermKind::StringIndex(n, StringOffset::Constant(i), None) => {
                Some(Text::Grapheme(self.place(n)?, *i))
            }
            _ => None,
        }
    }

    fn body(&mut self, function: &Function) -> Result<Body, Diagnostic> {
        let mut steps = Vec::new();
        for stmt in &function.body.statements {
            let span = (stmt.start, stmt.end);
            let err = |msg: &str| problem(self.source, span, msg);
            if matches!(
                steps.last(),
                Some(Step {
                    action: Action::Return(_),
                    ..
                })
            ) {
                return Err(err("return 後不可再有語句"));
            }
            let action = match &stmt.kind {
                StmtKind::Declare(d) => {
                    if self.locals.contains_key(&d.name) || self.aliases.contains_key(&d.name) {
                        return Err(err("重複區域宣告"));
                    }
                    let init = d
                        .initializer
                        .as_ref()
                        .ok_or_else(|| err("此執行路徑需要初值"))?;
                    if d.ty == "str" {
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
                    } else if d.ty == "int" {
                        let value = self
                            .number(init)
                            .ok_or_else(|| err("需要 int 初值或已驗證的同步函式呼叫"))?;
                        self.locals.insert(d.name.clone(), d.mutable);
                        Action::Bind(d.name.clone(), value)
                    } else {
                        return Err(err("此模組執行路徑尚未支援該區域型別"));
                    }
                }
                StmtKind::Assign(a) if self.locals.contains_key(&a.name) => {
                    if !self.locals[&a.name] {
                        return Err(err("const 不可重賦值"));
                    }
                    Action::Bind(
                        a.name.clone(),
                        self.number(&a.expression).ok_or_else(|| err("需要 int"))?,
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
                StmtKind::GraphemeWrite(t) => {
                    let TermKind::StringReplace(
                        name,
                        StringOffset::Constant(i),
                        GraphemeReplacement::Literal(text),
                    ) = &t.kind
                    else {
                        return Err(err("字素覆寫需要已驗證的索引與替換值"));
                    };
                    if !crate::owned_str_runtime::single_grapheme(text) {
                        return Err(err("替換值必須為單一字素"));
                    }
                    let p = self.place(name).ok_or_else(|| err("字串未宣告"))?;
                    if self.readonly_slots.contains(&p.slot) || matches!(p.alias, Some((_, BorrowKind::SharedRead))) {
                        return Err(err("唯讀借用不可寫入"));
                    }
                    Action::Replace(p, *i, text.clone())
                }
                StmtKind::Return(r) => Action::Return(
                    self.number(&r.expression)
                        .ok_or_else(|| err("需要已驗證的 int 返回值"))?,
                ),
                _ => return Err(err("此模組語句缺少共享存取證明")),
            };
            steps.push(Step { action, span });
        }
        if !matches!(
            steps.last(),
            Some(Step {
                action: Action::Return(_),
                ..
            })
        ) {
            return Err(problem(
                self.source,
                (function.start, function.end),
                "函式必須明確返回 int",
            ));
        }
        Ok(steps)
    }
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
            let mut lower = Lower {
                source: &source,
                tokens: &lexed.tokens,
                linked_names: None, readonly_slots: BTreeSet::new(),
                globals: &slots,
                functions: &names,
                locals: HashMap::new(),
                aliases: HashMap::new(),
            };
            bodies.insert(f.name.clone(), lower.body(&f)?);
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

impl Number {
    fn uses<'a>(&'a self, list: &mut Vec<Use<'a>>) {
        match self {
            Self::Byte(p, ..) => list.push(Use::Direct(p)),
            Self::Binary(left, _, right) => {
                left.uses(list);
                right.uses(list);
            }
            Self::Call(name, waited) => list.push(Use::Call(name, *waited)),
            _ => (),
        }
    }
}

impl Action {
    fn uses(&self) -> Vec<Use<'_>> {
        let mut uses = Vec::new();
        match self {
            Self::Bind(_, value) | Self::Return(value) => value.uses(&mut uses),
            Self::Loan(alias, p, kind) => uses.push(Use::Loan(alias, p, *kind)),
            Self::Replace(p, ..) => uses.push(Use::Direct(p)),
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
                .flat_map(|s| s.action.uses().into_iter().map(move |u| (u, s.span)))
                .collect();
            let mut last = HashMap::new();
            for (i, (site, _)) in uses.iter().enumerate() {
                if let Use::Direct(Place {
                    alias: Some((name, _)),
                    ..
                }) = site
                {
                    last.insert(name.as_str(), i);
                }
            }
            let mut active = HashMap::<&str, SafetyState>::new();
            for (i, (site, span)) in uses.into_iter().enumerate() {
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
                        if (waited || effects[n].2) && !snapshot.facts().is_empty() {
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
        let mut state = self.instantiate(ModuleId(0))?;
        let result = self.call("main", &mut state, 0);
        state
            .loans
            .verify_static_snapshot(std::iter::empty())
            .map_err(|_| StrRuntimeError::OutstandingResource)?;
        Ok((result, state.heap.finish()?))
    }

    fn instantiate(&self, owner: ModuleId) -> Result<State<'_>, StrRuntimeError> {
        let mut state = State {
            owner,
            heap: OwnedStrRuntime::new(),
            steps: 0,
            loans: BorrowSafety::new(),
            declarations: &self.globals,
            _local: Default::default(),
        };
        for g in &self.globals {
            if let Err(error) = state.heap.declare(&g.name, true, &g.initial) {
                state.heap.finish()?;
                return Err(error);
            }
        }
        Ok(state)
    }

    fn call(&self, name: &str, state: &mut State, depth: usize) -> Result<i64, StrRuntimeError> {
        let budget = crate::execution::ExecutionBudget::default();
        if depth >= budget.max_call_frames {
            return Err(StrRuntimeError::ExecutionBudgetExhausted);
        }
        let body = &self.bodies[name];
        let mut frame = Frame::default();
        for site in body.iter().flat_map(|s| s.action.uses()) {
            if let Use::Direct(Place {
                alias: Some((name, _)),
                ..
            }) = site
            {
                *frame.remaining.entry(name.clone()).or_default() += 1;
            }
        }
        let result = self.run_body(body, &mut frame, state, depth, budget.max_steps);
        for ticket in frame.tickets.into_values() {
            state
                .loans
                .release(ticket)
                .map_err(|_| StrRuntimeError::OutstandingResource)?;
        }
        result
    }

    fn run_body(
        &self,
        body: &Body,
        frame: &mut Frame,
        state: &mut State,
        depth: usize,
        max_steps: usize,
    ) -> Result<i64, StrRuntimeError> {
        for step in body {
            state.steps += 1;
            if state.steps > max_steps {
                return Err(StrRuntimeError::ExecutionBudgetExhausted);
            }
            match &step.action {
                Action::Bind(n, rhs) => {
                    let value = self.eval(rhs, frame, state, depth)?;
                    frame.locals.insert(n.clone(), value);
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
                Action::Return(rhs) => return self.eval(rhs, frame, state, depth),
                Action::Replace(p, i, rhs) => {
                    frame.authorize(
                        &mut state.loans,
                        p,
                        self.globals[p.slot].id,
                        BorrowKind::ExclusiveWrite,
                    )?;
                    let result = state
                        .heap
                        .replace_grapheme(&self.globals[p.slot].name, *i, rhs)
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
                                .read_grapheme(&self.globals[from.slot].name, *i)
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
        }
        Err(StrRuntimeError::IncompleteReturn)
    }

    fn eval(
        &self,
        n: &Number,
        frame: &mut Frame,
        state: &mut State,
        depth: usize,
    ) -> Result<i64, StrRuntimeError> {
        match n {
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
            Number::Byte(p, i, j) => {
                frame.authorize(
                    &mut state.loans,
                    p,
                    self.globals[p.slot].id,
                    BorrowKind::SharedRead,
                )?;
                state.byte(state.owner, true, &self.globals[p.slot].name, *i, *j)
            }
            Number::Binary(left, op, right) => {
                let a = self.eval(left, frame, state, depth)?;
                let b = self.eval(right, frame, state, depth)?;
                (if *op == '+' {
                    a.checked_add(b)
                } else {
                    a.checked_sub(b)
                })
                .ok_or(StrRuntimeError::EvaluationFailed)
            }
        }
    }
}

#[derive(Default)]
struct Frame {
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
    declarations: &'a [Declaration],
    _local: std::marker::PhantomData<Rc<()>>,
    loans: BorrowSafety,
    owner: ModuleId,
    heap: OwnedStrRuntime,
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
