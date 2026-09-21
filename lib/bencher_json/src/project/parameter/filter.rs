use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[cfg(feature = "db")]
use super::jsonb;
use super::{ParameterSet, ParametersError};

/// The most sets one parameters filter may name.
///
/// Deliberately low, the same way [`MAX_PARAMETER_KEYS`](super::MAX_PARAMETER_KEYS)
/// is: raising the cap is a release note and lowering it is a breaking change, so
/// the asymmetry runs one way.
pub const MAX_FILTER_SETS: usize = 8;

/// A filter over variants: a list of parameter sets, OR across the list and
/// subset match within each set.
///
/// A variant matches when any set of the list is a subset of it, so a filter
/// names only the keys it cares about and a variant that pins more keys still
/// matches. The empty list matches every variant, and so does a list holding the
/// empty set, because the empty set is a subset of everything: both are match all,
/// and both canonicalize to the same empty list here and to `NULL` in the column.
///
/// The canonical form sorts the sets by their RFC 8785 canonical bytes and drops
/// duplicates, so `[{"a":1},{"a":1.0}]` is one set and any spelling of one filter
/// is one stored value.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Jsonb))]
pub struct ParameterFilter(Vec<ParameterSet>);

impl ParameterFilter {
    /// The canonical filter over these sets.
    #[must_use]
    pub fn new(sets: Vec<ParameterSet>) -> Self {
        // The empty set is a subset of every variant, so a list holding it
        // matches everything the rest of the list could have narrowed.
        if sets.iter().any(ParameterSet::is_empty) {
            return Self(Vec::new());
        }
        let mut canonical = sets
            .into_iter()
            .map(|set| (set.canonical(), set))
            .collect::<Vec<_>>();
        canonical.sort_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
        canonical.dedup_by(|(left, _), (right, _)| left == right);
        Self(canonical.into_iter().map(|(_, set)| set).collect())
    }

    /// Whether this filter matches every variant.
    ///
    /// A match all filter is stored as `NULL`, so this is what decides that.
    #[must_use]
    pub fn is_match_all(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether this filter matches a variant.
    #[must_use]
    pub fn matches(&self, variant: &ParameterSet) -> bool {
        self.is_match_all() || self.0.iter().any(|set| set.is_subset_of(variant))
    }

    /// The sets this filter names, in canonical order.
    #[must_use]
    pub fn sets(&self) -> &[ParameterSet] {
        &self.0
    }

    /// The RFC 8785 (JCS) canonical serialization of this filter.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut canonical = String::from("[");
        for (index, set) in self.0.iter().enumerate() {
            if index > 0 {
                canonical.push(',');
            }
            canonical.push_str(&set.canonical());
        }
        canonical.push(']');
        canonical
    }

    /// The `SQLite` JSONB encoding of the canonical form.
    ///
    /// A filter is only ever written by this encoder, never minted in SQL, but it
    /// is read back through `SQLite`'s own JSON functions, so it is encoded the way
    /// `SQLite` encodes a JSON array of the same objects.
    #[cfg(feature = "db")]
    pub fn to_jsonb(&self) -> Result<Vec<u8>, jsonb::JsonbError> {
        let mut array = jsonb::Array::default();
        for set in &self.0 {
            array.push(&set.to_jsonb()?);
        }
        array.into_blob()
    }
}

impl fmt::Display for ParameterFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

impl FromStr for ParameterFilter {
    type Err = ParametersError;

    fn from_str(filter: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(filter).map_err(ParametersError::Json)
    }
}

impl Serialize for ParameterFilter {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ParameterFilter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The cap bounds what was written, before duplicates collapse, so a filter
        // that spells one set eight times is still eight sets here.
        let sets = Vec::<ParameterSet>::deserialize(deserializer)?;
        if sets.len() > MAX_FILTER_SETS {
            return Err(de::Error::custom(format!(
                "A parameters filter may carry at most {MAX_FILTER_SETS} entries, found {}",
                sets.len()
            )));
        }
        Ok(Self::new(sets))
    }
}

