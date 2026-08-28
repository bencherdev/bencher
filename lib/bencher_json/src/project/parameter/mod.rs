use std::{cmp::Ordering, collections::BTreeMap, fmt, str::FromStr};

use bencher_valid::{DateTime, ParameterKey as ValidParameterKey, ParameterValue};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap as _};

use crate::BenchmarkUuid;

#[cfg(feature = "db")]
pub mod jsonb;

crate::typed_uuid::typed_uuid!(ParameterUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewParameter {
    /// The parameter set.
    /// Each key maps to a JSON scalar: a string, a number, or a boolean.
    pub set: ParameterSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonParameters(pub Vec<JsonParameter>);

crate::from_vec!(JsonParameters[JsonParameter]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonParameter {
    pub uuid: ParameterUuid,
    pub benchmark: BenchmarkUuid,
    pub set: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.set)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateParameter {
    /// Set whether the parameter set is archived.
    pub archived: Option<bool>,
}

/// The most keys one parameter set may carry, anchored to the number of named
/// values one measure may carry.
///
/// Deliberately low: raising the cap is a release note and lowering it is a
/// breaking change, so the asymmetry runs one way.
pub const MAX_PARAMETER_KEYS: usize = 8;

/// A benchmark parameter set: the permutation of inputs that a benchmark ran with.
///
/// The canonical form is [RFC 8785][jcs] (JSON Canonicalization Scheme):
/// object keys sorted by UTF-16 code unit, ECMAScript number formatting,
/// and no insignificant whitespace.
/// Canonicalization happens here, before the write,
/// so the database's `UNIQUE(benchmark_id, "set")` constraint
/// is the enforcement point for canonical equality.
///
/// [jcs]: https://www.rfc-editor.org/rfc/rfc8785
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Jsonb))]
pub struct ParameterSet(BTreeMap<ParameterKey, ParameterScalar>);

impl ParameterSet {
    /// The RFC 8785 (JCS) canonical serialization of this parameter set.
    pub fn canonical(&self) -> String {
        let mut canonical = String::from("{");
        for (index, (key, value)) in self.0.iter().enumerate() {
            if index > 0 {
                canonical.push(',');
            }
            write_json_string(key.as_ref(), &mut canonical);
            canonical.push(':');
            match value {
                ParameterScalar::Bool(boolean) => {
                    canonical.push_str(if *boolean { "true" } else { "false" });
                },
                ParameterScalar::Number(number) => {
                    canonical.push_str(ryu_js::Buffer::new().format(number.into_inner()));
                },
                ParameterScalar::String(string) => {
                    write_json_string(string.as_ref(), &mut canonical);
                },
            }
        }
        canonical.push('}');
        canonical
    }

    /// Whether this is the empty parameter set.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether every key of this set is in `superset` with an identical value.
    ///
    /// This is what a parameters filter element matches on. Values compare
    /// canonically, so `16`, `16.0`, and `1.6e1` are one number and one match.
    pub fn is_subset_of(&self, superset: &Self) -> bool {
        self.0
            .iter()
            .all(|(key, value)| superset.0.get(key) == Some(value))
    }

    /// The `SQLite` JSONB encoding of the canonical form.
    ///
    /// Byte identical to what `SQLite`'s own `jsonb()` produces over
    /// [`Self::canonical`], which is what lets a set written here collide with a
    /// set minted in SQL on `UNIQUE(benchmark_id, "set")`.
    #[cfg(feature = "db")]
    pub fn to_jsonb(&self) -> Result<Vec<u8>, jsonb::JsonbError> {
        let mut object = jsonb::Object::default();
        for (key, value) in &self.0 {
            match value {
                ParameterScalar::Bool(boolean) => object.insert_bool(key.as_ref(), *boolean)?,
                ParameterScalar::Number(number) => object.insert_number(
                    key.as_ref(),
                    ryu_js::Buffer::new().format(number.into_inner()),
                )?,
                ParameterScalar::String(string) => {
                    object.insert_string(key.as_ref(), string.as_ref())?;
                },
            }
        }
        object.into_blob()
    }
}

