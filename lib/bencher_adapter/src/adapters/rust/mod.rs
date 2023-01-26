pub mod bench;
pub mod criterion;

use self::criterion::AdapterRustCriterion;
use bench::AdapterRustBench;
use nom::{
    bytes::complete::tag,
    character::complete::anychar,
    combinator::{eof, map, peek},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};

use crate::{Adapter, AdapterError, AdapterResults, Settings};

pub struct AdapterRust;

impl Adapter for AdapterRust {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        let bench = AdapterRustBench::parse(input, settings)?;
        if !bench.is_empty() {
            return Ok(bench);
        }

        let criterion = AdapterRustCriterion::parse(input, settings)?;
        if !criterion.is_empty() {
            return Ok(criterion);
        }

        Ok(AdapterResults::default())
    }
}

fn rust_panic(line: &str, settings: Settings) -> Result<(), AdapterError> {
    if let Ok((remainder, (thread, context, location))) = parse_panic(line) {
        if remainder.is_empty() && !settings.allow_failure {
            return Err(AdapterError::Panic {
                thread,
                context,
                location,
            });
        }
    }

    Ok(())
}

fn parse_panic(input: &str) -> IResult<&str, (String, String, String)> {
    map(
        tuple((
            tag("thread "),
            delimited(tag("'"), many_till(anychar, peek(tag("'"))), tag("'")),
            tag(" panicked at "),
            delimited(tag("'"), many_till(anychar, peek(tag("'"))), tag("'")),
            tag(", "),
            many_till(anychar, eof),
        )),
        |(_, (thread, _), _, (context, _), _, (location, _))| {
            (
                thread.into_iter().collect(),
                context.into_iter().collect(),
                location.into_iter().collect(),
            )
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_rust {

    use pretty_assertions::assert_eq;

    use super::parse_panic;

    #[test]
    fn test_parse_panic() {
        for (index, (expected, input)) in [(
            Ok((
                "",
                (
                    "main".into(),
                    "explicit panic".into(),
                    "trace4rs/benches/trace4rs_bench.rs:42:5".into(),
                ),
            )),
            "thread 'main' panicked at 'explicit panic', trace4rs/benches/trace4rs_bench.rs:42:5",
        )]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_panic(input), "#{index}: {input}")
        }
    }
}
