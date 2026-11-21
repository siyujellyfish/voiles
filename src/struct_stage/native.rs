//! 將已檢查的模組計畫轉為獨立的安全 Rust；不嵌入來源解析器。
use super::*;
use std::fmt::Write;

struct Emitter<'a> {
    functions: BTreeMap<&'a str, usize>,
    locals: BTreeMap<String, usize>,
}

impl Emitter<'_> {
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
            Number::Call(name, false) =>
                format!("f{}(steps, depth + 1, site)?", self.functions.get(name.as_str())?),
            _ => return None,
        })
    }
}

impl LinkedProgram {
    pub(crate) fn native_source(&self) -> Result<String, ModuleError> {
        let plan = self.checked_plan()?;
        let root = &self.graph.units[0];
        if !plan.globals.is_empty() {
            return Err(root.error(0, 0, "原生後端尚未支援共享資源"));
        }
        let mut emitter = Emitter {
            functions: plan.bodies.keys().enumerate().map(|(i, n)| (n.as_str(), i)).collect(),
            locals: BTreeMap::new(),
        };
        let mut out = String::from("#![forbid(unsafe_code)]\n#![allow(unused_variables, unused_parens)]\n");
        let budget = crate::execution::ExecutionBudget::default();
        out.push_str("type Failure = (&'static str, &'static str);\n");
        out.push_str("const ARITH: &str = \"運算失敗\";\nconst LIMIT: &str = \"執行步數已耗盡\";\n");
        for (name, body) in &plan.bodies {
            emitter.locals.clear();
            let id = emitter.functions[name.as_str()];
            writeln!(out, "fn f{id}(steps: &mut usize, depth: usize, caller: &'static str) -> Result<i64, Failure> {{").unwrap();
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
                        let next = emitter.locals.len();
                        let local = emitter.locals.entry(name.clone()).or_insert(next);
                        writeln!(out, "let v{local} = {expr};").unwrap();
                    }
                    Action::Return(value) => {
                        let expr = emitter.number(value).ok_or_else(unsupported)?;
                        writeln!(out, "return Ok({expr});").unwrap();
                    }
                    _ => return Err(unsupported()),
                }
            }
            out.push_str("}\n");
        }
        let main = emitter.functions["main"];
        writeln!(out, "fn main() -> std::process::ExitCode {{ match f{main}(&mut 0, 0, \"\") {{").unwrap();
        out.push_str("Ok(n) => std::process::ExitCode::from(n as u8),\nErr((s, e)) => { eprintln!(\"{s}: 執行期錯誤：{e}\"); std::process::ExitCode::FAILURE } } }\n");
        Ok(out)
    }
}
