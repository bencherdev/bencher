pub mod hyperfine;

use crate::{Adaptable, AdapterResults, Settings};
use hyperfine::AdapterShellHyperfine;

pub struct AdapterShell;

impl Adaptable for AdapterShell {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterShellHyperfine::parse(input, settings)
    }
}

#[cfg(test)]
mod test_shell {
    use super::AdapterShell;
    use crate::adapters::{
        shell::hyperfine::{v1::test_shell_hyperfine_v1, v2::test_shell_hyperfine_v2},
        test_util::convert_file_path,
    };

    #[test]
    fn adapter_shell_hyperfine() {
        {
            let results =
                convert_file_path::<AdapterShell>("./tool_output/shell/hyperfine/v2_two.json");
            test_shell_hyperfine_v2::validate_adapter_shell_hyperfine_v2(&results);
        }

        {
            let results =
                convert_file_path::<AdapterShell>("./tool_output/shell/hyperfine/two.json");
            test_shell_hyperfine_v1::validate_adapter_shell_hyperfine_v1(&results);
        }
    }
}
