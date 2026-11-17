//! Trusted single-threaded executable bootstrap; never a host fallback runner.
fn main() {
    #[cfg(target_os = "linux")]
    {
        let args = std::env::args().skip(1).take(7).collect::<Vec<_>>();
        if let Err(error) = cortex_harness::native_execution::bootstrap::run(&args) {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "linux"))]
    std::process::exit(1);
}
