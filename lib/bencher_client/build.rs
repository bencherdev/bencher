#![allow(let_underscore_drop, clippy::unwrap_used)]

#[cfg(unix)]
use std::os::unix;
use std::{fs, path::Path};

fn main() {
    let src = "../../services/api/swagger.json";
    println!("cargo:rerun-if-changed={src}");
    let file = fs::File::open(src).unwrap();
    let spec = serde_json::from_reader(file).unwrap();
    let mut generator = progenitor::Generator::new(
        progenitor::GenerationSettings::default()
            .with_interface(progenitor::InterfaceStyle::Builder),
    );

    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = Path::new(&std::env::var("OUT_DIR").unwrap()).to_path_buf();
    out_file.push("codegen.rs");

    #[cfg(unix)]
    {
        let _ = fs::remove_file("./codegen.rs");
        let _ = unix::fs::symlink(&out_file, "./codegen.rs");
    }

    fs::write(out_file, content).unwrap();
}
