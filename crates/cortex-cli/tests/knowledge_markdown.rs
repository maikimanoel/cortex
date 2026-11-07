//! Actual committed Markdown -> existing index -> contextual qualification.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{inspect, knowledge::markdown::*};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;

const FIXTURE: &str = include_str!("../src/knowledge/markdown/fixtures/cortex.md");
const PROFILE_DOC: &str = include_str!("../../../docs/architecture/knowledge-markdown.md");
const CANONICAL: &str = include_str!("../../../docs/architecture/candidate-binding.md");
fn id(n: u64) -> ObservationId {
    ObservationId::new("markdown-context", n).unwrap()
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("markdown.integration").unwrap(),
            "Inspect explicit documentation references",
            "Retain exact candidate, source and lexical relationships",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            inspect(&r.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    )
}
fn extract(
    r: &Repo,
    c: &CandidateBindingContract,
    path: &[u8],
) -> Result<ReferenceIndex, ExtractionError> {
    extract_markdown_references(
        &r.path,
        c,
        id(1),
        SourcePath::new(path).unwrap(),
        UNIX_EPOCH,
    )
}
fn text<'a>(g: &'a ReferenceIndex, locator: &str) -> &'a str {
    let span = g.node(locator).unwrap().span();
    std::str::from_utf8(&g.bytes()[span.start()..span.end()]).unwrap()
}
fn occurrence<'a>(g: &'a ReferenceIndex, markup: &str) -> &'a ExtractedReference {
    let at = std::str::from_utf8(g.bytes())
        .unwrap()
        .find(markup)
        .unwrap();
    g.references()
        .iter()
        .find(|r| r.from() == format!("link@{at}"))
        .unwrap()
}
fn snapshot(r: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, u32)> {
    use std::os::unix::fs::PermissionsExt;
    let mut pending = vec![r.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let m = fs::symlink_metadata(&path).unwrap();
        assert!(!m.is_symlink());
        let bytes = if m.is_dir() {
            pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        result.insert(
            path.strip_prefix(&r.path).unwrap().to_owned(),
            (
                bytes,
                m.modified().unwrap(),
                m.len(),
                m.permissions().mode(),
            ),
        );
    }
    result
}

#[test]
fn committed_cortex_fixture_has_exact_spans_distinct_occurrences_and_two_step_relationships() {
    let r = Repo::new();
    r.commit_file("contract.md", FIXTURE);
    let c = contract(&r);
    let g = extract(&r, &c, b"contract.md").unwrap();
    assert_eq!(g.bytes(), FIXTURE.as_bytes());
    assert_eq!(g, extract(&r, &c, b"contract.md").unwrap());
    assert_eq!(
        text(&g, "#cortex-binding-contract"),
        "# Cortex binding contract"
    );
    assert_eq!(text(&g, "#invariants"), "## Invariants");
    assert_eq!(text(&g, "#invariants-1"), "## Invariants");
    assert_eq!(text(&g, "#invariants-1-1"), "## Invariants-1");
    assert_eq!(
        text(&g, "#escaped-name-and-value"),
        r"## Escaped \[name\] and \(value\) ###"
    );
    for markup in [
        "[invariants](#invariants)",
        "[the same target](#invariants)",
        "[ação](#invariants)",
    ] {
        let link = occurrence(&g, markup);
        assert_eq!(text(&g, link.from()), markup);
        assert_eq!(link.spelling(), "#invariants");
        assert_eq!(link.target(), &ReferenceTarget::Local("#invariants".into()));
    }
    let definition = format!("definition@{}", FIXTURE.rfind("[Core Binding]:").unwrap());
    assert_eq!(text(&g, &definition), "[Core Binding]: #invariants");
    for markup in [
        "[binding][Core Binding]",
        "[Core Binding][]",
        "[Core Binding] retains",
    ] {
        let link = occurrence(&g, markup);
        assert_eq!(link.target(), &ReferenceTarget::Local(definition.clone()));
    }
    assert_eq!(g.incoming(&definition).unwrap().count(), 3);
    assert_eq!(g.incoming("#invariants").unwrap().count(), 4);
    assert_eq!(g.incoming("#cortex-binding-contract").unwrap().count(), 0);
    assert!(matches!(
        g.node("#invented"),
        Err(KnowledgeError::MissingNode)
    ));
    assert!(
        g.references()
            .iter()
            .all(|r| !text(&g, r.from()).contains("fake"))
    );
    assert_eq!(g.references().len(), 16);
    assert_eq!(g.id(), &id(3));
    assert_eq!(g.provenance().source(), ADAPTER);
    assert_eq!(g.provenance().version(), PROFILE);
    assert_eq!(g.recorded_at(), UNIX_EPOCH);
    assert!(
        g.references()
            .iter()
            .all(|r| r.origin() == RelationOrigin::Extracted
                && r.disposition() == ValidationDisposition::Unverified)
    );
    assert_eq!(reinspect(&r.path, &g, id(4)).unwrap().validity(), None);
    for private in ["invalid.example", "Core Binding", "invariants"] {
        assert!(!format!("{g:?}").contains(private));
    }
}
#[test]
fn existing_canonical_architecture_document_is_real_committed_source_not_a_text_inventory() {
    let r = Repo::new();
    r.commit_file("binding.md", CANONICAL);
    let g = extract(&r, &contract(&r), b"binding.md").unwrap();
    assert_eq!(
        text(&g, "#decision-a-identities-and-contract-revisions"),
        "## Decision A: identities and contract revisions"
    );
    let link = occurrence(
        &g,
        "[CAP-CORE-002](../product/capability-catalog.md#cap-core-002)",
    );
    assert_eq!(
        link.spelling(),
        "../product/capability-catalog.md#cap-core-002"
    );
    assert_eq!(
        link.target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
    );
    assert_eq!(g.references().len(), 9);
    assert_eq!(g.bytes(), CANONICAL.as_bytes());
    r.commit_file("profile.md", PROFILE_DOC);
    let profile = extract(&r, &contract(&r), b"profile.md").unwrap();
    assert_eq!(
        text(&profile, "#native-v1-coverage"),
        "## Native v1 coverage"
    );
}
#[test]
fn only_covered_fragments_resolve_and_no_nonlocal_target_is_invented_or_accessed() {
    let r = Repo::new();
    // Existing target with an unsupported anchor would fail if read recursively.
    r.commit_file("other.md", "# ação\n<a id=\"found\">\n");
    r.commit_file("doc.md", "# Found\n[x](#found) [x](#absent) [x](#) [x](#%66ound) [x](#ação) [x](#found/child) [x][missing]\n[x](other.md#found) [x](/etc/passwd) [x](file:///etc/passwd) [x](https://invalid.example/) [x](mailto:nobody@invalid.example) [x](javascript:alert(1)) [x](//invalid.example/)\n");
    let before = snapshot(&r);
    let g = extract(&r, &contract(&r), b"doc.md").unwrap();
    assert_eq!(g.references().len(), 14);
    assert_eq!(
        occurrence(&g, "[x](#found)").target(),
        &ReferenceTarget::Local("#found".into())
    );
    assert_eq!(
        occurrence(&g, "[x](#absent)").target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
    );
    for raw in ["[x](#)", "[x](#%66ound)", "[x](#ação)", "[x](#found/child)"] {
        assert_eq!(
            occurrence(&g, raw).target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment)
        );
    }
    for raw in [
        "[x][missing]",
        "[x](other.md#found)",
        "[x](/etc/passwd)",
        "[x](file:///etc/passwd)",
        "[x](https://invalid.example/)",
        "[x](mailto:nobody@invalid.example)",
        "[x](javascript:alert(1))",
        "[x](//invalid.example/)",
    ] {
        assert_eq!(
            occurrence(&g, raw).target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
        );
    }
    assert_eq!(snapshot(&r), before);
}
#[test]
fn current_profile_qualification_and_repeated_actual_captures_preserve_git_content_and_metadata() {
    let r = Repo::new();
    r.commit_file(
        "doc.md",
        "# Local\n[one](#local) [two][id] [id][] [id]\n[id]: #local\n",
    );
    r.git(&["config", "core.splitIndex", "false"]);
    let c = contract(&r);
    let before = snapshot(&r);
    let g = extract(&r, &c, b"doc.md").unwrap();
    assert_eq!(
        reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_eq!(
        reinspect(&r.path, &g, id(5)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_eq!(inspect_validity(&g, None).unwrap().validity(), None);
    assert_eq!(
        reinspect(&r.path, &g, id(3)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert_eq!(snapshot(&r), before);
    for (adapter, profile) in [(ADAPTER, "old-rules"), ("other-adapter", PROFILE)] {
        let old = ReferenceIndex::new(
            g.candidate().clone(),
            g.path().clone(),
            g.bytes().to_vec(),
            ExtractionRecord::new(
                ObservationId::new("old-profile", 1).unwrap(),
                adapter,
                profile,
                UNIX_EPOCH,
            )
            .unwrap(),
            g.nodes().to_vec(),
            g.references().to_vec(),
        )
        .unwrap();
        let view = reinspect(&r.path, &old, id(6)).unwrap();
        assert!(!view.profile_matches());
        assert_eq!(view.validity(), Some(Validity::Stale));
        assert_eq!(view.index(), &old);
    }
    let unknown = CandidateObservation::new(
        id(7),
        &c,
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate,
        )
        .unwrap(),
        None,
    );
    assert_eq!(
        inspect_validity(&g, Some(&unknown)).unwrap().validity(),
        None
    );
}
#[test]
fn detached_subdirectory_and_literal_non_utf8_source_paths_keep_candidate_identity() {
    use std::os::unix::ffi::OsStringExt;
    let r = Repo::new();
    let raw = std::ffi::OsString::from_vec(b"literal-[*]-\xff.md".to_vec());
    fs::write(r.path.join(&raw), "# Heading\n[x](#heading)\n").unwrap();
    assert!(
        r.command()
            .args(["add", "--"])
            .arg(&raw)
            .status()
            .unwrap()
            .success()
    );
    r.git(&["commit", "-m", "literal byte path"]);
    r.git(&["checkout", "--detach"]);
    fs::create_dir(r.path.join("sub")).unwrap();
    let c = contract(&r);
    let g = extract_markdown_references(
        &r.path.join("sub"),
        &c,
        id(1),
        SourcePath::new(b"literal-[*]-\xff.md").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(g.path().bytes(), b"literal-[*]-\xff.md");
    assert!(matches!(
        g.candidate().observation().state().head(),
        Head::Detached { .. }
    ));
    assert_eq!(g.incoming("#heading").unwrap().count(), 1);
}
#[test]
fn dirty_mismatched_missing_symlink_and_unborn_sources_publish_no_index() {
    let r = Repo::new();
    r.commit_file("doc.md", "# Heading\n[x](#heading)\n");
    let c = contract(&r);
    let g = extract(&r, &c, b"doc.md").unwrap();
    fs::write(r.path.join("doc.md"), "# Changed\n").unwrap();
    assert!(matches!(
        extract(&r, &c, b"doc.md"),
        Err(ExtractionError::Candidate(_))
    ));
    assert_eq!(
        reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Stale)
    );
    r.git(&["add", "doc.md"]);
    assert!(matches!(
        extract(&r, &c, b"doc.md"),
        Err(ExtractionError::Candidate(_))
    ));
    r.git(&["commit", "-m", "change"]);
    assert_eq!(
        reinspect(&r.path, &g, id(5)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert!(matches!(
        extract(&r, &c, b"doc.md"),
        Err(ExtractionError::Candidate(_))
    ));
    let other = Repo::new();
    other.commit_file("doc.md", "# Heading\n");
    assert!(matches!(
        extract(&other, &c, b"doc.md"),
        Err(ExtractionError::Candidate(_))
    ));
    assert!(matches!(
        extract(&r, &contract(&r), b"missing.md"),
        Err(ExtractionError::Inspection(_))
    ));
    std::os::unix::fs::symlink("doc.md", r.path.join("link.md")).unwrap();
    r.git(&["add", "link.md"]);
    r.git(&["commit", "-m", "link"]);
    assert!(matches!(
        extract(&r, &contract(&r), b"link.md"),
        Err(ExtractionError::Inspection(_))
    ));
    assert!(matches!(
        extract(&Repo::new(), &c, b"doc.md"),
        Err(ExtractionError::MissingCommit)
    ));
}
#[test]
fn malformed_unsupported_and_oversized_committed_input_cannot_become_covered() {
    let r = Repo::new();
    for (source, expected) in [
        ("# ação\n[x](#absent)\n", 0),
        ("# Good\n[x](#good", 1),
        ("# Good\n[a]: #good\n[A]: #good\n", 2),
    ] {
        r.commit_file("doc.md", source);
        let error = extract(&r, &contract(&r), b"doc.md").unwrap_err();
        assert!(match expected {
            0 => matches!(
                error,
                ExtractionError::Unsupported(UnsupportedSyntax::Anchor)
            ),
            1 => matches!(error, ExtractionError::InvalidMarkdown),
            _ => matches!(error, ExtractionError::DuplicateDefinition),
        });
        assert!(!format!("{error:?} {error}").contains("#good"));
    }
    r.commit_file("doc.md", &"a".repeat(MAX_SOURCE_BYTES + 1));
    assert!(matches!(
        extract(&r, &contract(&r), b"doc.md"),
        Err(ExtractionError::Inspection(_))
    ));
    fs::write(r.path.join("doc.md"), b"# invalid \xff").unwrap();
    r.git(&["add", "doc.md"]);
    r.git(&["commit", "-m", "invalid utf8"]);
    assert!(matches!(
        extract(&r, &contract(&r), b"doc.md"),
        Err(ExtractionError::InvalidUtf8)
    ));
}

#[test]
fn missing_committed_object_and_untracked_or_changed_configuration_never_establish_current() {
    let r = Repo::new();
    r.commit_file("doc.md", "# A\n[x](#a)\n");
    let c = contract(&r);
    let g = extract(&r, &c, b"doc.md").unwrap();
    r.git(&["config", "core.fileMode", "false"]);
    assert_eq!(
        reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Stale)
    );
    fs::write(r.path.join("untracked.md"), "private").unwrap();
    assert!(matches!(
        extract(&r, &c, b"doc.md"),
        Err(ExtractionError::Candidate(_))
    ));
    fs::remove_file(r.path.join("untracked.md")).unwrap();
    let oid = String::from_utf8(r.git(&["rev-parse", "HEAD:doc.md"]).stdout).unwrap();
    let oid = oid.trim();
    fs::remove_file(r.path.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    assert!(matches!(
        extract(&r, &c, b"doc.md"),
        Err(ExtractionError::Inspection(_))
    ));
}

#[test]
fn unsupported_syntax_repair_refuses_committed_sources_before_any_index_can_be_qualified() {
    let r = Repo::new();
    for (finding, source, category) in [
        (
            "DOCS-1",
            "# Local\n[key]: #local\n \"[hidden](#local)\"\n[key]\n",
            UnsupportedSyntax::Link,
        ),
        (
            "DOCS-1",
            "# Local\n[key]: #local\n\t ([hidden](#local))\n[key]\n",
            UnsupportedSyntax::Link,
        ),
        (
            "DOCS-2",
            "Hidden\n=== \n[read](#hidden)\n",
            UnsupportedSyntax::Anchor,
        ),
        ("DOCS-2", "Hidden\n---\t\n", UnsupportedSyntax::Anchor),
        (
            "DOCS-3",
            "-\t# Hidden\n[read](#hidden)\n",
            UnsupportedSyntax::Container,
        ),
        (
            "DOCS-4",
            "# Local\n<123@example.invalid>\n[read](#local)\n",
            UnsupportedSyntax::Html,
        ),
    ] {
        r.commit_file("repair.md", source);
        let before = snapshot(&r);
        let result = extract(&r, &contract(&r), b"repair.md");
        assert!(
            matches!(result, Err(ExtractionError::Unsupported(actual)) if actual == category),
            "{finding} must refuse with its typed syntax category"
        );
        assert_eq!(
            snapshot(&r),
            before,
            "{finding} must preserve content and metadata on refusal"
        );
    }
}

#[test]
fn syntax_repair_retains_committed_near_neighbors_spans_and_scoped_current_validity() {
    let r = Repo::new();
    let source = "# Local\n[key]: #local\n\n \"[visible](#local)\"\n[key]\n-\tFlat [bullet](#local) prose.\n1.\tFlat [ordered](#local) prose.\n2 < 3 and x > y; 123@example.invalid, <123>, ordinary [notes].\n[angle](<#local>)\n";
    r.commit_file("repair.md", source);
    let c = contract(&r);
    let before = snapshot(&r);
    let g = extract(&r, &c, b"repair.md").unwrap();
    assert_eq!(g.bytes(), source.as_bytes());
    assert_eq!(g.references().len(), 6);
    assert_eq!(g.incoming("#local").unwrap().count(), 5);
    let definition = format!("definition@{}", source.find("[key]:").unwrap());
    assert_eq!(g.incoming(&definition).unwrap().count(), 1);
    for markup in [
        "[visible](#local)",
        "[bullet](#local)",
        "[ordered](#local)",
        "[angle](<#local>)",
    ] {
        let link = occurrence(&g, markup);
        assert_eq!(text(&g, link.from()), markup);
        assert_eq!(link.target(), &ReferenceTarget::Local("#local".into()));
    }
    let view = reinspect(&r.path, &g, id(4)).unwrap();
    assert!(view.profile_matches());
    assert_eq!(view.validity(), Some(Validity::Current));
    assert_eq!(snapshot(&r), before);
}

#[test]
fn docs5_committed_control_bearing_reference_uses_refuse_at_the_adapter_boundary() {
    let r = Repo::new();
    for markup in [
        "[read][key\tname]",
        "[key\tname][]",
        "[key\tname]",
        "[read\ttext][key name]",
        "[read][missing\tname]",
    ] {
        let source = format!("# Local\n{markup}\n[key name]: #local\n");
        r.commit_file("control-use.md", &source);
        let c = contract(&r);
        let before = snapshot(&r);
        assert!(
            matches!(
                extract(&r, &c, b"control-use.md"),
                Err(ExtractionError::Unsupported(UnsupportedSyntax::Link))
            ),
            "DOCS-5 must refuse the raw reference use, not return a Core domain error"
        );
        assert_eq!(snapshot(&r), before);
    }
}

#[test]
fn docs5_committed_tabbed_definitions_preserve_control_free_use_spellings_and_neighbors() {
    let r = Repo::new();
    let source = "# Local\n[KEY\tNAME]: #local\n[ação][key  name] [key name][] [KEY NAME]\n-\tFlat [read](#local) prose.\n`[key\tname]` and ordinary [not\tdefined], 2 < 3.\n";
    r.commit_file("control-neighbor.md", source);
    let c = contract(&r);
    let before = snapshot(&r);
    let g = extract(&r, &c, b"control-neighbor.md").unwrap();
    assert_eq!(g.bytes(), source.as_bytes());
    assert_eq!(g.references().len(), 5);
    let definition = format!("definition@{}", source.find("[KEY\tNAME]:").unwrap());
    assert_eq!(text(&g, &definition), "[KEY\tNAME]: #local");
    assert_eq!(g.incoming(&definition).unwrap().count(), 3);
    assert_eq!(g.incoming("#local").unwrap().count(), 2);
    for markup in ["[ação][key  name]", "[key name][]", "[KEY NAME]"] {
        let reference = occurrence(&g, markup);
        assert_eq!(reference.spelling(), markup);
        assert_eq!(text(&g, reference.from()), markup);
        assert_eq!(
            reference.target(),
            &ReferenceTarget::Local(definition.clone())
        );
    }
    assert_eq!(
        reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_eq!(snapshot(&r), before);
}
