#[derive(Default)]
struct Object(std::collections::BTreeMap<&'static str, Field>);
enum Field { Int(i64), Bool(bool), Text(String) }
impl Object {
    fn copy_scalars(&self) -> Option<Self> {
        let mut copy = Self::default();
        for (&name, field) in &self.0 {
            let value = match field {
                Field::Int(n) => Field::Int(*n),
                Field::Bool(b) => Field::Bool(*b),
                Field::Text(_) => return None,
            };
            copy.0.insert(name,value);
        }
        Some(copy)
    }
    fn boolean(&self, name: &str) -> Option<i64> {
        match self.0.get(name)? { Field::Bool(value) => Some(i64::from(*value)), _ => None }
    }
    fn integer(&self, name: &str) -> Option<i64> {
        match self.0.get(name)? { Field::Int(value) => Some(*value), _ => None }
    }
}