impl fmt::Display for ParameterSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

impl FromStr for ParameterSet {
    type Err = ParametersError;

    fn from_str(parameters: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(parameters).map_err(ParametersError::Json)
    }
}

impl Serialize for ParameterSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ParameterSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The map has already collapsed duplicate keys by the time it is counted,
        // so the cap bounds distinct keys and a key spelled twice still counts once.
        let parameters = BTreeMap::deserialize(deserializer)?;
        if parameters.len() > MAX_PARAMETER_KEYS {
            return Err(de::Error::custom(format!(
                "Parameter set has {} keys, more than the {MAX_PARAMETER_KEYS} allowed",
                parameters.len()
            )));
        }
        Ok(Self(parameters))
    }
}

/// An object whose every value is a JSON scalar: string, number, or bool.
///
/// Written out by hand because the serialized form is a map with a custom key
/// order and a hand written value enum, neither of which derives.
#[cfg(feature = "schema")]
impl JsonSchema for ParameterSet {
    fn schema_name() -> String {
        "ParameterSet".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, ObjectValidation, SchemaObject, SubschemaValidation};

        let scalar = SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                any_of: Some(vec![
                    String::json_schema(generator),
                    f64::json_schema(generator),
                    bool::json_schema(generator),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        };

        SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                additional_properties: Some(Box::new(scalar.into())),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParametersError {
    #[error("Failed to parse benchmark parameters: {0}")]
    Json(serde_json::Error),
}

/// A parameter set key, ordered by UTF-16 code unit as RFC 8785 requires.
///
/// That order is not the code point order: a supplementary plane character
/// (U+10000 and above) leads with a surrogate in U+D800..U+DBFF, so it sorts
/// before every character in U+E000..U+FFFF.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ParameterKey(ValidParameterKey);

impl AsRef<str> for ParameterKey {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Ord for ParameterKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref()
            .encode_utf16()
            .cmp(other.as_ref().encode_utf16())
    }
}

impl PartialOrd for ParameterKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for ParameterKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for ParameterKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ValidParameterKey::deserialize(deserializer).map(Self)
    }
}

/// A parameter set value: a JSON scalar.
///
/// Null, arrays, and objects are rejected. Numbers are ECMAScript doubles,
/// so `16`, `16.0`, and `1.6e1` are one value with one canonical spelling.
///
/// Only the string arm carries a bound. A number and a boolean each have one
/// canonical spelling that is inherently short.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ParameterScalar {
    Bool(bool),
    Number(OrderedFloat<f64>),
    String(ParameterValue),
}

impl Serialize for ParameterScalar {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Bool(boolean) => serializer.serialize_bool(*boolean),
            // Round trip through the canonical spelling so an integral value
            // goes out as `16` and not `16.0`.
            Self::Number(number) => {
                let canonical = ryu_js::Buffer::new().format(number.into_inner()).to_owned();
                serde_json::from_str::<serde_json::Number>(&canonical)
                    .map_err(serde::ser::Error::custom)?
                    .serialize(serializer)
            },
            Self::String(string) => serializer.serialize_str(string.as_ref()),
        }
    }
}

impl<'de> Deserialize<'de> for ParameterScalar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ParameterScalarVisitor)
    }
}

struct ParameterScalarVisitor;

impl ParameterScalarVisitor {
    fn number<E>(number: &serde_json::Number) -> Result<ParameterScalar, E>
    where
        E: de::Error,
    {
        // Deferred to `serde_json` so the integer to double conversion that
        // RFC 8785 specifies happens in one place.
        number
            .as_f64()
            .filter(|number| number.is_finite())
            .map(|number| ParameterScalar::Number(OrderedFloat(number)))
            .ok_or_else(|| E::custom(format!("Parameter value ({number}) is not a finite number")))
    }
}

