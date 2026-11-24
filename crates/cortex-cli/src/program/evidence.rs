//! Schema-3 retained Evidence and release-witness correspondence, without recapture.
use cortex_evidence::{
    program::{Command, CoordinationKernel},
    unified::{Applicability, AssessmentResult, ValidationContext, Validity},
};
use serde_json::{Value, json};

pub(super) fn project(kernel: &CoordinationKernel, value: &mut Value) {
    value["evidence"] = json!(kernel.records().iter().map(|record| {
        let observation = record.observation();
        let assessment = record.assessment();
        // Inspection may discover sticky invalidation in a draft. Never mutate
        // canonical records while projecting a read-only journal.
        let mut draft = record.clone();
        let view = draft.inspect(&ValidationContext::new(
            observation.subject().contract(),
            assessment.and_then(|a| a.candidate_at_assessment()),
            assessment.and_then(|a| a.profile_at_assessment()),
        ));
        json!({
            "observation_scope": observation.id().scope(),
            "observation_sequence": observation.id().sequence(),
            "task": observation.subject().contract().task().id().as_str(),
            "contract_revision": observation.subject().contract().revision().get(),
            "candidate": observation.subject().contract().candidate().commit().as_str(),
            "mechanism_version": observation.profile().version(),
            "assessment": assessment.map(|a| a.id().as_str()),
            "historical_result": assessment.map(|a| match a.result() {
                AssessmentResult::Pass => "PASS", AssessmentResult::Fail => "FAIL",
                AssessmentResult::Unknown => "UNKNOWN", AssessmentResult::NotRun => "NOT_RUN",
                AssessmentResult::Blocked => "BLOCKED", AssessmentResult::NotApplicable => "N/A",
            }),
            "applicability": assessment.map(|a| match a.applicability() {
                Applicability::Applicable => "APPLICABLE", Applicability::NotApplicable => "NOT_APPLICABLE",
                Applicability::Uncertain => "UNCERTAIN",
            }),
            "contextual_validity": match view.validity() {
                Some(Validity::Current) => "CURRENT", Some(Validity::Stale) => "STALE", None => "UNESTABLISHED",
            },
            "validity_basis": "RETAINED_ASSESSMENT_CONTEXT_NOT_LIVE_VERIFICATION",
            "authority": "NONE",
        })
    }).collect::<Vec<_>>());
    for decision in value["support_decisions"]
        .as_array_mut()
        .expect("decision array")
    {
        let release = kernel
            .operations()
            .iter()
            .find_map(|operation| {
                if decision["operation"].as_str() != Some(operation.id.as_str()) {
                    return None;
                }
                match &operation.command {
                    Command::Release(release) => Some(release.as_ref()),
                    Command::ReleaseMaterial(material) => Some(&material.release),
                    _ => None,
                }
            })
            .expect("retained release decision has its canonical operation");
        decision["witnesses"] = json!(
            release
                .bindings
                .iter()
                .map(|binding| json!({
                    "predicate": binding.predicate().as_str(),
                    "observation_scope": binding.evidence().observation().scope(),
                    "observation_sequence": binding.evidence().observation().sequence(),
                    "assessment": binding.evidence().assessment().map(|a| a.as_str()),
                    "artifact": binding.artifact().id().as_str(),
                    "artifact_revision": binding.artifact().revision().get(),
                    "artifact_candidate": binding.artifact().candidate().as_str(),
                }))
                .collect::<Vec<_>>()
        );
    }
}