/// A list of parameter sets.
///
/// Written out by hand for the same reason [`ParameterSet`]'s is: the element has
/// a hand written schema, and a newtype over a `Vec` does not derive to the array
/// the wire carries.
#[cfg(feature = "schema")]
impl JsonSchema for ParameterFilter {
    fn schema_name() -> String {
        "ParameterFilter".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{ArrayValidation, InstanceType, SchemaObject};

        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(ParameterSet::json_schema(generator).into()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

/// The JSONB encoding is `SQLite`'s, so these impls are too.
///
/// Every other backend spells `Jsonb` differently, and a generic impl would be
/// claiming an encoding it does not have.
#[cfg(feature = "db")]
mod db {
    use super::{ParameterFilter, jsonb};

    impl diesel::serialize::ToSql<diesel::sql_types::Jsonb, diesel::sqlite::Sqlite>
        for ParameterFilter
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, diesel::sqlite::Sqlite>,
        ) -> diesel::serialize::Result {
            out.set_value(self.to_jsonb()?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl diesel::deserialize::FromSql<diesel::sql_types::Jsonb, diesel::sqlite::Sqlite>
        for ParameterFilter
    {
        fn from_sql(
            mut bytes: diesel::sqlite::SqliteValue<'_, '_, '_>,
        ) -> diesel::deserialize::Result<Self> {
            Ok(jsonb::to_json(bytes.read_blob())?.parse()?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_FILTER_SETS, ParameterFilter};
    use crate::ParameterSet;

    fn filter(filter: &str) -> ParameterFilter {
        filter.parse().expect("Invalid parameters filter")
    }

    fn variant(variant: &str) -> ParameterSet {
        variant.parse().expect("Invalid parameter set")
    }

    /// One filter has one canonical spelling: the sets sort by their canonical bytes
    /// and duplicates collapse, so a number written two ways is one set.
    #[test]
    fn filter_is_canonicalized() {
        let one_way = filter(r#"[{"a": 1}, {"a": 1.0}]"#);
        assert_eq!(one_way.sets().len(), 1);
        assert_eq!(one_way.canonical(), r#"[{"a":1}]"#);

        let sorted = filter(r#"[{"b": 2}, {"a": 1}]"#);
        assert_eq!(sorted.canonical(), r#"[{"a":1},{"b":2}]"#);
        assert_eq!(sorted, filter(r#"[{"a": 1}, {"b": 2}]"#));
    }

    /// A set matches a variant that pins at least its keys, so a filter narrows on
    /// the keys it names and ignores the rest.
    #[test]
    fn matches_is_subset_within_a_set() {
        let one = filter(r#"[{"size": 512}]"#);
        assert!(one.matches(&variant(r#"{"size": 512}"#)));
        assert!(one.matches(&variant(r#"{"size": 512, "threads": 4}"#)));
        assert!(!one.matches(&variant(r#"{"size": 1024}"#)));
        assert!(!one.matches(&variant(r#"{"threads": 4}"#)));
        assert!(!one.matches(&ParameterSet::default()));
    }

    /// The list is an OR: a variant matches when any one set does.
    #[test]
    fn matches_is_or_across_sets() {
        let either = filter(r#"[{"size": 512}, {"threads": 4}]"#);
        assert!(either.matches(&variant(r#"{"size": 512}"#)));
        assert!(either.matches(&variant(r#"{"threads": 4}"#)));
        assert!(either.matches(&variant(r#"{"size": 512, "threads": 4}"#)));
        assert!(!either.matches(&variant(r#"{"size": 1024, "threads": 8}"#)));
    }

    /// Both spellings of match all match every variant and report themselves as such.
    #[test]
    fn matches_everything_when_match_all() {
        for spelling in ["[]", "[{}]", r#"[{"size": 512}, {}]"#] {
            let all = filter(spelling);
            assert!(all.is_match_all(), "{spelling} is match all");
            assert!(all.sets().is_empty(), "{spelling} canonicalizes to no sets");
            assert!(all.matches(&ParameterSet::default()));
            assert!(all.matches(&variant(r#"{"size": 1024}"#)));
        }
        assert!(!filter(r#"[{"size": 512}]"#).is_match_all());
    }

    /// `Display` writes the canonical form and `FromStr` reads it back.
    #[test]
    fn display_and_from_str_round_trip() {
        for spelling in [
            "[]",
            r#"[{"size":512}]"#,
            r#"[{"size":1024,"threads":4},{"size":512}]"#,
        ] {
            let parsed = filter(spelling);
            assert_eq!(parsed.to_string(), spelling);
            assert_eq!(filter(&parsed.to_string()), parsed);
        }
        // A non canonical spelling still lands on the canonical one.
        assert_eq!(
            filter(r#"[{"b": 2}, {"a": 1.0}]"#).to_string(),
            r#"[{"a":1},{"b":2}]"#
        );
    }

    /// Serializing emits the canonical order, not the order the wire gave.
    #[test]
    fn serialize_emits_canonical_order() {
        let parsed = filter(r#"[{"size": 512}, {"a": 1}]"#);
        assert_eq!(
            serde_json::to_string(&parsed).expect("Failed to serialize"),
            r#"[{"a":1},{"size":512}]"#
        );
    }

    /// The cap bounds what was written, before duplicates collapse.
    #[test]
    fn deserialize_caps_the_set_count() {
        let at_cap = (0..MAX_FILTER_SETS)
            .map(|index| format!(r#"{{"k{index}":1}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let parsed = filter(&format!("[{at_cap}]"));
        assert_eq!(parsed.sets().len(), MAX_FILTER_SETS);

        let over_cap = (0..=MAX_FILTER_SETS)
            .map(|index| format!(r#"{{"k{index}":1}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!("[{over_cap}]")
            .parse::<ParameterFilter>()
            .expect_err("A filter past the cap is refused");

        // Eight spellings of one set are eight sets at the cap and one set after.
        let restated = [r#"{"size":512}"#; MAX_FILTER_SETS].join(",");
        let collapsed = filter(&format!("[{restated}]"));
        assert_eq!(collapsed.sets().len(), 1);
        assert_eq!(collapsed.canonical(), r#"[{"size":512}]"#);
    }
}
