use bencher_json::{
    BenchmarkName, JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::MetricResults,
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
    pub fn get_results(self) -> Result<String, RunError> {
        let results = JsonNewMetric::results(self.to_metric_results()?);
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
    use clap::Parser as _;

    use super::super::Runner;
    use crate::parser::run::CliRun;

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
