use bencher_client::types::JsonUpdateStartPoint;
use bencher_json::{BranchNameId, GitHash, HeadIdempotencyKey};

use crate::parser::project::branch::CliStartPointUpdate;

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: Option<BranchNameId>,
    pub hash: Option<GitHash>,
    pub max_versions: u32,
    pub clone_thresholds: bool,
    pub reset: bool,
    pub idempotency_key: Option<HeadIdempotencyKey>,
}

impl From<CliStartPointUpdate> for StartPoint {
    fn from(start_point: CliStartPointUpdate) -> Self {
        let CliStartPointUpdate {
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
            start_point_idempotency_key,
        } = start_point;
        Self {
            branch: start_point_branch,
            hash: start_point_hash,
            max_versions: start_point_max_versions,
            clone_thresholds: start_point_clone_thresholds,
            reset: start_point_reset,
            idempotency_key: reset_idempotency_key(
                start_point_reset,
                start_point_idempotency_key.as_deref(),
                None,
            ),
        }
    }
}

impl From<StartPoint> for Option<JsonUpdateStartPoint> {
    fn from(start_point: StartPoint) -> Self {
        let StartPoint {
            branch,
            hash,
            max_versions,
            clone_thresholds,
            reset,
            idempotency_key,
        } = start_point;
        (branch.is_some() || reset).then(|| JsonUpdateStartPoint {
            branch: branch.map(Into::into),
            hash: hash.map(Into::into),
            max_versions: Some(max_versions),
            clone_thresholds: Some(clone_thresholds),
            reset: Some(reset),
            idempotency_key: idempotency_key.map(|key| uuid::Uuid::from(key).into()),
        })
    }
}

/// The key of a reset: the given key, else the key of the CI run, as a UUID.
/// A key without a reset has no effect, so none is sent, and a blank key is no key.
pub fn reset_idempotency_key(
    reset: bool,
    key: Option<&str>,
    ci_key: Option<&str>,
) -> Option<HeadIdempotencyKey> {
    if !reset {
        return None;
    }
    let key = key.filter(|key| !key.trim_ascii().is_empty()).or(ci_key)?;
    Some(uuid::Uuid::new_v5(&IDEMPOTENCY_KEY_NAMESPACE, key.as_bytes()).into())
}

// The jobs of one CI run share a key only while their CLI versions share this namespace.
const IDEMPOTENCY_KEY_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x59a0_1aa3_fba1_44cb_90a9_34ba_792f_758b);

#[cfg(test)]
mod tests {
    use super::reset_idempotency_key;

    #[test]
    fn no_key_without_a_reset() {
        assert_eq!(reset_idempotency_key(false, Some("key"), Some("ci")), None);
    }

    #[test]
    fn the_given_key_wins_over_the_ci_key() {
        let given = reset_idempotency_key(true, Some("key"), Some("ci"));
        assert!(given.is_some());
        assert_eq!(given, reset_idempotency_key(true, Some("key"), None));
        assert_ne!(given, reset_idempotency_key(true, None, Some("ci")));
    }

    #[test]
    fn a_blank_key_is_no_key() {
        for blank in ["", "  "] {
            assert_eq!(reset_idempotency_key(true, Some(blank), None), None);
            assert_eq!(
                reset_idempotency_key(true, Some(blank), Some("ci")),
                reset_idempotency_key(true, None, Some("ci"))
            );
        }
    }
}