impl de::Visitor<'_> for ParameterScalarVisitor {
    type Value = ParameterScalar;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON scalar parameter value (string, number, or boolean)")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(ParameterScalar::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Self::number(&serde_json::Number::from(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Self::number(&serde_json::Number::from(v))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(v).map_or_else(
            || {
                Err(E::custom(format!(
                    "Parameter value ({v}) is not a finite number"
                )))
            },
            |number| Self::number(&number),
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string(v.to_owned())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ParameterValue::try_from(v)
            .map(ParameterScalar::String)
            .map_err(E::custom)
    }
}

/// Append the RFC 8785 escaping of a JSON string, which is the escaping
/// ECMAScript `JSON.stringify` performs: the seven short escapes, `\u00xx` in
/// lowercase hex for the remaining control characters, and everything else literal.
fn write_json_string(string: &str, canonical: &mut String) {
    canonical.push('"');
    write_json_string_body(string, canonical);
    canonical.push('"');
}

/// Append the body of an RFC 8785 escaped JSON string, without its quotes.
fn write_json_string_body(string: &str, canonical: &mut String) {
    for character in string.chars() {
        match character {
            '"' => canonical.push_str("\\\""),
            '\\' => canonical.push_str("\\\\"),
            '\u{8}' => canonical.push_str("\\b"),
            '\u{9}' => canonical.push_str("\\t"),
            '\u{a}' => canonical.push_str("\\n"),
            '\u{c}' => canonical.push_str("\\f"),
            '\u{d}' => canonical.push_str("\\r"),
            control if control < '\u{20}' => {
                let control = u32::from(control);
                canonical.push_str("\\u00");
                canonical.push(hex_digit(control >> 4));
                canonical.push(hex_digit(control & 0xf));
            },
            character => canonical.push(character),
        }
    }
}

