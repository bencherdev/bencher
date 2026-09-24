use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::{CallbackError, MAX_CALLBACK_BODY_LEN};
use crate::runner::{JobStatus, JobUuid};
use crate::{JsonReport, ProjectSlug, ProjectUuid, ReportUuid, ResourceName};

const REPORT_NOT_BUILT: &str = "the callback body's report was not built";
const NOT_TEXT: &str = "a callback body placeholder's value is not text";

/// The values a callback body's placeholders take, all but the report, which is built only when
/// the body sends it.
#[derive(Debug, Clone)]
pub struct CallbackContext {
    pub project_uuid: ProjectUuid,
    pub project_name: ResourceName,
    pub project_slug: ProjectSlug,
    pub report_uuid: ReportUuid,
    pub job_uuid: JobUuid,
    pub job_status: JobStatus,
}

pub(super) fn render(
    body: Option<&Value>,
    context: &CallbackContext,
    report: Option<&JsonReport>,
) -> serde_json::Result<String> {
    let Some(value) = body else {
        return serde_json::to_string(
            report.ok_or_else(|| serde_json::Error::custom(REPORT_NOT_BUILT))?,
        );
    };
    serde_json::to_string(&Rendered {
        value,
        context,
        report,
    })
}

/// Refuses a body over the limit as written, a placeholder with an unknown name, the report inside a
/// longer string, and a second report, which would send the report again with nothing counted for it.
pub(super) fn validate_body(body: &Value) -> Result<(), CallbackError> {
    let len = body.to_string().len();
    if len > MAX_CALLBACK_BODY_LEN {
        return Err(CallbackError::BodyTooLong(len));
    }
    let mut reports = 0;
    let refused = find_map_string(body, &mut |text| {
        for piece in pieces(text) {
            let Piece::Token(token) = piece else {
                continue;
            };
            match Placeholder::new(token.name) {
                None => return Some(CallbackError::UnknownPlaceholder(token.text.to_owned())),
                Some(Placeholder::Report) if token.text != text => {
                    return Some(CallbackError::ReportInText);
                },
                Some(Placeholder::Report) => {
                    reports += 1;
                    if reports > 1 {
                        return Some(CallbackError::RepeatedReport);
                    }
                },
                Some(
                    Placeholder::JobUuid
                    | Placeholder::JobStatus
                    | Placeholder::ReportUuid
                    | Placeholder::ProjectUuid
                    | Placeholder::ProjectName
                    | Placeholder::ProjectSlug,
                ) => {},
            }
        }
        None
    });
    refused.map_or(Ok(()), Err)
}

pub(super) fn sends_report(body: &Value) -> bool {
    find_map_string(body, &mut |text| is_report(text).then_some(())).is_some()
}

/// Whether the whole string is the report placeholder.
fn is_report(text: &str) -> bool {
    matches!(
        token(text),
        Some((Token { name, .. }, "")) if Placeholder::new(name) == Some(Placeholder::Report)
    )
}

/// The first value `f` finds in a string value of the body.
fn find_map_string<T, F>(value: &Value, f: &mut F) -> Option<T>
where
    F: FnMut(&str) -> Option<T>,
{
    match value {
        Value::String(text) => f(text),
        Value::Array(items) => items.iter().find_map(|item| find_map_string(item, f)),
        Value::Object(fields) => fields.values().find_map(|item| find_map_string(item, f)),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

/// Placeholder-shaped text: `{{`, optional ASCII whitespace, a name, optional ASCII whitespace,
/// and `}}`.
struct Token<'a> {
    /// The token as written.
    text: &'a str,
    name: &'a str,
}

/// A string, in order, as the text between its tokens and the tokens.
enum Piece<'a> {
    Text(&'a str),
    Token(Token<'a>),
}

fn pieces(text: &str) -> impl Iterator<Item = Piece<'_>> {
    let mut rest = text;
    let mut next = None;
    std::iter::from_fn(move || {
        if let Some(token) = next.take() {
            return Some(Piece::Token(token));
        }
        let mut search = 0;
        while let Some(offset) = rest.get(search..).and_then(|tail| tail.find("{{")) {
            let (before, from_braces) = rest.split_at_checked(search + offset)?;
            if let Some((token, after)) = token(from_braces) {
                rest = after;
                if before.is_empty() {
                    return Some(Piece::Token(token));
                }
                next = Some(token);
                return Some(Piece::Text(before));
            }
            search += offset + 1;
        }
        (!rest.is_empty()).then(|| Piece::Text(std::mem::take(&mut rest)))
    })
}

/// The token at the start of `text`, and the text after it.
fn token(text: &str) -> Option<(Token<'_>, &str)> {
    let inner = text.strip_prefix("{{")?.trim_ascii_start();
    let name_len = inner.bytes().take_while(|byte| is_name_byte(*byte)).count();
    let (name, after_name) = inner.split_at_checked(name_len)?;
    if name.is_empty() {
        return None;
    }
    let after = after_name.trim_ascii_start().strip_prefix("}}")?;
    let (token, _) = text.split_at_checked(text.len() - after.len())?;
    Some((Token { text: token, name }, after))
}

/// A name can be any of these, so a misspelled or miscased placeholder is refused, not sent.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

/// A name the body can use, and the value it is sent as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Placeholder {
    JobUuid,
    JobStatus,
    ReportUuid,
    ProjectUuid,
    ProjectName,
    ProjectSlug,
    Report,
}

