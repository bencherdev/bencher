use bencher_json::project::measure::built_in::{BuiltInMeasure as _, default::Throughput};

use crate::adapters::bmf_v1::{finite::Finite, leaf::Variant};

/// A rate of operations per second, `hz`, as its mean, with its interval at the 95% level the
/// harness fixes from `rme`, the margin as a percentage, as the harness prints it beside the rate.
pub(super) fn insert_rate(variant: &mut Variant<'_>, hz: Finite, rme: Finite) {
    let throughput = Throughput::name_id();
    hz.insert(variant, &throughput, "mean", 1.0);
    let (lower, upper) = match (hz.get(), rme.get()) {
        (Some(hz), Some(rme)) => {
            let margin = hz * rme / 100.0;
            (Some(hz - margin), Some(hz + margin))
        },
        _ => (None, None),
    };
    for (name, bound) in [("mean_ci95_lower", lower), ("mean_ci95_upper", upper)] {
        Finite::from(bound).insert_spread(hz, variant, &throughput, name, 1.0);
    }
}
