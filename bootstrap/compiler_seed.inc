fn v_main(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(v_imports(s, depth + 1)?.checked_add(v_declarations(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.finish_names()?).ok_or(Fault::Overflow)?)
}
fn v_imports(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("import")? != 0i64 {
Ok(s.expect("import")?.checked_add(s.module_import()?).ok_or(Fault::Overflow)?.checked_add(v_imports(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
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
if s.at("struct")? != 0i64 {
Ok(v_structure(s, depth + 1)?)
} else {
Ok(s.expect("fn")?.checked_add(s.emit("fn ")?).ok_or(Fault::Overflow)?.checked_add(s.function()?).ok_or(Fault::Overflow)?.checked_add(v_signature(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.finish_function()?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?)
}
}
fn v_structure(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("struct")?.checked_add(s.emit("#[derive(Clone,Copy)]\nstruct ")?).ok_or(Fault::Overflow)?.checked_add(s.structure()?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(s.emit("{\n")?).ok_or(Fault::Overflow)?.checked_add(v_fields(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_structure()?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?)
}
fn v_fields(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(s.expect("var")?.checked_add(s.field()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.field_default()?).ok_or(Fault::Overflow)?.checked_add(s.emit(",\n")?).ok_or(Fault::Overflow)?.checked_add(v_fields(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_signature(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("(")?.checked_add(s.emit("(s: &mut Machine, depth: usize")?).ok_or(Fault::Overflow)?.checked_add(v_parameters(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.expect("->")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.emit(") -> R {\ns.enter(depth)?;\n")?).ok_or(Fault::Overflow)?)
}
fn v_parameters(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.emit(", ")?.checked_add(s.parameter()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.emit(": i64")?).ok_or(Fault::Overflow)?.checked_add(v_following_parameters(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_parameters(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.emit(", ")?).ok_or(Fault::Overflow)?.checked_add(s.parameter()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.emit(": i64")?).ok_or(Fault::Overflow)?.checked_add(v_following_parameters(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_body(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("{")?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?)
}
fn v_statement(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at_str_local()? != 0i64 {
Ok(s.str_local()?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
if s.at("var")? != 0i64 {
Ok(v_local(s, depth + 1)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
if s.at_str_assignment()? != 0i64 {
Ok(s.str_assignment()?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
if s.at_field_write()? != 0i64 {
Ok(s.field_target()?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
if s.at("with")? != 0i64 {
if s.at_dict_borrow()? != 0i64 {
Ok(v_dict_borrow(s, depth + 1)?)
} else {
Ok(v_list_borrow(s, depth + 1)?)
}
} else {
if s.at("try")? != 0i64 {
Ok(v_try_statement(s, depth + 1)?)
} else {
if s.at("match")? != 0i64 {
Ok(v_match_statement(s, depth + 1)?)
} else {
if s.at("return")? != 0i64 {
Ok(s.expect("return")?.checked_add(s.return_start()?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(")\n")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_control(s, depth + 1)?)
}
}
}
}
}
}
}
}
}
fn v_try_statement(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("try")?.checked_add(s.try_start()?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.try_handler()?).ok_or(Fault::Overflow)?.checked_add(s.expect("except")?).ok_or(Fault::Overflow)?.checked_add(s.exception_category()?).ok_or(Fault::Overflow)?.checked_add(v_exception_binding(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_try()?).ok_or(Fault::Overflow)?.checked_add(v_after_try(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_exception_binding(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("as")? != 0i64 {
Ok(s.expect("as")?.checked_add(s.exception_binding()?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_after_try(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(v_statement(s, depth + 1)?)
}
}
fn v_match_statement(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("match")?.checked_add(s.match_start()?).ok_or(Fault::Overflow)?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(v_match_subject(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(" {\n")?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_match_arms(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_match()?).ok_or(Fault::Overflow)?.checked_add(v_after_match(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_match_subject(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("true")? != 0i64 {
Ok(s.match_boolean_subject()?)
} else {
if s.at("false")? != 0i64 {
Ok(s.match_boolean_subject()?)
} else {
Ok(s.match_integer_subject()?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
}
fn v_match_arms(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(s.expect("case")?.checked_add(v_match_pattern(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_match_arm()?).ok_or(Fault::Overflow)?.checked_add(v_match_arms(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_match_pattern(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("_")? != 0i64 {
Ok(s.match_wildcard()?)
} else {
Ok(s.match_pattern()?)
}
}
fn v_after_match(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(v_statement(s, depth + 1)?)
}
}
fn v_list_borrow(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("with")?.checked_add(s.list_owner()?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("as")?).ok_or(Fault::Overflow)?.checked_add(s.list_alias()?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_list_borrow()?).ok_or(Fault::Overflow)?.checked_add(v_after_list_borrow(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_dict_borrow(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("with")?.checked_add(s.dict_owner()?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(s.dict_selector()?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("as")?).ok_or(Fault::Overflow)?.checked_add(s.dict_alias()?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_dict_borrow()?).ok_or(Fault::Overflow)?.checked_add(v_after_list_borrow(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_after_list_borrow(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(v_statement(s, depth + 1)?)
}
}
fn v_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at_nested_dict_local()? != 0i64 {
Ok(v_nested_dict_local(s, depth + 1)?)
} else {
if s.at_nested_list_local()? != 0i64 {
Ok(v_nested_list_local(s, depth + 1)?)
} else {
if s.at_list_local()? != 0i64 {
Ok(v_list_local(s, depth + 1)?)
} else {
if s.at_dict_local()? != 0i64 {
Ok(v_dict_local(s, depth + 1)?)
} else {
Ok(v_struct_local(s, depth + 1)?)
}
}
}
}
}
fn v_struct_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("var")?.checked_add(s.local()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.local_type()?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.constructor()?).ok_or(Fault::Overflow)?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(v_constructor_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.finish_constructor()?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?)
}
fn v_list_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("var")?.checked_add(s.list_local()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("list")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_list_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_list()?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?)
}
fn v_list_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("]")? != 0i64 {
Ok(0i64)
} else {
Ok(s.list_item()?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_list_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_list_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.list_item()?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_list_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_nested_list_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("var")?.checked_add(s.nested_list_local()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("list")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(s.expect("list")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_nested_rows(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_list()?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?)
}
fn v_nested_rows(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("]")? != 0i64 {
Ok(0i64)
} else {
Ok(s.nested_row()?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_row()?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_rows(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_nested_rows(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.nested_row()?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_row()?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_rows(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_nested_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("]")? != 0i64 {
Ok(0i64)
} else {
Ok(s.nested_item()?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_nested_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.nested_item()?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_dict_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("var")?.checked_add(s.dict_local()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("dict")?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(s.expect("str")?).ok_or(Fault::Overflow)?.checked_add(s.expect(",")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_dict()?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?)
}
fn v_dict_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(s.dict_key()?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(")")?).ok_or(Fault::Overflow)?.checked_add(v_following_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_dict_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.dict_key()?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(")")?).ok_or(Fault::Overflow)?.checked_add(v_following_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_nested_dict_local(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("var")?.checked_add(s.nested_dict_local()?).ok_or(Fault::Overflow)?.checked_add(s.expect(":")?).ok_or(Fault::Overflow)?.checked_add(s.expect("dict")?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(s.expect("str")?).ok_or(Fault::Overflow)?.checked_add(s.expect(",")?).ok_or(Fault::Overflow)?.checked_add(s.expect("list")?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(s.expect("int")?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(v_nested_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_dict()?).ok_or(Fault::Overflow)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?)
}
fn v_nested_dict_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(s.nested_dict_key()?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_dict_value()?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_nested_dict_items(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.nested_dict_key()?).ok_or(Fault::Overflow)?.checked_add(s.expect("[")?).ok_or(Fault::Overflow)?.checked_add(v_nested_items(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("]")?).ok_or(Fault::Overflow)?.checked_add(s.finish_nested_dict_value()?).ok_or(Fault::Overflow)?.checked_add(v_following_nested_dict_items(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_constructor_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.constructor_field()?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_constructor_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_constructor_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(v_constructor_tail(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_constructor_tail(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.constructor_field()?.checked_add(s.expect("=")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_constructor_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_control(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("while")? != 0i64 {
Ok(s.expect("while")?.checked_add(s.emit("while ")?).ok_or(Fault::Overflow)?.checked_add(v_comparison(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("{")?).ok_or(Fault::Overflow)?.checked_add(s.emit("s.enter(depth)?;\n")?).ok_or(Fault::Overflow)?.checked_add(v_actions(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("}")?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?.checked_add(v_statement(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(v_condition(s, depth + 1)?.checked_add(s.if_scope_start()?).ok_or(Fault::Overflow)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect("else")?).ok_or(Fault::Overflow)?.checked_add(s.if_scope_else()?).ok_or(Fault::Overflow)?.checked_add(s.emit("} else {\n")?).ok_or(Fault::Overflow)?.checked_add(v_body(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.if_scope_finish()?).ok_or(Fault::Overflow)?.checked_add(s.emit("}\n")?).ok_or(Fault::Overflow)?)
}
}
fn v_condition(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("if")?.checked_add(s.emit("if ")?).ok_or(Fault::Overflow)?.checked_add(v_comparison(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_actions(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("}")? != 0i64 {
Ok(0i64)
} else {
Ok(v_call(s, depth + 1)?.checked_add(s.emit(";\n")?).ok_or(Fault::Overflow)?.checked_add(v_actions(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_comparison(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(s.expect("(")?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_relation(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(" {\n")?).ok_or(Fault::Overflow)?)
}
fn v_relation(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("!=")? != 0i64 {
Ok(s.expect("!=")?.checked_add(s.emit(" != ")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_equality(s, depth + 1)?)
}
}
fn v_equality(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("==")? != 0i64 {
Ok(s.expect("==")?.checked_add(s.emit(" == ")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_ordering(s, depth + 1)?)
}
}
fn v_ordering(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("<=")? != 0i64 {
Ok(s.expect("<=")?.checked_add(s.emit(" <= ")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_upper(s, depth + 1)?)
}
}
fn v_upper(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(">=")? != 0i64 {
Ok(s.expect(">=")?.checked_add(s.emit(" >= ")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_strict(s, depth + 1)?)
}
}
fn v_strict(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("<")? != 0i64 {
Ok(s.expect("<")?.checked_add(s.emit(" < ")?).ok_or(Fault::Overflow)?)
} else {
Ok(s.expect(">")?.checked_add(s.emit(" > ")?).ok_or(Fault::Overflow)?)
}
}
fn v_expression(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(v_product(s, depth + 1)?.checked_add(v_additions(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_additions(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("+")? != 0i64 {
Ok(s.expect("+")?.checked_add(s.emit(".checked_add(")?).ok_or(Fault::Overflow)?.checked_add(v_product(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(").ok_or(Fault::Overflow)?")?).ok_or(Fault::Overflow)?.checked_add(v_additions(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(v_subtractions(s, depth + 1)?)
}
}
fn v_subtractions(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("-")? != 0i64 {
Ok(s.expect("-")?.checked_add(s.emit(".checked_sub(")?).ok_or(Fault::Overflow)?.checked_add(v_product(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(").ok_or(Fault::Overflow)?")?).ok_or(Fault::Overflow)?.checked_add(v_additions(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_product(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
Ok(v_primary(s, depth + 1)?.checked_add(v_factors(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
fn v_factors(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("*")? != 0i64 {
Ok(s.expect("*")?.checked_add(s.emit(".checked_mul(")?).ok_or(Fault::Overflow)?.checked_add(v_primary(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(").ok_or(Fault::Overflow)?")?).ok_or(Fault::Overflow)?.checked_add(v_factors(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(v_divisions(s, depth + 1)?)
}
}
fn v_divisions(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("/")? != 0i64 {
Ok(s.expect("/")?.checked_add(s.emit(".checked_div(")?).ok_or(Fault::Overflow)?.checked_add(v_primary(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.emit(").ok_or(Fault::Overflow)?")?).ok_or(Fault::Overflow)?.checked_add(v_factors(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
fn v_primary(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("(")? != 0i64 {
Ok(s.expect("(")?.checked_add(s.emit("(")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(")")?).ok_or(Fault::Overflow)?)
} else {
Ok(v_atom(s, depth + 1)?)
}
}
fn v_atom(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at_conversion()? != 0i64 {
Ok(s.conversion()?)
} else {
if s.at_integer()? != 0i64 {
Ok(s.literal()?)
} else {
if s.at("host")? != 0i64 {
Ok(v_call(s, depth + 1)?)
} else {
if s.at_len()? != 0i64 {
Ok(s.list_len()?)
} else {
if s.at_dict_set()? != 0i64 {
Ok(s.dict_set()?.checked_add(s.dict_set_key()?).ok_or(Fault::Overflow)?.checked_add(s.expect(",")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.finish_dict_set()?).ok_or(Fault::Overflow)?)
} else {
if s.at_list_append()? != 0i64 {
Ok(s.list_append()?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.finish_list_append()?).ok_or(Fault::Overflow)?)
} else {
if s.at_list_pop()? != 0i64 {
Ok(s.list_pop()?)
} else {
if s.at_module_call()? != 0i64 {
Ok(v_call(s, depth + 1)?)
} else {
if s.at_call()? != 0i64 {
Ok(v_call(s, depth + 1)?)
} else {
if s.at_field()? != 0i64 {
Ok(s.field_value()?)
} else {
Ok(s.value()?)
}
}
}
}
}
}
}
}
}
}
}
fn v_call(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at("host")? != 0i64 {
Ok(s.expect("host")?.checked_add(s.expect(".")?).ok_or(Fault::Overflow)?.checked_add(s.method()?).ok_or(Fault::Overflow)?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(")?).ok_or(Fault::Overflow)?.checked_add(v_host_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.emit(")?")?).ok_or(Fault::Overflow)?)
} else {
if s.at_module_call()? != 0i64 {
Ok(s.module_call()?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(s, depth + 1")?).ok_or(Fault::Overflow)?.checked_add(v_call_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.finish_call()?).ok_or(Fault::Overflow)?.checked_add(s.emit(")?")?).ok_or(Fault::Overflow)?)
} else {
Ok(s.call()?.checked_add(s.expect("(")?).ok_or(Fault::Overflow)?.checked_add(s.emit("(s, depth + 1")?).ok_or(Fault::Overflow)?.checked_add(v_call_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(s.expect(")")?).ok_or(Fault::Overflow)?.checked_add(s.finish_call()?).ok_or(Fault::Overflow)?.checked_add(s.emit(")?")?).ok_or(Fault::Overflow)?)
}
}
}
fn v_host_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.literal()?)
}
}
fn v_call_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(")")? != 0i64 {
Ok(0i64)
} else {
Ok(s.argument()?.checked_add(s.emit(", ")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?)
}
}
fn v_following_arguments(s: &mut Machine, depth: usize) -> R {
s.enter(depth)?;
if s.at(",")? != 0i64 {
Ok(s.expect(",")?.checked_add(s.argument()?).ok_or(Fault::Overflow)?.checked_add(s.emit(", ")?).ok_or(Fault::Overflow)?.checked_add(v_expression(s, depth + 1)?).ok_or(Fault::Overflow)?.checked_add(v_following_arguments(s, depth + 1)?).ok_or(Fault::Overflow)?)
} else {
Ok(0i64)
}
}
