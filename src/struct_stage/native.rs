//! 將已檢查的模組計畫轉為獨立的安全 Rust；不嵌入來源解析器。
use super::*;
use std::fmt::Write;

struct Emitter<'a> {
    functions: BTreeMap<&'a str, usize>,
    locals: BTreeMap<String, usize>,
}

impl Emitter<'_> {
    fn local_id(&mut self, name: &str) -> usize {
        let next = self.locals.len();
        *self.locals.entry(name.to_string()).or_insert(next)
    }

    fn number(&self, value: &Number) -> Option<String> {
        Some(match value {
            Number::Constant(n) => format!("({n}i64)"),
            Number::Local(name) => format!("v{}", self.locals.get(name)?),
            Number::Binary(a, op, b) => {
                let method = if *op == '+' { "checked_add" } else { "checked_sub" };
                format!("({}).{method}({}).ok_or((site, ARITH))?", self.number(a)?, self.number(b)?)
            }
            Number::Choice(test, yes, no) =>
                format!("(if {test} {{ {} }} else {{ {} }})", self.number(yes)?, self.number(no)?),
            Number::Member { owner, field, .. } => {
                let object = self.locals.get(owner)?;
                format!("v{object}.integer({field:?}).ok_or((site, ARITH))?")
            }
            Number::Consume { target, from, .. } => {
                let object = self.locals.get(from)?;
                let function = self.functions.get(target.as_str())?;
                format!("f{function}(globals, steps, depth + 1, site, v{object})?")
            }
            Number::Byte(place, i, j) => format!(
                "globals[{}].byte({i}, {j}).ok_or((site, ARITH))?", place.slot),
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
        let mut out = String::from("#![forbid(unsafe_code)]\n#![allow(unused_variables, unused_parens, dead_code)]\n");
        let budget = crate::execution::ExecutionBudget::default();
        out.push_str("type Failure = (&'static str, &'static str);\n");
        out.push_str("const ARITH: &str = \"運算失敗\";\nconst LIMIT: &str = \"執行步數已耗盡\";\n");
        out.push_str(include_str!("native_object.inc"));
        out.push_str("struct ResourceText { text: String, starts: std::cell::OnceCell<Vec<usize>> }\n");
        if !plan.globals.is_empty() {
            out.push_str(include_str!("../grapheme_data.rs"));
            out.push_str(include_str!("../grapheme_boundaries.rs"));
            out.push_str(include_str!("native_strings.inc"));
        }
        for (name, body) in &plan.bodies {
            emitter.locals.clear();
            let id = emitter.functions[name.as_str()];
            let step = &body[0]; // Lower::body 要求每個函式都有明確 return。
            let unit = &self.graph.units[step.module];
            let local_name = name.split_once(':').map_or(name.as_str(), |(_, n)| n);
            let signature = unit.functions.iter().find(|f| f.name == local_name).unwrap();
            let argument = signature.params.first().map(|p|
                format!(", v{}: Object", emitter.local_id(&p.name))).unwrap_or_default();
            writeln!(out, "fn f{id}(globals: &mut [ResourceText], steps: &mut usize, depth: usize, caller: &'static str{argument}) -> Result<i64, Failure> {{").unwrap();
            writeln!(out, "if depth >= {} {{ return Err((caller, LIMIT)); }}", budget.max_call_frames).unwrap();
            for step in body {
                let unit = &self.graph.units[step.module];
                let span = unit.source.span(step.span.0, step.span.1);
                let site = format!("{}:{}:{}", unit.path.display(), span.start.line, span.start.column);
                writeln!(out, "let site = {site:?}; *steps += 1; if *steps > {} {{ return Err((site, LIMIT)); }}", budget.max_steps).unwrap();
                let unsupported = || unit.error(step.span.0, step.span.1, "原生後端尚未支援此操作");
                match &step.action {
                    Action::Bind(name, value) => {
                        let expr = emitter.number(value).ok_or_else(unsupported)?;
                        let local = emitter.local_id(name);
                        writeln!(out, "let v{local} = {expr};").unwrap();
                    }
                    Action::Return(value) => {
                        let expr = emitter.number(value).ok_or_else(unsupported)?;
                        writeln!(out, "return Ok({expr});").unwrap();
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
                                "globals[{}].grapheme({i}).ok_or((site, ARITH))?.to_owned()", p.slot),
                        };
                        writeln!(out, "globals[{}] = ResourceText::new({rhs});", place.slot).unwrap();
                    }
                    Action::Replace(place, i, text) => {
                        writeln!(out, "globals[{}].replace({i}, {text:?}).ok_or((site, ARITH))?;", place.slot).unwrap();
                    }
                }
            }
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