impl Placeholder {
    fn new(name: &str) -> Option<Self> {
        Some(match name {
            "job.uuid" => Self::JobUuid,
            "job.status" => Self::JobStatus,
            "report.uuid" => Self::ReportUuid,
            "project.uuid" => Self::ProjectUuid,
            "project.name" => Self::ProjectName,
            "project.slug" => Self::ProjectSlug,
            "report" => Self::Report,
            _ => return None,
        })
    }

    /// The text of a placeholder other than the report, as its value serializes.
    fn text(self, context: &CallbackContext) -> serde_json::Result<String> {
        let CallbackContext {
            project_uuid,
            project_name,
            project_slug,
            report_uuid,
            job_uuid,
            job_status,
        } = context;
        let value = match self {
            Self::JobUuid => serde_json::to_value(job_uuid),
            Self::JobStatus => serde_json::to_value(job_status),
            Self::ReportUuid => serde_json::to_value(report_uuid),
            Self::ProjectUuid => serde_json::to_value(project_uuid),
            Self::ProjectName => serde_json::to_value(project_name),
            Self::ProjectSlug => serde_json::to_value(project_slug),
            Self::Report => return Err(serde_json::Error::custom(NOT_TEXT)),
        }?;
        match value {
            Value::String(text) => Ok(text),
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Array(_)
            | Value::Object(_) => Err(serde_json::Error::custom(NOT_TEXT)),
        }
    }
}

/// The string with each placeholder replaced by its text, in one pass, so a value is never
/// searched for placeholders itself.
fn render_text(text: &str, context: &CallbackContext) -> serde_json::Result<String> {
    let mut rendered = String::new();
    for piece in pieces(text) {
        match piece {
            Piece::Text(text) => rendered.push_str(text),
            Piece::Token(Token { text, name }) => match Placeholder::new(name) {
                Some(placeholder) => rendered.push_str(&placeholder.text(context)?),
                None => rendered.push_str(text),
            },
        }
    }
    Ok(rendered)
}

/// The body as sent: every string with its placeholders replaced, and the report as itself, so it
/// keeps the field order the report endpoint gives it.
struct Rendered<'a> {
    value: &'a Value,
    context: &'a CallbackContext,
    report: Option<&'a JsonReport>,
}

