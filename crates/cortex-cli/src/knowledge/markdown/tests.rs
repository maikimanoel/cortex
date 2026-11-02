use super::*;
use crate::support::Repo;
use cortex_core::{CandidateId, TaskContract, TaskId};
use std::time::UNIX_EPOCH;
fn parsed(s: &str) -> (Vec<KnowledgeNode>, Vec<ExtractedReference>) {
    scanner::extract(s.as_bytes()).unwrap()
}
#[test]
fn escaped_delimiters_balanced_parentheses_code_runs_and_legitimate_neighbors() {
    let s = "# Local\r\n[escaped \\] text](#local) \\[fake](#missing) `[fake](#missing)` ``a ` [fake](#missing)`` [普通](<#local>) [ordinary]\r\n[x](https://invalid.example/a(b(c))) [x](file:a\\(b\\))\r\n";
    let (nodes, refs) = parsed(s);
    assert_eq!(refs.len(), 4);
    assert_eq!(refs[0].target(), &ReferenceTarget::Local("#local".into()));
    assert_eq!(refs[1].target(), &ReferenceTarget::Local("#local".into()));
    assert_eq!(refs[2].spelling(), "https://invalid.example/a(b(c))");
    assert_eq!(refs[3].spelling(), r"file:a\(b\)");
    for r in &refs {
        let n = nodes.iter().find(|n| n.locator() == r.from()).unwrap();
        assert!(s[n.span().start()..n.span().end()].starts_with('['));
    }
}
#[test]
fn forward_normalized_definitions_and_duplicate_headings_preserve_occurrences() {
    let (nodes, refs) = parsed("# A\n# A-1\n# A\n# A\n[x][ A  B ] [a b][] [A B]\n[a\tb]: #a-2\n");
    for key in ["#a", "#a-1", "#a-2", "#a-3"] {
        assert!(nodes.iter().any(|n| n.locator() == key));
    }
    assert_eq!(refs.len(), 4);
    let definition = refs
        .iter()
        .find(|r| r.from().starts_with("definition@"))
        .unwrap();
    assert_eq!(definition.target(), &ReferenceTarget::Local("#a-2".into()));
    assert_eq!(
        refs.iter()
            .filter(|r| r.target() == &ReferenceTarget::Local(definition.from().into()))
            .count(),
        3
    );
}
#[test]
fn unsupported_anchor_mechanisms_and_ambiguous_link_syntax_refuse_entire_source() {
    for source in [
        "# ação\n[x](#missing)",
        "# `code`\n",
        "# title {#id}\n",
        "# &amp;\n",
        "# *bold*\n",
        "# Good\n{#id}\n[x](#id)",
        "[<a id=\"x\">text</a>](#x)",
        "- intro\n    ```\n# Fake\n[x](#fake)\n    ```",
        "- intro\n  ```\n# Fake\n[x](#fake)\n  ```",
        "<a id=\"x\"></a>\n",
        "<https://invalid.example/>\n",
        "Title\n===\n",
        "> # Block quote\n",
        "- ```text\n[fake](#x)\n",
        "- # Nested\n",
        "[x](#x \"title\")",
        "![x](#x)",
        "[nested [x]](#x)",
        "[x][ação]",
        "[text]\n(#target)\n",
        "[text]\n[label]\n[label]: #target\n",
        "[text]\n    (#target)\n",
        "[{#id}](#id)",
        "[x]: #x\n[X]: #x\n",
    ] {
        assert!(
            matches!(scanner::extract(source.as_bytes()), Err(error) if matches!(error, ExtractionError::Unsupported(_) | ExtractionError::DuplicateDefinition)),
            "source category must refuse: {source:?}"
        );
    }
}
#[test]
fn malformed_truncated_controls_empty_and_encoding_fail_atomically() {
    for source in [
        b"".as_slice(),
        b"\xff",
        b"a\0b",
        b"a\rb",
        b"```text\n[x](#x)",
        b"[x](#x",
        b"[x](a(b)",
        b"[x][label",
        b"[open",
        b"`unclosed",
        b"[x](<unclosed)",
    ] {
        assert!(scanner::extract(source).is_err());
    }
    assert!(matches!(
        scanner::extract(b"\xff"),
        Err(ExtractionError::InvalidUtf8)
    ));
    assert!(matches!(
        scanner::extract(b"```\n"),
        Err(ExtractionError::InvalidMarkdown)
    ));
}
#[test]
fn finite_source_spelling_node_reference_and_parenthesis_bounds() {
    assert!(scanner::extract(&vec![b'a'; MAX_SOURCE_BYTES]).is_ok());
    assert!(matches!(
        scanner::extract(&vec![b'a'; MAX_SOURCE_BYTES + 1]),
        Err(ExtractionError::Limit)
    ));
    let at = format!("# {}\n", "a".repeat(MAX_LOCATION_BYTES - 1));
    assert!(scanner::extract(at.as_bytes()).is_ok());
    assert!(matches!(
        scanner::extract(format!("# {}", "a".repeat(MAX_LOCATION_BYTES)).as_bytes()),
        Err(ExtractionError::Limit)
    ));
    let links = format!("# A\n{}", "[x](#a) ".repeat(MAX_REFERENCES));
    assert_eq!(parsed(&links).1.len(), MAX_REFERENCES);
    assert!(matches!(
        scanner::extract(format!("{links}[x](#a)").as_bytes()),
        Err(ExtractionError::Limit)
    ));
    let headings = (0..MAX_NODES - 1)
        .map(|n| format!("# H{n}\n"))
        .collect::<String>();
    assert_eq!(parsed(&headings).0.len(), MAX_NODES);
    assert!(matches!(
        scanner::extract(format!("{headings}# Extra\n").as_bytes()),
        Err(ExtractionError::Limit)
    ));
    let dest = format!("[x]({})", "a".repeat(MAX_LOCATION_BYTES));
    assert_eq!(parsed(&dest).1[0].spelling().len(), MAX_LOCATION_BYTES);
    assert!(matches!(
        scanner::extract(format!("[x]({})", "a".repeat(MAX_LOCATION_BYTES + 1)).as_bytes()),
        Err(ExtractionError::Limit)
    ));
    assert!(
        scanner::extract(format!("[x](a{}b{})", "(".repeat(32), ")".repeat(32)).as_bytes()).is_ok()
    );
    assert!(matches!(
        scanner::extract(format!("[x](a{}b{})", "(".repeat(33), ")".repeat(33)).as_bytes()),
        Err(ExtractionError::Limit)
    ));
}
#[test]
fn actual_between_capture_change_withholds_index_and_overflow_precedes_io() {
    let r = Repo::new();
    r.commit_file("doc.md", "# Heading\n[x](#heading)\n");
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let c = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("race").unwrap(),
            "Observe Markdown",
            "Reject racing source",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            crate::inspect(&r.path)
                .unwrap()
                .repository_identity()
                .unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    );
    let result = extract_between(
        &r.path,
        &c,
        ObservationId::new("race", 1).unwrap(),
        SourcePath::new(b"doc.md").unwrap(),
        UNIX_EPOCH,
        || std::fs::write(r.path.join("doc.md"), "# Changed\n").unwrap(),
    );
    assert!(matches!(result, Err(ExtractionError::Candidate(_))));
    assert!(matches!(
        extract_markdown_references(
            Path::new("/does-not-exist"),
            &c,
            ObservationId::new("overflow", u64::MAX - 1).unwrap(),
            SourcePath::new(b"doc.md").unwrap(),
            UNIX_EPOCH
        ),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn varied_fences_and_indented_code_do_not_create_anchors_or_link_occurrences() {
    let source = "   ````md\n### Fake\n[x](#fake)\n```\n[x](#fake)\n   `````  \n   ~~~\n[x](#fake)\n````\n   ~~~~\n\t# Fake\n\t[x](#fake)\n    # Fake\n    [x](#fake)\n# Real\n`[fake](#fake)` < 3 and \\[ordinary\\] plus [real](#real)\n";
    let (nodes, refs) = parsed(source);
    assert_eq!(nodes.len(), 3);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target(), &ReferenceTarget::Local("#real".into()));
    let span = nodes
        .iter()
        .find(|n| n.locator() == refs[0].from())
        .unwrap()
        .span();
    assert_eq!(&source[span.start()..span.end()], "[real](#real)");
}
#[test]
fn aggregate_locator_limit_duplicate_suffix_overflow_and_errors_do_not_disclose_inputs() {
    let many = format!("# {}\n", "a".repeat(80)).repeat(3100);
    assert!(many.len() < MAX_SOURCE_BYTES);
    assert!(matches!(
        scanner::extract(many.as_bytes()),
        Err(ExtractionError::Limit)
    ));
    let full = format!("# {}\n", "a".repeat(MAX_LOCATION_BYTES - 1));
    assert!(matches!(
        scanner::extract(format!("{full}{full}").as_bytes()),
        Err(ExtractionError::Limit)
    ));
    let private = "PRIVATE_SENTINEL";
    let error =
        ExtractionError::Inspection(crate::InspectionError::Io(std::io::Error::other(private)));
    assert!(!format!("{error:?} {error}").contains(private));
    assert_eq!(
        std::error::Error::source(&error)
            .unwrap()
            .source()
            .unwrap()
            .to_string(),
        private
    );
}

#[test]
fn complete_reference_links_and_neighboring_definitions_are_not_multiline_fragments() {
    let (_, refs) = parsed("# A\n[x][id]\n[y][id]\n[x](#a)\n[ordinary]\n[z](#a)\n[id]: #a\n");
    assert_eq!(refs.len(), 5);
}

#[test]
fn repair_definition_title_continuations_refuse_but_separated_quoted_links_survive() {
    for title in [
        r#""[not prose](#local)""#,
        "'[not prose](#local)'",
        "([not prose](#local))",
    ] {
        for indentation in ["", " ", "   ", "\t", "\t  ", " \t "] {
            let source = format!("# Local\n[key]: #local\n{indentation}{title}\n[key]\n");
            assert!(
                matches!(
                    scanner::extract(source.as_bytes()),
                    Err(ExtractionError::Unsupported(UnsupportedSyntax::Link))
                ),
                "adjacent reference title must refuse: {source:?}"
            );
            let separated = format!("# Local\n[key]: #local\n \t\n{title}\n[key]\n");
            let (_, refs) = parsed(&separated);
            assert_eq!(refs.len(), 3);
            assert_eq!(
                refs.iter()
                    .filter(|r| r.target() == &ReferenceTarget::Local("#local".into()))
                    .count(),
                2
            );
        }
    }
    let (_, refs) = parsed(
        "# Local\n[key]: #local\nOrdinary prose breaks title adjacency.\n\"[visible](#local)\"\n[key]\n",
    );
    assert_eq!(refs.len(), 3);
}

#[test]
fn repair_padded_setext_markers_refuse_while_prose_and_code_neighbors_survive() {
    for marker in ["=== ", "---\t", "= \t", "-- \t  "] {
        let source = format!("Hidden\n  {marker}\n[read](#hidden)\n");
        assert!(
            matches!(
                scanner::extract(source.as_bytes()),
                Err(ExtractionError::Unsupported(UnsupportedSyntax::Anchor))
            ),
            "padded Setext marker must refuse: {source:?}"
        );
    }
    let (_, refs) = parsed(
        "# Local\nEquality === and dashes --- are ordinary prose.\n\\=== \n```text\nHidden\n=== \n---\t\n```\n    === \n\t---\t\n[read](#local)\n",
    );
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target(), &ReferenceTarget::Local("#local".into()));
}

