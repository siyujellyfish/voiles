//! 公開直譯與原生共用的容器儲存。元素綁定只保留持有者與索引路徑。
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Key {
    Index(i64),
    Text(String),
}

#[derive(Clone)]
pub(super) enum Value {
    CopyFrom(String),
    Missing,
    Integer(i64),
    Boolean(bool),
    Text(String),
    Decimal(u64, i32),
    List(Vec<Value>),
    Dict(BTreeMap<Key, Value>),
}

impl Value {
    fn child(&self, key: &Key) -> Option<&Value> {
        match (self, key) {
            (Self::List(items), Key::Index(i)) => items.get(usize::try_from(*i).ok()?),
            (Self::Dict(items), key) => items.get(key),
            _ => None,
        }
    }
    pub(super) fn length(&self) -> Option<i64> {
        match self {
            Self::List(items) => items.len().try_into().ok(),
            Self::Dict(items) => items.len().try_into().ok(),
            _ => None,
        }
    }
    pub(super) fn scalar(&self) -> Option<i64> {
        match self { Self::Integer(n) => Some(*n), Self::Boolean(b) => Some(i64::from(*b)), _ => None }
    }
    pub(super) fn tag(&self) -> Option<i64> {
        Some(match self {
            Self::Missing => 0, Self::Integer(_) => 1, Self::Boolean(_) => 2,
            Self::Text(_) => 3, Self::Decimal(..) => 4, _ => return None,
        })
    }
}

#[derive(Default)]
pub(super) struct Store {
    owners: BTreeMap<String, Value>,
    views: BTreeMap<String, (String, Vec<Key>)>,
}

impl Store {
    fn resolve(&self, value: &mut Value) -> Option<()> {
        match value {
            Value::CopyFrom(slot) => *value = match self.get(slot)? {
                Value::Missing => Value::Missing, Value::Integer(n) => Value::Integer(*n),
                Value::Boolean(b) => Value::Boolean(*b), Value::Decimal(c,e) => Value::Decimal(*c,*e),
                _ => return None,
            },
            Value::List(xs) => { for x in xs { self.resolve(x)?; } }
            Value::Dict(xs) => { for x in xs.values_mut() { self.resolve(x)?; } }
            _ => (),
        }
        Some(())
    }
    pub(super) fn put(&mut self, slot: &str, mut value: Value) -> Option<()> {
        self.resolve(&mut value)?;
        self.owners.insert(slot.into(), value);
        Some(())
    }
    pub(super) fn relocate(&mut self, from: &str, to: &str) -> Option<()> {
        let value = self.owners.remove(from)?;
        self.owners.insert(to.into(), value);
        Some(())
    }
    pub(super) fn get(&self, slot: &str) -> Option<&Value> {
        if let Some((owner,path)) = self.views.get(slot) {
            let mut value = self.owners.get(owner)?;
            for key in path { value = value.child(key)?; }
            Some(value)
        } else { self.owners.get(slot) }
    }
    pub(super) fn select(&mut self, slot: &str, owner: &str, key: Key) -> Option<()> {
        self.get(owner)?.child(&key)?;
        let (root, mut path) = self.views.get(owner).cloned().unwrap_or_else(|| (owner.into(), Vec::new()));
        path.push(key);
        self.views.insert(slot.into(), (root,path));
        Some(())
    }
    pub(super) fn append(&mut self, slot: &str, mut value: Value) -> Option<i64> {
        self.resolve(&mut value)?;
        let Value::List(items) = self.owners.get_mut(slot)? else {return None};
        items.push(value);
        items.len().try_into().ok()
    }
    pub(super) fn set(&mut self, slot: &str, key: Key, mut value: Value) -> Option<i64> {
        self.resolve(&mut value)?;
        let Value::Dict(items) = self.owners.get_mut(slot)? else {return None};
        items.insert(key,value);
        items.len().try_into().ok()
    }
    pub(super) fn pop(&mut self, slot: &str) -> Option<i64> {
        let Value::List(items) = self.owners.get_mut(slot)? else {return None};
        items.pop()?.scalar()
    }
    pub(super) fn drop_slot(&mut self, slot: &str) {
        self.owners.remove(slot);
        self.views.remove(slot);
    }
}
