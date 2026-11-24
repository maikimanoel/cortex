//! Policy-bound trusted executable components. Preparation is not permission.
//!
//! The strict path requires an explicitly authorized delegated cgroup. There is
//! no unrestricted fallback, provider route or implicit production resource policy.
//! Returned process observations never settle canonical reservations.
pub mod accounting;
pub mod artifacts;
pub mod cgroup;
mod policy;
pub mod transport;
pub use policy::{ExecutionError, ExecutionPlan, ExecutionSpec, FileRole, FileSpec, PinnedFile};
pub mod backend;
pub mod bootstrap;
pub mod managed;

pub mod managed_backend;
pub mod normal_end;
pub mod p1;

pub(crate) mod stdio;
