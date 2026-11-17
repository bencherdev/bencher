use bencher_json::{
    BenchmarkName, BmfVersion, JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::{JsonNewResults, MetricResults},
    },
};

use crate::RunError;

use super::command::Command;

#[derive(Debug, Clone, Copy)]
pub struct BuildTime;

impl BuildTime {
    #[expect(
        clippy::unused_self,
        reason = "consumes self for API consistency with other runners"
    )]
    pub fn command(self, command: &Command, duration: f64) -> BuildCommand {
        BuildCommand {
            args: command.to_args(),
            duration,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildCommand {
    args: Vec<String>,
    duration: f64,
}

impl BuildCommand {
    pub fn get_results(self, bmf_version: BmfVersion) -> Result<String, RunError> {
        let results = JsonNewResults::new(self.to_metric_results()?, bmf_version);
        serde_json::to_string(&results).map_err(RunError::SerializeBuildTime)
    }

    pub fn to_metric_results(&self) -> Result<MetricResults, RunError> {
        Ok(vec![(
            BenchmarkName::from_args(&self.args).map_err(RunError::CommandName)?,
            vec![(
                built_in::json::BuildTime::name_id(),
                JsonNewMetric {
                    value: self.duration.into(),
                    ..Default::default()
                },
            )],
        )])
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::BmfVersion;
    use clap::Parser as _;

    use super::super::{Runner, command::Command};
    use super::BuildTime;
    use crate::parser::run::CliRun;

    // Kills: a build time written in one shape whatever the report's version.
    #[test]
    fn build_time_is_written_at_the_report_s_version() {
        let command = Command::new_exec("./build.sh".to_owned(), Vec::new());
        for (bmf_version, results) in [
            (
                BmfVersion::V0,
                serde_json::json!({
                    "./build.sh": {
                        "build-time": { "value": 1.5, "lower_value": null, "upper_value": null }
                    }
                }),
            ),
            (
                BmfVersion::V1,
                serde_json::json!({
                    "./build.sh": [{ "measures": { "build-time": { "value": 1.5 } } }]
                }),
            ),
        ] {
            let written = BuildTime
                .command(&command, 1.5)
                .get_results(bmf_version)
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&written).unwrap(),
                results,
                "{bmf_version}"
            );
        }
    }

    #[test]
    fn build_time_benchmark_is_named_after_the_command() {
        for (args, name) in [
            (
                &["--exec", "./my_build_script.sh"][..],
                "./my_build_script.sh",
            ),
            (
                &["./my_build_script.sh", "--release"],
                "./my_build_script.sh --release",
            ),
            (
                &["--shell", "/bin/sh", "--flag=-c", "./my_build_script.sh"],
                "/bin/sh -c ./my_build_script.sh",
            ),
        ] {
            let run = CliRun::try_parse_from(
                ["run", "--build-time"]
                    .into_iter()
                    .chain(args.iter().copied()),
            )
            .unwrap();
            let Ok(Runner::Command(command, Some(build_time))) = Runner::try_from(run.cmd) else {
                panic!("{args:?} should time the build of a command");
            };
            let results = build_time
                .command(&command, 0.0)
                .to_metric_results()
                .unwrap();
            assert_eq!(results[0].0.as_ref(), name, "{args:?}");
        }
    }
}
