//! Explicit presentation versions. Legacy spellings are protocol data, not Rust names.
use cortex_core::program::scheduling::Resource;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Schema {
    Legacy,
    Canonical,
}

impl Schema {
    pub(super) fn program_name(self) -> &'static str {
        match self {
            Self::Legacy => "cortex.program.v1",
            Self::Canonical => "cortex.program.v2",
        }
    }
    pub(super) fn tasks_key(self) -> &'static str {
        match self {
            Self::Legacy => "missions",
            Self::Canonical => "tasks",
        }
    }
    pub(super) fn task_label(self) -> &'static str {
        match self {
            Self::Legacy => "Mission",
            Self::Canonical => "Task",
        }
    }
    pub(super) fn guidance_key(self) -> &'static str {
        match self {
            Self::Legacy => "next_permissible_action",
            Self::Canonical => "next_step_guidance",
        }
    }
    pub(super) fn task_blocker(self) -> &'static str {
        // The v1 protocol used the redacted type Debug spelling for non-edge blockers.
        match self {
            Self::Legacy => "MissionBlocker([redacted])",
            Self::Canonical => "TaskBlocker([redacted])",
        }
    }
    pub(super) fn resource(self, resource: Resource) -> &'static str {
        match (self, resource) {
            (Self::Legacy, Resource::WorkerSlots) => "Workers",
            (Self::Canonical, Resource::WorkerSlots) => "WorkerSlots",
            (Self::Legacy, Resource::BuildJobs) => "Builds",
            (Self::Canonical, Resource::BuildJobs) => "BuildJobs",
            (Self::Legacy, Resource::TestProcesses) => "Tests",
            (Self::Canonical, Resource::TestProcesses) => "TestProcesses",
            (Self::Legacy, Resource::CpuMillicores) => "CpuMillis",
            (Self::Canonical, Resource::CpuMillicores) => "CpuMillicores",
            (Self::Legacy, Resource::ElapsedMillis) => "TimeMillis",
            (Self::Canonical, Resource::ElapsedMillis) => "ElapsedMillis",
            (_, Resource::RamBytes) => "RamBytes",
            (_, Resource::DiskBytes) => "DiskBytes",
            (_, Resource::OutputBytes) => "OutputBytes",
            (_, Resource::CiJobs) => "CiJobs",
            (_, Resource::ModelTokens) => "ModelTokens",
        }
    }
    pub(super) fn project_program(self, value: &mut Value) {
        if self == Self::Legacy {
            return;
        }
        value["schema"] = json!(self.program_name());
        rename(value, "missions", "tasks");
        rename(
            value,
            "mission_execution_flags_scope",
            "task_execution_flags_scope",
        );
        for task in items(&mut value["tasks"]) {
            rename(task, "mission_status", "task_status");
        }
        for stream in items(&mut value["workstreams"]) {
            rename(stream, "missions", "tasks");
        }
        for package in items(&mut value["packages"]) {
            rename(package, "mission", "task");
        }
        for (field, version) in [
            ("fixture_projection", "cortex.fixture-effects.v2"),
            ("supervised_projection", "cortex.supervised-fixtures.v2"),
        ] {
            if let Some(projection) = value.get_mut(field).filter(|v| !v.is_null()) {
                projection["schema"] = json!(version);
                for effect in items(&mut projection["effects"]) {
                    rename(effect, "mission", "task");
                    rename(effect, "mission_acceptance", "task_acceptance");
                }
            }
        }
        if let Some(projection) = value.get_mut("domain_projection") {
            projection["schema"] = json!("cortex.domains.v2");
            for domain in items(&mut projection["domains"]) {
                rename(
                    domain,
                    "present_capability_evidence",
                    "closure_receipt_status",
                );
                rename(
                    domain,
                    "closure_evidence_recorded",
                    "closure_receipt_recorded",
                );
                rename(domain, "closure_source", "closure_receipt_source");
                if domain["closure_receipt_recorded"] == false {
                    domain["closure_receipt_status"] = json!("NO_CLOSURE_RECEIPT_RETAINED");
                    domain["closure_receipt_source"] = Value::Null;
                }
                rename(domain, "next_permissible_action", "next_step_guidance");
                domain["next_step_guidance"] = json!(match domain["next_step_guidance"].as_str() {
                    Some("FRESH_AUTHORITY_AND_QUALIFICATION_REQUIRED_BEFORE_CREATION") =>
                        "REVIEW_CURRENT_CREATION_PREREQUISITES",
                    Some(
                        "PREPARED_RELEASE_WITHHELD_EXPLICIT_OPERATION_REQUIRES_FRESH_AUTHORITY_AND_QUALIFICATION",
                    ) => "REVIEW_RETAINED_BINDING_AND_CURRENT_RELEASE_PREREQUISITES",
                    Some("REVIEW_REMAINING_DEBT_AND_CAPACITY_BEFORE_DISTINCT_WORK") =>
                        "REVIEW_REMAINING_DEBT_AND_CAPACITY_BEFORE_DISTINCT_WORK",
                    Some("NO_CREATION_REQUEST_RECORDED_NEW_WORK_REQUIRES_ALL_GATES") =>
                        "REVIEW_ALL_NEW_WORK_GATES_NO_CREATION_REQUEST_RECORDED",
                    _ => "RETAIN_APPLICABLE_HOLDS_PENDING_EXACT_DOMAIN_EVIDENCE",
                });
                domain["guidance_scope"] = json!("RETAINED_STATE_ADVISORY_NOT_EXHAUSTIVE");
                domain["prepared_handle_availability"] = json!("UNESTABLISHED_BY_JOURNAL");
                let inspection = &mut domain["native_closure_inspection"];
                rename(inspection, "status", "native_support_status");
                inspection["native_support_status"] = json!("INCOMPLETE");
                rename(inspection, "latest", "latest_direct_observation");
                rename(
                    inspection,
                    "retained_observations",
                    "retained_native_observation_count",
                );
                rename(inspection, "next_action", "guidance");
                inspection["guidance"] =
                    json!("DO_NOT_USE_PARTIAL_NATIVE_SAMPLES_AS_CLOSURE_OR_SETTLEMENT");
            }
        }
    }
}

