pub mod jmh;

use crate::{Adaptable, AdapterResults, Settings};
use jmh::AdapterJavaJmh;

pub struct AdapterJava;

impl Adaptable for AdapterJava {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJavaJmh::parse(input, settings)
    }
}

#[cfg(test)]
mod test_java {
    use super::AdapterJava;
    use crate::adapters::{java::jmh::test_java_jmh, test_util::convert_file_path};

    #[test]
    fn test_adapter_java_jmh() {
        let results = convert_file_path::<AdapterJava>("./tool_output/java/jmh/six.json");
        test_java_jmh::validate_adapter_java_jmh(&results);
    }
}
