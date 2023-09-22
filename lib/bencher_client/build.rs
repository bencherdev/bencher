fn main() {
    let src = "../../services/console/src/content/api/swagger.json";
    println!("cargo:rerun-if-changed={src}");
    let file = std::fs::File::open(src).unwrap();
    let spec = serde_json::from_reader(file).unwrap();
    let mut generator = progenitor::Generator::new(
        progenitor::GenerationSettings::default()
            .with_interface(progenitor::InterfaceStyle::Builder),
    );

    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).to_path_buf();
    out_file.push("codegen.rs");

    let _ = std::fs::remove_file("./codegen.rs");
    let _ = std::os::unix::fs::symlink(&out_file, "./codegen.rs");

    std::fs::write(out_file, content).unwrap();
}
