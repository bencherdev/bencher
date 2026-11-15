use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[cfg(feature = "db")]
use super::jsonb;
use super::{ParameterSet, ParameterSetInput, ParametersError};

/// The most sets one parameters filter may name as input.
///
/// Only [`ParameterFilterInput`] enforces it, never a read, for the same reason
/// [`MAX_PARAMETER_KEYS`](super::MAX_PARAMETER_KEYS) is only enforced on input.
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
/// The canonical form sorts the sets by their RFC 8785 canonical bytes, drops
/// duplicates, and drops any set that another set of the list already covers, so
/// `[{"a":1},{"a":1.0}]` and `[{"a":1},{"a":1,"b":2}]` are both one set and any
/// spelling of one filter is one stored value.
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
        // A set that another one covers matches nothing that other one does not, so
        // it narrows the filter by nothing. Duplicates are already gone, so no two
        // sets cover each other and this can never drop both.
        let covered = canonical
            .iter()
            .map(|(_, set)| {
                canonical
                    .iter()
                    .any(|(_, smaller)| smaller != set && smaller.is_subset_of(set))
            })
            .collect::<Vec<_>>();
        Self(
            canonical
                .into_iter()
                .zip(covered)
                .filter(|(_, covered)| !covered)
                .map(|((_, set), _)| set)
                .collect(),
        )
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
        Vec::<ParameterSet>::deserialize(deserializer).map(Self::new)
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
        parameter_filter_schema(ParameterSet::json_schema(generator), None)
    }
}

/// A parameters filter as input, which refuses more than [`MAX_FILTER_SETS`] sets
/// and any set past [`MAX_PARAMETER_KEYS`](super::MAX_PARAMETER_KEYS) keys.
#[derive(Debug, Clone, Default)]
pub struct ParameterFilterInput(ParameterFilter);

impl From<ParameterFilterInput> for ParameterFilter {
    fn from(input: ParameterFilterInput) -> Self {
        input.0
    }
}

impl Serialize for ParameterFilterInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ParameterFilterInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The cap bounds what was written, before duplicates collapse, so a filter
        // that spells one set eight times is still eight sets here.
        let sets = Vec::<ParameterSetInput>::deserialize(deserializer)?;
        if sets.len() > MAX_FILTER_SETS {
            return Err(de::Error::custom(format!(
                "A parameters filter may carry at most {MAX_FILTER_SETS} entries, found {}",
                sets.len()
            )));
        }
        Ok(Self(ParameterFilter::new(
            sets.into_iter().map(Into::into).collect(),
        )))
    }
}

#[cfg(feature = "schema")]
impl JsonSchema for ParameterFilterInput {
    fn schema_name() -> String {
        "ParameterFilterInput".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        parameter_filter_schema(
            ParameterSetInput::json_schema(generator),
            u32::try_from(MAX_FILTER_SETS).ok(),
        )
    }
}

