use std::{collections::BTreeMap, fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(ParameterUuid);

/// A benchmark parameter set: the permutation of inputs that a benchmark ran with.
///
/// The canonical form is [RFC 8785][jcs] (JSON Canonicalization Scheme):
/// object keys sorted by UTF-16 code unit, ECMAScript number formatting,
/// and no insignificant whitespace.
/// Canonicalization happens here, before the write,
/// so the database's `UNIQUE(benchmark_id, parameters)` constraint
/// is the enforcement point for canonical equality.
///
/// [jcs]: https://www.rfc-editor.org/rfc/rfc8785
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Json))]
pub struct JsonParameters(BTreeMap<String, serde_json::Value>);

impl JsonParameters {
    /// The RFC 8785 (JCS) canonical serialization of this parameter set.
    pub fn canonical(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_default()
    }

    /// Whether this is the empty parameter set.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for JsonParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

impl FromStr for JsonParameters {
    type Err = ParametersError;

    fn from_str(parameters: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(parameters).map_err(ParametersError::Json)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParametersError {
    #[error("Failed to parse benchmark parameters: {0}")]
    Json(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::JsonParameters;

    fn canonical(parameters: &str) -> String {
        parameters
            .parse::<JsonParameters>()
            .expect("Failed to parse parameters")
            .canonical()
    }

    #[test]
    fn canonical_empty_set() {
        assert_eq!(canonical("{}"), "{}");
        assert_eq!(JsonParameters::default().canonical(), "{}");
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
        let parameters = canonical(
            r#"{
                "\u20ac": "Euro Sign",
                "\r": "Carriage Return",
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
                r#"{"\r":"Carriage Return","1":"One","#,
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
                parameters.parse::<JsonParameters>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_non_object_payloads() {
        for parameters in ["null", "[]", "1", r#""a""#, "true"] {
            assert!(
                parameters.parse::<JsonParameters>().is_err(),
                "expected {parameters} to be rejected"
            );
        }
    }

    #[test]
    fn accepts_scalar_values() {
        let parameters = r#"{"s": "a", "n": 1, "b": true}"#
            .parse::<JsonParameters>()
            .expect("Failed to parse parameters");
        assert!(!parameters.is_empty());
    }
}

#[cfg(feature = "db")]
mod db {
    use super::JsonParameters;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Json, DB> for JsonParameters
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(self.canonical());
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Json, DB> for JsonParameters
    where
        DB: diesel::backend::Backend,
        String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(String::from_sql(bytes)?.parse()?)
        }
    }
}