impl Serialize for Rendered<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let Self {
            value,
            context,
            report,
        } = *self;
        let rendered = |value| Rendered {
            value,
            context,
            report,
        };
        match value {
            Value::String(text) if is_report(text) => report
                .ok_or_else(|| S::Error::custom(REPORT_NOT_BUILT))?
                .serialize(serializer),
            Value::String(text) => {
                serializer.serialize_str(&render_text(text, context).map_err(S::Error::custom)?)
            },
            Value::Array(items) => serializer.collect_seq(items.iter().map(rendered)),
            Value::Object(fields) => {
                serializer.collect_map(fields.iter().map(|(key, value)| (key, rendered(value))))
            },
            Value::Null | Value::Bool(_) | Value::Number(_) => value.serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    use super::CallbackContext;
    use crate::runner::callback::{CallbackError, JsonNewCallback, MAX_CALLBACK_BODY_LEN};
    use crate::{JobStatus, JobUuid, JsonReport, ProjectUuid, ReportUuid, ResourceName};

    const URL: &str = "https://example.com/hook";
    const TERMINAL: [JobStatus; 3] = [JobStatus::Processed, JobStatus::Failed, JobStatus::Canceled];
    const JOB: &str = "00000000-0000-0000-0000-00000000000a";
    const REPORT: &str = "00000000-0000-0000-0000-00000000000b";
    const PROJECT: &str = "00000000-0000-0000-0000-00000000000c";

    fn with_body(body: Value) -> Result<JsonNewCallback, CallbackError> {
        JsonNewCallback::new(URL, [], Some(body))
    }

    fn context(status: JobStatus) -> CallbackContext {
        named("My Project", status)
    }

    fn named(project_name: &str, status: JobStatus) -> CallbackContext {
        CallbackContext {
            project_uuid: PROJECT.parse::<ProjectUuid>().unwrap(),
            project_name: project_name.parse::<ResourceName>().unwrap(),
            project_slug: "my-project".parse().unwrap(),
            report_uuid: REPORT.parse::<ReportUuid>().unwrap(),
            job_uuid: JOB.parse::<JobUuid>().unwrap(),
            job_status: status,
        }
    }

    /// A report whose fields are not in alphabetical order, so a render that sorts them shows.
    fn json_report() -> JsonReport {
        let time = "2026-01-01T00:00:00Z";
        serde_json::from_value(json!({
            "uuid": REPORT,
            "user": null,
            "project": {
                "uuid": PROJECT,
                "organization": "00000000-0000-0000-0000-00000000000d",
                "name": "My Project",
                "slug": "my-project",
                "visibility": "public",
                "bmf_version": 1,
                "created": time,
                "modified": time,
            },
            "branch": {
                "uuid": "00000000-0000-0000-0000-00000000000e",
                "project": PROJECT,
                "name": "main",
                "slug": "main",
                "head": { "uuid": "00000000-0000-0000-0000-00000000000f", "created": time },
                "created": time,
                "modified": time,
            },
            "testbed": {
                "uuid": "00000000-0000-0000-0000-000000000010",
                "project": PROJECT,
                "name": "localhost",
                "slug": "localhost",
                "created": time,
                "modified": time,
            },
            "start_time": time,
            "end_time": time,
            "adapter": "magic",
            "results": [],
            "alerts": [],
            "job": JOB,
            "created": time,
        }))
        .unwrap()
    }

    fn render(callback: &JsonNewCallback, context: &CallbackContext) -> String {
        callback.render(context, Some(&json_report())).unwrap()
    }

    fn render_json(callback: &JsonNewCallback, status: JobStatus) -> Value {
        serde_json::from_str(&render(callback, &context(status))).unwrap()
    }

    fn unknown_placeholder(body: Value) -> String {
        let err = with_body(body).unwrap_err();
        let CallbackError::UnknownPlaceholder(text) = err else {
            panic!("expected an unknown placeholder, got {err}");
        };
        text
    }

    #[test]
    fn refuses_an_unknown_name_by_its_text() {
        for body in [
            json!("{{ job.id }}"),
            json!({ "job": "{{ job.id }}" }),
            json!({ "a": [0, { "b": "{{ job.id }}" }] }),
            json!([[], ["x", "{{ job.id }}"]]),
            json!({ "a": "{{ job.uuid }}", "b": "{{ job.id }}" }),
            json!({ "a": "see {{ job.id }} now" }),
            json!({ "a": "{{ job.uuid }} and {{ job.id }}" }),
        ] {
            assert_eq!(unknown_placeholder(body.clone()), "{{ job.id }}", "{body}");
        }
    }

    /// A name is ASCII letters, digits, `.`, `_`, and `-`, so a misspelled or miscased one is refused.
    #[test]
    fn refuses_any_placeholder_shaped_text_with_an_unknown_name() {
        for (text, placeholder) in [
            ("{{x}}", "{{x}}"),
            ("{{ JOB.UUID }}", "{{ JOB.UUID }}"),
            ("{{ job.uuid2 }}", "{{ job.uuid2 }}"),
            ("{{ job.uuid.x }}", "{{ job.uuid.x }}"),
            ("{{ job_uuid }}", "{{ job_uuid }}"),
            ("{{ project-slug }}", "{{ project-slug }}"),
            ("{{{ job.id }}}", "{{ job.id }}"),
        ] {
            assert_eq!(
                unknown_placeholder(json!({ "a": text })),
                placeholder,
                "{text:?}"
            );
        }
    }

    #[test]
    fn brace_text_that_is_not_placeholder_shaped_is_sent_as_written() {
        for text in [
            "{{}}",
            "{{ }}",
            "{{ job .uuid }}",
            "{{ job:uuid }}",
            "{{ job.uuid }",
            "{ job.uuid }}",
            "{{\u{a0}job.uuid}}",
            "}} {{",
        ] {
            let body = json!({ "text": text, "list": [text] });
            let callback = with_body(body.clone()).unwrap();
            assert_eq!(
                render(&callback, &context(JobStatus::Processed)),
                body.to_string(),
                "{text:?}"
            );
        }
    }

    #[test]
    fn an_unknown_name_error_quotes_only_the_placeholder_escaped() {
        let err = with_body(json!({
            "key-marker": "text-marker {{\r\nforged\r\n}} more-marker",
        }))
        .unwrap_err()
        .to_string();
        for marker in ["text-marker", "more-marker", "key-marker", "\r", "\n"] {
            assert!(!err.contains(marker), "{marker:?} in {err}");
        }
        assert!(err.contains(r#""{{\r\nforged\r\n}}""#), "{err}");
    }

    #[test]
    fn whitespace_inside_the_braces_is_optional() {
        for placeholder in [
            "{{job.uuid}}",
            "{{ job.uuid }}",
            "{{   job.uuid}}",
            "{{job.uuid   }}",
            "{{\t\r\n\u{c}job.uuid\n}}",
        ] {
            let callback = with_body(json!({ "job": placeholder })).unwrap();
            assert_eq!(
                render_json(&callback, JobStatus::Processed),
                json!({ "job": JOB }),
                "{placeholder:?}"
            );
        }
    }

    #[test]
    fn a_placeholder_is_replaced_anywhere_inside_a_string() {
        for (text, rendered) in [
            ("job {{ job.uuid }} done", format!("job {JOB} done")),
            (" {{ job.status }}", " processed".to_owned()),
            (
                "https://bencher.dev/console/projects/{{ project.slug }}/reports/{{report.uuid}}",
                format!("https://bencher.dev/console/projects/my-project/reports/{REPORT}"),
            ),
            ("{{job.uuid}}{{ job.uuid }}", format!("{JOB}{JOB}")),
            ("{{{ job.uuid }}}", format!("{{{JOB}}}")),
        ] {
            let callback = with_body(json!({ "text": text, "list": [[text]] })).unwrap();
            assert_eq!(
                render_json(&callback, JobStatus::Processed),
                json!({ "text": rendered, "list": [[rendered]] }),
                "{text:?}"
            );
        }
    }

    /// A value is never searched for placeholders, so a Project named like one is sent as named.
    #[test]
    fn substitution_is_one_pass() {
        let callback = with_body(json!({
            "name": "{{ project.name }}",
            "text": "{{ project.name }} is {{ job.uuid }}",
        }))
        .unwrap();
        let rendered = callback
            .render(&named("{{ job.uuid }}", JobStatus::Processed), None)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).unwrap(),
            json!({
                "name": "{{ job.uuid }}",
                "text": format!("{{{{ job.uuid }}}} is {JOB}"),
            })
        );
    }

    /// The body is serialized from its tree, so no value can break it; a render that joined raw
    /// text into the JSON would.
    #[test]
    fn a_value_with_quotes_or_backslashes_stays_valid_json() {
        let name = r#"My "quoted" \ Project"#;
        let callback = with_body(json!({
            "name": "{{ project.name }}",
            "text": "*{{ project.name }}*",
        }))
        .unwrap();
        let rendered = callback
            .render(&named(name, JobStatus::Processed), None)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).unwrap(),
            json!({ "name": name, "text": format!("*{name}*") })
        );
    }

    #[test]
    fn a_placeholder_in_a_key_is_text() {
        let body =
            json!({ "{{ job.uuid }}": "{{ job.uuid }}", "{{ job.id }}": 1, "{{report}}": {} });
        let callback = with_body(body).unwrap();
        assert_eq!(
            render_json(&callback, JobStatus::Processed),
            json!({ "{{ job.uuid }}": JOB, "{{ job.id }}": 1, "{{report}}": {} })
        );
    }

    #[test]
    fn values_other_than_strings_pass_through() {
        for body in [
            json!({
                "int": 1,
                "negative": -2,
                "float": 1.5,
                "exponent": 1e300,
                "max": u64::MAX,
                "min": i64::MIN,
                "true": true,
                "false": false,
                "null": null,
                "array": [],
                "object": {},
                "nested": [[{ "a": [1, [2, [3]]] }]],
            }),
            json!(true),
            json!("text"),
        ] {
            let callback = with_body(body.clone()).unwrap();
            assert!(!callback.sends_report(), "{body}");
            assert_eq!(
                callback
                    .render(&context(JobStatus::Processed), None)
                    .unwrap(),
                body.to_string(),
                "{body}"
            );
        }
    }

    #[test]
    fn renders_each_placeholder() {
        let callback = with_body(json!({
            "job": { "uuid": "{{ job.uuid }}", "status": "{{ job.status }}" },
            "report": { "uuid": "{{ report.uuid }}" },
            "project": {
                "uuid": "{{ project.uuid }}",
                "name": "{{ project.name }}",
                "slug": "{{ project.slug }}",
            },
        }))
        .unwrap();
        for (status, name) in TERMINAL
            .into_iter()
            .zip(["processed", "failed", "canceled"])
        {
            let rendered = callback.render(&context(status), None).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&rendered).unwrap(),
                json!({
                    "job": { "uuid": JOB, "status": name },
                    "report": { "uuid": REPORT },
                    "project": { "uuid": PROJECT, "name": "My Project", "slug": "my-project" },
                }),
                "{status}"
            );
        }
    }

    #[test]
    fn the_report_placeholder_sends_the_report_in_an_envelope() {
        let callback = with_body(json!({
            "event": "bencher_run",
            "status": "{{ job.status }}",
            "data": { "report": "{{ report }}" },
        }))
        .unwrap();
        assert!(callback.sends_report());
        let rendered = render(&callback, &context(JobStatus::Processed));
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).unwrap(),
            json!({
                "event": "bencher_run",
                "status": "processed",
                "data": { "report": serde_json::to_value(json_report()).unwrap() },
            })
        );
        // The report keeps the field order the report endpoint gives it.
        let report = serde_json::to_string(&json_report()).unwrap();
        assert!(report.starts_with(r#"{"uuid":"#), "{report}");
        assert!(
            rendered.contains(&format!(r#"{{"report":{report}}}"#)),
            "{rendered}"
        );
    }

    #[test]
    fn a_body_of_only_the_report_is_the_default_body() {
        let callback = JsonNewCallback::new(URL, [], None).unwrap();
        assert!(callback.sends_report());
        let default = render(&callback, &context(JobStatus::Processed));
        assert_eq!(default, serde_json::to_string(&json_report()).unwrap());
        for body in ["{{ report }}", "{{report}}", "{{\n  report\t}}"] {
            let callback = with_body(json!(body)).unwrap();
            assert!(callback.sends_report(), "{body:?}");
            assert_eq!(
                render(&callback, &context(JobStatus::Processed)),
                default,
                "{body:?}"
            );
        }
    }

    /// A second report would send the report again with nothing counted for it.
    #[test]
    fn a_body_sends_the_report_once() {
        with_body(json!(["{{ report }}", { "again": "{{ report.uuid }}" }])).unwrap();
        let err = with_body(json!(["{{ report }}", { "again": "{{report}}" }])).unwrap_err();
        assert!(matches!(err, CallbackError::RepeatedReport), "{err}");
    }

    /// The report is an object, so it is only ever a whole string.
    #[test]
    fn the_report_inside_a_longer_string_is_refused() {
        for text in [
            "see {{ report }}",
            " {{report}}",
            "{{ report }}{{ report }}",
            "{{ job.uuid }}{{ report }}",
        ] {
            let err = with_body(json!({ "a": [text] })).unwrap_err();
            assert!(
                matches!(err, CallbackError::ReportInText),
                "{text:?}: {err}"
            );
        }
    }

    #[test]
    fn a_body_without_the_report_renders_without_one() {
        let callback = with_body(json!({
            "report": "{{ report.uuid }}",
            "{{ report }}": "{{ job.uuid }}",
        }))
        .unwrap();
        assert!(!callback.sends_report());
        let rendered = callback
            .render(&context(JobStatus::Processed), None)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).unwrap(),
            json!({ "report": REPORT, "{{ report }}": JOB })
        );
    }

    #[test]
    fn a_body_that_sends_the_report_fails_without_one() {
        for callback in [
            JsonNewCallback::new(URL, [], None).unwrap(),
            with_body(json!({ "report": "{{ report }}" })).unwrap(),
        ] {
            let err = callback
                .render(&context(JobStatus::Processed), None)
                .unwrap_err();
            assert!(err.to_string().contains("report was not built"), "{err}");
        }
    }

    /// The limit counts the body as written, placeholders included.
    #[test]
    fn the_limit_is_the_body_as_written() {
        let body = |padding: usize| json!(["{{ project.slug }}", "p".repeat(padding)]);
        let at_limit = MAX_CALLBACK_BODY_LEN - body(0).to_string().len();
        with_body(body(at_limit)).unwrap();
        let err = with_body(body(at_limit + 1)).unwrap_err();
        assert!(
            matches!(err, CallbackError::BodyTooLong(len) if len == MAX_CALLBACK_BODY_LEN + 1),
            "{err}"
        );
    }
}
