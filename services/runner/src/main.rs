fn main() {
    #[cfg(all(feature = "plus", target_os = "linux"))]
    {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async {
            if let Err(e) = bencher_runner::run().await {
                eprintln!("Runner error: {e}");
                std::process::exit(1);
            }
        });
    }

    #[cfg(not(all(feature = "plus", target_os = "linux")))]
    {
        eprintln!("Bencher Runner requires the `plus` feature and Linux");
        std::process::exit(1);
    }
}