#[test]
fn repair_tab_list_markers_refuse_containers_but_preserve_flat_list_links() {
    for marker in ["-", "+", "*", "1.", "2)"] {
        for separator in ["\t", " \t"] {
            for body in ["# Hidden", "```text", "> Nested", "[key]: #local"] {
                let source = format!("{marker}{separator}{body}\n[read](#hidden)\n");
                assert!(
                    matches!(
                        scanner::extract(source.as_bytes()),
                        Err(ExtractionError::Unsupported(UnsupportedSyntax::Container))
                    ),
                    "container with tab separator must refuse: {source:?}"
                );
            }
            let source = format!("# Local\n{marker}{separator}Ordinary [read](#local) prose.\n");
            let (nodes, refs) = parsed(&source);
            assert_eq!(refs.len(), 1);
            assert_eq!(refs[0].target(), &ReferenceTarget::Local("#local".into()));
            let span = nodes
                .iter()
                .find(|n| n.locator() == refs[0].from())
                .unwrap()
                .span();
            assert_eq!(&source[span.start()..span.end()], "[read](#local)");
        }
    }
}

#[test]
fn repair_nonletter_email_autolinks_refuse_without_rejecting_literal_neighbors() {
    for address in [
        "123@example.invalid",
        "_reader@example.invalid",
        "+reader@example.invalid",
        "9.reader+tag@example.invalid",
    ] {
        for line in [
            format!("<{address}>"),
            format!("[text <{address}>](#local)"),
        ] {
            let source = format!("# Local\n{line}\n[read](#local)\n");
            assert!(
                matches!(
                    scanner::extract(source.as_bytes()),
                    Err(ExtractionError::Unsupported(UnsupportedSyntax::Html))
                ),
                "angle email must refuse: {source:?}"
            );
        }
        let source = format!(
            "# Local\n{address} and 2 < 3, x > y, <123>, \\<{address}> and `<{address}>`.\n[read](<#local>)\n"
        );
        let (_, refs) = parsed(&source);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target(), &ReferenceTarget::Local("#local".into()));
    }
}

