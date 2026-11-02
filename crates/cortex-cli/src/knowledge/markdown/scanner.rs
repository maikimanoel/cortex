//! Finite native lexical profile. No renderer, filesystem or target discovery.
use super::{ExtractionError as E, UnsupportedSyntax as U};
use cortex_core::knowledge::*;
use std::collections::{BTreeMap, BTreeSet};

type Parsed = (Vec<KnowledgeNode>, Vec<ExtractedReference>);
struct Scan {
    nodes: Vec<KnowledgeNode>,
    references: Vec<ExtractedReference>,
    anchors: BTreeSet<String>,
    definitions: BTreeMap<String, String>,
    locator_bytes: usize,
}
impl Scan {
    fn node(&mut self, locator: &str, start: usize, end: usize) -> Result<(), E> {
        if self.nodes.len() == MAX_NODES || locator.len() > MAX_LOCATION_BYTES {
            return Err(E::Limit);
        }
        self.locator_bytes += locator.len();
        if self.locator_bytes > MAX_SOURCE_BYTES {
            return Err(E::Limit);
        }
        self.nodes
            .push(KnowledgeNode::new(locator, SourceSpan::new(start, end)?)?);
        Ok(())
    }
    fn reference(&mut self, from: &str, spelling: &str, target: ReferenceTarget) -> Result<(), E> {
        if self.references.len() == MAX_REFERENCES || spelling.len() > MAX_LOCATION_BYTES {
            return Err(E::Limit);
        }
        self.references
            .push(ExtractedReference::new(from, spelling, target)?);
        Ok(())
    }
    fn target(&self, destination: &str) -> ReferenceTarget {
        let Some(fragment) = destination.strip_prefix('#') else {
            return ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage);
        };
        if fragment.is_empty()
            || !fragment
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
        {
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment)
        } else if self.anchors.contains(destination) {
            ReferenceTarget::Local(destination.to_owned())
        } else {
            ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
        }
    }
}
fn escaped(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'\\' && bytes.get(i + 1).is_some_and(u8::is_ascii_punctuation)
}
fn decode(raw: &str) -> String {
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if escaped(b, i) {
            i += 1;
        }
        out.push(b[i]);
        i += 1;
    }
    // Removing ASCII backslashes before ASCII punctuation preserves UTF-8.
    String::from_utf8(out).expect("ASCII escape removal preserves UTF-8")
}
fn label(raw: &str) -> Result<String, E> {
    if raw.len() > MAX_LOCATION_BYTES {
        return Err(E::Limit);
    }
    if !raw.is_ascii() || raw.bytes().any(|b| matches!(b, b'`' | b'<' | b'>' | b'&')) {
        return Err(E::Unsupported(U::Label));
    }
    let decoded = decode(raw);
    let normalized = decoded
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(E::Unsupported(U::Label));
    }
    Ok(normalized)
}
fn code_end(b: &[u8], start: usize) -> Result<usize, E> {
    let count = b[start..].iter().take_while(|c| **c == b'`').count();
    let mut i = start + count;
    while i < b.len() {
        if b[i] == b'`' {
            let run = b[i..].iter().take_while(|c| **c == b'`').count();
            i += run;
            if run == count {
                return Ok(i);
            }
        } else {
            i += 1;
        }
    }
    Err(E::InvalidMarkdown)
}
fn html_start(b: &[u8], i: usize) -> bool {
    if b[i] != b'<' {
        return false;
    }
    if b.get(i + 1)
        .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, b'/' | b'!' | b'?'))
    {
        return true;
    }
    // Email autolinks may start with digits or punctuation. Refuse an angled
    // email candidate without interpreting or following it. Stop at another '<'
    // so repeated angle characters cannot cause repeated scans of the same tail.
    let mut at = false;
    for c in &b[i + 1..] {
        match c {
            b'>' => return at,
            b'<' => break,
            b'@' => at = true,
            c if c.is_ascii_whitespace() => break,
            _ => (),
        }
    }
    false
}
// The opening bracket is known. Nested brackets would need renderer precedence.
fn bracket_end(b: &[u8], start: usize) -> Result<usize, E> {
    let mut i = start + 1;
    while i < b.len() {
        if escaped(b, i) {
            i += 2;
            continue;
        }
        if html_start(b, i) {
            return Err(E::Unsupported(U::Html));
        }
        if b[i] == b'{' && matches!(b.get(i + 1), Some(b'#' | b':')) {
            return Err(E::Unsupported(U::Anchor));
        }
        match b[i] {
            b']' => return Ok(i),
            b'[' => return Err(E::Unsupported(U::Link)),
            b'`' => i = code_end(b, i)?,
            _ => i += 1,
        }
    }
    Err(E::InvalidMarkdown)
}
struct Destination<'a> {
    raw: &'a str,
    decoded: String,
    end: usize,
}
// For inline syntax, end is immediately after its final ')'. Definitions end at EOL.
fn destination(s: &str, start: usize, inline: bool) -> Result<Destination<'_>, E> {
    let b = s.as_bytes();
    let angle = b.get(start) == Some(&b'<');
    let begin = start + usize::from(angle);
    let mut i = begin;
    let mut depth = 0usize;
    while i < b.len() {
        if escaped(b, i) {
            i += 2;
            continue;
        }
        if angle && b[i] == b'>' {
            break;
        }
        if b[i].is_ascii_whitespace() {
            break;
        }
        if b[i] == b'<' || (!angle && b[i] == b'>') {
            return Err(E::Unsupported(U::Link));
        }
        if !angle {
            match b[i] {
                b'(' => {
                    depth += 1;
                    if depth > 32 {
                        return Err(E::Limit);
                    }
                }
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                _ => (),
            }
        }
        i += 1;
    }
    if depth != 0 {
        return Err(E::InvalidMarkdown);
    }
    let raw = &s[begin..i];
    if raw.len() > MAX_LOCATION_BYTES {
        return Err(E::Limit);
    }
    if angle {
        if b.get(i) != Some(&b'>') {
            return Err(E::InvalidMarkdown);
        }
        i += 1;
    }
    if inline {
        if b.get(i) != Some(&b')') {
            return Err(if i == b.len() {
                E::InvalidMarkdown
            } else {
                E::Unsupported(U::Link)
            });
        }
        i += 1;
    } else if !s[i..].trim_matches([' ', '\t']).is_empty() || raw.is_empty() {
        return Err(E::Unsupported(U::Link));
    }
    Ok(Destination {
        raw,
        decoded: decode(raw),
        end: i,
    })
}
fn heading(s: &str) -> Result<Option<String>, E> {
    let n = s.bytes().take_while(|b| *b == b'#').count();
    if n == 0
        || n > 6
        || s.as_bytes()
            .get(n)
            .is_some_and(|b| !b.is_ascii_whitespace())
    {
        return Ok(None);
    }
    let mut text = s[n..].trim_matches([' ', '\t']);
    let closing = text.bytes().rev().take_while(|b| *b == b'#').count();
    if closing > 0
        && (closing == text.len()
            || text.as_bytes()[text.len() - closing - 1].is_ascii_whitespace())
    {
        text = text[..text.len() - closing].trim_end_matches([' ', '\t']);
    }
    if !text.is_ascii() {
        return Err(E::Unsupported(U::Anchor));
    }
    let b = text.as_bytes();
    let mut anchor = String::from("#");
    let mut i = 0;
    while i < b.len() {
        if escaped(b, i) {
            i += 1;
        } else if matches!(
            b[i],
            b'*' | b'`' | b'[' | b']' | b'<' | b'>' | b'{' | b'}' | b'&' | b'\\'
        ) {
            return Err(E::Unsupported(U::Anchor));
        }
        let c = b[i];
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_') {
            anchor.push(c.to_ascii_lowercase() as char);
        } else if matches!(c, b' ' | b'\t') {
            anchor.push('-');
        }
        i += 1;
    }
    if anchor.len() == 1 {
        return Err(E::Unsupported(U::Anchor));
    }
    if anchor.len() > MAX_LOCATION_BYTES {
        return Err(E::Limit);
    }
    Ok(Some(anchor))
}
fn setext_marker(s: &str) -> bool {
    let marker = s.trim_end_matches([' ', '\t']);
    !marker.is_empty() && (marker.bytes().all(|b| b == b'=') || marker.bytes().all(|b| b == b'-'))
}
fn list_item(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    let marker_end = if matches!(b.first(), Some(b'-' | b'+' | b'*')) {
        1
    } else {
        let n = b.iter().take_while(|c| c.is_ascii_digit()).count();
        if n > 0 && matches!(b.get(n), Some(b'.' | b')')) {
            n + 1
        } else {
            return None;
        }
    };
    if !matches!(b.get(marker_end), Some(b' ' | b'\t')) {
        return None;
    }
    Some(s[marker_end + 1..].trim_start_matches([' ', '\t']))
}
fn container(s: &str) -> bool {
    if s.starts_with('>') {
        return true;
    }
    let Some(rest) = list_item(s) else {
        return false;
    };
    rest.starts_with(['#', '>', '~'])
        || rest.starts_with("```")
        || (rest.starts_with('[') && rest.contains("]:"))
}
fn inline(scan: &mut Scan, s: &str, offset: usize) -> Result<(), E> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if escaped(b, i) {
            i += 2;
            continue;
        }
        match b[i] {
            b'`' => {
                i = code_end(b, i)?;
                continue;
            }
            b'!' if b.get(i + 1) == Some(&b'[') => return Err(E::Unsupported(U::Link)),
            b'<' if html_start(b, i) => return Err(E::Unsupported(U::Html)),
            b'{' if matches!(b.get(i + 1), Some(b'#' | b':')) => {
                return Err(E::Unsupported(U::Anchor));
            }
            b'[' => {
                let close = bracket_end(b, i)?;
                let next = close + 1;
                let locator = format!("link@{}", offset + i);
                if b.get(next) == Some(&b'(') {
                    let d = destination(s, next + 1, true)?;
                    scan.node(&locator, offset + i, offset + d.end)?;
                    scan.reference(&locator, d.raw, scan.target(&d.decoded))?;
                    i = d.end;
                    continue;
                }
                let (end, key, explicit) = if b.get(next) == Some(&b'[') {
                    let end = bracket_end(b, next)?;
                    let raw = if end == next + 1 {
                        &s[i + 1..close]
                    } else {
                        &s[next + 1..end]
                    };
                    (end + 1, label(raw)?, true)
                } else {
                    // Undefined ordinary bracket text is not a shortcut link.
                    match label(&s[i + 1..close]) {
                        Ok(key) => (next, key, false),
                        Err(E::Limit) => return Err(E::Limit),
                        Err(_) => {
                            i = next;
                            continue;
                        }
                    }
                };
                let definition = scan.definitions.get(&key).cloned();
                if explicit || definition.is_some() {
                    let spelling = &s[i..end];
                    // Core retains the whole original use and forbids controls;
                    // refuse this unsupported spelling instead of rewriting it.
                    if spelling.chars().any(char::is_control) {
                        return Err(E::Unsupported(U::Link));
                    }
                    scan.node(&locator, offset + i, offset + end)?;
                    let target = definition.map_or(
                        ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
                        ReferenceTarget::Local,
                    );
                    scan.reference(&locator, spelling, target)?;
                }
                i = end;
                continue;
            }
            _ => (),
        }
        i += 1;
    }
    Ok(())
}

