use bencher_json::BmfVersion;
use derive_more::Display;
use dropshot::HttpError;

use crate::error::locked_error;

/// The project gate for BMF payload versions, carried through one report's ingest.
///
/// A project names the highest BMF version it accepts, so the gate is a maximum
/// and not an exact match: a project at version 1 still ingests a version 0
/// payload, and a payload that names no version at all.
///
/// A payload states its version twice and both statements are checked against the
/// same gate. The `bmf_version` key the payload declares is checked before anything
/// is created for the report, because later layers hang payload shapes off the
/// declared version and none of them should reach a project that does not accept
/// them. The version the results actually parsed as is checked after parsing,
/// because a payload can reach a v1 leaf without declaring anything: the `json_v1`
/// adapter names the leaf outright, and the `magic` and `json` nodes fall back to
/// it. Checking only the declared key would leave those doors open and make the
/// gate decorative.
#[derive(Debug, Clone, Copy)]
pub struct BmfVersionGate {
    project: BmfVersion,
    declared: BmfVersion,
}

impl BmfVersionGate {
    /// The gate for a payload that declared `declared` on a project that accepts up
    /// to `project`, or the refusal if the project does not accept that version.
    pub fn new(project: BmfVersion, declared: BmfVersion) -> Result<Self, HttpError> {
        check(BmfVersionSource::Declared, declared, project)?;
        Ok(Self { project, declared })
    }

    /// The version the payload declared, which is what orders the adapter attempts.
    pub fn declared(self) -> BmfVersion {
        self.declared
    }

    /// Refuse results that parsed as a version the project does not accept.
    pub fn check_parsed(self, parsed: BmfVersion) -> Result<(), HttpError> {
        check(BmfVersionSource::Parsed, parsed, self.project)
    }
}

/// Where a BMF version the gate refused was stated.
#[derive(Debug, Clone, Copy, Display)]
enum BmfVersionSource {
    #[display("The report payload declared BMF version")]
    Declared,
    #[display("The report results parsed as BMF version")]
    Parsed,
}

/// Both refusals are the same class and both name both versions, the payload's and
/// the project's, so the message says what to change and what to change it to.
fn check(
    source: BmfVersionSource,
    version: BmfVersion,
    project: BmfVersion,
) -> Result<(), HttpError> {
    if version > project {
        return Err(locked_error(format!(
            "{source} {version}, but this project accepts BMF version {project} at most. A server admin can raise the project's `bmf_version`."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bencher_json::BmfVersion;
    use pretty_assertions::assert_eq;

    use super::BmfVersionGate;

    /// A project at the lowest version accepts only that version.
    #[test]
    fn gate_v0_accepts_v0_and_refuses_v1() {
        let gate = BmfVersionGate::new(BmfVersion::V0, BmfVersion::V0).expect("v0 declared on v0");
        assert_eq!(gate.declared(), BmfVersion::V0);
        gate.check_parsed(BmfVersion::V0).expect("v0 parsed on v0");
        gate.check_parsed(BmfVersion::V1)
            .expect_err("v1 parsed on v0");
        BmfVersionGate::new(BmfVersion::V0, BmfVersion::V1).expect_err("v1 declared on v0");
    }

    /// The gate is a maximum, so a raised project still accepts every lower version.
    #[test]
    fn gate_v1_accepts_every_version() {
        let gate = BmfVersionGate::new(BmfVersion::V1, BmfVersion::V0).expect("v0 declared on v1");
        gate.check_parsed(BmfVersion::V0).expect("v0 parsed on v1");
        gate.check_parsed(BmfVersion::V1).expect("v1 parsed on v1");

        let gate = BmfVersionGate::new(BmfVersion::V1, BmfVersion::V1).expect("v1 declared on v1");
        assert_eq!(gate.declared(), BmfVersion::V1);
        gate.check_parsed(BmfVersion::V1).expect("v1 parsed on v1");
    }

    /// Every refusal names the payload's version and the project's.
    #[test]
    fn refusal_names_both_versions() {
        let declared = BmfVersionGate::new(BmfVersion::V0, BmfVersion::V1)
            .expect_err("v1 declared on v0")
            .external_message;
        let parsed = BmfVersionGate::new(BmfVersion::V0, BmfVersion::V0)
            .expect("v0 declared on v0")
            .check_parsed(BmfVersion::V1)
            .expect_err("v1 parsed on v0")
            .external_message;

        for message in [&declared, &parsed] {
            assert!(message.contains('1'), "{message}");
            assert!(message.contains('0'), "{message}");
        }
        assert_ne!(declared, parsed);
    }
}
