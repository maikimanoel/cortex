//! Explicit version-one material operation payloads. The legacy release shape
//! and all legacy token meanings remain unchanged.

use super::*;
use cortex_core::knowledge::{MAX_LOCATION_BYTES, SourcePath};
use cortex_evidence::program::material::{
    IntegrationAcceptance, MaterialFile, MaterialMapping, MaterialMode, MaterialObservation,
    MaterialRelease, MaterialSubject,
};

fn path_read(value: &V) -> Result<SourcePath, ProgramStoreError> {
    let bytes = value.as_array().ok_or_else(bad)?;
    if bytes.len() > MAX_LOCATION_BYTES {
        return Err(ProgramStoreError::Capacity);
    }
    let bytes = bytes.iter().map(byte).collect::<Result<Vec<_>, _>>()?;
    SourcePath::new(&bytes).map_err(|_| bad())
}

fn byte(value: &V) -> Result<u8, ProgramStoreError> {
    u8::try_from(u64::read(value)?).map_err(|_| bad())
}

symbols!(MaterialMode; Regular => "100644", Executable => "100755");

impl Wire for MaterialFile {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([
            self.path().bytes(),
            self.mode().write()?,
            self.length(),
            self.sha256()
        ]))
    }
    fn read(value: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(value, 4)?;
        let mut sha256 = [0; 32];
        for (output, input) in sha256.iter_mut().zip(arr(&a[3], 32)?) {
            *output = byte(input)?;
        }
        Self::new(path_read(&a[0])?, read(&a[1])?, read(&a[2])?, sha256)
            .map_err(|_| ProgramStoreError::Capacity)
    }
}
impl Wire for MaterialMapping {
    fn write(&self) -> Result<V, ProgramStoreError> {
        Ok(json!([self.source.write()?, self.destination.bytes()]))
    }
    fn read(value: &V) -> Result<Self, ProgramStoreError> {
        let a = arr(value, 2)?;
        Ok(Self {
            source: read(&a[0])?,
            destination: path_read(&a[1])?,
        })
    }
}

fn contract_write(contract: &EdgeEvidenceContract) -> Result<V, ProgramStoreError> {
    list([
        scope(contract.scope()),
        contract.revision().write(),
        contract.artifact_requirement().write(),
        list(contract.correspondences().iter().map(write)),
        contract.basis().write(),
    ])
}

fn contract_read(
    value: &V,
    kernel: &CoordinationKernel,
) -> Result<EdgeEvidenceContract, ProgramStoreError> {
    let a = arr(value, 5)?;
    let s = arr(&a[0], 6)?;
    let limits = kernel.state().definition().limits();
    let bounded = SupportLimits::new(limits.entries, limits.entries, limits.entries, limits.bytes)
        .map_err(|_| bad())?;
    let snapshot = EdgeScopeSnapshot::capture(kernel.state().readiness(), &read(&s[1])?)
        .map_err(|_| bad())?
        .with_historical_cursor(read(&s[5])?)
        .map_err(|_| bad())?
        .with_original_scopes(read(&s[2])?, read(&s[3])?)
        .map_err(|_| bad())?;
    if scope(&snapshot)? != a[0] || a[3].as_array().ok_or_else(bad)?.len() > limits.entries {
        return Err(bad());
    }
    EdgeEvidenceContract::new(
        snapshot,
        read(&a[1])?,
        read(&a[2])?,
        read(&a[3])?,
        read(&a[4])?,
        bounded,
    )
    .map_err(|_| bad())
}

pub(super) fn observe_write(value: &MaterialObservation) -> Result<V, ProgramStoreError> {
    let subject = match &value.subject {
        MaterialSubject::Release {
            contract,
            release,
            review,
            artifact_report,
            mapping,
        } => json!([
            "release",
            contract_write(contract)?,
            release.write()?,
            review.write()?,
            artifact_report.write()?,
            mapping.write()?
        ]),
        MaterialSubject::Integration {
            scope,
            releases,
            output,
        } => json!([
            "integration",
            scope.write()?,
            list(releases.iter().map(write))?,
            output.write()?
        ]),
    };
    Ok(json!([value.report.write()?, subject]))
}

