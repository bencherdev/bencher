pub mod dotnet;

use crate::{Adapter, AdapterResults};
use dotnet::AdapterCSharpDotNet;

pub struct AdapterCSharp;

impl Adapter for AdapterCSharp {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterCSharpDotNet::parse(input)
    }
}

#[cfg(test)]
mod test_c_sharp {
    use super::AdapterCSharp;
    use crate::adapters::{c_sharp::dotnet::test_c_sharp_dotnet, test_util::convert_file_path};

    #[test]
    fn test_adapter_c_sharp_dotnet() {
        let results = convert_file_path::<AdapterCSharp>("./tool_output/c_sharp/dotnet/two.json");
        test_c_sharp_dotnet::validate_adapter_c_sharp_dotnet(results);
    }
}