// Only a final bare bracket atom could continue onto a following line.
fn trailing_bracket(line: &str) -> Result<bool, E> {
    let b = line.as_bytes();
    let mut i = 0;
    let mut bare = false;
    while i < b.len() {
        if escaped(b, i) {
            bare = false;
            i += 2;
            continue;
        }
        match b[i] {
            b'`' => {
                bare = false;
                i = code_end(b, i)?;
            }
            b'[' => {
                let close = bracket_end(b, i)?;
                if b.get(close + 1) == Some(&b'(') {
                    i = destination(line, close + 2, true)?.end;
                    bare = false;
                } else if b.get(close + 1) == Some(&b'[') {
                    i = bracket_end(b, close + 1)? + 1;
                    bare = false;
                } else {
                    i = close + 1;
                    bare = true;
                }
            }
            b' ' | b'\t' => i += 1,
            _ => {
                bare = false;
                i += 1;
            }
        }
    }
    Ok(bare)
}
fn continues_bracket(line: &str) -> Result<bool, E> {
    if line.starts_with('(') {
        return Ok(true);
    }
    if line.starts_with('[') {
        let close = bracket_end(line.as_bytes(), 0)?;
        // A new definition or complete inline link is a separate neighboring atom.
        return Ok(!matches!(line.as_bytes().get(close + 1), Some(b':' | b'(')));
    }
    Ok(false)
}

