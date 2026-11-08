//! Narrow source-API regression guards; wire tokens are tested in Storage.
//! These checks do not ban ordinary technical words or replace semantic tests.

#[test]
fn retired_public_api_declarations_do_not_return_to_active_modules() {
    let parallel = include_str!("../src/parallel.rs");
    for retired in [
        "pub struct Mission {",
        "pub struct MissionContext {",
        "pub struct MissionScope {",
        "pub enum MissionStatus {",
        "pub enum MissionBlocker {",
    ] {
        assert!(
            !parallel.contains(retired),
            "retired public declaration: {retired}"
        );
    }
    let program = include_str!("../src/program/mod.rs");
    assert!(!program.contains("pub struct Plan {"));
    assert!(!program.contains("id!(ResourceId)"));
    let scheduling = include_str!("../src/program/scheduling.rs");
    for retired in [
        "    Workers,",
        "    Builds,",
        "    Tests,",
        "    CpuMillis,",
        "    TimeMillis,",
        "    RamBytes,",
    ] {
        assert!(
            !scheduling.contains(retired),
            "retired resource declaration: {retired}"
        );
    }
    let knowledge = include_str!("../src/knowledge.rs");
    assert!(!knowledge.contains("pub fn complete("));
    let source_set = include_str!("../src/knowledge/source_set.rs");
    assert!(!source_set.contains("pub fn complete("));
}
