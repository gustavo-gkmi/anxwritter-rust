//! Attribute values and i2 attribute-type inference.
//!
//! Entity/link `attributes` are an open `name -> value` map whose i2 type is
//! inferred from the Python value (`utils._infer_attr_type`): `bool` -> Flag,
//! `int`/`float` -> Number, `date`/`datetime` -> DateTime, everything else
//! -> Text. [`AttrValue`] preserves that distinction across JSON/YAML, where the
//! crucial subtlety is that **bool must be checked before int** (in Python
//! `bool` is a subclass of `int`).

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

/// The inferred i2 attribute type for an attribute value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferredType {
    Text,
    Flag,
    Number,
    DateTime,
}

impl InferredType {
    /// The ANB XML type token (`AttText`, `AttFlag`, `AttNumber`, `AttTime`).
    pub fn anb_token(self) -> &'static str {
        match self {
            InferredType::Text => "AttText",
            InferredType::Flag => "AttFlag",
            InferredType::Number => "AttNumber",
            InferredType::DateTime => "AttTime",
        }
    }
}

/// A scalar attribute value as supplied by the user or parsed from JSON/YAML.
///
/// Order of the variants matters for the custom `Deserialize`: a JSON `true`
/// becomes [`AttrValue::Bool`], an integer literal becomes [`AttrValue::Int`],
/// a fractional number [`AttrValue::Float`], and anything else a string.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AttrValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl AttrValue {
    /// Infer the i2 attribute type. Strings are always Text (a date supplied as
    /// a string stays Text, matching the Python behaviour for JSON input).
    pub fn infer_type(&self) -> InferredType {
        match self {
            AttrValue::Bool(_) => InferredType::Flag,
            AttrValue::Int(_) | AttrValue::Float(_) => InferredType::Number,
            AttrValue::Str(_) => InferredType::Text,
        }
    }

    /// Render the value the way it appears as an XML attribute `Value`.
    pub fn render(&self) -> String {
        match self {
            AttrValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            AttrValue::Int(i) => i.to_string(),
            // Match Python's str(float): an integral float still shows ".0".
            AttrValue::Float(x) => {
                if x.is_finite() && x.fract() == 0.0 {
                    format!("{x:.1}")
                } else {
                    x.to_string()
                }
            }
            AttrValue::Str(s) => s.clone(),
        }
    }
}

impl<'de> Deserialize<'de> for AttrValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = AttrValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a bool, integer, float, or string")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<AttrValue, E> {
                Ok(AttrValue::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<AttrValue, E> {
                Ok(AttrValue::Int(v))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<AttrValue, E> {
                Ok(AttrValue::Int(v as i64))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<AttrValue, E> {
                Ok(AttrValue::Float(v))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<AttrValue, E> {
                Ok(AttrValue::Str(v.to_string()))
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_infers_flag_not_number() {
        assert_eq!(AttrValue::Bool(true).infer_type(), InferredType::Flag);
    }

    #[test]
    fn numbers_infer_number() {
        assert_eq!(AttrValue::Int(12).infer_type(), InferredType::Number);
        assert_eq!(AttrValue::Float(1.5).infer_type(), InferredType::Number);
    }

    #[test]
    fn json_bool_deserializes_to_bool_variant() {
        let v: AttrValue = serde_json::from_str("true").unwrap();
        assert_eq!(v, AttrValue::Bool(true));
        let v: AttrValue = serde_json::from_str("12").unwrap();
        assert_eq!(v, AttrValue::Int(12));
    }
}
