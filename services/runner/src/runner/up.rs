use bencher_runner::up::{Up as RunnerUp, UpConfig, UpError};

use crate::error::RunnerCliError;
use crate::parser::CliUp;

#[derive(Debug)]
pub struct Up {
    config: UpConfig,
}

impl TryFrom<CliUp> for Up {
    type Error = RunnerCliError;

    fn try_from(task: CliUp) -> Result<Self, Self::Error> {
        let tuning = task.tuning.try_into()?;
        let jail_user = bencher_runner::JailUser::new(task.jail_uid, task.jail_gid)
            .map_err(bencher_runner::RunnerError::from)?;

        Ok(Self {
            config: UpConfig {
                host: task.host,
                key: task.key,
                runner: task.runner,
                poll_timeout_secs: task.poll_timeout,
                tuning,
                cpu_layout: None,
                max_output_size: task.max_output_size,
                max_file_count: task.max_file_count,
                max_symlinks: task.max_symlinks,
                sandbox_log_level: task.sandbox_log_level,
                allow_no_sandbox: task.danger_allow_no_sandbox,
                raid_pause: !task.no_raid_pause,
                no_auto_update: task.no_auto_update,
                update_channel: task.update_channel,
                max_download_size: task.max_download_size,
                state_dir: task.state_dir,
                jail_user,
            },
        })
    }
}

impl Up {
    pub fn exec(self, log: &slog::Logger) -> Result<(), RunnerCliError> {
        let up = RunnerUp::new(self.config);
        match up.run(log) {
            Ok(()) | Err(UpError::Shutdown) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::Up;
    use crate::parser::CliUp;

    fn up(extra: &[&str]) -> Up {
        let args = [
            "up",
            "--key",
            "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
            "--runner",
            "00000000-0000-0000-0000-000000000000",
        ];
        Up::try_from(CliUp::try_parse_from(args.iter().chain(extra)).unwrap()).unwrap()
    }

    #[test]
    fn the_raid_pause_is_on_unless_turned_off() {
        // Kills an inverted `--no-raid-pause`, and a pause that is off by default.
        assert!(up(&[]).config.raid_pause);
        assert!(!up(&["--no-raid-pause"]).config.raid_pause);
    }
}
