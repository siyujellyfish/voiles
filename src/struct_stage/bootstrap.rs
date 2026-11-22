//! 自舉用純整數核心 ABI。重用已連結的計畫，將結構的唯讀 int 欄位展開為值。
use super::*;
use std::fmt::Write;

fn scalar(value: &Number, fields: &[&str], owner: &str, locals: &BTreeMap<String, usize>) -> Option<String> {
    match value {
        Number::Constant(v) => Some(format!("({v}i64)")),
        Number::Local(name) => Some(format!("v{}", locals.get(name)?)),
        Number::Member { owner: n, field, receiver: None } if n == owner =>
            Some(format!("a{}", fields.iter().position(|f| *f == field)?)),
        Number::Binary(left, op, right) => {
            let method = match op { '+' => "checked_add", '-' => "checked_sub", _ => return None };
            Some(format!("({}).{method}({})?", scalar(left, fields, owner, locals)?, scalar(right, fields, owner, locals)?))
        }
        _ => None,
    }
}

impl LinkedProgram {
    pub(crate) fn bootstrap_digits(&self) -> Result<String, ModuleError> {
        let plan = self.checked_plan()?;
        let unit = &self.graph.units[0];
        let fields = ["value", "digit"];
        let mut output = String::new();
        for name in ["binary", "octal", "decimal", "hexadecimal"] {
            let function = unit.functions.iter().find(|f| f.name == name)
                .ok_or_else(|| unit.error(0, 0, "自舉數字核心缺少函式"))?;
            let fail = || unit.error(function.start, function.end, "自舉數字 ABI 需要唯讀整數欄位與純運算");
            let [param] = function.params.as_slice() else { return Err(fail()); };
            let (_, def) = self.graph.resolve_struct(0, &param.ty).ok_or_else(fail)?;
            if function.is_async || def.fields.len() != fields.len()
                || !fields.iter().all(|n| def.fields.iter().any(|f| f.name == *n && f.ty == "int")) {
                return Err(fail());
            }
            writeln!(output, "pub(super) fn {name}(a0:i64, a1:i64) -> Option<i64> {{").unwrap();
            let mut locals = BTreeMap::new();
            for step in &plan.bodies[&format!("0:{name}")] {
                match &step.action {
                    Action::Bind(n, value) => {
                        let expr = scalar(value, &fields, &param.name, &locals).ok_or_else(fail)?;
                        let next = locals.len();
                        let id = locals.entry(n.clone()).or_insert(next);
                        writeln!(output, "let v{id} = {expr};").unwrap();
                    }
                    Action::Return(value) => {
                        let expr = scalar(value, &fields, &param.name, &locals).ok_or_else(fail)?;
                        writeln!(output, "return Some({expr});").unwrap();
                    }
                    _ => return Err(fail()),
                }
            }
            output.push_str("}\n");
        }
        Ok(output)
    }
}