fn rename(value: &mut Value, old: &str, new: &str) {
    if let Some(object) = value.as_object_mut()
        && let Some(entry) = object.remove(old)
    {
        object.insert(new.to_owned(), entry);
    }
}

fn items(value: &mut Value) -> impl Iterator<Item = &mut Value> {
    value.as_array_mut().into_iter().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_and_redacted_blocker_protocols_are_explicit() {
        // Baseline v1 tokens are specified independently of the enum's Debug names.
        let expected = [
            (Resource::WorkerSlots, "Workers", "WorkerSlots"),
            (Resource::BuildJobs, "Builds", "BuildJobs"),
            (Resource::TestProcesses, "Tests", "TestProcesses"),
            (Resource::CpuMillicores, "CpuMillis", "CpuMillicores"),
            (Resource::ElapsedMillis, "TimeMillis", "ElapsedMillis"),
            (Resource::RamBytes, "RamBytes", "RamBytes"),
            (Resource::DiskBytes, "DiskBytes", "DiskBytes"),
            (Resource::OutputBytes, "OutputBytes", "OutputBytes"),
            (Resource::CiJobs, "CiJobs", "CiJobs"),
            (Resource::ModelTokens, "ModelTokens", "ModelTokens"),
        ];
        for (resource, legacy, canonical) in expected {
            assert_eq!(Schema::Legacy.resource(resource), legacy);
            assert_eq!(Schema::Canonical.resource(resource), canonical);
        }
        assert_eq!(Schema::Legacy.task_blocker(), "MissionBlocker([redacted])");
        assert_eq!(Schema::Canonical.task_blocker(), "TaskBlocker([redacted])");
    }
}

#[cfg(test)]
mod guidance_tests {
    use super::*;

    #[test]
    fn selected_guidance_is_conditional_and_retained_binding_is_not_a_handle() {
        for (legacy, canonical) in [
            (
                "FRESH_AUTHORITY_AND_QUALIFICATION_REQUIRED_BEFORE_CREATION",
                "REVIEW_CURRENT_CREATION_PREREQUISITES",
            ),
            (
                "PREPARED_RELEASE_WITHHELD_EXPLICIT_OPERATION_REQUIRES_FRESH_AUTHORITY_AND_QUALIFICATION",
                "REVIEW_RETAINED_BINDING_AND_CURRENT_RELEASE_PREREQUISITES",
            ),
            (
                "REVIEW_REMAINING_DEBT_AND_CAPACITY_BEFORE_DISTINCT_WORK",
                "REVIEW_REMAINING_DEBT_AND_CAPACITY_BEFORE_DISTINCT_WORK",
            ),
            (
                "NO_CREATION_REQUEST_RECORDED_NEW_WORK_REQUIRES_ALL_GATES",
                "REVIEW_ALL_NEW_WORK_GATES_NO_CREATION_REQUEST_RECORDED",
            ),
            (
                "OBSERVE_EXACT_DOMAIN_OR_RETAIN_HOLD_NEVER_BLIND_RETRY",
                "RETAIN_APPLICABLE_HOLDS_PENDING_EXACT_DOMAIN_EVIDENCE",
            ),
        ] {
            let mut value = json!({"schema":"cortex.program.v1", "missions":[], "domain_projection": {"domains":[{
                "next_permissible_action":legacy, "closure_evidence_recorded":false,
                "present_capability_evidence":"NO_CLOSURE_PROOF", "closure_source":"CALLER_ASSERTION_NOT_AUTHENTICATED",
                "retry_authorized":false, "native_closure_inspection":{"status":"HOLD_INCOMPLETE_NATIVE_PROOF", "latest":null, "retained_observations":0}
            }]}});
            let original = value.clone();
            Schema::Legacy.project_program(&mut value);
            assert_eq!(value, original);
            Schema::Canonical.project_program(&mut value);
            let domain = &value["domain_projection"]["domains"][0];
            assert_eq!(domain["next_step_guidance"], canonical);
            assert_eq!(
                domain["guidance_scope"],
                "RETAINED_STATE_ADVISORY_NOT_EXHAUSTIVE"
            );
            assert_eq!(
                domain["prepared_handle_availability"],
                "UNESTABLISHED_BY_JOURNAL"
            );
            assert_eq!(
                domain["closure_receipt_status"],
                "NO_CLOSURE_RECEIPT_RETAINED"
            );
            assert!(domain["closure_receipt_source"].is_null());
            assert_eq!(domain["retry_authorized"], false);
        }
    }
}
