//! Fixed provider-free bootstrap; no arbitrary executable or model transport.

fn main() {
    #[cfg(target_os = "linux")]
    let ok = cortex_harness::domain_bootstrap::nonce_bootstrap_main(
        &std::env::args().skip(1).collect::<Vec<_>>(),
    )
    .is_ok();
    #[cfg(not(target_os = "linux"))]
    let ok = false;
    if !ok {
        // No raw arguments, environment or payload in diagnostics.
        eprintln!("domain bootstrap: refused or interrupted");
        std::process::exit(1);
    }
}
