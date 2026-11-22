//! 將已檢查的模組計畫轉為獨立的安全 Rust；不嵌入來源解析器。
use super::*;
use std::fmt::Write;

struct Emitter<'a> {
    functions: BTreeMap<&'a str, usize>,
    locals: BTreeMap<String, usize>,
}

fn container_source(value: &containers::Value) -> String {
    use containers::{Key, Value};
    match value {
        Value::CopyFrom(slot) => format!("Value::CopyFrom(String::from({slot:?}))"),
        Value::Missing => "Value::Missing".into(),
        Value::Integer(n) => format!("Value::Integer({n})"),
        Value::Boolean(b) => format!("Value::Boolean({b})"),
        Value::Text(s) => format!("Value::Text(String::from({s:?}))"),
        Value::Decimal(c,e) => format!("Value::Decimal({c},{e})"),
        Value::List(xs) => format!("Value::List(vec![{}])",xs.iter().map(container_source).collect::<Vec<_>>().join(",")),
        Value::Dict(xs) => {
            let entries = xs.iter().map(|(k,v)| {
                let key = match k { Key::Index(n) => format!("Key::Index({n})"), Key::Text(s) => format!("Key::Text(String::from({s:?}))") };
                format!("({key},{})",container_source(v))
            }).collect::<Vec<_>>().join(",");
            format!("Value::Dict(std::collections::BTreeMap::from([{entries}]))")
        }
    }
}

