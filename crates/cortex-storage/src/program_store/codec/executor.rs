//! Exact descriptor/profile encoding, reconstructed through validated constructors.
use super::*;
use cortex_core::program::executor::ObservationKind as CapabilityObservationKind;
use cortex_core::program::executor::*;

symbols!(CapabilityObservationKind;Advertised => "Advertised",ControlledFixture => "ControlledFixture",HostQualificationReport => "HostQualificationReport");
symbols!(Support;Supported => "Supported",Unsupported => "Unsupported",Unknown => "Unknown");
symbols!(Transport;StandardInputJsonLines => "StandardInputJsonLines");
fields!(CapabilityObservation;control,kind,support,basis);
fields!(Requirement;control,kind);
fields!(ExecutorInputRequest;package,revision,descriptor,requirements,max_bytes);

impl Wire for Control {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(match self {
            Self::Resource(r) => json!(["resource", r.write()?]),
            other => json!([match other {
                Self::StandardInput => "stdin",
                Self::StructuredOutput => "structured-output",
                Self::Filesystem => "filesystem",
                Self::Network => "network",
                Self::Descendants => "descendants",
                Self::Revocation => "revocation",
                Self::Timeout => "timeout",
                Self::Cancellation => "cancellation",
                Self::WorkspaceIdentity => "workspace-identity",
                Self::SessionContinuation => "session-continuation",
                Self::Resource(_) => unreachable!(),
            }]),
        })
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let tag = v.get(0).and_then(V::as_str).ok_or_else(bad)?;
        if tag == "resource" {
            let a = arr(v, 2)?;
            return Ok(Self::Resource(read(&a[1])?));
        }
        arr(v, 1)?;
        Ok(match tag {
            "stdin" => Self::StandardInput,
            "structured-output" => Self::StructuredOutput,
            "filesystem" => Self::Filesystem,
            "network" => Self::Network,
            "descendants" => Self::Descendants,
            "revocation" => Self::Revocation,
            "timeout" => Self::Timeout,
            "cancellation" => Self::Cancellation,
            "workspace-identity" => Self::WorkspaceIdentity,
            "session-continuation" => Self::SessionContinuation,
            _ => return Err(bad()),
        })
    }
}
impl Wire for ExecutorIdentity {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.name(),
            self.version(),
            self.executable().write()?,
            self.host().write()?
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        Self::new(
            a[0].as_str().ok_or_else(bad)?,
            a[1].as_str().ok_or_else(bad)?,
            read(&a[2])?,
            read(&a[3])?,
        )
        .map_err(|_| bad())
    }
}
impl Wire for ExecutorDescriptor {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.reference().write()?,
            self.identity().write()?,
            self.transport().write()?,
            self.observations().to_vec().write()?
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 4)?;
        let observations: Vec<CapabilityObservation> = read(&a[3])?;
        let bound = observations.len().max(1);
        Self::new(
            read(&a[0])?,
            read(&a[1])?,
            read(&a[2])?,
            observations,
            bound,
        )
        .map_err(|_| bad())
    }
}
impl Wire for Requirements {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.reference().write()?,
            self.required().to_vec().write()?
        ]))
    }
    fn read(v: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(v, 2)?;
        let required: Vec<Requirement> = read(&a[1])?;
        let bound = required.len();
        Self::new(read(&a[0])?, required, bound).map_err(|_| bad())
    }
}
