//! Redacted retained facts. Inspection never contacts a manager.
use super::schema::{ResourceProjection, Schema};
use cortex_core::program::domain::*;
use serde_json::{Value, json};

pub(super) fn view(
    projection: &DomainProjection,
    schema: Schema,
    resources: &mut ResourceProjection,
) -> Value {
    let pool = projection.pool();
    let capacities: Vec<_> = pool
        .into_iter()
        .flat_map(|p| &p.capacities)
        .map(|c| {
            json!({"dimension":resources.project(schema, &c.resource),
            "declared_total":c.available,"verification_reserve":c.verification_reserve,
            "held":projection.held(c.resource.clone()).ok(),
            "enforcement":"NOT_ESTABLISHED_BY_JOURNAL"})
        })
        .collect();
    let consumption: Vec<_> = pool
        .into_iter()
        .flat_map(|p| &p.consumption)
        .map(|b| {
            let accounted = projection.consumed_or_reserved(b.dimension);
            json!({"dimension":format!("{:?}",b.dimension),"declared_total":b.total,
            "reserve":b.reserve,"per_operation":b.per_operation,
            "known_consumption_or_future_debit":accounted.as_ref().ok(),
            "blocked_by":accounted.err().map(|e|format!("{e:?}")),
            "reservation_is_proven_usage_bound":false})
        })
        .collect();
    let records: Vec<_> = projection.domains().iter().map(|d| {
        let capacity: Vec<_> = pool.into_iter().flat_map(|p| &p.capacities).map(|c| {
            json!({"dimension":resources.project(schema, &c.resource),"held":d.holds(c.resource.clone()),
                "admitted_quantity":d.intent.effect.demand.iter().find(|r|r.resource==c.resource).and_then(|r|r.amount)})
        }).collect();
        let usage: Vec<_> = d.intent.consumption.iter().map(|r| {
            let measured = d.measured(r.dimension);
            json!({"dimension":format!("{:?}",r.dimension),"retained_declared_debit":r.amount,
                "measured_final":measured,"unresolved_debt":d.unresolved_consumption(r.dimension),
                "proven_upper_bound":null})
        }).collect();
        let historical = match d.outcome {
            HistoricalOutcome::NotDispatched => json!({"kind":"NOT_DISPATCHED"}),
            HistoricalOutcome::Unknown => json!({"kind":"UNKNOWN"}),
            HistoricalOutcome::Reported(outcome) => json!({"kind":"REPORTED","observation":format!("{outcome:?}"),"accepted":false}),
        };
        let next = match d.phase {
            DomainPhase::Reserved if !d.cancelled && d.disposition==AdministrativeDisposition::Active => "FRESH_AUTHORITY_AND_QUALIFICATION_REQUIRED_BEFORE_CREATION",
            DomainPhase::Bound if !d.cancelled && d.disposition==AdministrativeDisposition::Active && d.binding_observation.is_some() => "PREPARED_RELEASE_WITHHELD_EXPLICIT_OPERATION_REQUIRES_FRESH_AUTHORITY_AND_QUALIFICATION",
            DomainPhase::Closed => "REVIEW_REMAINING_DEBT_AND_CAPACITY_BEFORE_DISTINCT_WORK",
            DomainPhase::CancelledBeforeCreate => "NO_CREATION_REQUEST_RECORDED_NEW_WORK_REQUIRES_ALL_GATES",
            _ => "OBSERVE_EXACT_DOMAIN_OR_RETAIN_HOLD_NEVER_BLIND_RETRY",
        };
        json!({"domain":d.intent.id.as_str(),"program":d.intent.effect.scope.program.as_str(),
            "effect":d.intent.effect.id.as_str(),"attempt":d.intent.effect.scope.attempt.as_str(),
            "generation":d.intent.effect.scope.generation.get(),
            "historical_outcome":historical,"retained_domain_phase":format!("{:?}",d.phase),
            "present_capability_evidence":if d.closure.is_some(){"CLOSURE_RECEIPT_RETAINED"}else{"NO_CLOSURE_PROOF"},
            "domain_identity_recorded":d.identity.is_some(),"cancellation_requested":d.cancelled,
            "retained_binding_observation":binding_view(d.binding_observation.as_ref(), schema, resources),
            "native_closure_inspection":closure_view(d, schema, resources),
            "administrative_disposition":format!("{:?}",d.disposition),
            "reusable_capacity":capacity,"cumulative_consumption":usage,
            "closure_evidence_recorded":d.closure.is_some(),"additional_settlements":d.settlements.len(),"closure_source":"CALLER_ASSERTION_NOT_AUTHENTICATED",
            "artifact_acceptance":false,"retry_authorized":false,"next_permissible_action":next})
    }).collect();
    json!({"schema":"cortex.domains.v1","mode":"RECOVERABLE_DOMAIN_FIXTURES",
        "pool_registered":pool.is_some(),"pool_identity":"PRIVATE_VALUES_OMITTED",
        "pool_correspondence":"SINGLE_OPERATOR_ASSERTION_ONE_AUTHORITATIVE_COLLECTION",
        "logical_tick":projection.last_tick(),"domains":records,
        "reusable_capacity":capacities,"cumulative_consumption":consumption,
        "inspection_authority":"NONE","host_reinspection":false,
        "current_liveness":"NOT_OBSERVED","host_qualification":"NOT_INFERRED",
        "live_provider":"UNAVAILABLE","physical_reserves":"NOT_ESTABLISHED"})
}

