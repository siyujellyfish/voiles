fn v_main(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("import")?.checked_add(s.expect("host")?).ok_or(Fault::Overflow)?.checked_add(v_declarations(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_declarations(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at_end()? != 0i64 {
Ok(0i64)
} else {
Ok(v_declaration(s, depth + 1)?.checked_add(v_declarations(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_declaration(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("fn")?.checked_add(s.emit("fn ")?).ok_or(Fault::Overflow)?.checked_add(s.name()?).ok_or(Fault::Overflow)?.checked_add(v_signature(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?)
}
fn v_signature(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("(")?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.expect("->")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(s: &mut Machine, depth: usize) -> R {\ns.enter(depth)?;\n")?).ok_or(Fault::Overflow)?)
}
fn v_body(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("{")?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?)
}
fn v_statement(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("return")? != 0i64 {
Ok(s.expect("return")?.checked_add(s.emit("Ok(")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(")\n")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_condition(s, depth + 1)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("else")?).ok_or(Fault::Overflow)?.checked_add(s.emit("} else {\n")?).ok_or(Fault::Overflow)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?)
}
}
fn v_condition(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("if")?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.emit("if ")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("!")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.emit(" != ")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(" {\n")?).ok_or(Fault::Overflow)?)
}
fn v_expression(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(v_atom(s, depth + 1)?.checked_add(v_additions(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_additions(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("+")? != 0i64 {
Ok(s.expect("+")?.checked_add(s.emit(".checked_add(")?).ok_or(Fault::Overflow)?.checked_add(v_atom(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(").ok_or(Fault::Overflow)?")?).ok_or(Fault::Overflow)?.checked_add(v_additions(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_atom(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at_integer()? != 0i64 {
Ok(s.literal()?)
} else {
Ok(v_call(s, depth + 1)?)
}
}
fn v_call(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("host")? != 0i64 {
Ok(s.expect("host")?.checked_add(s.expect(".")?).ok_or(Fault::Overflow)?.checked_add(s.method()?).ok_or(Fault::Overflow)?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(")?).ok_or(Fault::Overflow)?.checked_add(v_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(")?")?).ok_or(Fault::Overflow)?)
} else {
Ok(s.name()?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(s, depth + 1)?")?).ok_or(Fault::Overflow)?)
}
}
fn v_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.literal()?)
}
}
