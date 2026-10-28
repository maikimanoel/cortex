//! Bounded syntax extraction using the existing JSON parser's borrowed RawValue.
use super::ExtractionError;
use cortex_core::knowledge::*;
use serde::{
    Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{cell::Cell, collections::BTreeSet, fmt};

fn members(source: &str) -> Result<Vec<(String, &RawValue)>, ExtractionError> {
    struct Object<'a>(&'a Cell<bool>);
    impl<'de> Visitor<'de> for Object<'_> {
        type Value = Vec<(String, &'de RawValue)>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bounded JSON object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut keys = BTreeSet::new();
            let mut values = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                if keys.len() == MAX_NODES {
                    self.0.set(true);
                    return Err(de::Error::custom("member limit"));
                }
                if !keys.insert(key.clone()) {
                    return Err(de::Error::custom("duplicate key"));
                }
                values.push((key, map.next_value()?));
            }
            Ok(values)
        }
    }
    let limit = Cell::new(false);
    serde_json::Deserializer::from_str(source)
        .deserialize_map(Object(&limit))
        .map_err(|_| {
            if limit.get() {
                ExtractionError::Limit
            } else {
                ExtractionError::InvalidJson
            }
        })
}
fn elements(source: &str) -> Result<Vec<&RawValue>, ExtractionError> {
    struct Array<'a>(&'a Cell<bool>);
    impl<'de> Visitor<'de> for Array<'_> {
        type Value = Vec<&'de RawValue>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bounded JSON array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == MAX_NODES {
                    self.0.set(true);
                    return Err(de::Error::custom("element limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    let limit = Cell::new(false);
    serde_json::Deserializer::from_str(source)
        .deserialize_seq(Array(&limit))
        .map_err(|_| {
            if limit.get() {
                ExtractionError::Limit
            } else {
                ExtractionError::InvalidJson
            }
        })
}
struct Walk<'a> {
    bytes: &'a [u8],
    nodes: Vec<KnowledgeNode>,
    refs: Vec<(String, String)>,
    key_bytes: usize,
}
impl Walk<'_> {
    fn value(&mut self, raw: &RawValue, key: String, depth: usize) -> Result<(), ExtractionError> {
        if depth > 32 || self.nodes.len() == MAX_NODES || key.len() > MAX_LOCATION_BYTES {
            return Err(ExtractionError::Limit);
        }
        self.key_bytes = self
            .key_bytes
            .checked_add(key.len())
            .ok_or(ExtractionError::Limit)?;
        if self.key_bytes > MAX_SOURCE_BYTES {
            return Err(ExtractionError::Limit);
        }
        // RawValue borrows this exact input. Address arithmetic only locates a
        // slice; no pointer is dereferenced or forged, and contents/extent are checked.
        let start = raw
            .get()
            .as_ptr()
            .addr()
            .checked_sub(self.bytes.as_ptr().addr())
            .ok_or(ExtractionError::InvalidJson)?;
        let end = start
            .checked_add(raw.get().len())
            .ok_or(ExtractionError::Limit)?;
        if self.bytes.get(start..end) != Some(raw.get().as_bytes()) {
            return Err(ExtractionError::InvalidJson);
        }
        self.nodes
            .push(KnowledgeNode::new(&key, SourceSpan::new(start, end)?)?);
        match raw.get().as_bytes().first() {
            Some(b'{') => {
                for (name, value) in members(raw.get())? {
                    let next = format!("{}/{}", key, name.replace('~', "~0").replace('/', "~1"));
                    if name == "$ref" {
                        if self.refs.len() == MAX_REFERENCES {
                            return Err(ExtractionError::Limit);
                        }
                        let spelling: String = serde_json::from_str(value.get())
                            .map_err(|_| ExtractionError::Unsupported)?;
                        if spelling.len() > MAX_LOCATION_BYTES {
                            return Err(ExtractionError::Limit);
                        }
                        self.refs.push((next.clone(), spelling));
                    }
                    self.value(value, next, depth + 1)?;
                }
            }
            Some(b'[') => {
                for (n, value) in elements(raw.get())?.into_iter().enumerate() {
                    self.value(value, format!("{key}/{n}"), depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
fn local_pointer(s: &str) -> bool {
    if !s.is_empty() && !s.starts_with('/') {
        return false;
    }
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return false;
        }
    }
    true
}
pub(super) fn extract(
    bytes: &[u8],
) -> Result<(Vec<KnowledgeNode>, Vec<ExtractedReference>), ExtractionError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(ExtractionError::Limit);
    }
    std::str::from_utf8(bytes).map_err(|_| ExtractionError::InvalidJson)?;
    let raw: &RawValue = serde_json::from_slice(bytes).map_err(|_| ExtractionError::InvalidJson)?;
    let mut walk = Walk {
        bytes,
        nodes: Vec::new(),
        refs: Vec::new(),
        key_bytes: 0,
    };
    walk.value(raw, String::new(), 0)?;
    let keys: BTreeSet<_> = walk.nodes.iter().map(|n| n.locator()).collect();
    let mut refs = Vec::new();
    for (from, spelling) in walk.refs {
        let target = match spelling.strip_prefix('#') {
            None => ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
            Some(pointer) if pointer.contains('%') || !local_pointer(pointer) => {
                ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment)
            }
            Some(pointer) if !keys.contains(pointer) => {
                ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
            }
            Some(pointer) => ReferenceTarget::Local(pointer.to_owned()),
        };
        refs.push(ExtractedReference::new(&from, &spelling, target)?);
    }
    Ok((walk.nodes, refs))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_spans_escaped_names_unicode_arrays_and_unresolved_targets() {
        let bytes=br##" {"a/b":{"~":"value"},"items":[{"$ref":"#/a~1b/~0"}],"other":{"$ref":"remote.json#/x"}} "##;
        let (nodes, refs) = extract(bytes).unwrap();
        let n = nodes
            .iter()
            .find(|n| n.locator() == "/items/0/$ref")
            .unwrap();
        assert_eq!(
            &bytes[n.span().start()..n.span().end()],
            br##""#/a~1b/~0""##
        );
        assert!(matches!(&refs[0].target(),ReferenceTarget::Local(t) if t=="/a~1b/~0"));
        assert_eq!(
            refs[1].target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
        );
        let (n, r) = extract("{\"ação\":1,\"$ref\":\"#/ação\"}".as_bytes()).unwrap();
        assert_eq!(n.len(), 3);
        assert_eq!(r[0].target(), &ReferenceTarget::Local("/ação".into()));
    }
    #[test]
    fn invalid_ambiguous_and_bounded_inputs_never_produce_a_partial_index() {
        for source in [
            b"{\"x\":1,\"x\":2}".as_slice(),
            br#"{"x":1,"\u0078":2}"#,
            br#"{"$ref":42}"#,
            b"{} trailing",
            b"\xff",
        ] {
            assert!(extract(source).is_err());
        }
        assert!(matches!(
            extract(&vec![b' '; MAX_SOURCE_BYTES + 1]),
            Err(ExtractionError::Limit)
        ));
        let deep = format!("{}0{}", "[".repeat(34), "]".repeat(34));
        assert!(matches!(
            extract(deep.as_bytes()),
            Err(ExtractionError::Limit)
        ));
        let many = format!("[{}]", vec!["0"; MAX_NODES].join(","));
        assert!(matches!(
            extract(many.as_bytes()),
            Err(ExtractionError::Limit)
        ));
        let many_refs = format!(
            "[{}]",
            vec![r##"{"$ref":"#"}"##; MAX_REFERENCES + 1].join(",")
        );
        assert!(matches!(
            extract(many_refs.as_bytes()),
            Err(ExtractionError::Limit)
        ));
        assert!(matches!(
            extract(b"{\"x\":\"\xff\"}"),
            Err(ExtractionError::InvalidJson)
        ));
    }
    #[test]
    fn repeated_identical_tokens_have_distinct_actual_source_ranges() {
        let source = br##"{"a":{"$ref":"#"},"b":{"$ref":"#"}}"##;
        let (nodes, refs) = extract(source).unwrap();
        assert_eq!(refs.len(), 2);
        let a = nodes
            .iter()
            .find(|n| n.locator() == "/a/$ref")
            .unwrap()
            .span();
        let b = nodes
            .iter()
            .find(|n| n.locator() == "/b/$ref")
            .unwrap()
            .span();
        assert!(a.end() < b.start());
        assert_eq!(&source[a.start()..a.end()], br##""#""##);
        assert_eq!(&source[b.start()..b.end()], br##""#""##);
    }
}
