#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::websocket::{MdSyncAction, cap_string};

/// The most findings, arrays, or `NVMe` controllers the server keeps from one report.
pub const MAX_HEALTH_ITEMS: usize = 64;

/// A runner host's disk health, sent with `Ready` and `Paused`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunnerHealth {
    /// The worst state of any finding, `ok` when there is none
    pub state: HealthState,
    /// What is wrong, one per disk or array and condition
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<JsonHealthFinding>,
    /// Every md array on the host
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrays: Vec<JsonMdArray>,
    /// Every `NVMe` controller on the host
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nvme: Vec<JsonNvmeHealth>,
}

/// How healthy a disk, an array, or a whole host is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ok,
    Warning,
    Failing,
    /// A state this server does not know.
    #[serde(untagged)]
    Other(String),
}

/// One thing wrong with one disk or array.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonHealthFinding {
    /// The disk or array, such as `nvme0` or `md0`
    pub device: String,
    /// What is wrong
    pub kind: HealthFindingKind,
    /// How bad it is
    pub state: HealthState,
}

/// What a finding is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthFindingKind {
    /// An md array is missing members
    Degraded,
    /// An `NVMe` critical warning bit other than temperature is set
    CriticalWarning,
    /// `NVMe` available spare is low or below its threshold
    AvailableSpare,
    /// `NVMe` percentage used is over 80
    PercentageUsed,
    /// `NVMe` media and data integrity errors are nonzero
    MediaErrors,
    /// The `NVMe` temperature critical warning bit is set
    Temperature,
    /// A finding this server does not know.
    #[serde(untagged)]
    Other(String),
}

/// An md array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMdArray {
    /// The array, such as `md0`
    pub name: String,
    /// Its `array_state`, such as `clean` or `active`
    pub array_state: String,
    /// Its count of missing members, from `degraded`
    pub degraded: u32,
    /// Its `sync_action`
    pub action: MdSyncAction,
}

/// An `NVMe` controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNvmeHealth {
    /// The controller, such as `nvme0`
    pub name: String,
    /// Its model number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Its serial number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Its SMART / health log, absent when the runner cannot read it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<JsonNvmeHealthLog>,
}

/// The fields of an `NVMe` SMART / health log page that set a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNvmeHealthLog {
    /// Critical warning bits
    pub critical_warning: u8,
    /// Composite temperature, in kelvin
    pub temperature: u16,
    /// Available spare, in percent
    pub available_spare: u8,
    /// Available spare threshold, in percent
    pub available_spare_threshold: u8,
    /// Percentage of the rated endurance used, which can pass 100
    pub percentage_used: u8,
    /// Media and data integrity errors
    pub media_errors: u64,
}

string_schema!(HealthState, HealthFindingKind);

impl JsonRunnerHealth {
    /// Cap every list and string, and sort and deduplicate the findings.
    pub fn cap(&mut self) {
        let Self {
            state,
            findings,
            arrays,
            nvme,
        } = self;
        state.cap();
        findings.truncate(MAX_HEALTH_ITEMS);
        for finding in findings.iter_mut() {
            cap_string(&mut finding.device);
            finding.kind.cap();
            finding.state.cap();
        }
        findings.sort();
        findings.dedup();
        arrays.truncate(MAX_HEALTH_ITEMS);
        for array in arrays {
            cap_string(&mut array.name);
            cap_string(&mut array.array_state);
            array.action.cap();
        }
        nvme.truncate(MAX_HEALTH_ITEMS);
        for controller in nvme {
            cap_string(&mut controller.name);
            if let Some(model) = &mut controller.model {
                cap_string(model);
            }
            if let Some(serial) = &mut controller.serial {
                cap_string(serial);
            }
        }
    }
}

impl HealthState {
    fn cap(&mut self) {
        if let Self::Other(state) = self {
            cap_string(state);
        }
    }
}

impl HealthFindingKind {
    fn cap(&mut self) {
        if let Self::Other(kind) = self {
            cap_string(kind);
        }
    }
}

/// Describe an enum whose untagged last variant takes any other string as a plain string,
/// where a derive would describe that variant as an object.
macro_rules! string_schema {
    ($($name:ident),+) => {$(
        #[cfg(feature = "schema")]
        impl JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).to_owned()
            }

            fn json_schema(
                generator: &mut schemars::r#gen::SchemaGenerator,
            ) -> schemars::schema::Schema {
                String::json_schema(generator)
            }
        }
    )+};
}
pub(crate) use string_schema;

#[cfg(test)]
mod tests {
    use super::{HealthFindingKind, HealthState, JsonRunnerHealth, MAX_HEALTH_ITEMS};

    // A state from a newer runner must not fail the whole message.
    #[test]
    fn unknown_health_state_is_other() {
        let health: JsonRunnerHealth = serde_json::from_str(
            r#"{"state":"smoking","findings":[{"device":"nvme0","kind":"smoke","state":"smoking"}]}"#,
        )
        .unwrap();
        assert_eq!(health.state, HealthState::Other("smoking".to_owned()));
        assert_eq!(
            health.findings[0].kind,
            HealthFindingKind::Other("smoke".to_owned())
        );
    }

    // A runner's lists and strings are cut to size rather than refused, on a character boundary.
    #[test]
    fn cap_bounds_lists_and_strings() {
        let findings = (0..100)
            .map(|i| format!(r#"{{"device":"nvme{i}","kind":"media_errors","state":"warning"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let name = "€".repeat(100);
        let mut health: JsonRunnerHealth = serde_json::from_str(&format!(
            r#"{{"state":"warning","findings":[{findings}],"arrays":[{{"name":"{name}","array_state":"clean","degraded":0,"action":"idle"}}]}}"#
        ))
        .unwrap();
        health.cap();
        assert_eq!(health.findings.len(), MAX_HEALTH_ITEMS);
        assert_eq!(health.arrays[0].name, "€".repeat(21));
    }
}
