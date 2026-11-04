//! Explicit, bounded encrypted local persistence. Domain decisions remain in Core/Evidence.
//! Linux, one trusted local operator and cooperating clients; no worker authority.

#[cfg(target_os = "linux")]
pub mod history_store;

/// Explicit encrypted metadata-only program coordination store.
#[cfg(target_os = "linux")]
pub mod program_store;
