use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliUp {
    /// Detached mode: Run containers in the background
    #[clap(short, long)]
    pub detach: bool,

    /// Pull image before running ("always"|"missing"|"never")
    #[clap(long)]
    pub pull: Option<CliUpPull>,

    /// Pass an environment variable to the API container.
    /// Similar to the `--env` option for `docker run`.
    #[clap(long, value_parser = check_env)]
    pub api_env: Option<Vec<String>>,

    /// Pass an environment variable to the Console container.
    /// Similar to the `--env` option for `docker run`.
    #[clap(long, value_parser = check_env)]
    pub console_env: Option<Vec<String>>,

    /// Pass a mount volume to the API container.
    /// Similar to the `--volume` option for `docker run`.
    #[clap(long, value_parser = check_volume)]
    pub api_volume: Option<Vec<String>>,

    /// Pass a mount volume to the Console container.
    /// Similar to the `--volume` option for `docker run`.
    #[clap(long, value_parser = check_volume)]
    pub console_volume: Option<Vec<String>>,
}

#[derive(ValueEnum, Debug, Clone, Copy, Default)]
#[clap(rename_all = "snake_case")]
pub enum CliUpPull {
    #[default]
    Always,
    Missing,
    Never,
}

#[derive(Parser, Debug)]
pub struct CliDown {}

#[derive(Parser, Debug)]
pub struct CliLogs {
    /// Docker container name
    pub container: Option<String>,
}

fn check_env(arg: &str) -> Result<String, String> {
    check_key_value::<'='>("KEY", "VALUE", arg, false)
}

fn check_volume(arg: &str) -> Result<String, String> {
    check_key_value::<':'>("HOST", "CONTAINER", arg, true)
}

/// Check that input argument is in the form `left<separator>right`
fn check_key_value<const SEPARATOR: char>(
    left: &str,
    right: &str,
    arg: &str,
    require_right: bool,
) -> Result<String, String> {
    let index = arg.find(SEPARATOR)
        .ok_or_else(|| format!("Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{SEPARATOR}` was found in: `{arg}`"))?;
    if index == 0 {
        return Err(format!("Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{left}` was found in: `{arg}`"));
    }
    if require_right && index == arg.len() - 1 {
        return Err(format!("Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{right}` was found in: `{arg}`"));
    }
    Ok(arg.into())
}
