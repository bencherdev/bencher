use std::collections::{BTreeMap, btree_map::Entry};

use bencher_comment::{ReportComment, SectionCheck};
use bencher_json::ProjectUuid;

use super::{
    BENCHER_REPORT,
    external_id::{ExternalId, Kind},
};

/// The key of a section. Keys of the two kinds never collide.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectionKey {
    /// `--ci-id`, as the check name holds it.
    CiId(String),
    /// The tag ID of the report.
    Tag(String),
}

impl SectionKey {
    /// A `t` section needs its report for a key, so without one it is not a section.
    pub fn new(kind: Kind, check: &SectionCheck, report: Option<&ReportComment>) -> Option<Self> {
        match kind {
            Kind::CiId => ci_key(&check.name).map(|id| Self::CiId(id.to_owned())),
            Kind::Tag { .. } => report.map(|report| Self::Tag(report.tag_id(None))),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::CiId(label) | Self::Tag(label) => label,
        }
    }
}

/// The Project's other check runs, one per ID, preferring completed, then one naming its report, then the list.
pub fn by_check_run(
    listed: Vec<SectionCheck>,
    recorded: Vec<SectionCheck>,
    project: ProjectUuid,
    own: u64,
) -> Vec<Candidate> {
    let mut by_id = BTreeMap::new();
    for check in recorded.into_iter().chain(listed) {
        let Some(external_id) = ExternalId::parse(&check.external_id) else {
            continue;
        };
        if check.id == own || external_id.project != Some(project) {
            continue;
        }
        let candidate = Candidate { check, external_id };
        match by_id.entry(candidate.check.id) {
            Entry::Vacant(entry) => {
                entry.insert(candidate);
            },
            Entry::Occupied(mut entry) => {
                if candidate.progress() >= entry.get().progress() {
                    entry.insert(candidate);
                }
            },
        }
    }
    by_id.into_values().collect()
}

/// A check run of the Project with its `external_id` read.
pub struct Candidate {
    pub check: SectionCheck,
    pub external_id: ExternalId,
}

/// The newest check run of each key, other than the writer's own key,
/// or `None` when a newer check run than the writer's holds its key.
pub fn by_key(
    keyed: Vec<(SectionKey, SectionCheck)>,
    own_key: &SectionKey,
    own: u64,
) -> Option<BTreeMap<SectionKey, SectionCheck>> {
    let mut sections = BTreeMap::new();
    for (key, check) in keyed {
        match sections.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(check);
            },
            Entry::Occupied(mut entry) => {
                if check.id > entry.get().id {
                    entry.insert(check);
                }
            },
        }
    }
    let stale = sections
        .remove(own_key)
        .is_some_and(|newest: SectionCheck| newest.id > own);
    (!stale).then_some(sections)
}

impl Candidate {
    fn progress(&self) -> (bool, bool) {
        (
            self.check.conclusion.is_some(),
            self.external_id.report.is_some(),
        )
    }
}

/// The ID inside `Bencher Report ({id})`, which may itself hold parentheses.
fn ci_key(name: &str) -> Option<&str> {
    name.strip_prefix(BENCHER_REPORT)?
        .strip_prefix(" (")?
        .strip_suffix(')')
}

#[cfg(test)]
mod tests {
    use std::slice;

    use bencher_comment::SectionCheck;
    use bencher_json::ProjectUuid;

    use super::{SectionKey, by_check_run, by_key, ci_key};

    const PROJECT: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER_PROJECT: &str = "33333333-3333-4333-8333-333333333333";
    const REPORT: &str = "22222222-2222-4222-8222-222222222222";

    fn project() -> ProjectUuid {
        PROJECT.parse().unwrap()
    }

    fn check(id: u64, conclusion: Option<&str>, external_id: &str) -> SectionCheck {
        SectionCheck {
            id,
            name: format!("Bencher Report ({id})"),
            conclusion: conclusion.map(str::to_owned),
            external_id: external_id.to_owned(),
        }
    }

    fn merged(listed: Vec<SectionCheck>, recorded: Vec<SectionCheck>) -> Vec<SectionCheck> {
        by_check_run(listed, recorded, project(), 1)
            .into_iter()
            .map(|candidate| candidate.check)
            .collect()
    }

