/// The canonical name of the percentile at `percent`, a decimal number: `min`, `median`, and
/// `max` at 0, 50, and 100, else `p` and the number with `_` for its point, or `None` when
/// `percent` is not a number.
pub(crate) fn percentile_name(percent: &str) -> Option<String> {
    let percent = if percent.contains('.') {
        percent.trim_end_matches('0').trim_end_matches('.')
    } else {
        percent
    };
    let (whole, fraction) = percent.split_once('.').unwrap_or((percent, ""));
    if whole.is_empty()
        || !whole
            .chars()
            .chain(fraction.chars())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(match percent {
        "0" => "min".to_owned(),
        "50" => "median".to_owned(),
        "100" => "max".to_owned(),
        _ => format!("p{}", percent.replace('.', "_")),
    })
}