impl Emitter<'_> {
    fn local_id(&mut self, name: &str) -> usize {
        let next = self.locals.len();
        *self.locals.entry(name.to_string()).or_insert(next)
    }

    fn offset(&self, index: &StringOffset) -> Option<String> {
        match index {
            StringOffset::Constant(i) => Some(i.to_string()),
            StringOffset::Named(name) => Some(format!("v{}", self.locals.get(name)?)),
        }
    }

    fn local_replace(&self, change: &LocalReplacement) -> Option<String> {
        let text = match &change.rhs {
            GraphemeReplacement::Literal(s) => format!("String::from({s:?})"),
            GraphemeReplacement::Named(n) => format!("v{}.text.clone()", self.locals.get(n)?),
        };
        Some(format!("{{ let rhs = {text}; v{}.replace({}, &rhs).ok_or((site, ARITH))?; ResourceText::new(rhs) }}",
            self.locals.get(&change.owner)?, self.offset(&change.index)?))
    }

    fn local_text(&self, value: &LocalText) -> Option<String> {
        Some(match value {
            LocalText::Literal(s) => format!("ResourceText::new(String::from({s:?}))"),
            LocalText::Replace(change) => self.local_replace(change)?,
            LocalText::Move(source) => format!("v{}", self.locals.get(source)?),
            LocalText::Grapheme(source, index) => format!("ResourceText::new(v{}.grapheme({}).ok_or((site, ARITH))?.to_owned())",
                self.locals.get(source)?, self.offset(index)?),
            LocalText::Call(call) => self.invocation(call)?,

        })
    }

    fn invocation(&self, call: &TextInvocation) -> Option<String> {
        let mut code = String::from("{ ");
        for (_,slot,value) in &call.arguments {
            let expr = match value { Argument::Integer(n) => self.number(n)?, Argument::Text(t) => self.local_text(t)? };
            write!(code,"let arg{slot} = {expr}; ").ok()?;
        }
        let args: String = (0..call.arguments.len()).map(|i| format!(", arg{i}")).collect();
        write!(code, "f{}(globals, steps, depth + 1, site{args})? }}",self.functions.get(call.target.as_str())?).ok()?;
        Some(code)
    }

    fn container_item(&self, item: &ContainerItem) -> Option<String> {
        Some(match item {
            ContainerItem::Scalar(n,true) => format!("Value::Boolean(({}) != 0)",self.number(n)?),
            ContainerItem::Scalar(n,false) => format!("Value::Integer({})",self.number(n)?),
            ContainerItem::Literal(v) => container_source(v),
        })
    }

    fn key(&self, key:&ContainerKey) -> Option<String> {
        Some(match key {
            ContainerKey::Text(s) => format!("Key::Text(String::from({s:?}))"),
            ContainerKey::Index(n) => format!("Key::Index({})",self.number(n)?),
        })
    }

    fn number(&self, value: &Number) -> Option<String> {
        Some(match value {
            Number::ViewByte(slot,i,j) => format!("{{ let Some(Value::Text(s)) = containers.get({slot:?}) else {{ return Err((site,ARITH)) }}; text_byte(s,{},{}).ok_or((site,ARITH))? }}",self.offset(i)?,self.offset(j)?),
            Number::Edit(slot,edit) => match edit.as_ref() {
                Edit::Append(item) => format!("{{ let item = {}; containers.append({slot:?},item).ok_or((site,ARITH))? }}",self.container_item(item)?),
                Edit::Set(key,item) => format!("{{ let key = {}; let item = {}; containers.set({slot:?},key,item).ok_or((site,ARITH))? }}",self.key(key)?,self.container_item(item)?),
                Edit::Pop => format!("containers.pop({slot:?}).ok_or((site,ARITH))?"),
            },
            Number::Container(slot,read) => {
                let method = match read { ContainerRead::Length => "length", ContainerRead::Tag => "tag", ContainerRead::Scalar => "scalar" };
                format!("containers.get({slot:?}).and_then(Value::{method}).ok_or((site, ARITH))?")
            }
            Number::ScalarCall { target, arguments } => {
                let mut expr = String::from("{ ");
                for (_, slot, value) in arguments {
                    write!(expr, "let arg{slot} = {}; ", self.number(value)?).ok()?;
                }
                let args: String = (0..arguments.len()).map(|i| format!(", arg{i}")).collect();
                write!(expr, "f{}(globals, steps, depth + 1, site{args})? }}", self.functions.get(target.as_str())?).ok()?;
                expr
            }
            Number::Logic(op, a, b) => {
                let a = self.number(a)?;
                let b = self.number(b)?;
                match *op {
                    "and" => format!("i64::from(({a}) != 0 && ({b}) != 0)"),
                    "or" => format!("i64::from(({a}) != 0 || ({b}) != 0)"),
                    "==" => format!("i64::from(({a}) == ({b}))"),
                    "lt" => format!("i64::from(({a}) < ({b}))"),
                    _ => format!("i64::from(({a}) != ({b}))"),
                }
            }
            Number::Constant(n) => format!("({n}i64)"),
            Number::Local(name) => format!("v{}", self.locals.get(name)?),
            Number::Binary(a, op, b) => {
                let method = if *op == '+' { "checked_add" } else { "checked_sub" };
                format!("({}).{method}({}).ok_or((site, ARITH))?", self.number(a)?, self.number(b)?)
            }
            Number::Choice(test, yes, no) =>
                format!("(if ({}) != 0 {{ {} }} else {{ {} }})", self.number(test)?, self.number(yes)?, self.number(no)?),
            Number::Member { owner, field, .. } => {
                let object = self.locals.get(owner)?;
                format!("v{object}.integer({field:?}).ok_or((site, ARITH))?")
            }
            Number::TextCall(call) => self.invocation(call)?,
            Number::Consume { target, from, .. } => {
                let object = self.locals.get(from)?;
                let function = self.functions.get(target.as_str())?;
                format!("f{function}(globals, steps, depth + 1, site, v{object})?")
            }
            Number::BoolMember(owner, field) => format!("v{}.boolean({field:?}).ok_or((site, ARITH))?", self.locals.get(owner)?),
            Number::MemberByte(owner, field, i, j) => format!(
                "v{}.byte({field:?}, {}, {}).ok_or((site, ARITH))?", self.locals.get(owner)?, self.offset(i)?, self.offset(j)?),
            Number::LocalByte(owner, i, j) => format!("v{}.byte({}, {}).ok_or((site, ARITH))?",
                self.locals.get(owner)?, self.offset(i)?, self.offset(j)?),
            Number::Byte(place, i, j) => format!(
                "globals[{}].byte({}, {}).ok_or((site, ARITH))?", place.slot, self.offset(i)?, self.offset(j)?),
            Number::Call(name, false) =>
                format!("f{}(globals, steps, depth + 1, site)?", self.functions.get(name.as_str())?),
            _ => return None,
        })
    }
}

