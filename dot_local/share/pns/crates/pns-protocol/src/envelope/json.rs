use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::Rejection;
use crate::bounds::{MAX_DEPTH, Violation};

pub(super) fn parse(bytes: &[u8]) -> Result<Value, Rejection> {
    let mut exceeded = None;
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = Json {
        depth: 1,
        exceeded: &mut exceeded,
    }
    .deserialize(&mut parser)
    .and_then(|value| parser.end().map(|()| value));
    value.map_err(|error| match exceeded {
        Some(depth) => Rejection::Bound(Violation::Depth { depth }),
        None => Rejection::Malformed(error.to_string()),
    })
}

// The caller caps raw bytes before parsing. Containers also stop at the wire
// depth cap, before allocating their descendants; the shared walk checks the
// remaining structural limits after this parse rejects repeated fields.
struct Json<'a> {
    depth: usize,
    exceeded: &'a mut Option<usize>,
}

impl Json<'_> {
    fn container<E: de::Error>(&mut self) -> Result<(), E> {
        if self.depth > MAX_DEPTH {
            *self.exceeded = Some(self.depth);
            return Err(E::custom("container depth exceeds the wire limit"));
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for Json<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Json<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object fields")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut input: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut values = Vec::new();
        while let Some(value) = input.next_element_seed(Json {
            depth: self.depth + 1,
            exceeded: &mut *self.exceeded,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(mut self, mut input: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut object = Map::new();
        while let Some(key) = input.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object field"));
            }
            let value = input.next_value_seed(Json {
                depth: self.depth + 1,
                exceeded: &mut *self.exceeded,
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
