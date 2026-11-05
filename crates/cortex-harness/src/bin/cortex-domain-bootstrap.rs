//! Fixed provider-free bootstrap; no arbitrary executable or model transport.
fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .take(4)
        .map(|value| value.into_string())
        .collect::<Result<Vec<_>, _>>();
    let Ok(arguments) = arguments else {
        eprintln!("domain bootstrap: phase=Arguments kind=InvalidInput os=None");
        std::process::exit(1);
    };
    #[cfg(target_os = "linux")]
    if let Err(error) = cortex_harness::domain_bootstrap::nonce_bootstrap_diagnosed(&arguments) {
        // Fixed fields only; never raw arguments, environment, descriptor targets or payload.
        eprintln!("domain bootstrap: {error}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("domain bootstrap: unsupported platform");
        std::process::exit(1);
    }
}
