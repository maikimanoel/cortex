//! Read-only presentation of Harness metadata, never a backend activation path.
use cortex_harness::host_preflight::{Ceiling, HostPreflight, Observed, inspect_host};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::{self, Write},
};

const HELP: &str = "Usage: cortex host inspect [--json]\n\nRead bounded Linux host metadata without launching a process, activating a\nbackend, changing policy, or writing a store. No credentials or sessions are read.\nConfigured limits describe the observer's shared scope, not an attempt boundary.\nMetadata cannot establish delegation, enforcement, qualification or permission.\nOutput omits cgroup paths, raw file contents, device/inode IDs and user IDs.\nExit codes: 0 inspected (including blocked qualification), 2 invalid arguments,\n1 output failure. Metadata read failures remain explicit in the report.\n";

fn observed<T>(value: &Observed<T>, encode: impl FnOnce(&T) -> Value) -> Value {
    match value {
        Observed::Value(v) => json!({"status":"OBSERVED_METADATA", "value":encode(v)}),
        Observed::Unavailable(issue) => {
            json!({"status":"UNAVAILABLE", "reason":format!("{issue:?}")})
        }
    }
}
fn ceiling(value: &Ceiling) -> Value {
    match value {
        Ceiling::Unlimited => json!({"kind":"UNLIMITED"}),
        Ceiling::Limited(n) => json!({"kind":"CONFIGURED", "quantity":n}),
    }
}
fn view(report: &HostPreflight) -> Value {
    let c = &report.cgroup;
    let controls: Vec<_> = report
        .control_gaps()
        .iter()
        .map(|gap| {
            json!({
                "control":format!("{:?}", gap.control),
                "qualification":"BLOCKED",
                "behavioral_test":"NOT_RUN",
                "causes":gap.reasons.iter().map(|reason|format!("{reason:?}")).collect::<Vec<_>>()
            })
        })
        .collect();
    json!({
        "schema":"cortex.host-preflight.v1",
        "status":"INSPECTED",
        "host_qualification":"BLOCKED",
        "execution_authority":"INSPECTION_CONFERS_NONE",
        "effects_performed":false,
        "backend_activated":false,
        "live_executor_supported":false,
        "observation_scope":"OBSERVER_SHARED_SCOPE_METADATA_ONLY",
        "atomic_snapshot":false,
        "cgroup":{
            "depth":observed(&c.depth, |v| json!(v)),
            "unified_mount":observed(&c.unified_mount, |v| json!(v)),
            "controllers":observed(&c.controllers, |v| json!({"cpu":v.cpu,"memory":v.memory,"pids":v.pids})),
            "subtree_control":observed(&c.subtree_control, |v| json!({"cpu":v.cpu,"memory":v.memory,"pids":v.pids})),
            "cpu_max":observed(&c.cpu_max, |v| json!({"quota_microseconds":ceiling(&v.quota),"period_microseconds":v.period_micros})),
            "memory_max_bytes":observed(&c.memory_max, ceiling),
            "swap_max_bytes":observed(&c.swap_max, ceiling),
            "pids_max":observed(&c.pids_max, ceiling),
            "populated":observed(&c.populated, |v| json!(v)),
            "membership_consistent":c.membership_consistent,
            "delegation":"UNESTABLISHED"
        },
        "namespaces":{
            "max_user_namespaces":observed(&report.namespaces.max_user_namespaces, |v| json!(v)),
            "unprivileged_userns_clone":observed(&report.namespaces.unprivileged_userns_clone, |v| json!(v)),
            "creation_test":"NOT_RUN"
        },
        "bubblewrap":observed(&report.bubblewrap, |v| json!({"bytes":v.bytes,"mode":v.mode,"executed":false,"version":"UNESTABLISHED","byte_identity":"UNESTABLISHED"})),
        "controls":controls,
        "capture":{"metadata_reads":report.reads,"bytes_read":report.bytes_read,"elapsed_ms":report.elapsed_ms,"raw_metadata_retained":false},
        "limitations":["Metadata is not denial evidence or a resource reservation", "No owned attempt subtree or whole-process-set quiescence is established", "No provider, credential, filesystem or network qualification is performed", "Read deadlines are cooperative on responsive kernel interfaces"]
    })
}

pub fn run(mut args: impl Iterator<Item = OsString>, out: &mut impl Write) -> io::Result<u8> {
    let first = args.next();
    let second = args.next();
    if args.next().is_some() {
        out.write_all(HELP.as_bytes())?;
        return Ok(2);
    }
    if (first.as_deref() == Some(std::ffi::OsStr::new("--help")) && second.is_none())
        || (first.as_deref() == Some(std::ffi::OsStr::new("inspect"))
            && second.as_deref() == Some(std::ffi::OsStr::new("--help")))
    {
        out.write_all(HELP.as_bytes())?;
        return Ok(0);
    }
    if first.as_deref() != Some(std::ffi::OsStr::new("inspect"))
        || second.as_ref().is_some_and(|v| v != "--json")
    {
        out.write_all(HELP.as_bytes())?;
        return Ok(2);
    }
    let report = inspect_host();
    if second.is_some() {
        serde_json::to_writer(&mut *out, &view(&report)).map_err(io::Error::other)?;
        writeln!(out)?;
    } else {
        writeln!(out, "Host prerequisites inspected; qualification BLOCKED")?;
        writeln!(
            out,
            "Scope: observer shared metadata; delegation UNESTABLISHED"
        )?;
        writeln!(out, "CPU rate: {:?}", report.cgroup.cpu_max)?;
        writeln!(
            out,
            "Charged memory: {:?}; swap: {:?}",
            report.cgroup.memory_max, report.cgroup.swap_max
        )?;
        writeln!(out, "Process/thread ceiling: {:?}", report.cgroup.pids_max)?;
        for gap in report.control_gaps() {
            writeln!(
                out,
                "{:?}: BLOCKED; behavior NOT_RUN; {:?}",
                gap.control, gap.reasons
            )?;
        }
        writeln!(
            out,
            "No backend activated; inspection confers no execution authority"
        )?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_failure_retains_io_cause() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            run([OsString::from("--help")].into_iter(), &mut Broken)
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
