use bencher_json::{GitHash, ProjectSlug};

pub(crate) const TAG_START: &str = r#"<div id="bencher.dev/projects/"#;
const TAG_END: &str = r#""></div>"#;
const GROUP: &str = "/sha/";
const JOB: &str = "/id/";

/// The tag that ends the one comment every job of a Project writes for a head SHA.
pub fn group_tag(project: &ProjectSlug, head_sha: &GitHash) -> String {
    format!("{TAG_START}{project}{GROUP}{head_sha}{TAG_END}")
}

pub(crate) fn job_tag(project: &ProjectSlug, id: &str) -> String {
    format!("{TAG_START}{project}{JOB}{id}{TAG_END}")
}

/// The Bencher tag that a comment body ends with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentTag {
    /// The comment every job of the Project writes, last written for this head SHA.
    Group(GitHash),
    /// The comment of a single job.
    Job,
}

impl CommentTag {
    /// The tag of `project` that `body` ends with, if any.
    pub fn parse(body: &str, project: &ProjectSlug) -> Option<Self> {
        let project: &str = project.as_ref();
        let (_, id) = body.strip_suffix(TAG_END)?.rsplit_once(TAG_START)?;
        let tag = id.strip_prefix(project)?;
        if let Some(head_sha) = tag.strip_prefix(GROUP) {
            head_sha.parse().ok().map(Self::Group)
        } else {
            tag.starts_with(JOB).then_some(Self::Job)
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{GitHash, ProjectSlug};

    use super::{CommentTag, group_tag, job_tag};

    fn head_sha() -> GitHash {
        "0123456789abcdef0123456789abcdef01234567".parse().unwrap()
    }

    // Kills a match on the slug prefix alone, without the separator after it.
    #[test]
    fn slug_does_not_match_a_longer_slug() {
        let foo: ProjectSlug = "foo".parse().unwrap();
        let foo_bar: ProjectSlug = "foo-bar".parse().unwrap();

        let group = format!("<h2>Report</h2>{}", group_tag(&foo_bar, &head_sha()));
        assert_eq!(CommentTag::parse(&group, &foo), None);
        assert_eq!(
            CommentTag::parse(&group, &foo_bar),
            Some(CommentTag::Group(head_sha()))
        );

        let job = format!(
            "<h2>Report</h2>{}",
            job_tag(&foo_bar, "main/localhost/magic")
        );
        assert_eq!(CommentTag::parse(&job, &foo), None);
        assert_eq!(CommentTag::parse(&job, &foo_bar), Some(CommentTag::Job));
    }

    // Kills reading one tag as the other, and finding a tag that is not the end of the body.
    #[test]
    fn group_and_job_tags_are_told_apart() {
        let project: ProjectSlug = "foo".parse().unwrap();

        let group = format!("<h2>Report</h2>{}", group_tag(&project, &head_sha()));
        assert_eq!(
            CommentTag::parse(&group, &project),
            Some(CommentTag::Group(head_sha()))
        );

        // A job tag whose ID reads like a group tag is still a job tag.
        let job = format!(
            "<h2>Report</h2>{}",
            job_tag(&project, &format!("sha/{}", head_sha()))
        );
        assert_eq!(CommentTag::parse(&job, &project), Some(CommentTag::Job));

        assert_eq!(
            CommentTag::parse(&format!("{group}<p>edited</p>"), &project),
            None
        );
    }
}
