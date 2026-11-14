use bencher_json::MeasureNameId;

use crate::adapters::bmf_v1::leaf::Variant;

/// A measure's values from repeated runs of one variant: a single run's value under `single`,
/// else the statistics of the runs.
pub(super) fn insert_runs(
    variant: &mut Variant<'_>,
    measure: MeasureNameId,
    single: &str,
    values: Vec<f64>,
) {
    if let [value] = values.as_slice() {
        variant.insert(measure, single, *value);
    } else if let Some(statistics) = statistics(values) {
        for (name, value) in statistics {
            variant.insert(measure.clone(), name, value);
        }
    }
}

/// The mean, median, minimum, maximum, and sample standard deviation of two or more runs.
fn statistics(mut values: Vec<f64>) -> Option<[(&'static str, f64); 5]> {
    values.sort_by(f64::total_cmp);
    let count = f64::from(u32::try_from(values.len()).ok()?);
    let mean = values.iter().sum::<f64>() / count;
    let middle = values.len().div_euclid(2);
    let upper = *values.get(middle)?;
    let median = if values.len().is_multiple_of(2) {
        f64::midpoint(*values.get(middle.checked_sub(1)?)?, upper)
    } else {
        upper
    };
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    Some([
        ("mean", mean),
        ("median", median),
        ("min", *values.first()?),
        ("max", *values.last()?),
        ("std_dev", variance.sqrt()),
    ])
}