#[test]
fn docs5_full_reference_uses_refuse_controls_anywhere_in_retained_markup() {
    for markup in [
        "[read][key\tname]",
        "[read\ttext][key name]",
        "[`read\ttext`][key name]",
        "[read][missing\tname]",
    ] {
        let source = format!("# Local\n{markup}\n[key name]: #local\n");
        assert!(
            matches!(
                scanner::extract(source.as_bytes()),
                Err(ExtractionError::Unsupported(UnsupportedSyntax::Link))
            ),
            "raw control-bearing full reference must refuse: {markup:?}"
        );
    }
    for control in ['\0', '\u{7f}', '\u{85}'] {
        let source = format!("# Local\n[read][key{control}name]\n[key name]: #local\n");
        assert!(matches!(
            scanner::extract(source.as_bytes()),
            Err(ExtractionError::InvalidMarkdown)
        ));
    }
}

#[test]
fn docs5_collapsed_reference_use_refuses_control_bearing_original_markup() {
    let source = "# Local\n[key\tname][]\n[key name]: #local\n";
    assert!(matches!(
        scanner::extract(source.as_bytes()),
        Err(ExtractionError::Unsupported(UnsupportedSyntax::Link))
    ));
}

#[test]
fn docs5_defined_shortcut_refuses_control_bearing_original_markup() {
    let source = "# Local\n[key\tname]\n[key name]: #local\n";
    assert!(matches!(
        scanner::extract(source.as_bytes()),
        Err(ExtractionError::Unsupported(UnsupportedSyntax::Link))
    ));
}

#[test]
fn docs5_tabbed_definition_matches_space_uses_without_changing_their_spelling() {
    let source = "# Local\n[KEY\tNAME]: #local\n[ação][key  name] [key name][] [KEY NAME]\n-\tFlat [read](#local) prose.\n`[key\tname]` and ordinary [not\tdefined], 2 < 3.\n";
    let (nodes, refs) = parsed(source);
    assert_eq!(refs.len(), 5);
    let definition = format!("definition@{}", source.find("[KEY\tNAME]:").unwrap());
    for markup in ["[ação][key  name]", "[key name][]", "[KEY NAME]"] {
        let locator = format!("link@{}", source.find(markup).unwrap());
        let reference = refs.iter().find(|r| r.from() == locator).unwrap();
        assert_eq!(reference.spelling(), markup);
        assert_eq!(
            reference.target(),
            &ReferenceTarget::Local(definition.clone())
        );
        let span = nodes
            .iter()
            .find(|n| n.locator() == locator)
            .unwrap()
            .span();
        assert_eq!(&source[span.start()..span.end()], markup);
    }
}
