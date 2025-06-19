use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};

use nom::{
    IResult,
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
};

use crate::{
    Adaptable, Settings,
    adapters::util::{
        NomError, Units, nom_error, parse_benchmark_name_chars, parse_f64, parse_number_as_f64,
        parse_u64, throughput_as_secs,
    },
    results::adapter_results::AdapterResults,
};

pub struct AdapterJsBenchmark;

impl Adaptable for AdapterJsBenchmark {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Median) | None => {},
            Some(JsonAverage::Mean) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_benchmark(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        AdapterResults::new_throughput(benchmark_metrics)
    }
}

fn parse_benchmark(input: &str) -> IResult<&str, (BenchmarkName, JsonNewMetric)> {
    map_res(
        many_till(anychar, parse_benchmark_time),
        |(name_chars, json_metric)| -> Result<(BenchmarkName, JsonNewMetric), NomError> {
            if name_chars.is_empty() {
                return Err(nom_error(String::new()));
            }
            let benchmark_name = parse_benchmark_name_chars(&name_chars)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_benchmark_time(input: &str) -> IResult<&str, JsonNewMetric> {
    map(
        tuple((
            tuple((space1, tag("x"), space1)),
            parse_number_as_f64,
            tuple((space1, tag("ops/sec"), space1, tag("±"))),
            parse_f64,
            tuple((
                tag("%"),
                space1,
                delimited(
                    tag("("),
                    tuple((parse_u64, space1, tag("runs"), space1, tag("sampled"))),
                    tag(")"),
                ),
                eof,
            )),
        )),
        |(_, throughput, _, percent_error, _)| {
            let value = throughput_as_secs(throughput, Units::Sec);
            let error = value * (percent_error / 100.0);
            JsonNewMetric {
                value,
                lower_value: Some(value - error),
                upper_value: Some(value + error),
            }
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_js_benchmark {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_throughput},
    };

    use super::AdapterJsBenchmark;

    fn convert_js_benchmark(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/js/benchmark/{suffix}.txt");
        convert_file_path::<AdapterJsBenchmark>(&file_path)
    }

    #[test]
    fn test_adapter_js_benchmark_average() {
        let file_path = "./tool_output/js/benchmark/four.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterJsBenchmark>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Mean)
                }
            )
        );

        let results = opt_convert_file_path::<AdapterJsBenchmark>(
            file_path,
            Settings {
                average: Some(JsonAverage::Median),
            },
        )
        .unwrap();
        validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_js_benchmark("four");
        validate_adapter_js_benchmark(&results);
    }

    pub fn validate_adapter_js_benchmark(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("fib(10)").unwrap();
        validate_throughput(
            metrics,
            1_431_759.0,
            Some(1_421_163.983_4),
            Some(1_442_354.016_6),
        );

        let metrics = results.get("fib(20)").unwrap();
        validate_throughput(metrics, 12_146.0, Some(12_107.132_8), Some(12_184.867_2));

        let metrics = results.get("benchmark with x 2 many things").unwrap();
        validate_throughput(metrics, 50.0, Some(49.95), Some(50.05));

        let metrics = results.get("createObjectBuffer with 200 comments").unwrap();
        validate_throughput(metrics, 81.61, Some(80.222_63), Some(82.997_37));
    }

    #[test]
    fn test_adapter_js_benchmark_issue_506() {
        let results = convert_js_benchmark("issue_506");
        assert_eq!(results.inner.len(), 6);

        let metrics = results.get("text encoder utf8").unwrap();
        validate_throughput(
            metrics,
            28.33,
            Some(27.395_11),
            Some(29.264_889_999_999_998),
        );

        let metrics = results.get("text encoder utf16").unwrap();
        validate_throughput(metrics, 55.25, Some(46.078_5), Some(64.421_5));

        let metrics = results.get("sha512 wasm from string utf8").unwrap();
        validate_throughput(metrics, 15.20, Some(13.862_4), Some(16.537_599_999_999_998));

        let metrics = results.get("sha512 wasm from string utf16").unwrap();
        validate_throughput(
            metrics,
            19.23,
            Some(18.903_09),
            Some(19.556_910_000_000_002),
        );

        let metrics = results.get("sha512 native from string utf8").unwrap();
        validate_throughput(metrics, 21.46, Some(20.580_14), Some(22.339_86));

        let metrics = results.get("sha512 native from string utf16").unwrap();
        validate_throughput(
            metrics,
            29.76,
            Some(27.736_320_000_000_003),
            Some(31.783_68),
        );
    }
}
