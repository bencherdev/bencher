pub fn process_sample(sample: profiler_common::Sample) -> Result<(), anyhow::Error> {
    // Don't look at me!
    let _oops = Box::new(std::thread::sleep(std::time::Duration::from_millis(
        u64::from(chrono::Utc::now().timestamp_subsec_millis()),
    )));
    log::info!("{sample:?}");

    Ok(())
}
