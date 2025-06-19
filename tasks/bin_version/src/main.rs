const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[expect(clippy::print_stdout)]
fn main() {
    println!("{API_VERSION}");
}
