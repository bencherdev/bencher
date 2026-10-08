#![cfg(feature = "plus")]

use bencher_json::runner::{
    HealthFindingKind, HealthState, JsonHealthFinding, MdSyncAction, PauseReason,
};
use slog::Logger;

use super::FmtBody;
use crate::model::runner::{PAUSE_NOTICE_HOURS, RunnerNotice};

#[derive(Debug)]
pub struct RunnerStatusBody {
    pub admin: String,
    pub runner: String,
    pub notice: RunnerNotice,
}

impl RunnerStatusBody {
    pub fn subject(&self) -> String {
        let Self { runner, notice, .. } = self;
        match notice {
            RunnerNotice::Health { health, .. } => {
                format!("🐰 Runner {runner}: disk health {}", state(&health.state))
            },
            RunnerNotice::LongPause { .. } => {
                format!("🐰 Runner {runner}: paused for over {PAUSE_NOTICE_HOURS} hours")
            },
            RunnerNotice::PauseEnded { .. } => format!("🐰 Runner {runner}: taking Jobs again"),
        }
    }
}

impl FmtBody for RunnerStatusBody {
    fn text(&self) -> String {
        let Self {
            admin,
            runner,
            notice,
        } = self;
        let mut lines = vec![format!("Ahoy {admin},"), String::new()];
        match notice {
            RunnerNotice::Health { was, health, new } => {
                let was = was
                    .as_ref()
                    .map(|was| format!(" (it was {})", state(was)))
                    .unwrap_or_default();
                lines.push(format!(
                    "Runner {runner} reports its disk health as {}{was}.",
                    state(&health.state)
                ));
                lines.push(String::new());
                lines.extend(health.findings.iter().map(|finding| {
                    format!(
                        "- {}: {}, {}{}",
                        finding.device,
                        kind(&finding.kind),
                        state(&finding.state),
                        if is_new(new, finding) { " (new)" } else { "" }
                    )
                }));
            },
            RunnerNotice::LongPause { since, reasons } => {
                lines.push(format!(
                    "Runner {runner} has taken no Job since {since}, more than {PAUSE_NOTICE_HOURS} hours, because:"
                ));
                lines.push(String::new());
                lines.extend(
                    reasons
                        .iter()
                        .map(|reason| format!("- {}", pause_reason(reason))),
                );
            },
            RunnerNotice::PauseEnded { since } => {
                lines.push(format!(
                    "Runner {runner}, paused since {since}, is taking Jobs again."
                ));
            },
        }
        lines.push(String::new());
        lines.push("🐰 Bencher".to_owned());
        lines.join("\n")
    }

    fn html(&self, _log: &Logger) -> String {
        format!(
            "<!doctype html>
<html>
    <head>
        <meta charset=\"utf-8\" />
        <title>{subject}</title>
    </head>
    <body>
        <pre>{text}</pre>
    </body>
</html>",
            subject = escape(&self.subject()),
            text = escape(&self.text()),
        )
    }
}

fn is_new(new: &[JsonHealthFinding], finding: &JsonHealthFinding) -> bool {
    new.iter()
        .any(|seen| seen.device == finding.device && seen.kind == finding.kind)
}

fn state(state: &HealthState) -> &str {
    match state {
        HealthState::Ok => "ok",
        HealthState::Warning => "warning",
        HealthState::Failing => "failing",
        HealthState::Other(other) => other,
    }
}

fn kind(kind: &HealthFindingKind) -> &str {
    match kind {
        HealthFindingKind::Degraded => "degraded",
        HealthFindingKind::CriticalWarning => "critical warning",
        HealthFindingKind::AvailableSpare => "available spare",
        HealthFindingKind::PercentageUsed => "percentage used",
        HealthFindingKind::MediaErrors => "media errors",
        HealthFindingKind::Temperature => "temperature",
        HealthFindingKind::Other(other) => other,
    }
}

fn pause_reason(reason: &PauseReason) -> String {
    match reason {
        PauseReason::Raid {
            array,
            action,
            done,
            total,
            ..
        } => {
            let progress = done
                .zip(*total)
                .map(|(done, total)| format!(", {done} of {total} sectors"))
                .unwrap_or_default();
            format!(
                "RAID array {array} is running {}{progress}",
                sync_action(action)
            )
        },
        PauseReason::Maintenance { marker, lock } => match (marker, lock) {
            (true, true) => "host maintenance set its marker and holds the job lock".to_owned(),
            (true, false) => "host maintenance set its marker".to_owned(),
            (false, true) => "host maintenance holds the job lock".to_owned(),
            (false, false) => "host maintenance".to_owned(),
        },
        PauseReason::Other => "a reason this server does not know".to_owned(),
    }
}

fn sync_action(action: &MdSyncAction) -> &str {
    match action {
        MdSyncAction::Resync => "resync",
        MdSyncAction::Recover => "recover",
        MdSyncAction::Check => "check",
        MdSyncAction::Repair => "repair",
        MdSyncAction::Reshape => "reshape",
        MdSyncAction::Frozen => "frozen",
        MdSyncAction::Idle => "idle",
        MdSyncAction::Other(other) => other,
    }
}

// Runner strings reach the mail, so they are escaped before they become HTML.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use bencher_json::runner::{
        HealthFindingKind, HealthState, JsonHealthFinding, JsonRunnerHealth,
    };

    use super::{FmtBody as _, RunnerStatusBody};
    use crate::model::runner::RunnerNotice;

    // A runner names its own devices, so a name that is markup must reach the mail as text.
    #[test]
    fn html_escapes_runner_strings() {
        let finding = JsonHealthFinding {
            device: "<img src=x>".to_owned(),
            kind: HealthFindingKind::Degraded,
            state: HealthState::Failing,
        };
        let body = RunnerStatusBody {
            admin: "Admin".to_owned(),
            runner: "runner".to_owned(),
            notice: RunnerNotice::Health {
                was: None,
                health: JsonRunnerHealth {
                    state: HealthState::Failing,
                    findings: vec![finding.clone()],
                    arrays: Vec::new(),
                    nvme: Vec::new(),
                },
                new: vec![finding],
            },
        };
        let html = body.html(&slog::Logger::root(slog::Discard, slog::o!()));
        assert!(!html.contains("<img"), "markup escaped in {html}");
        assert!(html.contains("&lt;img src=x&gt;"), "text kept in {html}");
    }
}
