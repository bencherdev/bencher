pub mod asv;

use crate::{Adapter, AdapterResults};
use asv::AdapterPythonAsv;

pub struct AdapterPython;

impl Adapter for AdapterPython {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterPythonAsv::parse(input)
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterPython;
    use crate::adapters::{python::asv::test_python_asv, test_util::convert_file_path};

    #[test]
    fn test_adapter_python_asv() {
        let results = convert_file_path::<AdapterPython>("./tool_output/python/asv/six.txt");
        test_python_asv::validate_adapter_python_asv(results);
    }
}