fn hex_digit(nibble: u32) -> char {
    char::from_digit(nibble, 16).unwrap_or('0')
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;

    use super::{ParameterKey, ParameterScalar, ParameterSet};

    fn parse(parameters: &str) -> ParameterSet {
        parameters
            .parse::<ParameterSet>()
            .expect("Failed to parse parameters")
    }

    fn canonical(parameters: &str) -> String {
        parse(parameters).canonical()
    }

    /// A one key parameter set holding an exact `f64`, built without a parsing
    /// step so a bit pattern reaches the canonicalizer unchanged.
    fn number(value: f64) -> ParameterSet {
        ParameterSet(
            [(
                ParameterKey("n".parse().expect("Failed to parse parameter key")),
                ParameterScalar::Number(OrderedFloat(value)),
            )]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn canonical_empty_set() {
        assert_eq!(canonical("{}"), "{}");
        assert_eq!(ParameterSet::default().canonical(), "{}");
    }

    #[test]
    fn canonical_no_insignificant_whitespace() {
        assert_eq!(
            canonical("{\n  \"size_mb\": 16,\n  \"op\": \"read\"\n}"),
            r#"{"op":"read","size_mb":16}"#
        );
    }

    #[test]
    fn canonical_key_order_is_utf16_code_units() {
        // RFC 8785 section 3.2.3. The emoji is U+1F600, which sorts *before*
        // U+FB33 by UTF-16 code unit (its lead surrogate is U+D83D) and *after*
        // it by code point. A UTF-8 or code point sort silently gets this wrong.
        //
        // The low end of the range is U+0001 rather than a carriage return: a key
        // is trimmed and non-empty, so a key that is only whitespace is rejected.
        // U+0001 is a control character that is not whitespace, so it still pins
        // both the ordering floor and the `\u00xx` escape.
        let parameters = canonical(
            r#"{
                "\u20ac": "Euro Sign",
                "\u0001": "Start Of Heading",
                "\ufb33": "Hebrew Letter Dalet With Dagesh",
                "1": "One",
                "\ud83d\ude00": "Emoji: Grinning Face",
                "\u0080": "Control",
                "\u00f6": "Latin Small Letter O With Diaeresis"
            }"#,
        );
        assert_eq!(
            parameters,
            concat!(
                r#"{"\u0001":"Start Of Heading","1":"One","#,
                "\"\u{80}\":\"Control\",",
                "\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",",
                "\"\u{20ac}\":\"Euro Sign\",",
                "\"\u{1f600}\":\"Emoji: Grinning Face\",",
                "\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}"
            )
        );
    }

    #[test]
    fn canonical_string_escapes() {
        // The seven short escapes are preserved.
        assert_eq!(
            canonical(r#"{"a": "\b\t\n\f\r\"\\"}"#),
            r#"{"a":"\b\t\n\f\r\"\\"}"#
        );
        // Control characters without a short escape use lowercase hex.
        assert_eq!(
            canonical(r#"{"a": "\u001F\u0000"}"#),
            r#"{"a":"\u001f\u0000"}"#
        );
        // Everything else is literal, including the solidus and DEL.
        assert_eq!(canonical(r#"{"a": "/\u007f"}"#), "{\"a\":\"/\u{7f}\"}");
    }

    #[test]
    fn canonical_booleans() {
        assert_eq!(
            canonical(r#"{"b": false, "a": true}"#),
            r#"{"a":true,"b":false}"#
        );
    }

    // RFC 8785 appendix B: ECMAScript `Number::toString` formatting.
    #[test]
    fn canonical_number_formatting() {
        for (parameters, expected) in [
            (r#"{"n": 0}"#, r#"{"n":0}"#),
            (r#"{"n": -0.0}"#, r#"{"n":0}"#),
            (r#"{"n": 5e-324}"#, r#"{"n":5e-324}"#),
            (r#"{"n": -5e-324}"#, r#"{"n":-5e-324}"#),
            (
                r#"{"n": 1.7976931348623157e308}"#,
                r#"{"n":1.7976931348623157e+308}"#,
            ),
            (
                r#"{"n": -1.7976931348623157e308}"#,
                r#"{"n":-1.7976931348623157e+308}"#,
            ),
            (r#"{"n": 9007199254740992}"#, r#"{"n":9007199254740992}"#),
            (r#"{"n": -9007199254740992}"#, r#"{"n":-9007199254740992}"#),
            (
                r#"{"n": 295147905179352825856}"#,
                r#"{"n":295147905179352830000}"#,
            ),
            (r#"{"n": 1e21}"#, r#"{"n":1e+21}"#),
            (r#"{"n": 1e23}"#, r#"{"n":1e+23}"#),
            (r#"{"n": 0.000001}"#, r#"{"n":0.000001}"#),
            (
                r#"{"n": 9.999999999999997e-7}"#,
                r#"{"n":9.999999999999997e-7}"#,
            ),
            (r#"{"n": 333333333.3333333}"#, r#"{"n":333333333.3333333}"#),
            (r#"{"n": 1}"#, r#"{"n":1}"#),
            (r#"{"n": -1.5}"#, r#"{"n":-1.5}"#),
            (r#"{"n": 100}"#, r#"{"n":100}"#),
        ] {
            assert_eq!(canonical(parameters), expected, "for {parameters}");
        }
    }

    // RFC 8785 appendix B: every IEEE 754 value and its ECMAScript
    // `Number::toString` serialization, keyed by bit pattern so the table is read
    // exactly as the RFC states it, with no parsing step in between.
    #[test]
    fn canonical_number_conformance() {
        for (bits, expected) in [
            (0x0000_0000_0000_0000u64, "0"),
            (0x8000_0000_0000_0000u64, "0"),
            (0x0000_0000_0000_0001u64, "5e-324"),
            (0x8000_0000_0000_0001u64, "-5e-324"),
            (0x7fef_ffff_ffff_ffffu64, "1.7976931348623157e+308"),
            (0xffef_ffff_ffff_ffffu64, "-1.7976931348623157e+308"),
            (0x4340_0000_0000_0000u64, "9007199254740992"),
            (0xc340_0000_0000_0000u64, "-9007199254740992"),
            (0x4430_0000_0000_0000u64, "295147905179352830000"),
            (0x44b5_2d02_c7e1_4af5u64, "9.999999999999997e+22"),
            (0x44b5_2d02_c7e1_4af6u64, "1e+23"),
            (0x44b5_2d02_c7e1_4af7u64, "1.0000000000000001e+23"),
            (0x444b_1ae4_d6e2_ef4eu64, "999999999999999700000"),
            (0x444b_1ae4_d6e2_ef4fu64, "999999999999999900000"),
            (0x444b_1ae4_d6e2_ef50u64, "1e+21"),
            (0x3eb0_c6f7_a0b5_ed8cu64, "9.999999999999997e-7"),
            (0x3eb0_c6f7_a0b5_ed8du64, "0.000001"),
            (0x41b3_de43_5555_5553u64, "333333333.3333332"),
            (0x41b3_de43_5555_5554u64, "333333333.33333325"),
            (0x41b3_de43_5555_5555u64, "333333333.3333333"),
            (0x41b3_de43_5555_5556u64, "333333333.3333334"),
            (0x41b3_de43_5555_5557u64, "333333333.33333343"),
            (0xbecb_f647_612f_3696u64, "-0.0000033333333333333333"),
            // Round to even, where the shortest round trip digits are a tie.
            (0x4314_3ff3_c1cb_0959u64, "1424953923781206.2"),
        ] {
            assert_eq!(
                number(f64::from_bits(bits)).canonical(),
                format!(r#"{{"n":{expected}}}"#),
                "for {bits:016x}"
            );
        }
    }

    // The canonical form has to survive a write and a read: a parameter set read
    // back out of the database is parsed, and re-canonicalizing it must land on
    // the same bytes or `UNIQUE(benchmark_id, "set")` stops holding.
    #[test]
    fn canonical_survives_a_round_trip() {
        // Deterministic xorshift64, so the sample never varies between runs.
        let mut bits: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..10_000u32 {
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            let value = f64::from_bits(bits);
            if !value.is_finite() {
                continue;
            }

            let once = number(value).canonical();
            let twice = once
                .parse::<ParameterSet>()
                .expect("Failed to parse canonical parameters")
                .canonical();
            assert_eq!(once, twice, "for {bits:016x}");
        }
    }

    #[test]
    fn canonical_number_spellings_collapse() {
        let sixteen = canonical(r#"{"n": 16}"#);
        assert_eq!(sixteen, r#"{"n":16}"#);
        assert_eq!(canonical(r#"{"n": 16.0}"#), sixteen);
        assert_eq!(canonical(r#"{"n": 1.6e1}"#), sixteen);
    }

    #[test]
    fn canonical_key_order_collapses() {
        assert_eq!(
            canonical(r#"{"b": 1, "a": 2}"#),
            canonical(r#"{"a": 2, "b": 1}"#)
        );
    }

    // A duplicate key is last wins, which is what parsing into a map does and what
    // every JSON parser a harness is likely to use already does. That is the decided
    // behavior, not an accident: it is the least bad of the options, the easiest to
    // explain, and it just works. A duplicate is never an error.
    //
    // Two keys that differ only in case are not duplicates. They are two keys, and
    // RFC 8785 orders `A` before `a` by UTF-16 code unit.
    #[test]
    fn duplicate_keys_are_last_wins() {
        assert_eq!(canonical(r#"{"a": 1, "a": 2}"#), r#"{"a":2}"#);
        assert_eq!(canonical(r#"{"a": 1, "a": "two"}"#), r#"{"a":"two"}"#);
        assert_eq!(canonical(r#"{"A": 1, "a": 2}"#), r#"{"A":1,"a":2}"#);
    }

    #[test]
    fn canonical_round_trips_through_parsing() {
        let parameters = canonical(r#"{"size_mb": 16, "op": "read", "fsync": true}"#);
        assert_eq!(canonical(&parameters), parameters);
    }

    #[test]
    fn rejects_non_scalar_values() {
        for parameters in [
            r#"{"a": null}"#,
            r#"{"a": []}"#,
            r#"{"a": [1, 2]}"#,
            r#"{"a": {}}"#,
            r#"{"a": {"b": 1}}"#,
        ] {
            assert!(
                parameters.parse::<ParameterSet>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_non_object_payloads() {
        for parameters in ["null", "[]", "1", r#""a""#, "true"] {
            assert!(
                parameters.parse::<ParameterSet>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    /// A 64 byte key or string value, the longest either may be.
    const LEN_64_STR: &str = "0123456789012345678901234567890123456789012345678901234567890123";
    /// One byte past the longest.
    const LEN_65_STR: &str = "01234567890123456789012345678901234567890123456789012345678901234";

    /// A parameter set literal with `count` distinct keys.
    fn keys(count: usize) -> String {
        let entries = (0..count)
            .map(|index| format!(r#""k{index}": {index}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{entries}}}")
    }

    // The cap is deliberately low, so raising it stays easy and lowering it never
    // has to happen. Eight is where the named value cap starts too.
    #[test]
    fn rejects_more_than_the_key_cap() {
        keys(8)
            .parse::<ParameterSet>()
            .expect("Failed to parse a parameter set at the cap");
        assert!(
            keys(9).parse::<ParameterSet>().is_err(),
            "expected a ninth key to be rejected"
        );
    }

    // The cap counts keys, not entries. Duplicates collapse to last wins before the
    // count, so a key spelled nine times is one key and never trips the cap.
    #[test]
    fn duplicate_keys_do_not_count_against_the_cap() {
        let nine_spellings = format!(
            "{{{}}}",
            (0..9)
                .map(|index| format!(r#""a": {index}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(canonical(&nine_spellings), r#"{"a":8}"#);

        // Eight distinct keys stay eight distinct keys however often they repeat.
        let doubled = format!(
            "{{{}}}",
            (0..2)
                .flat_map(|_| (0..8).map(|index| format!(r#""k{index}": {index}"#)))
                .collect::<Vec<_>>()
                .join(",")
        );
        doubled
            .parse::<ParameterSet>()
            .expect("Failed to parse duplicated keys at the cap");
    }

    // Keys that differ only in case are two keys, so they count as two.
    #[test]
    fn case_distinct_keys_count_separately() {
        assert_eq!(canonical(r#"{"A": 1, "a": 2}"#), r#"{"A":1,"a":2}"#);
    }

    #[test]
    fn rejects_long_keys() {
        format!(r#"{{"{LEN_64_STR}": 1}}"#)
            .parse::<ParameterSet>()
            .expect("Failed to parse a 64 byte key");
        assert!(
            format!(r#"{{"{LEN_65_STR}": 1}}"#)
                .parse::<ParameterSet>()
                .is_err(),
            "expected a 65 byte key to be rejected"
        );
    }

    #[test]
    fn rejects_long_string_values() {
        format!(r#"{{"a": "{LEN_64_STR}"}}"#)
            .parse::<ParameterSet>()
            .expect("Failed to parse a 64 byte string value");
        assert!(
            format!(r#"{{"a": "{LEN_65_STR}"}}"#)
                .parse::<ParameterSet>()
                .is_err(),
            "expected a 65 byte string value to be rejected"
        );
    }

    #[test]
    fn rejects_empty_or_untrimmed_keys() {
        for key in ["", " ", "\\r", "\\n", "\\t", " a", "a "] {
            let parameters = format!(r#"{{"{key}": 1}}"#);
            assert!(
                parameters.parse::<ParameterSet>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_empty_or_untrimmed_string_values() {
        for value in ["", " ", "\\r", "\\n", "\\t", " a", "a "] {
            let parameters = format!(r#"{{"a": "{value}"}}"#);
            assert!(
                parameters.parse::<ParameterSet>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    // Numbers and booleans are untouched: their canonical spellings are inherently
    // short, so a length rule would only ever be a rule about nothing.
    #[test]
    fn numbers_and_booleans_are_not_length_bound() {
        assert_eq!(
            canonical(r#"{"a": 1.7976931348623157e308}"#),
            r#"{"a":1.7976931348623157e+308}"#
        );
        assert_eq!(
            canonical(r#"{"a": true, "b": false}"#),
            r#"{"a":true,"b":false}"#
        );
    }

    #[test]
    fn subset_match_includes_supersets() {
        let variant = parse(r#"{"fsync": true, "op": "read", "size_mb": 16}"#);

        for filter in [
            "{}",
            r#"{"op": "read"}"#,
            r#"{"size_mb": 16}"#,
            r#"{"op": "read", "size_mb": 16}"#,
            r#"{"fsync": true, "op": "read", "size_mb": 16}"#,
        ] {
            assert!(
                parse(filter).is_subset_of(&variant),
                "expected {filter} to match the variant"
            );
        }

        for filter in [
            r#"{"op": "write"}"#,
            r#"{"size_mb": 32}"#,
            r#"{"fsync": false}"#,
            // A key the variant does not pin at all.
            r#"{"threads": 4}"#,
            // Every key has to match, not just one of them.
            r#"{"op": "read", "size_mb": 32}"#,
        ] {
            assert!(
                !parse(filter).is_subset_of(&variant),
                "expected {filter} not to match the variant"
            );
        }
    }

    #[test]
    fn subset_match_of_the_empty_set() {
        let empty = ParameterSet::default();
        assert!(empty.is_subset_of(&empty));
        assert!(!parse(r#"{"op": "read"}"#).is_subset_of(&empty));
    }

    #[test]
    fn subset_match_is_number_spelling_blind() {
        let variant = parse(r#"{"n": 16}"#);
        for filter in [r#"{"n": 16}"#, r#"{"n": 16.0}"#, r#"{"n": 1.6e1}"#] {
            assert!(
                parse(filter).is_subset_of(&variant),
                "expected {filter} to match {{\"n\":16}}"
            );
        }
        assert!(!parse(r#"{"n": 16.5}"#).is_subset_of(&variant));
    }

    #[test]
    fn subset_match_is_type_aware() {
        let variant = parse(r#"{"a": 1, "b": "true", "c": true}"#);
        for filter in [r#"{"a": "1"}"#, r#"{"b": true}"#, r#"{"c": "true"}"#] {
            assert!(
                !parse(filter).is_subset_of(&variant),
                "expected {filter} not to match the variant"
            );
        }
    }

    #[test]
    fn accepts_scalar_values() {
        let parameters = r#"{"s": "a", "n": 1, "b": true}"#
            .parse::<ParameterSet>()
            .expect("Failed to parse parameters");
        assert!(!parameters.is_empty());
    }
}

/// The JSONB encoding is `SQLite`'s, so these impls are too.
///
/// Every other backend spells `Jsonb` differently, and a generic impl would be
/// claiming an encoding it does not have.
#[cfg(feature = "db")]
mod db {
    use super::{ParameterSet, jsonb};

    impl diesel::serialize::ToSql<diesel::sql_types::Jsonb, diesel::sqlite::Sqlite> for ParameterSet {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, diesel::sqlite::Sqlite>,
        ) -> diesel::serialize::Result {
            out.set_value(self.to_jsonb()?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl diesel::deserialize::FromSql<diesel::sql_types::Jsonb, diesel::sqlite::Sqlite>
        for ParameterSet
    {
        fn from_sql(
            mut bytes: diesel::sqlite::SqliteValue<'_, '_, '_>,
        ) -> diesel::deserialize::Result<Self> {
            Ok(jsonb::to_json(bytes.read_blob())?.parse()?)
        }
    }
}
