//! Named param fields: the one form that storage, migrations, presets and the sidecar share.

/// One field value. Choices (enums) are [`Value::Text`] with a stable name.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
    Floats(Vec<f32>),
}

impl Value {
    /// The codec tag of this variant (also hashed).
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Value::Bool(_) => 0,
            Value::Int(_) => 1,
            Value::Float(_) => 2,
            Value::Text(_) => 3,
            Value::Floats(_) => 4,
        }
    }
}

/// A module's params as named fields, in the module's own order.
///
/// Names are lowercase `snake_case` and unique within a module. Field order does not change
/// the hash: [`crate::model::hash`] sorts by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields(pub Vec<(String, Value)>);

impl Fields {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// Sets a field, replacing one of the same name in place or appending it.
    pub fn set(&mut self, name: &str, value: Value) {
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value,
            None => self.0.push((name.to_owned(), value)),
        }
    }

    /// Builder form of [`Fields::set`].
    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.set(name, value);
        self
    }

    pub fn remove(&mut self, name: &str) -> Option<Value> {
        let i = self.0.iter().position(|(n, _)| n == name)?;
        Some(self.0.remove(i).1)
    }

    /// Renames a field, keeping its position; false when `from` is absent.
    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        match self.0.iter_mut().find(|(n, _)| n == from) {
            Some((n, _)) => {
                *n = to.to_owned();
                true
            }
            None => false,
        }
    }

    /// Copies every field of `other` over this one (a preset onto params).
    pub fn overlay(&mut self, other: &Fields) {
        for (n, v) in &other.0 {
            self.set(n, v.clone());
        }
    }

    pub fn f32_or(&self, name: &str, default: f32) -> f32 {
        match self.get(name) {
            Some(Value::Float(v)) => *v,
            Some(Value::Int(v)) => *v as f32,
            _ => default,
        }
    }

    pub fn i64_or(&self, name: &str, default: i64) -> i64 {
        match self.get(name) {
            Some(Value::Int(v)) => *v,
            _ => default,
        }
    }

    pub fn bool_or(&self, name: &str, default: bool) -> bool {
        match self.get(name) {
            Some(Value::Bool(v)) => *v,
            _ => default,
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match self.get(name) {
            Some(Value::Text(v)) => Some(v),
            _ => None,
        }
    }

    /// A fixed-size float list; absent or the wrong length gives `default`.
    pub fn floats_or<const N: usize>(&self, name: &str, default: [f32; N]) -> [f32; N] {
        match self.get(name) {
            Some(Value::Floats(v)) => v.as_slice().try_into().unwrap_or(default),
            _ => default,
        }
    }

    /// A choice by its stable name; unknown or absent gives the default.
    pub fn choice_or<C: Choice>(&self, name: &str, default: C) -> C {
        self.text(name).and_then(C::from_name).unwrap_or(default)
    }
}

/// A param that is one of a fixed set of named options, stored by its stable name.
pub trait Choice: Copy + Sized + 'static {
    /// Every option with its stable name. Names never change once released.
    const ALL: &'static [(Self, &'static str)];

    fn name(self) -> &'static str
    where
        Self: PartialEq,
    {
        Self::ALL
            .iter()
            .find(|(c, _)| *c == self)
            .map_or("", |(_, n)| n)
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
    }
}

/// Declares a [`Choice`] enum with its stable names; the first option is the default.
macro_rules! choice {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $var:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($(#[$vm])* $var),+ }
        impl $crate::model::fields::Choice for $name {
            const ALL: &'static [(Self, &'static str)] = &[$(($name::$var, $s)),+];
        }
        impl Default for $name {
            fn default() -> Self {
                <Self as $crate::model::fields::Choice>::ALL[0].0
            }
        }
        impl From<$name> for $crate::model::fields::Value {
            fn from(c: $name) -> Self {
                $crate::model::fields::Value::Text(
                    $crate::model::fields::Choice::name(c).to_owned(),
                )
            }
        }
    };
}
pub(crate) use choice;
