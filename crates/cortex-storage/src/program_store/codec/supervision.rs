//! Complete supervised-fixture inputs, never trusted serialized process projections.
use super::*;
use cortex_core::program::memory::{MemoryBasis, MemoryResource, MemoryScope};
use cortex_core::program::scheduling::Resource;
use cortex_core::program::supervision::*;

impl Wire for i32 {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!(self))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        i32::try_from(v.as_i64().ok_or_else(bad)?).map_err(|_| bad())
    }
}
impl Wire for PathBuf {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!(self.as_os_str().as_bytes()))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let bytes: Vec<u8> = serde_json::from_value(v.clone()).map_err(|_| bad())?;
        Ok(OsString::from_vec(bytes).into())
    }
}
fields!(TransportLimits;startup_ms,idle_ms,stdout_bytes,stderr_bytes,records,buffer_bytes);
impl Wire for ProcessLimits {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let mut fields = vec![
            self.wall_ms.write()?,
            self.cleanup_ms.write()?,
            self.input_bytes.write()?,
            self.output_bytes.write()?,
            self.frame_bytes.write()?,
            self.authority_poll_ms.write()?,
            self.executable_bytes.write()?,
            self.arguments.write()?,
            self.argument_bytes.write()?,
        ];
        // Preserve the exact nine-field historical representation. Older readers
        // reject the extended length instead of dropping mandatory new controls.
        if let Some(transport) = &self.transport {
            fields.push(transport.write()?);
        }
        Ok(V::Array(fields))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = v.as_array().ok_or_else(bad)?;
        if !matches!(a.len(), 9 | 10) {
            return Err(bad());
        }
        Ok(Self {
            wall_ms: read(&a[0])?,
            cleanup_ms: read(&a[1])?,
            input_bytes: read(&a[2])?,
            output_bytes: read(&a[3])?,
            frame_bytes: read(&a[4])?,
            authority_poll_ms: read(&a[5])?,
            executable_bytes: read(&a[6])?,
            arguments: read(&a[7])?,
            argument_bytes: read(&a[8])?,
            transport: if a.len() == 10 {
                Some(read(&a[9])?)
            } else {
                None
            },
        })
    }
}
fields!(ReadOnlyMount;source,destination);
fields!(ScopeControls;cpu_quota_us,cpu_period_us,memory_bytes,swap_bytes,tasks_max);
fn scope_memory_valid(resource: &MemoryResource) -> bool {
    resource.basis() == MemoryBasis::CgroupCharged
        && matches!(resource.scope(), MemoryScope::AccountingPool(_))
}
impl Wire for OwnedScopeSpec {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let legacy = list([
            self.reference.write(),
            self.unit.write(),
            self.delegated_root.write(),
            self.runtime_dir.write(),
            self.launcher.write(),
            self.launcher_identity.write(),
            self.manager.write(),
            self.manager_identity.write(),
            self.controls.write(),
            self.cpu_time_ms.write(),
            self.cancellation_grace_ms.write(),
            self.escalation_ms.write(),
            self.isolation.write(),
        ])?;
        match &self.memory_resource {
            Some(resource) => {
                if !scope_memory_valid(resource) {
                    return Err(bad());
                }
                Ok(json!([
                    "owned-scope-memory-v1",
                    legacy,
                    Resource::MemoryBytes(resource.clone()).write()?
                ]))
            }
            None => Ok(legacy),
        }
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let (legacy, memory_resource) =
            if v.get(0).and_then(V::as_str) == Some("owned-scope-memory-v1") {
                let a = arr(v, 3)?;
                let Resource::MemoryBytes(resource) = read(&a[2])? else {
                    return Err(bad());
                };
                if !scope_memory_valid(&resource) {
                    return Err(bad());
                }
                (&a[1], Some(resource))
            } else {
                (v, None)
            };
        let a = arr(legacy, 13)?;
        Ok(Self {
            memory_resource,
            reference: read(&a[0])?,
            unit: read(&a[1])?,
            delegated_root: read(&a[2])?,
            runtime_dir: read(&a[3])?,
            launcher: read(&a[4])?,
            launcher_identity: read(&a[5])?,
            manager: read(&a[6])?,
            manager_identity: read(&a[7])?,
            controls: read(&a[8])?,
            cpu_time_ms: read(&a[9])?,
            cancellation_grace_ms: read(&a[10])?,
            escalation_ms: read(&a[11])?,
            isolation: read(&a[12])?,
        })
    }
}
fields!(ScopeIdentity;spec,unit,invocation_id,cgroup,device,inode);
fields!(ScopeObservation;identity,controls,membership_verified,helper_quiescent,lifecycle_verified,populated,cpu_usage_us,memory_peak_bytes,tasks_peak);
impl Wire for ScopeIsolation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::ScopeOnlyInertFixture => json!(["scope-only-inert-v1"]),
            Self::RootlessSyntheticCanary {
                bubblewrap,
                bubblewrap_identity,
                mounts,
                scratch_bytes,
            } => json!([
                "rootless-synthetic-v1",
                bubblewrap.write()?,
                bubblewrap_identity.write()?,
                mounts.write()?,
                scratch_bytes.write()?
            ]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(match v.get(0).and_then(V::as_str).ok_or_else(bad)? {
            "scope-only-inert-v1" => {
                arr(v, 1)?;
                Self::ScopeOnlyInertFixture
            }
            "rootless-synthetic-v1" => {
                let a = arr(v, 5)?;
                Self::RootlessSyntheticCanary {
                    bubblewrap: read(&a[1])?,
                    bubblewrap_identity: read(&a[2])?,
                    mounts: read(&a[3])?,
                    scratch_bytes: read(&a[4])?,
                }
            }
            _ => return Err(bad()),
        })
    }
}
symbols!(IdentityCheckpointPolicy;BeforeInputV1 => "BeforeInputV1");
impl Wire for ProcessSpec {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let mut fields = vec![
            self.reference.write()?,
            self.executable.write()?,
            self.executable_identity.write()?,
            self.cwd.write()?,
            self.args.write()?,
            self.prepared_input.write()?,
            self.input_len.write()?,
            self.host.write()?,
            self.profile.write()?,
            self.privacy.write()?,
            self.approval.write()?,
            self.limits.write()?,
        ];
        if let Some(scope) = &self.containment {
            fields.push(scope.write()?);
        }
        if let Some(policy) = &self.identity_checkpoint {
            if self.containment.is_none() {
                return Err(bad());
            }
            fields.push(policy.write()?);
        }
        Ok(V::Array(fields))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = v.as_array().ok_or_else(bad)?;
        if !matches!(a.len(), 12..=14) {
            return Err(bad());
        }
        Ok(Self {
            reference: read(&a[0])?,
            executable: read(&a[1])?,
            executable_identity: read(&a[2])?,
            cwd: read(&a[3])?,
            args: read(&a[4])?,
            prepared_input: read(&a[5])?,
            input_len: read(&a[6])?,
            host: read(&a[7])?,
            profile: read(&a[8])?,
            privacy: read(&a[9])?,
            approval: read(&a[10])?,
            limits: read(&a[11])?,
            containment: if a.len() >= 13 {
                Some(read(&a[12])?)
            } else {
                None
            },
            identity_checkpoint: if a.len() == 14 {
                Some(read(&a[13])?)
            } else {
                None
            },
        })
    }
}
fields!(ProcessIdentity;launch,pid,start_ticks);
fields!(ScopeHelperIdentity;pid,start_ticks);
impl Wire for ProcessObservation {
    fn write(&self) -> Result<V, ProgramStoreError> {
        let mut a = vec![
            self.id.write()?,
            self.spec.write()?,
            self.process.write()?,
            self.completion.write()?,
            self.quiescence.write()?,
            self.input_bytes.write()?,
            self.stdout_bytes.write()?,
            self.stderr_bytes.write()?,
            self.structured_records.write()?,
            self.malformed_output.write()?,
            self.elapsed_ms.write()?,
            self.basis.write()?,
        ];
        if !self.unresolved_scope_helpers.is_empty() {
            a.push(self.containment.write()?);
            a.push(self.unresolved_scope_helpers.write()?);
        } else if let Some(scope) = &self.containment {
            a.push(scope.write()?);
        }
        Ok(V::Array(a))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = v.as_array().ok_or_else(bad)?;
        if !matches!(a.len(), 12..=14) {
            return Err(bad());
        }
        let unresolved_scope_helpers: Vec<ScopeHelperIdentity> =
            if a.len() == 14 { read(&a[13])? } else { vec![] };
        if a.len() == 14 && unresolved_scope_helpers.is_empty() {
            return Err(bad());
        }
        Ok(Self {
            id: read(&a[0])?,
            spec: read(&a[1])?,
            process: read(&a[2])?,
            completion: read(&a[3])?,
            quiescence: read(&a[4])?,
            input_bytes: read(&a[5])?,
            stdout_bytes: read(&a[6])?,
            stderr_bytes: read(&a[7])?,
            structured_records: read(&a[8])?,
            malformed_output: read(&a[9])?,
            elapsed_ms: read(&a[10])?,
            basis: read(&a[11])?,
            containment: match a.len() {
                12 => None,
                13 => Some(read(&a[12])?),
                _ => read(&a[12])?,
            },
            unresolved_scope_helpers,
        })
    }
}
symbols!(Quiescence;NoChildStarted => "NoChildStarted",DirectChildReapedStreamsClosed => "DirectChildReapedStreamsClosed",OwnedScopeEmpty => "OwnedScopeEmpty",Unknown => "Unknown");
symbols!(InterruptionReason;Timeout => "Timeout",StartupTimeout => "StartupTimeout",IdleTimeout => "IdleTimeout",StdoutLimit => "StdoutLimit",StderrLimit => "StderrLimit",EventLimit => "EventLimit",BufferLimit => "BufferLimit",StartupIncomplete => "StartupIncomplete",ContainmentUnknown => "ContainmentUnknown",CpuTimeLimit => "CpuTimeLimit",Cancelled => "Cancelled",OutputLimit => "OutputLimit",InputClosed => "InputClosed",Io => "Io",MalformedOutput => "MalformedOutput",AuthorityLost => "AuthorityLost",AuthorityUnknown => "AuthorityUnknown");