#[cfg(feature = "schema")]
fn parameter_filter_schema(
    set: schemars::schema::Schema,
    max_items: Option<u32>,
) -> schemars::schema::Schema {
    use schemars::schema::{ArrayValidation, InstanceType, SchemaObject};

    SchemaObject {
        instance_type: Some(InstanceType::Array.into()),
        array: Some(Box::new(ArrayValidation {
            items: Some(set.into()),
            max_items,
            ..Default::default()
        })),
        ..Default::default()
    }
    .into()
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
    use super::{MAX_FILTER_SETS, ParameterFilter, ParameterFilterInput};
    use crate::{MAX_PARAMETER_KEYS, ParameterSet};

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

    /// A set another set covers is dropped, whichever order the two arrive in.
    #[test]
    fn covered_set_is_dropped() {
        let expected = filter(r#"[{"a": 1}]"#);
        for spelling in [
            r#"[{"a": 1}, {"a": 1, "b": 2}]"#,
            r#"[{"a": 1, "b": 2}, {"a": 1}]"#,
        ] {
            let parsed = filter(spelling);
            assert_eq!(
                parsed, expected,
                "{spelling} canonicalizes to the smaller set"
            );
            assert_eq!(parsed.canonical(), r#"[{"a":1}]"#);
        }
    }

    /// A chain of covers collapses to the one set that covers the rest.
    #[test]
    fn covered_chain_keeps_only_the_smallest() {
        let chain = filter(r#"[{"a": 1, "b": 2, "c": 3}, {"a": 1}, {"a": 1, "b": 2}]"#);
        assert_eq!(chain.sets().len(), 1);
        assert_eq!(chain.canonical(), r#"[{"a":1}]"#);
    }

    /// Two sets that cover neither way both stay.
    #[test]
    fn incomparable_sets_both_stay() {
        let both = filter(r#"[{"b": 2}, {"a": 1}]"#);
        assert_eq!(both.sets().len(), 2);
        assert_eq!(both.canonical(), r#"[{"a":1},{"b":2}]"#);
    }

    /// Covering uses the same value equality the subset match does, so a number
    /// spelled two ways is one value here too.
    #[test]
    fn covering_is_number_spelling_blind() {
        let parsed = filter(r#"[{"a": 1}, {"a": 1.0, "b": 2}]"#);
        assert_eq!(parsed.sets().len(), 1);
        assert_eq!(parsed.canonical(), r#"[{"a":1}]"#);
    }

    /// Dropping a covered set changes no verdict, because the set that covers it
    /// matches every variant it matched.
    #[test]
    fn dropping_a_covered_set_changes_no_verdict() {
        let dropped = filter(r#"[{"a": 1}, {"a": 1, "b": 2}]"#);
        let kept = ParameterFilter(vec![variant(r#"{"a": 1}"#), variant(r#"{"a": 1, "b": 2}"#)]);
        for spelling in [
            "{}",
            r#"{"a": 1}"#,
            r#"{"a": 2}"#,
            r#"{"b": 2}"#,
            r#"{"a": 1, "b": 2}"#,
            r#"{"a": 1, "b": 3}"#,
            r#"{"a": 1, "b": 2, "c": 3}"#,
            r#"{"a": 2, "b": 2}"#,
        ] {
            let variant = variant(spelling);
            assert_eq!(
                dropped.matches(&variant),
                kept.matches(&variant),
                "{spelling} is matched the same either way"
            );
        }
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

    /// Kills a read that enforces either cap, which breaks every client compiled
    /// with a lower cap the day a server raises it.
    #[test]
    fn reads_past_both_caps() {
        let parsed = filter(&sets(MAX_FILTER_SETS + 1, MAX_PARAMETER_KEYS + 1));
        assert_eq!(parsed.sets().len(), MAX_FILTER_SETS + 1);
    }

    /// Kills an input that drops either cap or is off by one. The set cap bounds what
    /// was written, before duplicates collapse.
    #[test]
    fn input_caps_the_set_count_and_the_key_count() {
        let at_caps = input(&sets(MAX_FILTER_SETS, MAX_PARAMETER_KEYS))
            .expect("Failed to parse a filter at both caps");
        assert_eq!(at_caps.sets().len(), MAX_FILTER_SETS);

        input(&sets(MAX_FILTER_SETS + 1, 1)).expect_err("A filter past the set cap is refused");
        input(&sets(1, MAX_PARAMETER_KEYS + 1))
            .expect_err("A filter with a set past the key cap is refused");

        // Eight spellings of one set are eight sets at the cap and one set after.
        let restated = [r#"{"size":512}"#; MAX_FILTER_SETS].join(",");
        let collapsed = input(&format!("[{restated}]")).expect("Failed to parse restated sets");
        assert_eq!(collapsed.sets().len(), 1);
        assert_eq!(collapsed.canonical(), r#"[{"size":512}]"#);
    }

    fn input(filter: &str) -> Result<ParameterFilter, serde_json::Error> {
        serde_json::from_str::<ParameterFilterInput>(filter).map(Into::into)
    }

    /// `count` distinct sets of `keys` keys each.
    fn sets(count: usize, keys: usize) -> String {
        let sets = (0..count)
            .map(|set| {
                let keys = (0..keys)
                    .map(|key| format!(r#""k{key}":{set}"#))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{keys}}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{sets}]")
    }
}