    // Kills a key cut at the first `)`, or taken from a name that is not a Bencher check's.
    #[test]
    fn ci_key_keeps_parentheses_inside() {
        assert_eq!(ci_key("Bencher Report (a (b))"), Some("a (b)"));
        assert_eq!(ci_key("Bencher Report ((x)"), Some("(x"));
        assert_eq!(ci_key("Bencher Report"), None);
        assert_eq!(ci_key("Other Report (a)"), None);
    }

    // Kills a merge in which a late in-progress list replaces a completed record,
    // or a body record replaces a completed listed check.
    #[test]
    fn completed_wins_over_in_progress() {
        let started = format!("bencher:{PROJECT}::c");
        let done = format!("bencher:{PROJECT}:{REPORT}:c");
        let recorded = check(2, Some("success"), &done);
        let late = check(2, None, &started);
        assert_eq!(
            merged(vec![late.clone()], vec![recorded.clone()]),
            slice::from_ref(&recorded)
        );
        assert_eq!(merged(vec![recorded.clone()], vec![late]), [recorded]);
    }

    // Kills a merge that drops the report a check named, or that lets the body win a tie.
    #[test]
    fn a_known_report_wins_then_the_list() {
        let started = check(2, None, &format!("bencher:{PROJECT}::t"));
        let submitted = check(2, None, &format!("bencher:{PROJECT}:{REPORT}:t"));
        assert_eq!(
            merged(vec![started], vec![submitted.clone()]),
            slice::from_ref(&submitted)
        );
        let renamed = SectionCheck {
            name: "Bencher Report (renamed)".to_owned(),
            ..submitted.clone()
        };
        assert_eq!(merged(vec![renamed.clone()], vec![submitted]), [renamed]);
    }

    // Kills a merge that keeps the writer's own check, another Project's, one with no Project,
    // or one that is not Bencher's, and one that drops a check only the body holds.
    #[test]
    fn only_other_checks_of_the_project() {
        let listed_only = check(3, Some("success"), &format!("bencher:{PROJECT}:{REPORT}:c"));
        let recorded = check(4, Some("failure"), &format!("bencher:{PROJECT}:{REPORT}:c"));
        let listed = vec![
            check(1, Some("success"), &format!("bencher:{PROJECT}:{REPORT}:c")),
            check(
                5,
                Some("success"),
                &format!("bencher:{OTHER_PROJECT}:{REPORT}:c"),
            ),
            check(6, None, "bencher:::t"),
            check(7, Some("success"), "ci-run-7"),
            listed_only.clone(),
        ];
        assert_eq!(
            merged(listed, vec![recorded.clone()]),
            [listed_only, recorded]
        );
    }

    fn keyed(key: &SectionKey, id: u64) -> (SectionKey, SectionCheck) {
        (key.clone(), check(id, Some("success"), ""))
    }

    // Kills a key that keeps an older check run over a rerun, or that lets a `c` key and a
    // `t` key of one ID collide.
    #[test]
    fn the_newest_check_run_of_each_key() {
        let ci = SectionKey::CiId("x".to_owned());
        let tag = SectionKey::Tag("x".to_owned());
        let own = SectionKey::CiId("own".to_owned());
        let sections = by_key(
            vec![keyed(&ci, 7), keyed(&ci, 9), keyed(&ci, 8), keyed(&tag, 2)],
            &own,
            5,
        )
        .unwrap();
        let ids = sections
            .iter()
            .map(|(key, check)| (key.clone(), check.id))
            .collect::<Vec<_>>();
        assert_eq!(ids, [(ci, 9), (tag, 2)]);
    }

    // Kills a writer that renders over a newer check run of its key, that steps aside for an
    // older one, or that keeps an older run of its key as a second section.
    #[test]
    fn a_newer_check_run_of_the_writer_s_key_makes_it_stale() {
        let own = SectionKey::Tag("main/base/magic".to_owned());
        let other = SectionKey::Tag("main/other/magic".to_owned());
        assert_eq!(by_key(vec![keyed(&own, 6)], &own, 5), None);
        let sections = by_key(vec![keyed(&own, 4), keyed(&other, 9)], &own, 5).unwrap();
        assert_eq!(sections.keys().collect::<Vec<_>>(), [&other]);
    }
}
