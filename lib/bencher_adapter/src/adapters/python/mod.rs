pub mod asv;
pub mod pytest;

use crate::{Adapter, AdapterResults};
use asv::AdapterPythonAsv;
use pytest::AdapterPythonPytest;

pub struct AdapterPython;

impl Adapter for AdapterPython {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterPythonAsv::parse(input).or_else(|| AdapterPythonPytest::parse(input))
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterPython;
    use crate::adapters::{
        python::asv::test_python_asv, python::pytest::test_python_pytest,
        test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_python_asv() {
        let results = convert_file_path::<AdapterPython>("./tool_output/python/asv/six.txt");
        test_python_asv::validate_adapter_python_asv(results);
    }

    #[test]
    fn test_adapter_python_pytest() {
        let results = convert_file_path::<AdapterPython>("./tool_output/python/pytest/four.json");
        test_python_pytest::validate_adapter_python_pytest(results);
    }
}
