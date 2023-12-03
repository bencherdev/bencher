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
    use crate::adapters::{shell::hyperfine::test_shell_hyperfine, test_util::convert_file_path};

    #[test]
    fn test_adapter_shell_hyperfine() {
        let results = convert_file_path::<AdapterShell>("./tool_output/shell/hyperfine/two.json");
        test_shell_hyperfine::validate_adapter_shell_hyperfine(&results);
    }
}
