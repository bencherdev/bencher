use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json::Value;
use uuid::fmt::Hyphenated;

use super::{CALLBACK_JOB_STATUSES, CallbackError, MAX_CALLBACK_BODY_LEN};
use crate::runner::{JobStatus, JobUuid};
use crate::{JsonReport, ProjectSlug, ProjectUuid, ReportUuid, Slug};

/// The quotes around a JSON string.
const QUOTES: usize = 2;

/// The values a callback body's placeholders take, all but the report, which is built only when
/// the body sends it.
#[derive(Debug, Clone)]
pub struct CallbackContext {
    pub project_uuid: ProjectUuid,
    pub project_slug: ProjectSlug,
    pub report_uuid: ReportUuid,
    pub job_uuid: JobUuid,
    pub job_status: JobStatus,
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackRenderError<E> {
    #[error("failed to build the report for the callback body: {0}")]
    Report(E),
    #[error("failed to serialize the callback body: {0}")]
    Json(serde_json::Error),
}

pub(super) async fn render<F, Fut, E>(
    body: Option<&Value>,
    context: &CallbackContext,
    report: F,
) -> Result<String, CallbackRenderError<E>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<JsonReport, E>>,
{
    let Some(body) = body else {
        let report = report().await.map_err(CallbackRenderError::Report)?;
        return serde_json::to_string(&report).map_err(CallbackRenderError::Json);
    };
    let report = if sends_report(body) {
        Some(report().await.map_err(CallbackRenderError::Report)?)
    } else {
        None
    };
    serde_json::to_string(&Rendered {
        value: body,
        context,
        report: report.as_ref(),
    })
    .map_err(CallbackRenderError::Json)
}

/// Refuses a body that could render over the limit, a placeholder with an unknown name, and a
/// second report, which would send the report again with nothing counted for it.
pub(super) fn validate_body(body: &Value) -> Result<(), CallbackError> {
    let PlaceholderLens { written, longest } = placeholder_lens(body);
    // Each placeholder as written is part of the body, so this never underflows.
    let len = body.to_string().len() + longest - written;
    if len > MAX_CALLBACK_BODY_LEN {
        return Err(CallbackError::BodyTooLong(len));
    }
    let escaped = |pointer: String| pointer.escape_debug().to_string();
    if let Some(pointer) = find_string(body, &mut |text| {
        matches!(BodyString::new(text), BodyString::Unknown)
    }) {
        return Err(CallbackError::UnknownPlaceholder(escaped(pointer)));
    }
    let mut reports = 0;
    if let Some(pointer) = find_string(body, &mut |text| {
        if matches!(
            BodyString::new(text),
            BodyString::Placeholder(Placeholder::Report)
        ) {
            reports += 1;
        }
        reports > 1
    }) {
        return Err(CallbackError::RepeatedReport(escaped(pointer)));
    }
    Ok(())
}

/// The JSON pointer to the first string value for which `f` is true.
fn find_string<F>(value: &Value, f: &mut F) -> Option<String>
where
    F: FnMut(&str) -> bool,
{
    match value {
        Value::String(text) => f(text).then(String::new),
        Value::Array(items) => items.iter().enumerate().find_map(|(index, item)| {
            find_string(item, f).map(|pointer| format!("/{index}{pointer}"))
        }),
        Value::Object(fields) => fields.iter().find_map(|(key, item)| {
            find_string(item, f)
                .map(|pointer| format!("/{}{pointer}", key.replace('~', "~0").replace('/', "~1")))
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

/// The bytes the body's placeholders take as written, and with their longest values.
#[derive(Default)]
struct PlaceholderLens {
    written: usize,
    longest: usize,
}

fn placeholder_lens(body: &Value) -> PlaceholderLens {
    let mut lens = PlaceholderLens::default();
    for_each_placeholder(body, &mut |text, placeholder| {
        lens.written += Value::from(text).to_string().len();
        lens.longest += placeholder.longest();
    });
    lens
}

fn sends_report(body: &Value) -> bool {
    let mut sends = false;
    for_each_placeholder(body, &mut |_, placeholder| {
        sends |= placeholder == Placeholder::Report;
    });
    sends
}

/// Calls `f` with each placeholder in the body and its text as written.
fn for_each_placeholder<F>(value: &Value, f: &mut F)
where
    F: FnMut(&str, Placeholder),
{
    match value {
        Value::String(text) => {
            if let BodyString::Placeholder(placeholder) = BodyString::new(text) {
                f(text, placeholder);
            }
        },
        Value::Array(items) => items.iter().for_each(|item| for_each_placeholder(item, f)),
        Value::Object(fields) => fields
            .values()
            .for_each(|item| for_each_placeholder(item, f)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

/// What a string in the body is.
enum BodyString {
    Text,
    Placeholder(Placeholder),
    /// The placeholder shape around a name that is not a placeholder's.
    Unknown,
}

impl BodyString {
    fn new(text: &str) -> Self {
        let Some(name) = text
            .strip_prefix("{{")
            .and_then(|text| text.strip_suffix("}}"))
        else {
            return Self::Text;
        };
        match name.trim_ascii() {
            "job.uuid" => Self::Placeholder(Placeholder::JobUuid),
            "job.status" => Self::Placeholder(Placeholder::JobStatus),
            "report.uuid" => Self::Placeholder(Placeholder::ReportUuid),
            "project.uuid" => Self::Placeholder(Placeholder::ProjectUuid),
            "project.slug" => Self::Placeholder(Placeholder::ProjectSlug),
            "report" => Self::Placeholder(Placeholder::Report),
            _ => Self::Unknown,
        }
    }
}

/// A body string that is exactly `{{`, a name, and `}}`, with optional ASCII whitespace around
/// the name, and that is sent as the named value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placeholder {
    JobUuid,
    JobStatus,
    ReportUuid,
    ProjectUuid,
    ProjectSlug,
    Report,
}

impl Placeholder {
    /// The most bytes its value takes in a rendered body. The report, which a body sends at most
    /// once, has no limit of its own, as the default body has none, so it counts as nothing.
    fn longest(self) -> usize {
        match self {
            Self::JobUuid | Self::ReportUuid | Self::ProjectUuid => Hyphenated::LENGTH + QUOTES,
            Self::JobStatus => CALLBACK_JOB_STATUSES
                .iter()
                .map(|status| serde_json::json!(status).to_string().len())
                .max()
                .unwrap_or_default(),
            // A slug is lowercase ASCII letters, digits, and dashes, none of which JSON escapes.
            Self::ProjectSlug => Slug::MAX_LEN + QUOTES,
            Self::Report => 0,
        }
    }

    fn serialize_value<S>(
        self,
        context: &CallbackContext,
        report: Option<&JsonReport>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let CallbackContext {
            project_uuid,
            project_slug,
            report_uuid,
            job_uuid,
            job_status,
        } = context;
        match self {
            Self::JobUuid => job_uuid.serialize(serializer),
            Self::JobStatus => job_status.serialize(serializer),
            Self::ReportUuid => report_uuid.serialize(serializer),
            Self::ProjectUuid => project_uuid.serialize(serializer),
            Self::ProjectSlug => project_slug.serialize(serializer),
            Self::Report => report
                .ok_or_else(|| S::Error::custom("the callback body's report was not built"))?
                .serialize(serializer),
        }
    }
}

/// The body as sent: each placeholder is its value, serialized as its own type is, so the report
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
            Value::String(text) => match BodyString::new(text) {
                BodyString::Placeholder(placeholder) => {
                    placeholder.serialize_value(context, report, serializer)
                },
                BodyString::Text | BodyString::Unknown => serializer.serialize_str(text),
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
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    use super::{CallbackContext, CallbackRenderError};
    use crate::runner::callback::{CallbackError, JsonNewCallback, MAX_CALLBACK_BODY_LEN};
    use crate::{JobStatus, JobUuid, JsonReport, ProjectUuid, ReportUuid, Slug};

    const URL: &str = "https://example.com/hook";
    const TERMINAL: [JobStatus; 3] = [JobStatus::Processed, JobStatus::Failed, JobStatus::Canceled];
    const JOB: &str = "00000000-0000-0000-0000-00000000000a";
    const REPORT: &str = "00000000-0000-0000-0000-00000000000b";
    const PROJECT: &str = "00000000-0000-0000-0000-00000000000c";

    fn with_body(body: Value) -> Result<JsonNewCallback, CallbackError> {
        JsonNewCallback::new(URL, [], Some(body))
    }

    fn context(status: JobStatus) -> CallbackContext {
        CallbackContext {
            project_uuid: PROJECT.parse::<ProjectUuid>().unwrap(),
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

    /// Polls a render to its end. The report sources here never wait, so one poll is enough.
    fn block_on<F>(future: F) -> F::Output
    where
        F: Future,
    {
        let mut future = pin!(future);
        let Poll::Ready(output) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        else {
            panic!("the render waited");
        };
        output
    }

    /// Renders with a report source that counts how many times it builds the report.
    fn render_counting(
        callback: &JsonNewCallback,
        context: &CallbackContext,
    ) -> (Result<String, CallbackRenderError<Infallible>>, usize) {
        let builds = Cell::new(0);
        let rendered = block_on(callback.render(context, || async {
            builds.set(builds.get() + 1);
            Ok(json_report())
        }));
        (rendered, builds.get())
    }

    fn render(callback: &JsonNewCallback, context: &CallbackContext) -> String {
        render_counting(callback, context).0.unwrap()
    }

    fn render_json(callback: &JsonNewCallback, status: JobStatus) -> Value {
        serde_json::from_str(&render(callback, &context(status))).unwrap()
    }

    fn unknown_placeholder(body: Value) -> String {
        let err = with_body(body).unwrap_err();
        let CallbackError::UnknownPlaceholder(pointer) = err else {
            panic!("expected an unknown placeholder, got {err}");
        };
        pointer
    }

    #[test]
    fn refuses_an_unknown_name_by_its_location() {
        for (body, pointer) in [
            (json!("{{ job.id }}"), ""),
            (json!({ "job": "{{ job.id }}" }), "/job"),
            (json!({ "a": [0, { "b": "{{ job.id }}" }] }), "/a/1/b"),
            (json!([[], ["x", "{{ job.id }}"]]), "/1/1"),
            (json!({ "a/b~c": "{{ job.id }}" }), "/a~1b~0c"),
            (json!({ "a": "{{ job.uuid }}", "b": "{{ job.id }}" }), "/b"),
        ] {
            assert_eq!(unknown_placeholder(body.clone()), pointer, "{body}");
        }
    }

    /// The placeholder shape is `{{` at the start and `}}` at the end, whatever is between.
    #[test]
    fn refuses_every_unknown_name() {
        for name in [
            "{{x}}",
            "{{}}",
            "{{ JOB.UUID }}",
            "{{ job.uuid.x }}",
            "{{ job .uuid }}",
            "{{ job.uuid }}}",
            "{{{ job.uuid }}",
            "{{ job.uuid }} and {{ job.status }}",
            "{{\u{a0}job.uuid}}",
        ] {
            assert_eq!(unknown_placeholder(json!({ "a": name })), "/a", "{name:?}");
        }
    }

    #[test]
    fn an_unknown_name_error_never_quotes_the_body() {
        let err = with_body(json!({
            "note": "text-marker",
            "a\r\nforged: yes": "{{ name-marker }}",
        }))
        .unwrap_err()
        .to_string();
        for marker in ["text-marker", "name-marker", "\r", "\n"] {
            assert!(!err.contains(marker), "{marker:?} in {err}");
        }
        assert!(err.contains(r#"at "/a\r\nforged: yes""#), "{err}");
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
    fn a_placeholder_inside_a_longer_string_is_text() {
        for text in [
            "job {{ job.uuid }} done",
            "job {{ job.uuid }}",
            "{{ job.uuid }} done",
            " {{ job.uuid }}",
            "{{ job.uuid }",
            "{ job.uuid }}",
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
            let (rendered, builds) = render_counting(&callback, &context(JobStatus::Processed));
            assert_eq!(rendered.unwrap(), body.to_string(), "{body}");
            assert_eq!(builds, 0, "{body}");
        }
    }

    #[test]
    fn renders_each_placeholder() {
        let callback = with_body(json!({
            "job": { "uuid": "{{ job.uuid }}", "status": "{{ job.status }}" },
            "report": { "uuid": "{{ report.uuid }}" },
            "project": { "uuid": "{{ project.uuid }}", "slug": "{{ project.slug }}" },
        }))
        .unwrap();
        for (status, name) in TERMINAL
            .into_iter()
            .zip(["processed", "failed", "canceled"])
        {
            let (rendered, builds) = render_counting(&callback, &context(status));
            assert_eq!(
                serde_json::from_str::<Value>(&rendered.unwrap()).unwrap(),
                json!({
                    "job": { "uuid": JOB, "status": name },
                    "report": { "uuid": REPORT },
                    "project": { "uuid": PROJECT, "slug": "my-project" },
                }),
                "{status}"
            );
            assert_eq!(builds, 0, "{status}");
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
        let (rendered, builds) = render_counting(&callback, &context(JobStatus::Processed));
        let rendered = rendered.unwrap();
        assert_eq!(builds, 1);
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
        let default = render(
            &JsonNewCallback::new(URL, [], None).unwrap(),
            &context(JobStatus::Processed),
        );
        assert_eq!(default, serde_json::to_string(&json_report()).unwrap());
        for body in ["{{ report }}", "{{report}}", "{{\n  report\t}}"] {
            let callback = with_body(json!(body)).unwrap();
            let (rendered, builds) = render_counting(&callback, &context(JobStatus::Processed));
            assert_eq!(rendered.unwrap(), default, "{body:?}");
            assert_eq!(builds, 1, "{body:?}");
        }
    }

    /// A second report would send the report again with nothing counted for it.
    #[test]
    fn a_body_sends_the_report_once() {
        with_body(json!(["{{ report }}", { "again": "{{ report.uuid }}" }])).unwrap();
        let err = with_body(json!(["{{ report }}", { "again": "{{report}}" }])).unwrap_err();
        assert!(
            matches!(&err, CallbackError::RepeatedReport(pointer) if pointer == "/1/again"),
            "{err}"
        );
    }

    #[test]
    fn a_body_without_the_report_never_builds_it() {
        let callback = with_body(json!({
            "report": "{{ report.uuid }}",
            "text": "{{ report }} and more",
            "{{ report }}": "{{ job.uuid }}",
        }))
        .unwrap();
        let rendered = block_on(callback.render(&context(JobStatus::Processed), || async {
            Err::<JsonReport, _>("no report")
        }))
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered).unwrap(),
            json!({ "report": REPORT, "text": "{{ report }} and more", "{{ report }}": JOB })
        );
    }

    #[test]
    fn a_report_that_does_not_build_fails_the_render() {
        for callback in [
            JsonNewCallback::new(URL, [], None).unwrap(),
            with_body(json!({ "report": "{{ report }}" })).unwrap(),
        ] {
            let err = block_on(callback.render(&context(JobStatus::Processed), || async {
                Err::<JsonReport, _>("no report")
            }))
            .unwrap_err();
            assert!(
                matches!(err, CallbackRenderError::Report("no report")),
                "{err}"
            );
        }
    }

    /// Each placeholder counted at its longest value, and the report at nothing.
    #[test]
    fn refuses_a_body_that_could_render_over_the_limit() {
        for (placeholder, longest) in [
            ("{{ job.uuid }}", 38),
            ("{{ report.uuid }}", 38),
            ("{{ project.uuid }}", 38),
            ("{{job.status}}", r#""processed""#.len()),
            ("{{ project.slug }}", Slug::MAX_LEN + 2),
            ("{{\n\tproject.slug\r\n}}", Slug::MAX_LEN + 2),
            ("{{ report }}", 0),
        ] {
            // `[`, the value, `,`, the padding in quotes, and `]`.
            let padding = |len: usize| json!([placeholder, "p".repeat(len)]);
            let at_limit = MAX_CALLBACK_BODY_LEN - longest - 5;
            with_body(padding(at_limit - 1)).unwrap();
            with_body(padding(at_limit)).unwrap();
            let err = with_body(padding(at_limit + 1)).unwrap_err();
            assert!(
                matches!(err, CallbackError::BodyTooLong(len) if len == MAX_CALLBACK_BODY_LEN + 1),
                "{placeholder:?}: {err}"
            );
        }
    }

    /// The longest values render to exactly the length the limit counts, so a body that passes
    /// never renders over it.
    #[test]
    fn the_limit_is_the_longest_render() {
        let longest = CallbackContext {
            project_slug: "s".repeat(Slug::MAX_LEN).parse().unwrap(),
            ..context(JobStatus::Processed)
        };
        let body = |padding: usize| {
            json!([
                "{{ job.uuid }}",
                "{{job.status}}",
                "{{ report.uuid }}",
                "{{\tproject.uuid\n}}",
                "{{ project.slug }}",
                "p".repeat(padding),
            ])
        };
        let base = render(&with_body(body(0)).unwrap(), &longest).len();
        let at_limit = MAX_CALLBACK_BODY_LEN - base;
        let callback = with_body(body(at_limit)).unwrap();
        assert_eq!(render(&callback, &longest).len(), MAX_CALLBACK_BODY_LEN);
        for status in TERMINAL {
            let context = CallbackContext {
                job_status: status,
                ..longest.clone()
            };
            assert!(render(&callback, &context).len() <= MAX_CALLBACK_BODY_LEN);
        }
        let err = with_body(body(at_limit + 1)).unwrap_err();
        assert!(
            matches!(err, CallbackError::BodyTooLong(len) if len == MAX_CALLBACK_BODY_LEN + 1),
            "{err}"
        );
    }
}
