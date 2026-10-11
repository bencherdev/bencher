use std::fmt;

use bencher_json::RunnerResourceId;

/// Every runner with no `field` in runners.json, and every pair of runners whose values are less than `min` `unit`s apart.
pub fn spacing_problems<T, F>(
    values: &[(RunnerResourceId, Option<T>)],
    field: &str,
    unit: &str,
    min: u8,
    apart: F,
) -> Vec<String>
where
    T: Copy + fmt::Display,
    F: Fn(T, T) -> u8,
{
    let mut problems: Vec<String> = values
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(runner, _)| format!("{runner} has no {field}"))
        .collect();
    let set: Vec<(&RunnerResourceId, T)> = values
        .iter()
        .filter_map(|(runner, value)| value.map(|value| (runner, value)))
        .collect();
    for (index, (runner, value)) in set.iter().enumerate() {
        for (other, other_value) in set.iter().skip(index + 1) {
            let apart = apart(*value, *other_value);
            if apart < min {
                problems.push(format!(
                    "{runner} ({unit} {value}) and {other} ({unit} {other_value}) are {apart} {unit}(s) apart"
                ));
            }
        }
    }
    problems
}
