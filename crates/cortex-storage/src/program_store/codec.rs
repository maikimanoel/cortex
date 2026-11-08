//! Versioned canonical input codec. Decode calls domain constructors and replay;
//! no projection, Debug output or serialized judgment bypass is trusted.
use super::*;
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    parallel::*,
    program::context::*,
    program::*,
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::{ExecutionObservation, Provenance, program::*, readiness::*, unified::*};
use serde_json::{Value as V, json};
use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) trait Wire: Sized {
    fn write(&self) -> Result<V, ProgramStoreError>;
    fn read(v: &V) -> Result<Self, ProgramStoreError>;
}
fn bad() -> ProgramStoreError {
    ProgramStoreError::Corrupt
}
fn arr(v: &V, n: usize) -> Result<&[V], ProgramStoreError> {
    let a = v.as_array().ok_or_else(bad)?;
    if a.len() != n {
        return Err(bad());
    }
    Ok(a)
}
fn read<T: Wire>(v: &V) -> Result<T, ProgramStoreError> {
    T::read(v)
}
fn write<T: Wire>(v: &T) -> Result<V, ProgramStoreError> {
    v.write()
}
fn list(
    values: impl IntoIterator<Item = Result<V, ProgramStoreError>>,
) -> Result<V, ProgramStoreError> {
    Ok(V::Array(values.into_iter().collect::<Result<_, _>>()?))
}
macro_rules! scalar {
    ($t:ty,$get:ident) => {
        impl Wire for $t {
            fn write(&self) -> Result<V, ProgramStoreError> {
                Ok(json!(self))
            }
            fn read(v: &V) -> Result<Self, ProgramStoreError> {
                v.$get().ok_or_else(bad)
            }
        }
    };
}
scalar!(u64, as_u64);
scalar!(bool, as_bool);
impl Wire for usize {
    fn write(&self) -> Result<V, ProgramStoreError> {
        (*self as u64).write()
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        usize::try_from(u64::read(v)?).map_err(|_| bad())
    }
}
impl Wire for String {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!(self))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(v.as_str().ok_or_else(bad)?.to_owned())
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list(self.iter().map(write))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        v.as_array().ok_or_else(bad)?.iter().map(read).collect()
    }
}
impl<T: Wire> Wire for Option<T> {
    fn write(&self) -> Result<V, ProgramStoreError> {
        self.as_ref().map(write).unwrap_or(Ok(V::Null))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        if v.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::read(v)?))
        }
    }
}
impl<T: Wire> Wire for Box<T> {
    fn write(&self) -> Result<V, ProgramStoreError> {
        (**self).write()
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(Box::new(T::read(v)?))
    }
}
macro_rules! identifiers{($($t:ty),+)=>{$(impl Wire for $t{fn write(&self)->Result<V,ProgramStoreError>{Ok(json!(self.as_str()))}fn read(v:&V)->Result<Self,ProgramStoreError>{Self::new(v.as_str().ok_or_else(bad)?).map_err(|_|bad())}})+};}
identifiers!(
    TaskId,
    ProgramId,
    WorkstreamId,
    IncrementId,
    AttemptId,
    ActorId,
    OperationId,
    GrantId,
    ClaimScopeId,
    PackageId,
    GraphId,
    EdgeId,
    ArtifactId,
    NotificationId,
    PredicateId,
    AssessmentId
);
impl Wire for CandidateId {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!(self.as_str()))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Self::from_git_commit(v.as_str().ok_or_else(bad)?).map_err(|_| bad())
    }
}
impl Wire for ContractRevision {
    fn write(&self) -> Result<V, ProgramStoreError> {
        self.get().write()
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Self::new(read(v)?).map_err(|_| bad())
    }
}
// These tokens belong to the retained wire format, not to Rust identifier spelling.
macro_rules! symbols {
    ($t:ty; $($name:ident => $token:literal),+)=>{impl Wire for $t {
        fn write(&self)->Result<V,ProgramStoreError>{
            Ok(V::String(match self {$(Self::$name=>$token),+}.into()))
        }
        fn read(v:&V)->Result<Self,ProgramStoreError>{
            match v.as_str().ok_or_else(bad)? { $($token=>Ok(Self::$name)),+, _=>Err(bad()) }
        }
    }};
}
symbols!(WorktreeRequirement;Clean => "Clean",AllowChanges => "AllowChanges");
symbols!(DependencyCoverage;DeclaredComplete => "DeclaredComplete",Unknown => "Unknown");
symbols!(ConsumerKind;Preview => "Preview",Effectful => "Effectful");
symbols!(DependencyKind;Artifact => "Artifact",TaskAcceptance => "MissionAcceptance");
symbols!(ReadinessLevel;ContractStable => "ContractStable",ImplementationProvisional => "ImplementationProvisional",IntegratedAccepted => "IntegratedAccepted");
symbols!(ReviewRole;Coordinator => "Coordinator",Producer => "Producer");
symbols!(CoordinationPermission;Amend => "Amend",Plan => "Plan",Claim => "Claim",Review => "Review");
symbols!(AttemptStatus;Claimed => "Claimed",Unknown => "Unknown",Reported => "Reported",Cancelled => "Cancelled",HandedOver => "HandedOver");
symbols!(AssessmentResult;Pass => "Pass",Fail => "Fail",Unknown => "Unknown",NotRun => "NotRun",Blocked => "Blocked",NotApplicable => "NotApplicable");
symbols!(Applicability;Applicable => "Applicable",NotApplicable => "NotApplicable",Uncertain => "Uncertain");
symbols!(OracleConclusion;Satisfied => "Satisfied",Violated => "Violated",Insufficient => "Insufficient");
symbols!(ScopeFinding;Present => "Present",Absent => "Absent",Uncertain => "Uncertain");
symbols!(CriterionCoverage;ExactSingleCriterion => "ExactSingleCriterion",Unestablished => "Unestablished");
macro_rules! fields {
    ($t:ty; $($field:ident),+)=>{impl Wire for $t {
        fn write(&self)->Result<V,ProgramStoreError>{list([$(self.$field.write()),+])}
        fn read(v:&V)->Result<Self,ProgramStoreError>{let a=arr(v,[$(stringify!($field)),+].len())?;let mut i=a.iter();Ok(Self{$($field:read(i.next().ok_or_else(bad)?)?),+})}
    }};
}
fields!(ProgramLimits;revisions,entries,bytes);
fields!(Workstream;id,responsibility,tasks);
fields!(Increment;id,task,artifacts,claim_scopes);
fields!(CoordinationGrant;id,program,baseline,actor,tasks,permissions,basis);
fields!(Attempt;id,increment,owner,baseline,generation,status,basis);
fields!(OrderingPlan;revision,order,basis);
fields!(SourceInput;source,candidate,text);
fields!(MethodProposal;method,applicability,qualification,exceptions);
fields!(PackageRequest;id,revision,task,baseline,generation,sources,methods,omissions,deliverable,stop_rules);
fields!(Examination;observation,contract,candidate,profile);
fields!(AssessmentInput;examination,id,at,result,applicability,rationale);
fields!(NonImpact;tasks,basis);