impl LinkedProgram {
    pub(crate) fn native_source(&self) -> Result<String, ModuleError> {
        let plan = self.checked_plan()?;
        let mut emitter = Emitter {
            functions: plan.bodies.keys().enumerate().map(|(i, n)| (n.as_str(), i)).collect(),
            locals: BTreeMap::new(),
        };
        let mut out = String::from("#![forbid(unsafe_code)]\n#![allow(unused_variables, unused_parens, unused_assignments, unused_mut, dead_code)]\n");
        let budget = crate::execution::ExecutionBudget::default();
        out.push_str("type Failure = (&'static str, &'static str);\n");
        out.push_str("const ARITH: &str = \"運算失敗\";\nconst LIMIT: &str = \"執行步數已耗盡\";\n");
        out.push_str("mod containers {\n");
        out.push_str(include_str!("public_containers.rs"));
        out.push_str("}\nuse containers::{Key, Value};\n");
        out.push_str(include_str!("native_object.inc"));
        out.push_str("struct ResourceText { text: String, starts: std::cell::OnceCell<Vec<usize>> }\n");
        if !plan.globals.is_empty() || self.graph.units.iter().any(|u| !u.structs.is_empty()) || self.graph.units.iter().any(|u| u.functions.iter().any(|f| f.result_ty == "str" || f.params.iter().any(|p| p.ty == "str"))) || plan.bodies.values().flatten().any(|s| matches!(s.action, Action::Construct(..) | Action::Container(..) | Action::StoreText { .. })) {
            out.push_str(include_str!("../grapheme_data.rs"));
            out.push_str(include_str!("../grapheme_boundaries.rs"));
            out.push_str(include_str!("native_strings.inc"));
            out.push_str("fn text_byte(s:&str,i:i64,j:i64)->Option<i64>{let g=s.get(indexed_range(&boundaries(s),i)?)?.as_bytes();Some(i64::from(g[offset(j,g.len())?]))}\n");
            out.push_str("impl Object { fn byte(&self, field: &str, i:i64, j:i64) -> Option<i64> { let Field::Text(s) = self.0.get(field)? else {return None}; text_byte(s,i,j) } }\n");
        }
        for (name, body) in &plan.bodies {
            emitter.locals.clear();
            let id = emitter.functions[name.as_str()];
            let step = &body[0]; // Lower::body 要求每個函式都有明確 return。
            let unit = &self.graph.units[step.module];
            let local_name = name.split_once(':').map_or(name.as_str(), |(_, n)| n);
            let signature = unit.functions.iter().find(|f| f.name == local_name).unwrap();
            let argument: String = signature.params.iter().map(|p| {
                let ty = match p.ty.as_str() { "int" | "bool" => "i64", "str" => "ResourceText", _ => "Object" };
                format!(", v{}: {ty}", emitter.local_id(&p.name))
            }).collect();
            let ret = if signature.result_ty == "str" { "ResourceText" } else { "i64" };
            writeln!(out, "fn f{id}(globals: &mut [ResourceText], steps: &mut usize, depth: usize, caller: &'static str{argument}) -> Result<{ret}, Failure> {{").unwrap();
            writeln!(out, "if depth >= {} {{ return Err((caller, LIMIT)); }}", budget.max_call_frames).unwrap();
            if body.iter().any(|step| matches!(step.action, Action::Container(..))) {
                out.push_str("let mut containers = containers::Store::default();\n");
            }
            let flow = body.iter().any(|s| matches!(s.action, Action::Jump(_) | Action::Unless(..)));
            if flow {
                for step in body {
                    if let Action::Bind(n, _) = &step.action {
                        if !emitter.locals.contains_key(n) {
                            writeln!(out, "let mut v{}: i64 = 0;", emitter.local_id(n)).unwrap();
                        }
                    }
                }
                out.push_str("let mut pc = 0usize; loop { match pc {\n");
            }
            for (index, step) in body.iter().enumerate() {
                if flow { writeln!(out, "{index} => {{ pc = {};", index+1).unwrap(); }
                let unit = &self.graph.units[step.module];
                let span = unit.source.span(step.span.0, step.span.1);
                let site = format!("{}:{}:{}", unit.path.display(), span.start.line, span.start.column);
                writeln!(out, "let site = {site:?}; *steps += 1; if *steps > {} {{ return Err((site, LIMIT)); }}", budget.max_steps).unwrap();
                let unsupported = || unit.error(step.span.0, step.span.1, "原生後端尚未支援此操作");
                match &step.action {
                    Action::Container(slot,init) => match init {
                        ContainerInit::Literal(v) => writeln!(out,"containers.put({slot:?},{}).ok_or((site, ARITH))?;",container_source(&v.runtime().ok_or_else(unsupported)?)).unwrap(),
                        ContainerInit::Move(from) => writeln!(out,"containers.relocate({from:?},{slot:?}).ok_or((site, ARITH))?;").unwrap(),
                    },
                    Action::Select { alias, owner, key } => {
                        let value = emitter.key(key).ok_or_else(unsupported)?;
                        writeln!(out,"let k = {value}; containers.select({alias:?},{owner:?},k).ok_or((site, ARITH))?;").unwrap();
                    }
                    Action::DropContainer(slot) => writeln!(out,"containers.drop_slot({slot:?});").unwrap(),
                    Action::Evaluate(n) => writeln!(out,"let _ = {};",emitter.number(n).ok_or_else(unsupported)?).unwrap(),
                    Action::Jump(target) => writeln!(out, "pc = {target};").unwrap(),
                    Action::Unless(test, target) => {
                        writeln!(out, "if ({}) == 0 {{ pc = {target}; }}", emitter.number(test).ok_or_else(unsupported)?).unwrap();
                    }
                    Action::Bind(name, value) => {
                        let expr = emitter.number(value).ok_or_else(unsupported)?;
                        let local = emitter.local_id(name);
                        writeln!(out, "{}v{local} = {expr};", if flow { "" } else { "let " }).unwrap();
                    }
                    Action::ReturnText(value) => {
                        writeln!(out, "return Ok({});", emitter.local_text(value).ok_or_else(unsupported)?).unwrap();
                    }
                    Action::Return(value) => {
                        let expr = emitter.number(value).ok_or_else(unsupported)?;
                        writeln!(out, "return Ok({expr});").unwrap();
                    }
                    Action::ReplaceLocal(change) => {
                        writeln!(out, "{};", emitter.local_replace(change).ok_or_else(unsupported)?).unwrap();
                    }
                    Action::StoreText { target, declaration, value } => {
                        let rhs = emitter.local_text(value).ok_or_else(unsupported)?;
                        writeln!(out, "{}v{} = {rhs};", if declaration.is_some() { "let mut " } else { "" },
                            emitter.local_id(target)).unwrap();
                    }
                    Action::RelocateObject { source, target, moving } => {
                        let from = *emitter.locals.get(source).ok_or_else(unsupported)?;
                        let rhs = if *moving { format!("v{from}") } else { format!("v{from}.copy_scalars().ok_or((site, ARITH))?") };
                        writeln!(out, "let mut v{} = {rhs};", emitter.local_id(target)).unwrap();
                    }
                    Action::WriteMember { owner, field, value } => {
                        let rhs = match value {
                            MemberValue::Integer(n) => format!("Field::Int({})", emitter.number(n).ok_or_else(unsupported)?),
                            MemberValue::Boolean(n) => format!("Field::Bool(({}) != 0)", emitter.number(n).ok_or_else(unsupported)?),
                            MemberValue::Text(s) => format!("Field::Text(String::from({s:?}))"),
                        };
                        writeln!(out, "let rhs = {rhs}; v{}.0.insert({field:?},rhs);", emitter.locals.get(owner).ok_or_else(unsupported)?).unwrap();
                    }
                    Action::Construct(name, fields, supplied) => {
                        let id = emitter.local_id(name);
                        writeln!(out, "let mut v{id} = Object::default();").unwrap();
                        for field in fields {
                            let seed = supplied.iter().find(|f| f.name == field.name)
                                .map(|f| &f.seed).or(field.default.as_ref()).ok_or_else(unsupported)?;
                            let value = match seed {
                                ApprovedSeed::Int(n) => format!("Field::Int({n})"),
                                ApprovedSeed::Str(s) => format!("Field::Text(String::from({s:?}))"),
                                ApprovedSeed::Bool(b) => format!("Field::Bool({b})"),
                                _ => return Err(unsupported()),
                            };
                            writeln!(out, "v{id}.0.insert({:?}, {value});", field.name).unwrap();
                        }
                    }
                    // 借用的有效期與權限已由同一模組計畫驗證；存取仍使用原 slot。
                    Action::Loan(..) => (),
                    Action::Write(place, text) => {
                        let rhs = match text {
                            Text::Literal(s) => format!("String::from({s:?})"),
                            Text::Grapheme(p, i) => format!(
                                "globals[{}].grapheme({}).ok_or((site, ARITH))?.to_owned()", p.slot, emitter.offset(i).ok_or_else(unsupported)?),
                        };
                        writeln!(out, "globals[{}] = ResourceText::new({rhs});", place.slot).unwrap();
                    }
                    Action::Replace(place, i, replacement) => {
                        let text = match replacement {
                            Replacement::Literal(s) => format!("{s:?}"),
                            Replacement::Named(source) => {
                                writeln!(out, "let rhs = globals[{}].text.clone();", source.slot).unwrap();
                                "&rhs".into()
                            }
                        };
                        writeln!(out, "globals[{}].replace({}, {text}).ok_or((site, ARITH))?;", place.slot, emitter.offset(i).ok_or_else(unsupported)?).unwrap();
                    }
                }
                if flow { out.push_str("}\n"); }
            }
            if flow { out.push_str("_ => unreachable!(), } }\n"); }
            out.push_str("}\n");
        }
        let main = emitter.functions["main"];
        let globals = plan.globals.iter().map(|g| format!("ResourceText::new(String::from({:?}))", g.initial))
            .collect::<Vec<_>>().join(", ");
        writeln!(out, "fn main() -> std::process::ExitCode {{ match f{main}(&mut [{globals}], &mut 0, 0, \"\") {{").unwrap();
        out.push_str("Ok(n) => std::process::ExitCode::from(n as u8),\nErr((s, e)) => { eprintln!(\"{s}: 執行期錯誤：{e}\"); std::process::ExitCode::FAILURE } } }\n");
        Ok(out)
    }
}
