use std::{fmt, str::FromStr};

use bencher_json::{ProjectUuid, ReportUuid};

const PREFIX: &str = "bencher";
const CI_ID: &str = "c";
const TAG: &str = "t";
const BUILD_TIME: &str = "-build_time";
const FILE_SIZE: &str = "-file_size";

/// What a Bencher check run carries in its `external_id`: `bencher:{Project UUID}:{report UUID}:{kind}`.
/// A field the job does not know yet is empty, so the length stays fixed whatever the section key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalId {
    pub project: Option<ProjectUuid>,
    pub report: Option<ReportUuid>,
    pub kind: Kind,
}

/// How the section of a check run is keyed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// By `--ci-id`, which the check name holds.
    CiId,
    /// By the tag ID of the report, with the sub-adapter suffix these flags name.
    Tag { build_time: bool, file_size: bool },
}

impl ExternalId {
    /// `None` for an `external_id` that is not a Bencher one.
    pub fn parse(external_id: &str) -> Option<Self> {
        let mut fields = external_id
            .strip_prefix(PREFIX)?
            .strip_prefix(':')?
            .split(':');
        let (Some(project), Some(report), Some(kind), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return None;
        };
        Some(Self {
            project: parse_field(project).ok()?,
            report: parse_field(report).ok()?,
            kind: Kind::parse(kind)?,
        })
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            project,
            report,
            kind,
        } = self;
        write!(f, "{PREFIX}:")?;
        if let Some(project) = project {
            write!(f, "{project}")?;
        }
        write!(f, ":")?;
        if let Some(report) = report {
            write!(f, "{report}")?;
        }
        write!(f, ":{kind}")
    }
}

impl Kind {
    pub fn new(ci_id: Option<&str>, sub_adapter: &bencher_comment::SubAdapter) -> Self {
        let bencher_comment::SubAdapter {
            build_time,
            file_size,
        } = *sub_adapter;
        if ci_id.is_some() {
            Self::CiId
        } else {
            Self::Tag {
                build_time,
                file_size,
            }
        }
    }

    fn parse(kind: &str) -> Option<Self> {
        if kind == CI_ID {
            return Some(Self::CiId);
        }
        let suffix = kind.strip_prefix(TAG)?;
        let (build_time, suffix) = suffix
            .strip_prefix(BUILD_TIME)
            .map_or((false, suffix), |rest| (true, rest));
        let (file_size, suffix) = suffix
            .strip_prefix(FILE_SIZE)
            .map_or((false, suffix), |rest| (true, rest));
        suffix.is_empty().then_some(Self::Tag {
            build_time,
            file_size,
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::CiId => write!(f, "{CI_ID}"),
            Self::Tag {
                build_time,
                file_size,
            } => write!(
                f,
                "{TAG}{}{}",
                if build_time { BUILD_TIME } else { "" },
                if file_size { FILE_SIZE } else { "" },
            ),
        }
    }
}

/// A `c` section's report keeps no flags, since its key is not the tag ID.
impl From<Kind> for bencher_comment::SubAdapter {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::CiId => Self {
                build_time: false,
                file_size: false,
            },
            Kind::Tag {
                build_time,
                file_size,
            } => Self {
                build_time,
                file_size,
            },
        }
    }
}

/// An empty field is not known yet.
fn parse_field<T: FromStr>(field: &str) -> Result<Option<T>, T::Err> {
    (!field.is_empty()).then(|| field.parse()).transpose()
}

#[cfg(test)]
mod tests {
    use super::{ExternalId, Kind};

    const PROJECT: &str = "11111111-1111-4111-8111-111111111111";
    const REPORT: &str = "22222222-2222-4222-8222-222222222222";

    fn tag(build_time: bool, file_size: bool) -> Kind {
        Kind::Tag {
            build_time,
            file_size,
        }
    }

    // Kills a format that drops an unknown field instead of leaving it empty, that orders the
    // fields otherwise, or whose suffixes do not follow the comment tag's order.
    #[test]
    fn format() {
        let full = ExternalId {
            project: Some(PROJECT.parse().unwrap()),
            report: Some(REPORT.parse().unwrap()),
            kind: tag(true, true),
        };
        assert_eq!(
            full.to_string(),
            format!("bencher:{PROJECT}:{REPORT}:t-build_time-file_size")
        );
        let started = ExternalId {
            project: Some(PROJECT.parse().unwrap()),
            report: None,
            kind: Kind::CiId,
        };
        assert_eq!(started.to_string(), format!("bencher:{PROJECT}::c"));
        let unknown = ExternalId {
            project: None,
            report: None,
            kind: tag(false, true),
        };
        assert_eq!(unknown.to_string(), "bencher:::t-file_size");
    }

    // Kills a parser that does not read back every kind and every empty field.
    #[test]
    fn parse_reads_back_every_format() {
        for project in [None, Some(PROJECT.parse().unwrap())] {
            for report in [None, Some(REPORT.parse().unwrap())] {
                for kind in [
                    Kind::CiId,
                    tag(false, false),
                    tag(true, false),
                    tag(false, true),
                    tag(true, true),
                ] {
                    let external_id = ExternalId {
                        project,
                        report,
                        kind,
                    };
                    assert_eq!(
                        ExternalId::parse(&external_id.to_string()),
                        Some(external_id)
                    );
                }
            }
        }
    }
}
