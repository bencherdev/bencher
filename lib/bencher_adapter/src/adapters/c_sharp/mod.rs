pub mod dotnet;

use crate::{Adapter, AdapterError, AdapterResults};
use dotnet::AdapterCSharpDotNet;

pub struct AdapterCSharp;

impl Adapter for AdapterCSharp {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let dotnet = AdapterCSharpDotNet::parse(input);
        if dotnet.is_ok() {
            return dotnet;
        }

        Ok(AdapterResults::default())
    }
}

// #[cfg(test)]
// mod test_c_sharp {
//     use super::AdapterCSharp;
//     use crate::adapters::{c_sharp::dotnet::test_c_sharp_dotnet, test_util::convert_file_path};

//     #[test]
//     fn test_adapter_c_sharp_dotnet() {
//         let results = convert_file_path::<AdapterCSharp>("./tool_output/java/jmh/six.json");
//         test_c_sharp_dotnet::validate_adapter_c_sharp_dotnet(results);
//     }
// }