impl Wire for ProcessCompletion {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Spawned => json!(["spawned"]),
            Self::IdentityVerified => json!(["identity-verified-v1"]),
            Self::StartFailed => json!(["start-failed"]),
            Self::Exited { code, signal } => json!(["exited", code.write()?, signal.write()?]),
            Self::Interrupted { reason } => json!(["interrupted", reason.write()?]),
            Self::Unknown => json!(["unknown"]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        Ok(match v.get(0).and_then(V::as_str).ok_or_else(bad)? {
            "spawned" => {
                arr(v, 1)?;
                Self::Spawned
            }
            "identity-verified-v1" => {
                arr(v, 1)?;
                Self::IdentityVerified
            }
            "start-failed" => {
                arr(v, 1)?;
                Self::StartFailed
            }
            "exited" => {
                let a = arr(v, 3)?;
                Self::Exited {
                    code: read(&a[1])?,
                    signal: read(&a[2])?,
                }
            }
            "interrupted" => {
                let a = arr(v, 2)?;
                Self::Interrupted {
                    reason: read(&a[1])?,
                }
            }
            "unknown" => {
                arr(v, 1)?;
                Self::Unknown
            }
            _ => return Err(bad()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_limit_bytes_stay_identical_and_extensions_are_explicit() {
        let old = json!([1000, 200, 10000, 1000, 100, 10, 1000000, 2, 50]);
        let limits = ProcessLimits::read(&old).unwrap();
        assert!(limits.transport.is_none());
        assert_eq!(limits.write().unwrap(), old);
        let mut extended = limits;
        extended.transport = Some(TransportLimits {
            startup_ms: 200,
            idle_ms: 100,
            stdout_bytes: 900,
            stderr_bytes: 100,
            records: 8,
            buffer_bytes: 16384,
        });
        let wire = extended.write().unwrap();
        assert_eq!(wire.as_array().unwrap().len(), 10);
        assert!(arr(&wire, 9).is_err()); // The original decoder cannot ignore it.
        assert_eq!(ProcessLimits::read(&wire).unwrap(), extended);
        let mut null_extension = old.as_array().unwrap().clone();
        null_extension.push(V::Null);
        assert!(ProcessLimits::read(&V::Array(null_extension)).is_err());
        let mut unknown = wire.as_array().unwrap().clone();
        unknown.push(V::Null);
        assert!(ProcessLimits::read(&V::Array(unknown)).is_err());
    }

    #[test]
    fn exact_new_interruption_causes_roundtrip_without_losing_old_causes() {
        for reason in [
            InterruptionReason::Timeout,
            InterruptionReason::StartupTimeout,
            InterruptionReason::IdleTimeout,
            InterruptionReason::StdoutLimit,
            InterruptionReason::StderrLimit,
            InterruptionReason::EventLimit,
            InterruptionReason::BufferLimit,
            InterruptionReason::StartupIncomplete,
            InterruptionReason::ContainmentUnknown,
            InterruptionReason::CpuTimeLimit,
        ] {
            let outcome = ProcessCompletion::Interrupted { reason };
            assert_eq!(
                ProcessCompletion::read(&outcome.write().unwrap()).unwrap(),
                outcome
            );
        }
        assert!(InterruptionReason::read(&json!("untrusted-success")).is_err());
    }
    #[test]
    fn owned_scope_extensions_preserve_legacy_bytes_and_refuse_silent_downgrade() {
        let source =
            cortex_core::temporal::SourceReference::operator_reported("codec", "fixture", "1")
                .unwrap();
        let reference = source.write().unwrap();
        let old_spec = json!([
            reference,
            PathBuf::from("/owned/fixture").write().unwrap(),
            reference,
            PathBuf::from("/owned/workspace").write().unwrap(),
            [],
            "prepared",
            4,
            reference,
            reference,
            reference,
            reference,
            [1000, 200, 10000, 1000, 100, 10, 1000000, 2, 50]
        ]);
        let mut spec = ProcessSpec::read(&old_spec).unwrap();
        assert_eq!(spec.write().unwrap(), old_spec);
        let scope = OwnedScopeSpec {
            memory_resource: None,
            reference: source.clone(),
            unit: "cortex-qualification-codec.scope".into(),
            delegated_root: "/owned/cgroup".into(),
            runtime_dir: "/owned/runtime".into(),
            launcher: "/owned/launcher".into(),
            launcher_identity: source.clone(),
            manager: "/owned/manager".into(),
            manager_identity: source.clone(),
            controls: ScopeControls {
                cpu_quota_us: 100000,
                cpu_period_us: 100000,
                memory_bytes: 536870912,
                swap_bytes: 0,
                tasks_max: 8,
            },
            cpu_time_ms: 2000,
            cancellation_grace_ms: 500,
            escalation_ms: 500,
            isolation: ScopeIsolation::ScopeOnlyInertFixture,
        };
        spec.containment = Some(scope);
        let extended = spec.write().unwrap();
        assert!(arr(&extended, 12).is_err());
        assert!(ProcessSpec::read(&extended).unwrap() == spec);
        spec.identity_checkpoint = Some(IdentityCheckpointPolicy::BeforeInputV1);
        let checkpoint = spec.write().unwrap();
        assert_eq!(checkpoint.as_array().unwrap().len(), 14);
        assert!(arr(&checkpoint, 13).is_err());
        assert!(ProcessSpec::read(&checkpoint).unwrap() == spec);
        let mut malformed = checkpoint.as_array().unwrap().clone();
        malformed[13] = V::Null;
        assert!(ProcessSpec::read(&V::Array(malformed)).is_err());
        let mut unknown = checkpoint.as_array().unwrap().clone();
        unknown[13] = json!("future-policy");
        assert!(ProcessSpec::read(&V::Array(unknown)).is_err());
        let completion = ProcessCompletion::IdentityVerified;
        assert_eq!(
            ProcessCompletion::read(&completion.write().unwrap()).unwrap(),
            completion
        );
        let old_observation = json!([
            cortex_core::binding::ObservationId::new("codec", 1)
                .unwrap()
                .write()
                .unwrap(),
            reference,
            null,
            ["unknown"],
            "Unknown",
            0,
            0,
            0,
            0,
            false,
            0,
            reference
        ]);
        let mut observation = ProcessObservation::read(&old_observation).unwrap();
        assert!(observation.containment.is_none());
        assert!(observation.unresolved_scope_helpers.is_empty());
        assert_eq!(observation.write().unwrap(), old_observation);
        observation
            .unresolved_scope_helpers
            .push(ScopeHelperIdentity {
                pid: 123,
                start_ticks: None,
            });
        let helpers = observation.write().unwrap();
        assert_eq!(helpers.as_array().unwrap().len(), 14);
        assert!(arr(&helpers, 12).is_err());
        assert!(ProcessObservation::read(&helpers).unwrap() == observation);
        let mut extra = helpers.as_array().unwrap().clone();
        extra.push(V::Null);
        assert!(ProcessObservation::read(&V::Array(extra)).is_err());
        let mut null = old_observation.as_array().unwrap().clone();
        null.push(V::Null);
        assert!(ProcessObservation::read(&V::Array(null)).is_err());
    }
}