pub(super) fn extract(bytes: &[u8]) -> Result<Parsed, E> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(E::Limit);
    }
    let source = std::str::from_utf8(bytes).map_err(|_| E::InvalidUtf8)?;
    if source.is_empty()
        || source
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(E::InvalidMarkdown);
    }
    if source.starts_with('\u{feff}') {
        return Err(E::Unsupported(U::Anchor));
    }
    let mut scan = Scan {
        nodes: Vec::new(),
        references: Vec::new(),
        anchors: BTreeSet::new(),
        definitions: BTreeMap::new(),
        locator_bytes: 0,
    };
    scan.node("", 0, bytes.len())?;
    let mut lines = Vec::new();
    let mut definitions = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut offset = 0;
    let mut list_context = false;
    let mut previous_bracket_line = false;
    let mut previous_definition_line = false;
    for line in source.split_inclusive('\n') {
        let raw = line.strip_suffix('\n').unwrap_or(line);
        let raw = if line.ends_with('\n') {
            raw.strip_suffix('\r').unwrap_or(raw)
        } else {
            raw
        };
        if raw.contains('\r') {
            return Err(E::InvalidMarkdown);
        }
        let trimmed = raw.trim_start_matches(' ');
        let indent = raw.len() - trimmed.len();
        let b = trimmed.as_bytes();
        if previous_definition_line
            && trimmed
                .trim_start_matches([' ', '\t'])
                .starts_with(['"', '\'', '('])
        {
            // A possible title on the next physical line may still belong to
            // an unsupported multiline definition; do not extract it as prose.
            return Err(E::Unsupported(U::Link));
        }
        previous_definition_line = false;
        if fence.is_none() && previous_bracket_line && continues_bracket(trimmed)? {
            return Err(E::Unsupported(U::Link));
        }
        previous_bracket_line = false;
        if let Some((ch, len)) = fence {
            let n = b.iter().take_while(|c| **c == ch).count();
            if indent <= 3 && n >= len && trimmed[n..].trim_matches([' ', '\t']).is_empty() {
                fence = None;
            }
        } else if indent >= 4 || raw.starts_with('\t') {
            if list_context && !trimmed.is_empty() {
                return Err(E::Unsupported(U::Container));
            }
            // Explicit lexical top-level indented-code rule; no container renderer.
        } else {
            let ch = b.first().copied().unwrap_or(0);
            let run = b.iter().take_while(|c| **c == ch).count();
            let item = list_item(trimmed).is_some();
            if list_context
                && indent > 0
                && ((matches!(ch, b'`' | b'~') && run >= 3) || heading(trimmed)?.is_some())
            {
                return Err(E::Unsupported(U::Container));
            }
            if !trimmed.is_empty() && (item || indent == 0) {
                list_context = item;
            }
            if matches!(ch, b'`' | b'~') && run >= 3 {
                if ch == b'`' && trimmed[run..].contains('`') {
                    return Err(E::Unsupported(U::Link));
                }
                fence = Some((ch, run));
            } else if container(trimmed) {
                return Err(E::Unsupported(U::Container));
            } else if setext_marker(trimmed) {
                return Err(E::Unsupported(U::Anchor));
            } else if let Some(base) = heading(trimmed)? {
                let mut anchor = base.clone();
                let mut suffix = 1usize;
                while scan.anchors.contains(&anchor) {
                    anchor = format!("{base}-{suffix}");
                    suffix += 1;
                }
                scan.node(&anchor, offset, offset + raw.len())?;
                scan.anchors.insert(anchor);
            } else if trimmed.starts_with('[') {
                let close = bracket_end(b, 0)?;
                if b.get(close + 1) == Some(&b':') {
                    let key = label(&trimmed[1..close])?;
                    let rest = &trimmed[close + 2..];
                    let skip = rest.len() - rest.trim_start_matches([' ', '\t']).len();
                    let d = destination(trimmed, close + 2 + skip, false)?;
                    let locator = format!("definition@{}", offset + indent);
                    scan.node(&locator, offset + indent, offset + raw.len())?;
                    if scan.definitions.insert(key, locator.clone()).is_some() {
                        return Err(E::DuplicateDefinition);
                    }
                    definitions.push((locator, d.raw, d.decoded));
                    previous_definition_line = true;
                } else {
                    previous_bracket_line = trailing_bracket(trimmed)?;
                    lines.push((trimmed, offset + indent));
                }
            } else {
                previous_bracket_line = trailing_bracket(trimmed)?;
                lines.push((trimmed, offset + indent));
            }
        }
        offset += line.len();
    }
    if fence.is_some() {
        return Err(E::InvalidMarkdown);
    }
    for (locator, raw, decoded) in definitions {
        scan.reference(&locator, raw, scan.target(&decoded))?;
    }
    for (line, offset) in lines {
        inline(&mut scan, line, offset)?;
    }
    Ok((scan.nodes, scan.references))
}
