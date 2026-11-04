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
fields!(ProcessLimits;wall_ms,cleanup_ms,input_bytes,output_bytes,frame_bytes,authority_poll_ms,executable_bytes,arguments,argument_bytes);
fields!(ProcessSpec;reference,executable,executable_identity,cwd,args,prepared_input,input_len,host,profile,privacy,approval,limits);
fields!(ProcessIdentity;launch,pid,start_ticks);
fields!(ProcessObservation;id,spec,process,completion,quiescence,input_bytes,stdout_bytes,stderr_bytes,structured_records,malformed_output,elapsed_ms,basis);
symbols!(Quiescence;NoChildStarted,DirectChildReapedStreamsClosed,Unknown);
symbols!(InterruptionReason;Timeout,Cancelled,OutputLimit,InputClosed,Io,MalformedOutput,AuthorityLost,AuthorityUnknown);

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
