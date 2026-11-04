//! Explicit read-only encrypted program inspection; no hidden recovery or dispatch.
use cortex_core::{
    parallel::*,
    program::{AttemptStatus, operational::EffectMode, supervision::*},
};
use cortex_evidence::program::CoordinationKernel;
use cortex_storage::{
    history_store::StoreKey,
    program_store::{ProgramStore, ReadLimits},
};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::{self, Read, Write},
    os::unix::fs::{FileTypeExt, OpenOptionsExt},
    path::PathBuf,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const HELP: &str = "Usage: cortex program inspect --store <ABSOLUTE-PATH> --program <ID> --key-fd <N> --max-bytes <N> --read-timeout-ms <N> [--json]\n\nRead-only program metadata, pure fixtures and supervised local fixture inspection.\nThe inherited descriptor supplies exactly 32 raw key bytes and EOF.\nNo key argument, default lookup, recovery or execution.\nDeadlines are cooperative on a responsive local Linux filesystem, not hard I/O\npreemption. No live Git recapture or verification is implied.\nExit codes: 0 inspected, 2 invalid arguments, 3 store/key/refusal, 1 output failure.\n";
fn error(out: &mut impl Write, json_output: bool, kind: &str) -> Result<u8, io::Error> {
    if json_output {
        serde_json::to_writer(&mut *out,&json!({"schema":"cortex.program.v1","status":"REFUSED","reason":kind,"execution_authority":"UNAVAILABLE"})).map_err(io::Error::other)?;
        writeln!(out)?;
    } else {
        writeln!(out, "Program inspection refused: {kind}")?;
    }
    Ok(3)
}
fn key(fd: u32, deadline: Instant) -> Result<StoreKey, &'static str> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(format!("/proc/self/fd/{fd}"))
        .map_err(|_| "KEY_DESCRIPTOR")?;
    let kind = file.metadata().map_err(|_| "KEY_DESCRIPTOR")?.file_type();
    if !kind.is_file() && !kind.is_fifo() {
        return Err("KEY_DESCRIPTOR");
    }
    let mut bytes = Zeroizing::new([0u8; 33]);
    let mut used = 0;
    loop {
        if Instant::now() >= deadline {
            return Err("KEY_DEADLINE");
        }
        match file.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(n) => {
                used += n;
                if used > 32 {
                    return Err("KEY_LENGTH");
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(_) => return Err("KEY_READ"),
        }
    }
    if used != 32 {
        return Err("KEY_LENGTH");
    }
    StoreKey::new(&bytes[..32]).map_err(|_| "KEY_INVALID")
}
fn view(k: &CoordinationKernel) -> Value {
    let d = k.state().definition();
    let missions:Vec<_>=d.graph().missions().iter().map(|m|{
  let s=k.state().readiness().mission(m.id()).expect("validated graph");
  let blockers:Vec<_>=s.blockers().iter().map(|b|match b{
   MissionBlocker::Dependency(id)=>json!({"kind":"DEPENDENCY","edge":id.as_str()}),
   other=>json!({"kind":format!("{other:?}")}),
  }).collect();
  json!({"id":m.id().as_str(),"objective":m.context().contract().task().objective(),"acceptance":m.context().contract().task().acceptance(),"contract_revision":m.context().contract().revision().get(),"candidate":m.context().contract().candidate().commit().as_str(),"generation":s.generation().get(),"dependency_ready":s.dependency_ready(),"eligible":s.eligible(),"authorized":false,"reserved":false,"dispatched":false,"mission_status":format!("{:?}",s.status()),"blockers":blockers})
 }).collect();
    let edges:Vec<_>=d.graph().edges().iter().map(|e|{
  let s=k.state().readiness().edge(e.id()).expect("validated graph");
  let disposition=match s.decision(){
   EdgeDecision::Released{review,report}=>json!({"kind":"RELEASED_NON_EFFECTFUL","review":review.as_str(),"report":report.as_str()}),
   EdgeDecision::Held(reason)=>{
    let cause=match reason{HoldReason::Invalidated(id)=>json!({"kind":"INVALIDATED","notification":id.as_str()}),r=>json!({"kind":format!("{r:?}")})};
    json!({"kind":"HELD","reason":cause})
   },
   EdgeDecision::Rejected(reason)=>json!({"kind":"REJECTED","reason":format!("{reason:?}")}),
  };
  json!({"id":e.id().as_str(),"producer":e.producer().as_str(),"consumer":e.consumer().as_str(),"level":match e.requirement().level(){ReadinessLevel::ContractStable=>"CONTRACT_STABLE",ReadinessLevel::ImplementationProvisional=>"IMPLEMENTATION_PROVISIONAL",ReadinessLevel::IntegratedAccepted=>"INTEGRATED_ACCEPTED"},"artifact":e.requirement().artifact().id().as_str(),"artifact_revision":e.requirement().artifact().revision().get(),"artifact_candidate":e.requirement().artifact().candidate().as_str(),"decision":disposition,"prior_releases":s.releases().len()})
 }).collect();
    let attempts:Vec<_>=k.state().attempts().iter().map(|a|json!({"id":a.id.as_str(),"increment":a.increment.as_str(),"owner":a.owner.as_str(),"baseline":a.baseline.get(),"generation":a.generation.get(),"status":match a.status{AttemptStatus::Claimed=>"CLAIMED",AttemptStatus::Unknown=>"UNKNOWN",AttemptStatus::Reported=>"REPORTED",AttemptStatus::Cancelled=>"CANCELLED",AttemptStatus::HandedOver=>"HANDED_OVER"},"external_execution":"NOT_SUPPORTED"})).collect();
    let streams:Vec<_>=d.workstreams().iter().map(|w|json!({"id":w.id.as_str(),"responsibility":w.responsibility,"missions":w.missions.iter().map(|m|m.as_str()).collect::<Vec<_>>()})).collect();
    let packages:Vec<_>=k.packages().iter().map(|p|json!({"id":p.request().id.as_str(),"revision":p.request().revision.get(),"mission":p.request().mission.as_str(),"bytes":p.bytes().len(),"current_structural_correspondence":p.check(k.state(),p.bytes()).is_ok(),"submitted":false})).collect();
    let decisions:Vec<_>=k.decisions().iter().map(|d|json!({"operation":d.operation.as_str(),"edge":d.edge.as_str(),"baseline":d.baseline.get(),"historically_released":d.released,"findings":d.findings.iter().map(|p|json!({"predicate":p.predicate().as_str(),"disposition":format!("{:?}",p.disposition()),"reasons":p.reasons().iter().map(|r|format!("{r:?}")).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect();
    let fixtures = fixture_view(k);
    let mut result = json!({"schema":"cortex.program.v1","fixture_projection":fixtures,"status":"INSPECTED","program":d.id().as_str(),"objective":d.outcome().objective(),"acceptance":d.outcome().acceptance(),"baseline":d.revision().get(),"retained_baselines":k.state().definitions().len(),"journal_revision":k.revision(),"trust":"CALLER_ASSERTIONS_SINGLE_LOCAL_OPERATOR","execution_authority":"UNAVAILABLE","live_candidate_verification":"NOT_RUN","workstreams":streams,"missions":missions,"edges":edges,"attempts":attempts,"support_decisions":decisions,"packages":packages,"resource_reservations":[],"integration":"NOT_SUPPORTED","owner_decisions_required":["Stage 2 effect, host, resource, privacy and authority qualification","Protected integration approval"]});
    if k.fixture_effects().mode() == EffectMode::SupervisedFixture {
        result["fixture_projection"] = Value::Null;
        result["supervised_projection"] = supervised_view(k);
        result["execution_authority"] = json!("INSPECTION_CONFERS_NONE");
        result["live_executor_authority"] = json!("UNAVAILABLE");
        result["mission_execution_flags_scope"] = json!("LIVE_AI_EXECUTION_NOT_SUPPORTED");
        result["resource_reservations"] = json!(k.fixture_effects().effects().iter().filter(|e| e.pending()).map(|e| json!({"effect":e.intent.id.as_str(),"scope":"TRUSTED_LOCAL_FIXTURE","quantities_kind":"ORIGINAL_ADMITTED_DEMAND","quantities":e.intent.demand.iter().map(|d|json!({"resource":format!("{:?}",d.resource),"quantity":d.amount})).collect::<Vec<_>>(),"hard_host_reservation":"NOT_ESTABLISHED"})).collect::<Vec<_>>());
        for package in result["packages"].as_array_mut().expect("packages") {
            let id = package["id"].as_str().expect("package ID").to_owned();
            let related = k
                .fixture_effects()
                .effects()
                .iter()
                .filter(|e| e.intent.scope.package.as_str() == id);
            let mut complete = false;
            let mut partial = false;
            for effect in related {
                if let Some(spec) = &effect.process_spec {
                    for observation in &effect.process_observations {
                        complete |= observation.input_bytes == spec.input_len;
                        partial |=
                            observation.input_bytes > 0 && observation.input_bytes < spec.input_len;
                    }
                }
            }
            package["submitted"] = json!(complete);
            package["submission_scope"] = json!("TRUSTED_LOCAL_FIXTURE_OBSERVATIONS");
            package["partial_input_observed"] = json!(partial);
            package["provider_submission"] = json!("NOT_SUPPORTED");
        }
        result["owner_decisions_required"] = json!([
            "Live executor budgets/reserves, provider disclosure, credential mediation and retention",
            "Applicable host qualification and live execution grant",
            "Protected integration approval"
        ]);
    }
    result
}
fn fixture_view(k: &CoordinationKernel) -> Value {
    let projection = k.fixture_effects();
    let effects: Vec<_> = projection.effects().iter().map(|e| {
        let scope = &e.intent.scope;
        let grant = projection.grants().iter().find(|g| g.id == e.intent.grant).expect("validated effect grant");
        let package_current = k.packages().iter().find(|p| p.request().id == scope.package && p.request().revision == scope.package_revision).is_some_and(|p| p.check(k.state(),p.bytes()).is_ok());
        json!({"id":e.intent.id.as_str(),"mission":scope.mission.as_str(),"attempt":scope.attempt.as_str(),"baseline":scope.baseline.get(),"generation":scope.generation.get(),"candidate":scope.candidate.commit().as_str(),"status":format!("{:?}",e.status),"pending":e.pending(),"cancellation_requested":e.cancellation_requested,"reported_outcome":e.report.map(|r| format!("{r:?}")),"reconciled":e.reconciliation.is_some(),"retry_depth":e.retries,"grant":grant.id.as_str(),"grant_revoked":projection.revoked().contains(&grant.id),"grant_expired_at_observed_tick":projection.last_tick() >= grant.expires_before,"baseline_current":scope.baseline == k.state().definition().revision(),"package_current":package_current,"reservation":e.intent.demand.iter().map(|d|json!({"resource":format!("{:?}",d.resource),"quantity":d.amount})).collect::<Vec<_>>(),"observation_kind":"CONTROLLED_FIXTURE","authorized":false,"real_dispatched":false})
    }).collect();
    let mut dimensions = std::collections::BTreeSet::new();
    for policy in projection.policies() {
        for capacity in &policy.capacities {
            dimensions.insert(capacity.resource);
        }
    }
    let accounting: Vec<_> = dimensions.into_iter().map(|r|json!({"resource":format!("{r:?}"),"fixture_accounted":projection.accounted(r).ok()})).collect();
    let prepared: Vec<_> = k.prepared_inputs().iter().map(|(id,p)|json!({"operation":id.as_str(),"package":p.package().request().id.as_str(),"package_revision":p.package().request().revision.get(),"executor":p.descriptor().identity().name(),"executor_version":p.descriptor().identity().version(),"bytes":p.bytes().len(),"current_structural_correspondence":p.check(k.state(),p.descriptor(),p.requirements(),p.bytes()).is_ok(),"submitted":false})).collect();
    json!({"schema":"cortex.fixture-effects.v1","mode":"CONTROLLED_FIXTURE_ONLY","real_execution_supported":false,"logical_tick":projection.last_tick(),"clock":"CALLER_ASSERTION","policies":projection.policies().len(),"proposed_grants":projection.grants().len(),"revoked_grants":projection.revoked().iter().map(|id|id.as_str()).collect::<Vec<_>>(),"effects":effects,"accounting":accounting,"prepared_inputs":prepared})
}
fn supervised_view(k: &CoordinationKernel) -> Value {
    let projection = k.fixture_effects();
    let accounting: Vec<_> = projection.current_policy().into_iter().flat_map(|p| &p.capacities).map(|capacity| {
        json!({"resource":format!("{:?}",capacity.resource),"accounted":projection.accounted(capacity.resource).ok(),"declared_capacity":capacity.available,"verification_reserve":capacity.verification_reserve,"enforcement_scope":"TRUSTED_FIXTURE_LEDGER_NOT_HARD_HOST_QUOTA"})
    }).collect();
    let effects: Vec<_> = projection.effects().iter().map(|effect| {
        let grant = projection.grants().iter().find(|g| g.id == effect.intent.grant).expect("effect grant");
        let observations: Vec<_> = effect.process_observations.iter().map(|o| {
            let completion = match o.completion {
                ProcessCompletion::Spawned => json!({"kind":"SPAWNED"}),
                ProcessCompletion::StartFailed => json!({"kind":"PRE_SPAWN_REFUSAL"}),
                ProcessCompletion::Exited { code, signal } => json!({"kind":"EXITED","code":code,"signal":signal}),
                ProcessCompletion::Interrupted { reason } => json!({"kind":"INTERRUPTED","reason":format!("{reason:?}")}),
                ProcessCompletion::Unknown => json!({"kind":"UNKNOWN"}),
            };
            json!({"observation_scope":o.id.scope(),"observation_sequence":o.id.sequence(),"completion":completion,"quiescence":format!("{:?}",o.quiescence),"process":o.process.as_ref().map(|p|json!({"pid":p.pid,"start_ticks":p.start_ticks,"adoption_supported":false})),"input_bytes":o.input_bytes,"stdout_bytes":o.stdout_bytes,"stderr_bytes":o.stderr_bytes,"structured_records":o.structured_records,"malformed_output":o.malformed_output,"elapsed_ms":o.elapsed_ms,"raw_output_retained":false})
        }).collect();
        json!({"effect":effect.intent.id.as_str(),"mission":effect.intent.scope.mission.as_str(),"attempt":effect.intent.scope.attempt.as_str(),"candidate":effect.intent.scope.candidate.commit().as_str(),"status":format!("{:?}",effect.status),"pending_reconciliation":effect.pending(),"cancellation_requested":effect.cancellation_requested,"grant_revoked":projection.revoked().contains(&grant.id),"grant_expired_at_observed_tick":projection.last_tick() >= grant.expires_before,"dispatch_intent_durable":effect.dispatch_at.is_some(),"dispatch_tick":effect.dispatch_at,"process_spec_retained":effect.process_spec.is_some(),"reported_outcome":effect.report.map(|v|format!("{v:?}")),"observations":observations,"mission_acceptance":"NOT_INFERRED"})
    }).collect();
    json!({"schema":"cortex.supervised-fixtures.v1","mode":"TRUSTED_LOCAL_PROCESS_FIXTURES","logical_tick":projection.last_tick(),"clock":"CONTROLLER_LOGICAL_MILLISECONDS","native_fixture_consumer":"AVAILABLE_REQUIRES_EXPLICIT_INVOCATION","live_executor_supported":false,"isolation_backend_qualified":false,"descendant_containment":"NOT_SUPPORTED","hard_host_quotas":"NOT_ESTABLISHED","accounting":accounting,"effects":effects})
}
/// Run explicit inspection; errors do not include keys, supplied paths or raw SQL.
pub fn run(args: impl Iterator<Item = OsString>, out: &mut impl Write) -> Result<u8, io::Error> {
    let args: Vec<_> = args.collect();
    if args == [OsString::from("--help")]
        || args == [OsString::from("inspect"), OsString::from("--help")]
    {
        out.write_all(HELP.as_bytes())?;
        return Ok(0);
    }
    let mut store = None;
    let mut program = None;
    let mut fd = None;
    let mut max = None;
    let mut timeout = None;
    let mut machine = false;
    let mut it = args.iter();
    if it.next().is_none_or(|a| a != "inspect") {
        out.write_all(HELP.as_bytes())?;
        return Ok(2);
    }
    let mut invalid = false;
    while let Some(flag) = it.next() {
        if flag == "--json" {
            if machine {
                invalid = true;
            }
            machine = true;
            continue;
        }
        let Some(value) = it.next() else {
            invalid = true;
            break;
        };
        match flag.to_str() {
            Some("--store") => {
                if store.replace(PathBuf::from(value)).is_some() {
                    invalid = true;
                }
            }
            Some("--program") => {
                if program
                    .replace(value.to_str().unwrap_or("").to_owned())
                    .is_some()
                {
                    invalid = true;
                }
            }
            Some("--key-fd") => {
                if fd.is_some() {
                    invalid = true;
                }
                fd = value.to_str().and_then(|s| s.parse::<u32>().ok());
                if fd.is_none() {
                    invalid = true;
                }
            }
            Some("--max-bytes") => {
                if max.is_some() {
                    invalid = true;
                }
                max = value.to_str().and_then(|s| s.parse::<usize>().ok());
                if max.is_none() {
                    invalid = true;
                }
            }
            Some("--read-timeout-ms") => {
                if timeout.is_some() {
                    invalid = true;
                }
                timeout = value.to_str().and_then(|s| s.parse::<u64>().ok());
                if timeout.is_none() {
                    invalid = true;
                }
            }
            _ => invalid = true,
        }
    }
    let (Some(store), Some(program), Some(fd), Some(max), Some(timeout)) =
        (store, program, fd, max, timeout)
    else {
        out.write_all(HELP.as_bytes())?;
        return Ok(2);
    };
    let selected = cortex_core::program::ProgramId::new(&program);
    let limits = ReadLimits::new(max, Duration::from_millis(timeout));
    if invalid || !store.is_absolute() || selected.is_err() || limits.is_err() {
        out.write_all(HELP.as_bytes())?;
        return Ok(2);
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout))
        .expect("validated duration");
    let key = match key(fd, deadline) {
        Ok(k) => k,
        Err(e) => return error(out, machine, e),
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let limits = match ReadLimits::new(max, remaining) {
        Ok(l) => l,
        Err(_) => return error(out, machine, "READ_DEADLINE"),
    };
    let (_, k) = match ProgramStore::inspect(&store, &key, limits) {
        Ok(value) => value,
        Err(e) => return error(out, machine, &e.to_string()),
    };
    if Instant::now() >= deadline {
        return error(out, machine, "READ_DEADLINE");
    }
    if k.state().definition().id() != &selected.expect("validated identity") {
        return error(out, machine, "PROGRAM_MISMATCH");
    }
    let value = view(&k);
    if machine {
        serde_json::to_writer(&mut *out, &value).map_err(io::Error::other)?;
        writeln!(out)?;
    } else {
        writeln!(
            out,
            "Program {} | baseline {} | journal {}",
            program,
            k.state().definition().revision().get(),
            k.revision()
        )?;
        if k.fixture_effects().mode() == EffectMode::SupervisedFixture {
            writeln!(
                out,
                "Scope: trusted local fixture records. Inspection grants no authority. Live executor and sandbox qualification: NOT_AVAILABLE."
            )?;
        } else {
            writeln!(
                out,
                "Scope: local coordination metadata. Execution authority: UNAVAILABLE. Live verification: NOT_RUN."
            )?;
        }
        for m in value["missions"].as_array().expect("array") {
            writeln!(
                out,
                "Mission {}: dependency_ready={} eligible={} blockers={}",
                m["id"].as_str().expect("ID"),
                m["dependency_ready"],
                m["eligible"],
                m["blockers"]
            )?;
        }
        for effect in value["fixture_projection"]["effects"]
            .as_array()
            .into_iter()
            .flatten()
        {
            writeln!(
                out,
                "Fixture effect {}: {} pending={} cancellation_requested={} grant_revoked={} package_current={} (no real dispatch)",
                effect["id"].as_str().expect("ID"),
                effect["status"].as_str().expect("status"),
                effect["pending"],
                effect["cancellation_requested"],
                effect["grant_revoked"],
                effect["package_current"]
            )?;
        }
        for effect in value["supervised_projection"]["effects"]
            .as_array()
            .into_iter()
            .flatten()
        {
            writeln!(
                out,
                "Supervised local fixture {}: {} pending_reconciliation={} cancellation_requested={} grant_revoked={} dispatch_intent_durable={} observations={} (no live executor or sandbox guarantee)",
                effect["effect"].as_str().expect("effect ID"),
                effect["status"].as_str().expect("status"),
                effect["pending_reconciliation"],
                effect["cancellation_requested"],
                effect["grant_revoked"],
                effect["dispatch_intent_durable"],
                effect["observations"]
                    .as_array()
                    .expect("observations")
                    .len()
            )?;
        }
        for a in value["attempts"].as_array().expect("array") {
            writeln!(
                out,
                "Attempt {}: {} (increment {})",
                a["id"].as_str().expect("ID"),
                a["status"].as_str().expect("status"),
                a["increment"].as_str().expect("ID")
            )?;
        }
    }
    Ok(0)
}