fn recursive_sample(
    record: &DomainRecord,
    schema: Schema,
    resources: &mut ResourceProjection,
) -> Value {
    record
        .closure_tree_observations
        .last()
        .map(|tree| {
            let mut value = json!({
                "observed_at":tree.sample.observed_at,
                "nodes":tree.nodes.len(),
                "observed_members":tree.nodes.iter().map(|node|node.members.len()).sum::<usize>(),
                "any_population":tree.nodes.iter().any(|node|node.populated),
                "sampled_cpu_micros":tree.sample.cpu_micros,
                "sampled_memory_peak_bytes":tree.sample.memory_peak,
                "measurement_is_final":false,
                "private_identity_values":"OMITTED"
            });
            if schema == Schema::Current {
                value
                    .as_object_mut()
                    .expect("object")
                    .remove("sampled_memory_peak_bytes");
                value["memory_peak_observation"] = tree
                    .sample
                    .charged_memory_peak()
                    .ok()
                    .as_ref()
                    .map(|v| resources.observation(v))
                    .unwrap_or(json!({"status":"UNESTABLISHED"}));
            }
            value
        })
        .unwrap_or(Value::Null)
}

fn closure_view(
    record: &DomainRecord,
    schema: Schema,
    resources: &mut ResourceProjection,
) -> Value {
    let inspection = record.closure_inspection();
    let latest = record.closure_observations.last().map(|observation| {
        let mut value = json!({
            "observed_at":observation.observed_at,
            "direct_member_count":observation.direct_members.len(),
            "main_present":observation.observed_main.is_some(),
            "populated":observation.populated,
            "restart_prevented_readback":observation.restart_prevented,
            "sampled_cpu_micros":observation.cpu_micros,
            "sampled_memory_peak_bytes":observation.memory_peak,
            "measurement_is_final":false,
            "private_identity_values":"OMITTED"
        });
        if schema == Schema::Current {
            value
                .as_object_mut()
                .expect("object")
                .remove("sampled_memory_peak_bytes");
            value["memory_peak_observation"] = observation
                .charged_memory_peak()
                .ok()
                .as_ref()
                .map(|v| resources.observation(v))
                .unwrap_or(json!({"status":"UNESTABLISHED"}));
        }
        value
    });
    let mut view = json!({
        "status":"HOLD_INCOMPLETE_NATIVE_PROOF",
        "retained_observations":inspection.retained_observations,
        "latest":latest,
        "native_observation_missing_proofs":inspection.missing.iter().map(|gap|format!("{gap:?}")).collect::<Vec<_>>(),
        "separate_caller_receipt_recorded":inspection.receipt_recorded,
        "source":"CALLER_OBSERVATION_NOT_AUTHENTICATED",
        "membership_coverage":"BOUNDED_DIRECT_MEMBERS_NON_ATOMIC",
        "current_liveness":"NOT_OBSERVED",
        "settlement_authorized":false,
        "next_action":"RETAIN_HOLDS_UNTIL_APPLICABLE_WHOLE_DOMAIN_AND_RESOURCE_PROOF"
    });
    if schema != Schema::Legacy {
        view["latest_recursive_observation"] = recursive_sample(record, schema, resources);
        view["recursive_sample_membership_coverage"] =
            json!("BOUNDED_RECURSIVE_CGROUP_READBACK_NON_ATOMIC");
        view["combined_native_inspection_basis"] =
            json!("LATEST_DIRECT_OR_RECURSIVE_SAMPLE_AND_LATEST_RECURSIVE_POPULATION_AND_MEMBERS");
        view["whole_domain_closure_proven"] = json!(false);
        view["helper_and_stream_coverage_proven"] = json!(false);
        view["snapshot_exclusion_proven"] = json!(false);
        view["final_consumption_proven"] = json!(false);
    }
    view
}

fn binding_view(
    observation: Option<&DomainBindingObservation>,
    schema: Schema,
    resources: &mut ResourceProjection,
) -> Value {
    let mut value = match observation {
        None => json!({"status":"NOT_RECORDED","current_liveness":"NOT_OBSERVED"}),
        Some(observation) => json!({
            "status":"RECORDED",
            "source":"CALLER_OBSERVATION_NOT_AUTHENTICATED",
            "configuration_correspondence_recorded":true,
            "main_process_birth_identity_recorded":true,
            "direct_member_count":observation.direct_members.len(),
            "membership_coverage":"DIRECT_CGROUP_MEMBERS_NON_ATOMIC_OBSERVATION",
            "populated_at_binding":observation.populated,
            "current_liveness":"NOT_OBSERVED",
            "private_identity_values":"OMITTED",
            "controls":{
                "cpu_quota_micros":observation.controls.cpu_quota_us,
                "cpu_period_micros":observation.controls.cpu_period_us,
                "charged_memory_bytes":observation.controls.memory_bytes,
                "swap_bytes":observation.controls.swap_bytes,
                "tasks":observation.controls.tasks,
                "runtime_max_micros":observation.controls.runtime_max_micros,
                "timeout_stop_micros":observation.controls.timeout_stop_micros,
                "restart_prevented_readback":observation.controls.restart_prevented,
                "kill_control_group_readback":observation.controls.kill_control_group,
                "no_new_privileges_readback":observation.controls.no_new_privileges,
                "send_sigkill_readback":observation.controls.send_sigkill,
                "remain_after_exit_readback":observation.controls.remain_after_exit
            },
            "whole_domain_closure_proven":false,
            "snapshot_exclusion_proven":false,
            "qualification":"NOT_ESTABLISHED_BY_READBACK"
        }),
    };
    if schema == Schema::Current
        && let Some(observation) = observation
    {
        value["memory_limit"] = observation
            .charged_memory_limit()
            .ok()
            .as_ref()
            .map(|v| resources.limit(v))
            .unwrap_or(json!({"status":"UNESTABLISHED"}));
    }
    value
}
