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
fields!(ProcessSpec;reference,executable,executable_identity,cwd,args,prepared_input,input_len,host,profile,privacy,approval,limits);
fields!(ProcessIdentity;launch,pid,start_ticks);
fields!(ProcessObservation;id,spec,process,completion,quiescence,input_bytes,stdout_bytes,stderr_bytes,structured_records,malformed_output,elapsed_ms,basis);
symbols!(Quiescence;NoChildStarted,DirectChildReapedStreamsClosed,Unknown);
symbols!(InterruptionReason;Timeout,StartupTimeout,IdleTimeout,StdoutLimit,StderrLimit,EventLimit,BufferLimit,StartupIncomplete,Cancelled,OutputLimit,InputClosed,Io,MalformedOutput,AuthorityLost,AuthorityUnknown);

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
        ] {
            let outcome = ProcessCompletion::Interrupted { reason };
            assert_eq!(
                ProcessCompletion::read(&outcome.write().unwrap()).unwrap(),
                outcome
            );
        }
        assert!(InterruptionReason::read(&json!("untrusted-success")).is_err());
    }
}