impl Wire for ObservationId {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([self.scope(), self.sequence()]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Self::new(a[0].as_str().ok_or_else(bad)?, read(&a[1])?).map_err(|_| bad())
    }
}
impl Wire for SourceReference {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let origin = match self.origin() {
            SourceOrigin::OperatorReported { author } => json!(["operator", author]),
            SourceOrigin::ObservationReference(id) => json!(["observation", id.write()?]),
        };
        Ok(json!([origin, self.source(), self.version()]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        let o = arr(&a[0], 2)?;
        let source = a[1].as_str().ok_or_else(bad)?;
        let version = a[2].as_str().ok_or_else(bad)?;
        match o[0].as_str() {
            Some("operator") => {
                Self::operator_reported(o[1].as_str().ok_or_else(bad)?, source, version)
            }
            Some("observation") => Self::observation_reference(read(&o[1])?, source, version),
            _ => return Err(bad()),
        }
        .map_err(|_| bad())
    }
}
impl Wire for RepositoryIdentity {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.root().as_os_str().as_bytes(),
            self.git_dir().as_os_str().as_bytes(),
            self.common_dir().as_os_str().as_bytes()
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        let path = |v: &V| -> Result<PathBuf, ProgramStoreError> {
            let b: Vec<u8> = serde_json::from_value(v.clone()).map_err(|_| bad())?;
            Ok(OsString::from_vec(b).into())
        };
        Self::new(path(&a[0])?, path(&a[1])?, path(&a[2])?).map_err(|_| bad())
    }
}
impl Wire for CandidateRequirement {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.repository().write(),
            self.commit().write(),
            self.worktree().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
impl Wire for CandidateBindingContract {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.task().id().write()?,
            self.task().objective(),
            self.task().acceptance(),
            self.revision().write()?,
            self.candidate().write()?
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 5)?;
        Ok(Self::new(
            TaskContract::new(
                read(&a[0])?,
                a[1].as_str().ok_or_else(bad)?,
                a[2].as_str().ok_or_else(bad)?,
            )
            .map_err(|_| bad())?,
            read(&a[3])?,
            read(&a[4])?,
        ))
    }
}
impl Wire for CandidateObservation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        serde_json::from_slice(&crate::history_store::codec::encode_context(self)?)
            .map_err(|_| bad())
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(crate::history_store::codec::decode_context(
            &serde_json::to_vec(v).map_err(|_| bad())?,
        )?)
    }
}
impl Wire for TaskContext {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([self.contract().write(), self.capture().cloned().write()])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Self::new(read(&a[0])?, read(&a[1])?).map_err(|_| bad())
    }
}
impl Wire for TaskScope {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([self.context().write(), self.generation().write()])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?))
    }
}
impl Wire for Task {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.context().write(),
            self.kind().write(),
            self.coverage().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
impl Wire for GraphReference {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([self.id().write(), self.revision().write()])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?))
    }
}
impl Wire for GraphLimits {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.get(LimitKind::Tasks),
            self.get(LimitKind::Edges),
            self.get(LimitKind::Notifications),
            self.get(LimitKind::Supports),
            self.get(LimitKind::Bytes)
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 5)?;
        Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
            read(&a[4])?,
        )
        .map_err(|_| bad())
    }
}
impl Wire for ArtifactReference {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.id().write(),
            self.revision().write(),
            self.candidate().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
impl Wire for ArtifactRequirement {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.artifact().write(),
            self.kind().write(),
            self.level().write(),
            self.predicates().to_vec().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?, read(&a[3])?).map_err(|_| bad())
    }
}
impl Wire for DependencyEdge {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.id().write(),
            self.producer().write(),
            self.consumer().write(),
            self.requirement().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Ok(Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
        ))
    }
}
impl Wire for DependencyGraph {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.reference().write(),
            self.tasks().to_vec().write(),
            self.edges().to_vec().write(),
            self.limits().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?, read(&a[3])?).map_err(|_| bad())
    }
}
impl Wire for ProgramOutcome {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([self.objective(), self.acceptance()]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Self::new(
            a[0].as_str().ok_or_else(bad)?,
            a[1].as_str().ok_or_else(bad)?,
        )
        .map_err(|_| bad())
    }
}
impl Wire for ProgramDefinition {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.id().write(),
            self.outcome().write(),
            self.revision().write(),
            self.predecessor().write(),
            self.approval().write(),
            self.graph().write(),
            self.workstreams().to_vec().write(),
            self.increments().to_vec().write(),
            self.limits().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 9)?;
        Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
            read(&a[4])?,
            read(&a[5])?,
            read(&a[6])?,
            read(&a[7])?,
            read(&a[8])?,
        )
        .map_err(|_| bad())
    }
}
impl Wire for PredicateSupport {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let r = self.references();
        list([
            self.predicate().write(),
            r[0].write(),
            r[1].write(),
            r[2].write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Ok(Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
        ))
    }
}
impl Wire for ProvisionalUse {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list(self.references().iter().map(|r| r.write()))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
impl Wire for ReviewAttestation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let r = self.references();
        list([
            r[0].write(),
            r[1].write(),
            self.provisional().cloned().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
impl Wire for EdgeReview {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let s = self.scopes();
        list([
            self.edge().write(),
            self.report().write(),
            s[0].write(),
            s[1].write(),
            self.role().write(),
            self.attestation().write(),
            self.supports().to_vec().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 7)?;
        Ok(Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
            read(&a[4])?,
            read(&a[5])?,
            read(&a[6])?,
        ))
    }
}
impl Wire for ArtifactReport {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.producer().write(),
            self.artifact().write(),
            self.kind().write(),
            self.level().write(),
            self.source().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 5)?;
        Ok(Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
            read(&a[4])?,
        ))
    }
}
impl Wire for Notification {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let event = match self.event() {
            Event::Signal { task, message } => json!(["signal", task.write()?, message]),
            Event::Artifact(r) => json!(["artifact", r.write()?]),
            Event::Review(r) => json!(["review", r.write()?]),
            Event::Invalidate {
                roots,
                cause,
                coverage,
            } => json!([
                "invalidate",
                roots.write()?,
                cause.write()?,
                coverage.write()?
            ]),
            Event::Preconditions {
                scope,
                ready,
                source,
            } => json!(["preconditions", scope.write()?, ready, source.write()?]),
            Event::Capacity { available, source } => {
                json!(["capacity", available, source.write()?])
            }
            // Unsupported effects cannot be persisted as accepted inputs.
            _ => {
                return Err(ProgramStoreError::Domain(
                    ProgramError::EffectUnavailable.into(),
                ));
            }
        };
        Ok(json!([self.id().write()?, self.graph().write()?, event]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        let e = a[2].as_array().ok_or_else(bad)?;
        let tag = e.first().and_then(V::as_str).ok_or_else(bad)?;
        let event = match tag {
            "signal" => {
                let e = arr(&a[2], 3)?;
                Event::Signal {
                    task: read(&e[1])?,
                    message: read(&e[2])?,
                }
            }
            "artifact" => {
                let e = arr(&a[2], 2)?;
                Event::Artifact(read(&e[1])?)
            }
            "review" => {
                let e = arr(&a[2], 2)?;
                Event::Review(read(&e[1])?)
            }
            "invalidate" => {
                let e = arr(&a[2], 4)?;
                Event::Invalidate {
                    roots: read(&e[1])?,
                    cause: read(&e[2])?,
                    coverage: read(&e[3])?,
                }
            }
            "preconditions" => {
                let e = arr(&a[2], 4)?;
                Event::Preconditions {
                    scope: read(&e[1])?,
                    ready: read(&e[2])?,
                    source: read(&e[3])?,
                }
            }
            "capacity" => {
                let e = arr(&a[2], 3)?;
                Event::Capacity {
                    available: read(&e[1])?,
                    source: read(&e[2])?,
                }
            }
            _ => return Err(bad()),
        };
        Self::new(read(&a[0])?, read(&a[1])?, event).map_err(|_| bad())
    }
}
impl Wire for VerificationProfile {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let p = self.provenance();
        Ok(json!([
            p.mechanism(),
            p.inputs(),
            p.configuration(),
            self.version(),
            self.environment(),
            self.components()
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 6)?;
        let s = |i: usize| a[i].as_str().ok_or_else(bad);
        let components: Vec<(String, String)> =
            serde_json::from_value(a[5].clone()).map_err(|_| bad())?;
        let pairs: Vec<_> = components
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        Self::new(
            Provenance::new(s(0)?, s(1)?, s(2)?).map_err(|_| bad())?,
            s(3)?,
            s(4)?,
            &pairs,
        )
        .map_err(|_| bad())
    }
}
impl Wire for SystemTime {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let (before, d) = match self.duration_since(UNIX_EPOCH) {
            Ok(d) => (false, d),
            Err(e) => (true, e.duration()),
        };
        Ok(json!([before, d.as_secs(), d.subsec_nanos()]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        let ns = u64::read(&a[2])?;
        if ns >= 1_000_000_000 {
            return Err(bad());
        }
        let d = Duration::new(read(&a[1])?, ns as u32);
        if read(&a[0])? {
            UNIX_EPOCH.checked_sub(d)
        } else {
            UNIX_EPOCH.checked_add(d)
        }
        .ok_or_else(bad)
    }
}
impl Wire for ExecutionObservation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Exited { code } => json!(["exited", code]),
            Self::TimedOut => json!(["timed-out"]),
            Self::Cancelled => json!(["cancelled"]),
            Self::Unavailable => json!(["unavailable"]),
            Self::NotRun => json!(["not-run"]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = v.as_array().ok_or_else(bad)?;
        match a.first().and_then(V::as_str) {
            Some("exited") => {
                let a = arr(v, 2)?;
                Ok(Self::Exited {
                    code: u32::try_from(u64::read(&a[1])?).map_err(|_| bad())?,
                })
            }
            Some(s) => {
                arr(v, 1)?;
                match s {
                    "timed-out" => Ok(Self::TimedOut),
                    "cancelled" => Ok(Self::Cancelled),
                    "unavailable" => Ok(Self::Unavailable),
                    "not-run" => Ok(Self::NotRun),
                    _ => Err(bad()),
                }
            }
            _ => Err(bad()),
        }
    }
}
impl Wire for OracleEvaluation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([self.conclusion().write()?, self.reasoning()]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Self::new(read(&a[0])?, a[1].as_str().ok_or_else(bad)?).map_err(|_| bad())
    }
}
impl Wire for ObservationKind {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Execution(o) => json!([
                "execution",
                o.execution().write()?,
                o.checks(),
                o.oracle().cloned().write()?
            ]),
            Self::Inference => json!(["inference"]),
            Self::ScopeInspection(s) => json!(["scope", s.write()?]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = v.as_array().ok_or_else(bad)?;
        match a.first().and_then(V::as_str) {
            Some("execution") => {
                let a = arr(v, 4)?;
                Ok(Self::Execution(
                    VerificationOutcome::new(read(&a[1])?, read(&a[2])?, read(&a[3])?)
                        .map_err(|_| bad())?,
                ))
            }
            Some("inference") => {
                arr(v, 1)?;
                Ok(Self::Inference)
            }
            Some("scope") => {
                let a = arr(v, 2)?;
                Ok(Self::ScopeInspection(read(&a[1])?))
            }
            _ => Err(bad()),
        }
    }
}
impl Wire for ReportedObservation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.id().write()?,
            self.subject().write()?,
            self.profile().write()?,
            self.times().observed().write()?,
            self.times().recorded().write()?,
            self.kind().write()?,
            self.detail()
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 7)?;
        Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            ObservationTimes::new(read(&a[3])?, read(&a[4])?).map_err(|_| bad())?,
            read(&a[5])?,
            a[6].as_str().ok_or_else(bad)?,
        )
        .map_err(|_| bad())
    }
}
impl Wire for SupportLimits {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.get(SupportLimitKind::Records),
            self.get(SupportLimitKind::Bindings),
            self.get(SupportLimitKind::Predicates),
            self.get(SupportLimitKind::MaterialBytes)
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?, read(&a[3])?).map_err(|_| bad())
    }
}
impl Wire for PredicateCorrespondence {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.predicate().write(),
            self.verification().write(),
            self.profile().write(),
            self.coverage().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Ok(Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            read(&a[3])?,
        ))
    }
}
impl Wire for EvidenceReference {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.observation().write(),
            self.assessment().cloned().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?))
    }
}
impl Wire for WitnessBinding {
    fn write(&self) -> Result<V, ProgramStoreError> {
        list([
            self.predicate().write(),
            self.evidence().write(),
            self.artifact().write(),
        ])
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 3)?;
        Ok(Self::new(read(&a[0])?, read(&a[1])?, read(&a[2])?))
    }
}
fn scope(s: &EdgeScopeSnapshot) -> Result<V, ProgramStoreError> {
    list([
        s.graph().write(),
        s.edge().write(),
        s.producer().write(),
        s.consumer().write(),
        s.requirement().write(),
        s.notification_cursor().write(),
    ])
}
fn release_write(r: &ReleaseInput) -> Result<V, ProgramStoreError> {
    let c = &r.contract;
    Ok(json!([
        scope(c.scope())?,
        c.revision().write()?,
        c.artifact_requirement().write()?,
        c.correspondences().to_vec().write()?,
        c.basis().write()?,
        r.producer.write()?,
        r.consumer.write()?,
        r.examinations.write()?,
        r.bindings.write()?,
        r.review.write()?,
        r.limits.write()?
    ]))
}
fn release_read(v: &V, k: &CoordinationKernel) -> Result<ReleaseInput, ProgramStoreError> {
    let a = arr(v, 11)?;
    let s = arr(&a[0], 6)?;
    let limits = read(&a[10])?;
    let snapshot = EdgeScopeSnapshot::capture(k.state().readiness(), &read(&s[1])?)
        .map_err(|_| bad())?
        .with_historical_cursor(read(&s[5])?)
        .map_err(|_| bad())?
        .with_original_scopes(read(&s[2])?, read(&s[3])?)
        .map_err(|_| bad())?;
    if scope(&snapshot)? != a[0] {
        return Err(bad());
    }
    let contract = EdgeEvidenceContract::new(
        snapshot,
        read(&a[1])?,
        read(&a[2])?,
        read(&a[3])?,
        read(&a[4])?,
        limits,
    )
    .map_err(|_| bad())?;
    Ok(ReleaseInput {
        contract,
        producer: read(&a[5])?,
        consumer: read(&a[6])?,
        examinations: read(&a[7])?,
        bindings: read(&a[8])?,
        review: read(&a[9])?,
        limits,
    })
}
fn command_write(c: &Command) -> Result<V, ProgramStoreError> {
    if let Command::DomainMember { operation, .. } = c
        && matches!(
            operation.command,
            Command::Domain(_) | Command::DomainMember { .. } | Command::RegisterDomainMember(_)
        )
    {
        return Err(ProgramStoreError::Unsupported);
    }
    Ok(match c {
        Command::Domain(event) => {
            let tag = match event.as_ref() {
                cortex_core::program::domain::DomainEvent::BoundObserved { .. } => {
                    "domain-observation-v1"
                }
                cortex_core::program::domain::DomainEvent::ClosureObserved { .. } => {
                    "domain-closure-observation-v1"
                }
                cortex_core::program::domain::DomainEvent::ClosureTreeObserved { .. } => {
                    "domain-closure-tree-v1"
                }
                _ => "domain-v1",
            };
            json!([tag, event.write()?])
        }
        Command::RegisterDomainMember(definition) => {
            json!(["domain-member-v1", definition.write()?])
        }
        Command::DomainMember { program, operation } => json!([
            "domain-member-input-v1",
            program.write()?,
            operation.id.write()?,
            operation.expected,
            operation.grant.write()?,
            command_write(&operation.command)?
        ]),
        Command::PrepareExecutorInput(request) => json!(["executor-input-v1", request.write()?]),
        Command::FixtureEffect(event) => json!(["fixture-effect-v1", event.write()?]),
        Command::SupervisedEffect(event) => json!(["supervised-effect-v1", event.write()?]),
        Command::Notify(n) => json!(["notify", n.write()?]),
        Command::Observe(r) => json!(["observe", r.write()?]),
        Command::Assess(a) => json!(["assess", a.write()?]),
        Command::Examine(e) => json!(["examine", e.write()?]),
        Command::InvalidateEvidence {
            observation,
            reason,
        } => json!(["invalidate-evidence", observation.write()?, reason]),
        Command::Release(r) => json!(["release", release_write(r)?]),
        Command::OrderingPlan(p) => json!(["plan", p.write()?]),
        Command::Claim(a) => json!(["claim", a.write()?]),
        Command::Attempt {
            id,
            generation,
            status,
            basis,
        } => json!([
            "attempt",
            id.write()?,
            generation.write()?,
            status.write()?,
            basis.write()?
        ]),
        Command::Handoff {
            old,
            recipient,
            next,
        } => json!(["handoff", old.write()?, recipient.write()?, next.write()?]),
        Command::Amend { next, non_impact } => json!(["amend", next.write()?, non_impact.write()?]),
        Command::Package(p) => json!(["package", p.write()?]),
    })
}
fn command_read(v: &V, k: &CoordinationKernel) -> Result<Command, ProgramStoreError> {
    let a = v.as_array().ok_or_else(bad)?;
    let tag = a.first().and_then(V::as_str).ok_or_else(bad)?;
    if tag == "domain-member-input-v1" {
        let a = arr(v, 6)?;
        let program: ProgramId = read(&a[1])?;
        let member = k.domain_member(&program).ok_or_else(bad)?;
        if a[5].get(0).and_then(V::as_str).is_some_and(|tag| {
            matches!(
                tag,
                "domain-v1"
                    | "domain-observation-v1"
                    | "domain-closure-observation-v1"
                    | "domain-closure-tree-v1"
                    | "domain-member-v1"
                    | "domain-member-input-v1"
            )
        }) {
            return Err(bad());
        }
        return Ok(Command::DomainMember {
            program,
            operation: Box::new(Operation {
                id: read(&a[2])?,
                expected: read(&a[3])?,
                grant: read(&a[4])?,
                command: command_read(&a[5], member)?,
            }),
        });
    }
    if [
        "domain-v1",
        "domain-observation-v1",
        "domain-closure-observation-v1",
        "domain-closure-tree-v1",
        "domain-member-v1",
        "notify",
        "observe",
        "assess",
        "examine",
        "release",
        "plan",
        "claim",
        "package",
        "fixture-effect-v1",
        "supervised-effect-v1",
        "executor-input-v1",
    ]
    .contains(&tag)
    {
        let a = arr(v, 2)?;
        return Ok(match tag {
            "domain-v1"
            | "domain-observation-v1"
            | "domain-closure-observation-v1"
            | "domain-closure-tree-v1" => {
                let event: cortex_core::program::domain::DomainEvent = read(&a[1])?;
                let expected = match &event {
                    cortex_core::program::domain::DomainEvent::BoundObserved { .. } => {
                        "domain-observation-v1"
                    }
                    cortex_core::program::domain::DomainEvent::ClosureObserved { .. } => {
                        "domain-closure-observation-v1"
                    }
                    cortex_core::program::domain::DomainEvent::ClosureTreeObserved { .. } => {
                        "domain-closure-tree-v1"
                    }
                    _ => "domain-v1",
                };
                if tag != expected {
                    return Err(bad());
                }
                Command::Domain(Box::new(event))
            }
            "domain-member-v1" => Command::RegisterDomainMember(read(&a[1])?),
            "executor-input-v1" => Command::PrepareExecutorInput(read(&a[1])?),
            "fixture-effect-v1" => Command::FixtureEffect(read(&a[1])?),
            "supervised-effect-v1" => Command::SupervisedEffect(read(&a[1])?),
            "notify" => Command::Notify(read(&a[1])?),
            "observe" => Command::Observe(read(&a[1])?),
            "assess" => Command::Assess(read(&a[1])?),
            "examine" => Command::Examine(read(&a[1])?),
            "release" => Command::Release(Box::new(release_read(&a[1], k)?)),
            "plan" => Command::OrderingPlan(read(&a[1])?),
            "claim" => Command::Claim(read(&a[1])?),
            "package" => Command::Package(read(&a[1])?),
            _ => unreachable!(),
        });
    }
    Ok(match tag {
        "invalidate-evidence" => {
            let a = arr(v, 3)?;
            Command::InvalidateEvidence {
                observation: read(&a[1])?,
                reason: read(&a[2])?,
            }
        }
        "attempt" => {
            let a = arr(v, 5)?;
            Command::Attempt {
                id: read(&a[1])?,
                generation: read(&a[2])?,
                status: read(&a[3])?,
                basis: read(&a[4])?,
            }
        }
        "handoff" => {
            let a = arr(v, 4)?;
            Command::Handoff {
                old: read(&a[1])?,
                recipient: read(&a[2])?,
                next: read(&a[3])?,
            }
        }
        "amend" => {
            let a = arr(v, 3)?;
            Command::Amend {
                next: read(&a[1])?,
                non_impact: read(&a[2])?,
            }
        }
        _ => return Err(bad()),
    })
}
fn encoded(v: &V, max: usize) -> Result<Zeroizing<Vec<u8>>, ProgramStoreError> {
    struct Bounded {
        bytes: Zeroizing<Vec<u8>>,
        max: usize,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if b.len() > self.max.saturating_sub(self.bytes.len()) {
                return Err(std::io::ErrorKind::OutOfMemory.into());
            }
            self.bytes.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Bounded {
        bytes: Zeroizing::new(Vec::new()),
        max,
    };
    serde_json::to_writer(&mut out, v).map_err(|_| ProgramStoreError::Capacity)?;
    Ok(out.bytes)
}
fn parsed(b: &[u8], max: usize) -> Result<V, ProgramStoreError> {
    if b.len() > max {
        return Err(ProgramStoreError::Capacity);
    }
    serde_json::from_slice(b).map_err(|_| bad())
}
pub(super) fn definition(d: &ProgramDefinition) -> Result<Zeroizing<Vec<u8>>, ProgramStoreError> {
    encoded(&d.write()?, d.limits().bytes)
}
pub(super) fn decode_definition(
    b: &[u8],
    max: usize,
) -> Result<ProgramDefinition, ProgramStoreError> {
    let d = ProgramDefinition::read(&parsed(b, max)?)?;
    if definition(&d)?.as_slice() != b {
        return Err(bad());
    }
    Ok(d)
}
pub(super) fn operation(
    o: &Operation,
    max: usize,
) -> Result<Zeroizing<Vec<u8>>, ProgramStoreError> {
    encoded(
        &json!([
            o.id.write()?,
            o.expected,
            o.grant.write()?,
            command_write(&o.command)?
        ]),
        max,
    )
}
pub(super) fn decode_operation(
    b: &[u8],
    k: &CoordinationKernel,
) -> Result<Operation, ProgramStoreError> {
    let v = parsed(b, k.initial().limits().bytes)?;
    let a = arr(&v, 4)?;
    let o = Operation {
        id: read(&a[0])?,
        expected: read(&a[1])?,
        grant: read(&a[2])?,
        command: command_read(&a[3], k)?,
    };
    if operation(&o, k.initial().limits().bytes)?.as_slice() != b {
        return Err(bad());
    }
    Ok(o)
}

mod domain;
mod operational;

mod executor;
mod supervision;

#[cfg(test)]
mod compatibility;
