fn main() {
    let schema = schemars::schema_for!(reports::Reports);
    let schema = serde_json::to_string_pretty(&schema).unwrap();
    println!("{schema}");

    let schema = schemars::schema_for!(reports::Data);
    let schema = serde_json::to_string_pretty(&schema).unwrap();
    println!("{schema}");
}
