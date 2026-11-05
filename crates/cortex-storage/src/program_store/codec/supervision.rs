//! Complete supervised-fixture inputs, never trusted serialized process projections.
use super::*;
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
// Retain the original prefix byte-for-byte when the extension is absent. An
// older exact-length decoder rejects the additional mandatory field.
macro_rules! optional_tail {
    ($t:ident; $tail:ident; $($field:ident),+) => {
        impl Wire for $t {
            fn write(&self) -> Result<V, ProgramStoreError> {
                let mut fields = vec![$(self.$field.write()?),+];
                if let Some(tail) = &self.$tail { fields.push(tail.write()?); }
                Ok(V::Array(fields))
            }
            fn read(v: &V) -> Result<Self, ProgramStoreError> {
                let a = v.as_array().ok_or_else(bad)?;
                let n = [$(stringify!($field)),+].len();
                if a.len() != n && a.len() != n + 1 { return Err(bad()); }
                let mut fields = a.iter();
                Ok(Self { $($field: read(fields.next().ok_or_else(bad)?)?),+,
                    $tail: if a.len() == n + 1 { Some(read(fields.next().ok_or_else(bad)?)?) } else { None } })
            }
        }
    };
}
fields!(ReadOnlyMount;source,destination);
fields!(ScopeControls;cpu_quota_us,cpu_period_us,memory_bytes,swap_bytes,tasks_max);
fields!(OwnedScopeSpec;reference,unit,delegated_root,runtime_dir,launcher,launcher_identity,manager,manager_identity,controls,cpu_time_ms,cancellation_grace_ms,escalation_ms,isolation);
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
optional_tail!(ProcessSpec;containment;reference,executable,executable_identity,cwd,args,prepared_input,input_len,host,profile,privacy,approval,limits);
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
symbols!(Quiescence;NoChildStarted,DirectChildReapedStreamsClosed,OwnedScopeEmpty,Unknown);
symbols!(InterruptionReason;Timeout,StartupTimeout,IdleTimeout,StdoutLimit,StderrLimit,EventLimit,BufferLimit,StartupIncomplete,ContainmentUnknown,CpuTimeLimit,Cancelled,OutputLimit,InputClosed,Io,MalformedOutput,AuthorityLost,AuthorityUnknown);

impl Wire for ProcessCompletion {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Spawned => json!(["spawned"]),
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
