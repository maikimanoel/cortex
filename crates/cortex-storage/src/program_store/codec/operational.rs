//! Complete fixture input encoding. Projection fields are never persisted as authority.
use super::*;
use cortex_core::program::{
    operational::{Reconciliation as EffectReconciliation, *},
    scheduling::*,
};

identifiers!(EffectId);
symbols!(Resource;WorkerSlots => "Workers",BuildJobs => "Builds",TestProcesses => "Tests",CpuMillicores => "CpuMillis",RamBytes => "RamBytes",DiskBytes => "DiskBytes",ElapsedMillis => "TimeMillis",OutputBytes => "OutputBytes",CiJobs => "CiJobs",ModelTokens => "ModelTokens");
symbols!(EffectMode;ControlledFixture => "ControlledFixture",SupervisedFixture => "SupervisedFixture",ExternalExecutor => "ExternalExecutor");
symbols!(EffectOutcome;Succeeded => "Succeeded",Failed => "Failed",Interrupted => "Interrupted");
impl Wire for u32 {
    fn write(&self) -> Result<V, ProgramStoreError> {
        u64::from(*self).write()
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        u32::try_from(u64::read(v)?).map_err(|_| bad())
    }
}
fields!(Capacity;resource,available,verification_reserve,declared_enforceable);
fields!(Demand;resource,amount);
fields!(Policy;reference,program,baseline,mode,host,privacy,capacities,cumulative,max_effects,max_retries);
fields!(EffectScope;program,baseline,task,candidate,attempt,generation,package,package_revision);
fields!(OperationalGrant;id,principal,policy,scope,not_before,expires_before,ceilings,basis);
fields!(EffectIntent;id,grant,scope,demand,retry_of);

impl Wire for EffectReconciliation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Absent => json!(["absent"]),
            Self::Observed { outcome, usage } => {
                json!(["observed", outcome.write()?, usage.write()?])
            }
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        match v.get(0).and_then(V::as_str) {
            Some("absent") => {
                arr(v, 1)?;
                Ok(Self::Absent)
            }
            Some("observed") => {
                let a = arr(v, 3)?;
                Ok(Self::Observed {
                    outcome: read(&a[1])?,
                    usage: read(&a[2])?,
                })
            }
            _ => Err(bad()),
        }
    }
}
impl Wire for EffectEvent {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Policy(p) => json!(["policy", p.write()?]),
            Self::Grant(g) => json!(["grant", g.write()?]),
            Self::Revoke { grant, at, basis } => {
                json!(["revoke", grant.write()?, at, basis.write()?])
            }
            Self::Reserve { intent, at } => json!(["reserve", intent.write()?, at]),
            Self::PrepareProcess {
                effect,
                scope,
                at,
                spec,
            } => json!([
                "prepare-process-v1",
                effect.write()?,
                scope.write()?,
                at,
                spec.write()?
            ]),
            Self::ProcessObserved {
                effect,
                scope,
                at,
                observation,
            } => json!([
                "process-observed-v1",
                effect.write()?,
                scope.write()?,
                at,
                observation.write()?
            ]),
            Self::Dispatch {
                effect,
                scope,
                at,
                basis,
            } => json!([
                "dispatch",
                effect.write()?,
                scope.write()?,
                at,
                basis.write()?
            ]),
            Self::Running {
                effect,
                scope,
                at,
                basis,
            } => json!([
                "running",
                effect.write()?,
                scope.write()?,
                at,
                basis.write()?
            ]),
            Self::Report {
                effect,
                scope,
                at,
                outcome,
                basis,
            } => json!([
                "report",
                effect.write()?,
                scope.write()?,
                at,
                outcome.write()?,
                basis.write()?
            ]),
            Self::Unknown {
                effect,
                scope,
                at,
                basis,
            } => json!([
                "unknown",
                effect.write()?,
                scope.write()?,
                at,
                basis.write()?
            ]),
            Self::Cancel {
                effect,
                scope,
                at,
                basis,
            } => json!([
                "cancel",
                effect.write()?,
                scope.write()?,
                at,
                basis.write()?
            ]),
            Self::Reconcile {
                effect,
                scope,
                at,
                resolution,
                basis,
            } => json!([
                "reconcile",
                effect.write()?,
                scope.write()?,
                at,
                resolution.write()?,
                basis.write()?
            ]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(match v.get(0).and_then(V::as_str).ok_or_else(bad)? {
            "policy" => {
                let a = arr(v, 2)?;
                Self::Policy(read(&a[1])?)
            }
            "grant" => {
                let a = arr(v, 2)?;
                Self::Grant(read(&a[1])?)
            }
            "revoke" => {
                let a = arr(v, 4)?;
                Self::Revoke {
                    grant: read(&a[1])?,
                    at: read(&a[2])?,
                    basis: read(&a[3])?,
                }
            }
            "reserve" => {
                let a = arr(v, 3)?;
                Self::Reserve {
                    intent: read(&a[1])?,
                    at: read(&a[2])?,
                }
            }
            "dispatch" | "running" | "unknown" | "cancel" => {
                let a = arr(v, 5)?;
                let effect = read(&a[1])?;
                let scope = read(&a[2])?;
                let at = read(&a[3])?;
                let basis = read(&a[4])?;
                match a[0].as_str().ok_or_else(bad)? {
                    "dispatch" => Self::Dispatch {
                        effect,
                        scope,
                        at,
                        basis,
                    },
                    "running" => Self::Running {
                        effect,
                        scope,
                        at,
                        basis,
                    },
                    "unknown" => Self::Unknown {
                        effect,
                        scope,
                        at,
                        basis,
                    },
                    _ => Self::Cancel {
                        effect,
                        scope,
                        at,
                        basis,
                    },
                }
            }
            "prepare-process-v1" => {
                let a = arr(v, 5)?;
                Self::PrepareProcess {
                    effect: read(&a[1])?,
                    scope: read(&a[2])?,
                    at: read(&a[3])?,
                    spec: Box::new(read(&a[4])?),
                }
            }
            "process-observed-v1" => {
                let a = arr(v, 5)?;
                Self::ProcessObserved {
                    effect: read(&a[1])?,
                    scope: read(&a[2])?,
                    at: read(&a[3])?,
                    observation: read(&a[4])?,
                }
            }
            "report" => {
                let a = arr(v, 6)?;
                Self::Report {
                    effect: read(&a[1])?,
                    scope: read(&a[2])?,
                    at: read(&a[3])?,
                    outcome: read(&a[4])?,
                    basis: read(&a[5])?,
                }
            }
            "reconcile" => {
                let a = arr(v, 6)?;
                Self::Reconcile {
                    effect: read(&a[1])?,
                    scope: read(&a[2])?,
                    at: read(&a[3])?,
                    resolution: read(&a[4])?,
                    basis: read(&a[5])?,
                }
            }
            _ => return Err(bad()),
        })
    }
}
