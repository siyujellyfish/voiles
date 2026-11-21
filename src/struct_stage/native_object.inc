#[derive(Default)]
struct Object(std::collections::BTreeMap<&'static str, Field>);
enum Field { Int(i64), Bool(bool), Text(String) }
impl Object {
    fn integer(&self, name: &str) -> Option<i64> {
        match self.0.get(name)? { Field::Int(value) => Some(*value), _ => None }
    }
}
