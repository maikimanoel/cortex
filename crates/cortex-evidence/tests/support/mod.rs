use cortex_evidence::{
    Provenance,
    unified::{ObservationTimes, VerificationProfile},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}
pub fn times() -> ObservationTimes {
    ObservationTimes::new(time(100), time(101)).unwrap()
}
pub fn profile(version: &str) -> VerificationProfile {
    VerificationProfile::new(
        Provenance::new(
            "fixture verifier",
            "single retained contract criterion",
            "offline deterministic checks",
        )
        .unwrap(),
        version,
        "fixture environment",
        &[],
    )
    .unwrap()
}
