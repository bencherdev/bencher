use std::collections::HashMap;

/// Check that an environment variable argument is in `KEY=VALUE` format.
pub fn check_env(arg: &str) -> Result<String, String> {
    check_key_value::<'='>("KEY", "VALUE", arg, false)
}

/// Convert a list of `KEY=VALUE` strings into a `HashMap`.
pub fn parse_env(env: Vec<String>) -> HashMap<String, String> {
    env.into_iter()
        .filter_map(|s| {
            let (key, value) = s.split_once('=')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Check that a volume mount argument is in `HOST:CONTAINER` format.
pub fn check_volume(arg: &str) -> Result<String, String> {
    check_key_value::<':'>("HOST", "CONTAINER", arg, true)
}

/// Check that input argument is in the form `left<separator>right`.
pub fn check_key_value<const SEPARATOR: char>(
    left: &str,
    right: &str,
    arg: &str,
    require_right: bool,
) -> Result<String, String> {
    let index = arg.find(SEPARATOR)
        .ok_or_else(|| format!("Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{SEPARATOR}` was found in: `{arg}`"))?;
    if index == 0 {
        return Err(format!(
            "Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{left}` was found in: `{arg}`"
        ));
    }
    if require_right && index == arg.len() - 1 {
        return Err(format!(
            "Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{right}` was found in: `{arg}`"
        ));
    }
    Ok(arg.into())
}
