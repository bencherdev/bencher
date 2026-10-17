pub mod v1;
pub mod v2;

use crate::{Adaptable, AdapterResults, Settings};

use self::{v1::AdapterShellHyperfineV1, v2::AdapterShellHyperfineV2};

pub struct AdapterShellHyperfine;

impl Adaptable for AdapterShellHyperfine {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterShellHyperfineV2::parse(input, settings)
            .or_else(|| AdapterShellHyperfineV1::parse(input, settings))
    }
}

#[cfg(test)]
pub(crate) mod test_shell_hyperfine {
    use crate::adapters::{
        shell::hyperfine::{
            v1::test_shell_hyperfine_v1::validate_adapter_shell_hyperfine_v1,
            v2::test_shell_hyperfine_v2::validate_adapter_shell_hyperfine_v2,
        },
        test_util::convert_file_path,
    };

    use super::AdapterShellHyperfine;

    #[test]
    fn v1() {
        let results =
            convert_file_path::<AdapterShellHyperfine>("./tool_output/shell/hyperfine/two.json");
        validate_adapter_shell_hyperfine_v1(&results);
    }

    #[test]
    fn v2() {
        let results =
            convert_file_path::<AdapterShellHyperfine>("./tool_output/shell/hyperfine/v2_two.json");
        validate_adapter_shell_hyperfine_v2(&results);
    }
}
