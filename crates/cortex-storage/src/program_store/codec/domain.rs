//! Explicit version-four inputs. Legacy encodings are unchanged.
use super::*;
use cortex_core::program::domain::*;
identifiers!(DomainId);
symbols!(Consumption;CpuExecutionMillis => "CpuExecutionMillis",ElapsedMillis => "ElapsedMillis",InputBytes => "InputBytes",OutputBytes => "OutputBytes",Events => "Events");
symbols!(AdministrativeDisposition;Active => "Active",Paused => "Paused",Abandoned => "Abandoned");
fields!(PoolIdentity;physical,incarnation,host,boot);
fields!(ConsumptionBudget;dimension,total,reserve,per_operation);
fields!(ConsumptionReservation;dimension,amount);
fields!(PoolContract;identity,basis,capacities,consumption,max_domains);
fields!(DomainIntent;id,effect,pool,request,configuration,consumption);
fields!(DomainIdentity;pool,request,manager,invocation,control_group,cgroup_device,cgroup_inode,boot);
fields!(DomainProcessObservation;pid,start_ticks);
fields!(DomainControlReadback;cpu_quota_us,cpu_period_us,memory_bytes,swap_bytes,tasks,runtime_max_micros,timeout_stop_micros,restart_prevented,kill_control_group,no_new_privileges,send_sigkill,remain_after_exit);
fields!(DomainBindingObservation;identity,configuration,main,direct_members,controls,populated,basis);
fields!(DomainClosureObservation;identity,configuration,scope,binding_main,observed_main,direct_members,populated,restart_prevented,cpu_micros,memory_peak,observed_at,basis);
fields!(DomainCgroupObservation;relative_path,device,inode,populated,members);
fields!(DomainClosureTreeObservation;sample,nodes);
fields!(ConsumptionObservation;dimension,measured);
fields!(ClosureReceipt;domain,identity,configuration,scope,whole_domain,restart_prevented,snapshot_safe,quiescent,released,consumption,evidence);
impl Wire for DomainEvent {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::ConfigurePool(p) => json!(["pool-v1", p.write()?]),
            Self::Reserve { intent, at } => json!(["reserve-v1", intent.write()?, at]),
            Self::CreateRequested { domain, at } => json!(["create-v1", domain.write()?, at]),
            Self::Bound {
                domain,
                identity,
                at,
            } => json!(["bound-v1", domain.write()?, identity.write()?, at]),
            Self::BoundObserved {
                domain,
                observation,
                at,
            } => {
                observation.validate().map_err(|_| bad())?;
                json!([
                    "bound-observed-v1",
                    domain.write()?,
                    observation.write()?,
                    at
                ])
            }
            Self::ClosureObserved {
                domain,
                observation,
                at,
            } => {
                observation.validate().map_err(|_| bad())?;
                if observation.observed_at != *at {
                    return Err(bad());
                }
                json!([
                    "closure-observed-v1",
                    domain.write()?,
                    observation.write()?,
                    at
                ])
            }
            Self::ClosureTreeObserved {
                domain,
                observation,
                at,
            } => {
                observation.validate().map_err(|_| bad())?;
                if observation.sample.observed_at != *at {
                    return Err(bad());
                }
                json!([
                    "closure-tree-observed-v1",
                    domain.write()?,
                    observation.write()?,
                    at
                ])
            }
            Self::ReleaseRequested {
                domain,
                identity,
                at,
            } => json!(["release-v1", domain.write()?, identity.write()?, at]),
            Self::Unknown { domain, at, basis } => {
                json!(["unknown-v1", domain.write()?, at, basis.write()?])
            }
            Self::Report {
                domain,
                identity,
                outcome,
                at,
                basis,
            } => json!([
                "report-v1",
                domain.write()?,
                identity.write()?,
                outcome.write()?,
                at,
                basis.write()?
            ]),
            Self::Cancel { domain, at, basis } => {
                json!(["cancel-v1", domain.write()?, at, basis.write()?])
            }
            Self::Closed { receipt, at } => json!(["closed-v1", receipt.write()?, at]),
            Self::RecoveredClosed { receipt, at } => {
                json!(["recovered-closed-v1", receipt.write()?, at])
            }
            Self::Settle { receipt, at } => json!(["settle-v1", receipt.write()?, at]),
            Self::Disposition {
                domain,
                disposition,
                at,
                basis,
            } => json!([
                "disposition-v1",
                domain.write()?,
                disposition.write()?,
                at,
                basis.write()?
            ]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(match v.get(0).and_then(V::as_str).ok_or_else(bad)? {
            "pool-v1" => {
                let a = arr(v, 2)?;
                Self::ConfigurePool(read(&a[1])?)
            }
            "reserve-v1" => {
                let a = arr(v, 3)?;
                Self::Reserve {
                    intent: read(&a[1])?,
                    at: read(&a[2])?,
                }
            }
            "create-v1" => {
                let a = arr(v, 3)?;
                Self::CreateRequested {
                    domain: read(&a[1])?,
                    at: read(&a[2])?,
                }
            }
            "bound-v1" | "release-v1" => {
                let a = arr(v, 4)?;
                let domain = read(&a[1])?;
                let identity = read(&a[2])?;
                let at = read(&a[3])?;
                if a[0] == "bound-v1" {
                    Self::Bound {
                        domain,
                        identity,
                        at,
                    }
                } else {
                    Self::ReleaseRequested {
                        domain,
                        identity,
                        at,
                    }
                }
            }
            "bound-observed-v1" => {
                let a = arr(v, 4)?;
                let observation: DomainBindingObservation = read(&a[2])?;
                observation.validate().map_err(|_| bad())?;
                Self::BoundObserved {
                    domain: read(&a[1])?,
                    observation,
                    at: read(&a[3])?,
                }
            }
            "closure-observed-v1" => {
                let a = arr(v, 4)?;
                let observation: DomainClosureObservation = read(&a[2])?;
                observation.validate().map_err(|_| bad())?;
                let at = read(&a[3])?;
                if observation.observed_at != at {
                    return Err(bad());
                }
                Self::ClosureObserved {
                    domain: read(&a[1])?,
                    observation,
                    at,
                }
            }
            "closure-tree-observed-v1" => {
                let a = arr(v, 4)?;
                let observation: DomainClosureTreeObservation = read(&a[2])?;
                observation.validate().map_err(|_| bad())?;
                let at = read(&a[3])?;
                if observation.sample.observed_at != at {
                    return Err(bad());
                }
                Self::ClosureTreeObserved {
                    domain: read(&a[1])?,
                    observation,
                    at,
                }
            }
            "unknown-v1" | "cancel-v1" => {
                let a = arr(v, 4)?;
                let domain = read(&a[1])?;
                let at = read(&a[2])?;
                let basis = read(&a[3])?;
                if a[0] == "unknown-v1" {
                    Self::Unknown { domain, at, basis }
                } else {
                    Self::Cancel { domain, at, basis }
                }
            }
            "report-v1" => {
                let a = arr(v, 6)?;
                Self::Report {
                    domain: read(&a[1])?,
                    identity: read(&a[2])?,
                    outcome: read(&a[3])?,
                    at: read(&a[4])?,
                    basis: read(&a[5])?,
                }
            }
            "closed-v1" | "recovered-closed-v1" | "settle-v1" => {
                let a = arr(v, 3)?;
                let receipt = read(&a[1])?;
                let at = read(&a[2])?;
                if a[0] == "closed-v1" {
                    Self::Closed { receipt, at }
                } else if a[0] == "recovered-closed-v1" {
                    Self::RecoveredClosed { receipt, at }
                } else {
                    Self::Settle { receipt, at }
                }
            }
            "disposition-v1" => {
                let a = arr(v, 5)?;
                Self::Disposition {
                    domain: read(&a[1])?,
                    disposition: read(&a[2])?,
                    at: read(&a[3])?,
                    basis: read(&a[4])?,
                }
            }
            _ => return Err(bad()),
        })
    }
}
