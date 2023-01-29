use bencher_json::{BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space0, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    adapters::util::{
        nom_error, parse_benchmark_name_chars, parse_f64, parse_u64, parse_units, time_as_nanos,
        Units,
    },
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterCppCatch2;

impl Adapter for AdapterCppCatch2 {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let mut benchmark_metrics = Vec::new();

        // for line in input.lines() {
        //     if let Ok((remainder, benchmark_metric)) = parse_criterion(prior_line, line) {
        //         if remainder.is_empty() {
        //             benchmark_metrics.push(benchmark_metric);
        //         }
        //     }

        //     prior_line = Some(line);
        // }

        benchmark_metrics.try_into()
    }
}

fn parse_catch2_benchmark_name(input: &str) -> IResult<&str, BenchmarkName> {
    map_res(
        many_till(anychar, parse_catch2_prelude),
        |(name_chars, _)| -> Result<BenchmarkName, nom::Err<nom::error::Error<String>>> {
            if name_chars.is_empty() {
                return Err(nom_error(String::new()));
            }
            parse_benchmark_name_chars(&name_chars)
        },
    )(input)
}

struct Prelude {
    samples: u64,
    iterations: u64,
    estimated: f64,
    estimated_units: Units,
}

fn parse_catch2_prelude(input: &str) -> IResult<&str, Prelude> {
    map(
        tuple((
            space1,
            parse_u64,
            space1,
            parse_u64,
            space1,
            parse_f64,
            space1,
            parse_units,
            space0,
            eof,
        )),
        |(_, samples, _, iterations, _, estimated, _, estimated_units, _, _)| Prelude {
            samples,
            iterations,
            estimated,
            estimated_units,
        },
    )(input)
}

// fn parse_criterion_metric(input: &str) -> IResult<&str, JsonMetric> {
//     map(
//         delimited(
//             tag("["),
//             tuple((
//                 parse_criterion_duration,
//                 space1,
//                 parse_criterion_duration,
//                 space1,
//                 parse_criterion_duration,
//             )),
//             tag("]"),
//         ),
//         |(lower_bound, _, value, _, upper_bound)| JsonMetric {
//             value,
//             lower_bound: Some(lower_bound),
//             upper_bound: Some(upper_bound),
//         },
//     )(input)
// }

// fn parse_criterion_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
//     map_res(
//         tuple((parse_f64, space1, parse_units)),
//         |(duration, _, units)| -> Result<OrderedFloat<f64>, nom::Err<nom::error::Error<String>>> {
//             Ok(time_as_nanos(duration, units))
//         },
//     )(input)
// }

#[cfg(test)]
pub(crate) mod test_rust_criterion {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults,
    };

    use super::{parse_catch2_benchmark_name, AdapterCppCatch2};

    fn convert_cpp_catch2(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/criterion/{suffix}.txt");
        convert_file_path::<AdapterCppCatch2>(&file_path)
    }

    #[test]
    fn test_parse_benchmark_name() {
        for (index, (expected, input)) in [
            (
                Ok(("", "Fibonacci 10".parse().unwrap())),
                "Fibonacci 10                                              100           208     7.1968 ms ",
            ),
            (
                Ok(("", "Fibonacci 20".parse().unwrap())),
                "Fibonacci 20                                              100             2     8.3712 ms ",
            ),
            (
                Ok(("", "Fibonacci~ 5!".parse().unwrap())),
                "Fibonacci~ 5!                                             100          1961     7.0596 ms ",
            ),
            (
                Ok(("", "Fibonacci-15_bench".parse().unwrap())),
                "Fibonacci-15_bench                                        100            20       7.48 ms ",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                expected,
                parse_catch2_benchmark_name(input),
                "#{index}: {input}"
            )
        }
    }

    // #[test]
    // fn test_adapter_rust_criterion() {
    //     let results = convert_rust_criterion("many");
    //     assert_eq!(results.inner.len(), 5);

    //     let metrics = results.get("file").unwrap();
    //     validate_metrics(metrics, 0.32389999999999997, Some(0.32062), Some(0.32755));

    //     let metrics = results.get("rolling_file").unwrap();
    //     validate_metrics(metrics, 0.42966000000000004, Some(0.38179), Some(0.48328));

    //     let metrics = results.get("tracing_file").unwrap();
    //     validate_metrics(metrics, 18019.0, Some(16652.0), Some(19562.0));

    //     let metrics = results.get("tracing_rolling_file").unwrap();
    //     validate_metrics(metrics, 20930.0, Some(18195.0), Some(24240.0));

    //     let metrics = results.get("benchmark: name with spaces").unwrap();
    //     validate_metrics(metrics, 20.930, Some(18.195), Some(24.240));
    // }

    // #[test]
    // fn test_adapter_rust_criterion_failed() {
    //     let contents = std::fs::read_to_string("./tool_output/rust/criterion/failed.txt").unwrap();
    //     let results = AdapterCppCatch2::parse(&contents).unwrap();
    //     assert_eq!(results.inner.len(), 4);
    // }

    // #[test]
    // fn test_adapter_rust_criterion_dogfood() {
    //     let results = convert_rust_criterion("dogfood");
    //     assert_eq!(results.inner.len(), 4);

    //     let metrics = results.get("JsonAdapter::Magic (JSON)").unwrap();
    //     validate_metrics(
    //         metrics,
    //         3463.2000000000003,
    //         Some(3462.2999999999997),
    //         Some(3464.1000000000003),
    //     );

    //     let metrics = results.get("JsonAdapter::Json").unwrap();
    //     validate_metrics(metrics, 3479.6, Some(3479.2999999999997), Some(3480.0));

    //     let metrics = results.get("JsonAdapter::Magic (Rust)").unwrap();
    //     validate_metrics(metrics, 14726.0, Some(14721.0), Some(14730.0));

    //     let metrics = results.get("JsonAdapter::Rust").unwrap();
    //     validate_metrics(metrics, 14884.0, Some(14881.0), Some(14887.0));
    // }
}
