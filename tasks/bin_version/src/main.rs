const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[allow(clippy::print_stdout)]
fn main() {
    println!("{API_VERSION}");
}
