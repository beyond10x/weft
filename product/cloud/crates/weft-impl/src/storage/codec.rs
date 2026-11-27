//! How a stored row is written as JSON and read back. `build.rs` writes the codec of every type a
//! row reaches; this file holds the codec of the standard types they are built from and of the
//! one weft-types type whose field is private (`Binary64`).
//!
//! The form is for the state file only, never for a wire: a struct is an object of its fields, a
//! one-field tuple struct is its field, a unit variant is its name, any other variant an object
//! with one member named after it, and an `Option` an array of zero or one item (so `Some(None)`
//! and `None` stay apart).

use std::collections::BTreeMap;

pub use serde_json::Value;

/// A value of the state file.
pub trait Codec: Sized {
    /// The value as JSON.
    fn encode(&self) -> Value;

    /// The value written by [`Codec::encode`].
    ///
    /// # Errors
    ///
    /// Where `value` is not such a value: the place and what was found.
    fn decode(value: &Value) -> Result<Self, String>;
}

/// The member `name` of an object.
///
/// # Errors
///
/// When `value` is no object or lacks the member.
pub fn member<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value
        .as_object()
        .ok_or_else(|| format!("expected an object with {name}, found {value}"))?
        .get(name)
        .ok_or_else(|| format!("no member {name} in {value}"))
}

/// The item `index` of an array.
///
/// # Errors
///
/// When `value` is no array or is shorter.
pub fn item(value: &Value, index: usize) -> Result<&Value, String> {
    value
        .as_array()
        .and_then(|items| items.get(index))
        .ok_or_else(|| format!("no item {index} in {value}"))
}

/// The variant of an enum value and what it carries.
///
/// # Errors
///
/// When `value` is neither a variant name nor an object with one member.
pub fn variant(value: &Value) -> Result<(&str, Option<&Value>), String> {
    match value {
        Value::String(name) => Ok((name, None)),
        Value::Object(members) if members.len() == 1 => {
            let (name, inner) = members.iter().next().expect("one member");
            Ok((name, Some(inner)))
        }
        other => Err(format!("expected a variant, found {other}")),
    }
}

/// A variant carrying `inner`.
pub fn tagged(name: &str, inner: Value) -> Value {
    Value::Object(std::iter::once((name.to_owned(), inner)).collect())
}

/// `error` placed under `place`.
pub fn at(place: &str, error: String) -> String {
    format!("{place}: {error}")
}

impl Codec for String {
    fn encode(&self) -> Value {
        Value::String(self.clone())
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("expected a string, found {value}"))
    }
}

impl Codec for bool {
    fn encode(&self) -> Value {
        Value::Bool(*self)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_bool()
            .ok_or_else(|| format!("expected a boolean, found {value}"))
    }
}

impl Codec for i64 {
    fn encode(&self) -> Value {
        Value::from(*self)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_i64()
            .ok_or_else(|| format!("expected an integer, found {value}"))
    }
}

impl Codec for u8 {
    fn encode(&self) -> Value {
        Value::from(*self)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_u64()
            .and_then(|number| u8::try_from(number).ok())
            .ok_or_else(|| format!("expected a byte, found {value}"))
    }
}

impl Codec for usize {
    fn encode(&self) -> Value {
        Value::from(*self)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .ok_or_else(|| format!("expected a count, found {value}"))
    }
}

impl<T: Codec> Codec for Vec<T> {
    fn encode(&self) -> Value {
        Value::Array(self.iter().map(Codec::encode).collect())
    }
    fn decode(value: &Value) -> Result<Self, String> {
        value
            .as_array()
            .ok_or_else(|| format!("expected an array, found {value}"))?
            .iter()
            .map(T::decode)
            .collect()
    }
}

impl<T: Codec> Codec for Option<T> {
    fn encode(&self) -> Value {
        Value::Array(self.iter().map(Codec::encode).collect())
    }
    fn decode(value: &Value) -> Result<Self, String> {
        match value.as_array().map(Vec::as_slice) {
            Some([]) => Ok(None),
            Some([inner]) => T::decode(inner).map(Some),
            _ => Err(format!(
                "expected an array of at most one item, found {value}"
            )),
        }
    }
}

impl<T: Codec> Codec for Box<T> {
    fn encode(&self) -> Value {
        T::encode(self)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        T::decode(value).map(Box::new)
    }
}

impl<A: Codec, B: Codec> Codec for (A, B) {
    fn encode(&self) -> Value {
        Value::Array(vec![self.0.encode(), self.1.encode()])
    }
    fn decode(value: &Value) -> Result<Self, String> {
        Ok((A::decode(item(value, 0)?)?, B::decode(item(value, 1)?)?))
    }
}

impl<K: Codec + Ord, V: Codec> Codec for BTreeMap<K, V> {
    fn encode(&self) -> Value {
        Value::Array(
            self.iter()
                .map(|(key, value)| Value::Array(vec![key.encode(), value.encode()]))
                .collect(),
        )
    }
    fn decode(value: &Value) -> Result<Self, String> {
        Vec::<(K, V)>::decode(value).map(|pairs| pairs.into_iter().collect())
    }
}

impl Codec for weft_types::primitives::Binary64 {
    fn encode(&self) -> Value {
        serde_json::Number::from_f64(self.get()).map_or(Value::Null, Value::Number)
    }
    fn decode(value: &Value) -> Result<Self, String> {
        let number = value
            .as_f64()
            .ok_or_else(|| format!("expected a number, found {value}"))?;
        Self::new(number).map_err(|error| error.to_string())
    }
}