pub(super) fn observe_read(
    value: &V,
    kernel: &CoordinationKernel,
) -> Result<MaterialObservation, ProgramStoreError> {
    let a = arr(value, 2)?;
    let subject = match a[1].get(0).and_then(V::as_str) {
        Some("release") => {
            let s = arr(&a[1], 6)?;
            MaterialSubject::Release {
                contract: Box::new(contract_read(&s[1], kernel)?),
                release: read(&s[2])?,
                review: read(&s[3])?,
                artifact_report: read(&s[4])?,
                mapping: read(&s[5])?,
            }
        }
        Some("integration") => {
            let s = arr(&a[1], 4)?;
            let ids = arr(&s[2], 2)?;
            MaterialSubject::Integration {
                scope: read(&s[1])?,
                releases: [read(&ids[0])?, read(&ids[1])?],
                output: read(&s[3])?,
            }
        }
        _ => return Err(bad()),
    };
    Ok(MaterialObservation {
        report: read(&a[0])?,
        subject,
    })
}

pub(super) fn release_material_write(value: &MaterialRelease) -> Result<V, ProgramStoreError> {
    Ok(json!([
        release_write(&value.release)?,
        value.material.write()?
    ]))
}
pub(super) fn release_material_read(
    value: &V,
    kernel: &CoordinationKernel,
) -> Result<MaterialRelease, ProgramStoreError> {
    let a = arr(value, 2)?;
    Ok(MaterialRelease {
        release: release_read(&a[0], kernel)?,
        material: read(&a[1])?,
    })
}

fields!(IntegrationAcceptance; observation, scope, approval, examination);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_store::tests::fixture;

    #[test]
    fn material_file_has_exact_literal_path_mode_digest_and_length_encoding() {
        const GOLDEN: &[u8] = br#"[[97,47,98],"100755",3,[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31]]"#;
        let digest = std::array::from_fn(|index| index as u8);
        let file = MaterialFile::new(
            SourcePath::new(b"a/b").unwrap(),
            MaterialMode::Executable,
            3,
            digest,
        )
        .unwrap();
        assert_eq!(
            encoded(&file.write().unwrap(), 1000).unwrap().as_slice(),
            GOLDEN
        );
        let parsed = parsed(GOLDEN, 1000).unwrap();
        assert!(MaterialFile::read(&parsed).unwrap() == file);
        assert!(
            MaterialFile::read(&json!([
                b"a/b",
                "100755",
                cortex_core::knowledge::MAX_SOURCE_BYTES,
                digest
            ]))
            .is_ok()
        );
        for invalid in [
            json!([b"a/b", "120000", 3, digest]),
            json!([b"../b", "100755", 3, digest]),
            json!([b"a/b", "100755", 3, vec![0; 31]]),
            json!([
                b"a/b",
                "100755",
                cortex_core::knowledge::MAX_SOURCE_BYTES + 1,
                digest
            ]),
        ] {
            assert!(MaterialFile::read(&invalid).is_err());
        }
    }

    #[test]
    fn exact_material_tag_is_canonical_and_unknown_versions_refuse() {
        let initial = fixture::definition();
        let kernel = CoordinationKernel::new_protected_material(initial.clone()).unwrap();
        let scope = kernel
            .state()
            .readiness()
            .scope(&fixture::task("B"))
            .unwrap();
        let contract = scope.context().contract().clone();
        let observation = ObservationId::new("typed-comparison", 1).unwrap();
        let operation = fixture::operation(
            &kernel,
            "typed-acceptance",
            Command::AcceptIntegration(Box::new(IntegrationAcceptance {
                observation: EvidenceReference::new(
                    observation.clone(),
                    Some(AssessmentId::new("typed-judgment").unwrap()),
                ),
                scope: Box::new(scope),
                approval: fixture::source("scoped-approval"),
                examination: Examination {
                    observation,
                    contract,
                    candidate: None,
                    profile: Some(fixture::profile()),
                },
            })),
        );
        let bytes = super::operation(&operation, 1_000_000).unwrap();
        assert!(decode_operation(&bytes, &kernel).unwrap() == operation);
        let mut noncanonical = bytes.to_vec();
        noncanonical.push(b' ');
        assert!(matches!(
            decode_operation(&noncanonical, &kernel),
            Err(ProgramStoreError::Corrupt)
        ));
        let unknown = std::str::from_utf8(&bytes)
            .unwrap()
            .replace("accept-integration-v1", "accept-integration-v2");
        assert!(matches!(
            decode_operation(unknown.as_bytes(), &kernel),
            Err(ProgramStoreError::Corrupt)
        ));

        // Decodable input never migrates a legacy profile or confers acceptance.
        let mut legacy = CoordinationKernel::new(initial).unwrap();
        assert!(matches!(
            legacy.admit(operation.clone()),
            Err(KernelError::Core(ProgramError::EffectUnavailable))
        ));
        assert_eq!(legacy.revision(), 0);
        assert!(kernel.accepted_integration(&operation.id).is_err());
    }
}
