//! Whole-input bounded TOML subset parser. Transient syntax state only; Core owns facts.
use super::ExtractionError as E;
use cortex_core::knowledge::*;
use std::collections::BTreeMap;

const DEPTH: usize = 32;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Root,
    Implicit,
    Header,
    Dotted,
    Inline,
}
enum Kind {
    Table(Table, BTreeMap<String, usize>),
    Array(Vec<usize>),
    TableArray(Vec<usize>),
    String(String),
    Bool(bool),
    Integer,
}
struct Syntax {
    span: (usize, usize),
    key: Option<(usize, usize)>,
    kind: Kind,
}
struct Key {
    text: String,
    span: (usize, usize),
}
struct Parser<'a> {
    s: &'a str,
    pos: usize,
    nodes: Vec<Syntax>,
}
impl<'a> Parser<'a> {
    fn byte(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }
    fn alloc(&mut self, kind: Kind, span: (usize, usize)) -> Result<usize, E> {
        if self.nodes.len() == MAX_NODES {
            return Err(E::Limit);
        }
        let id = self.nodes.len();
        self.nodes.push(Syntax {
            span,
            key: None,
            kind,
        });
        Ok(id)
    }
    fn spaces(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }
    fn newline(&mut self) -> Result<bool, E> {
        if self.byte() == Some(b'\r') {
            self.pos += 1;
            if self.byte() != Some(b'\n') {
                return Err(E::InvalidToml);
            }
        }
        if self.byte() == Some(b'\n') {
            self.pos += 1;
            return Ok(true);
        }
        Ok(false)
    }
    fn comment(&mut self) -> Result<(), E> {
        if self.byte() == Some(b'#') {
            while let Some(b) = self.byte() {
                if matches!(b, b'\r' | b'\n') {
                    break;
                }
                if (b < 0x20 && b != b'\t') || b == 0x7f {
                    return Err(E::InvalidToml);
                }
                self.pos += 1;
            }
        }
        Ok(())
    }
    fn trivia(&mut self) -> Result<(), E> {
        loop {
            self.spaces();
            self.comment()?;
            if !self.newline()? {
                return Ok(());
            }
        }
    }
    fn end_line(&mut self) -> Result<(), E> {
        self.spaces();
        self.comment()?;
        if self.newline()? || self.byte().is_none() {
            Ok(())
        } else {
            Err(E::InvalidToml)
        }
    }
    fn take(&mut self, byte: u8) -> Result<(), E> {
        if self.byte() != Some(byte) {
            return Err(E::InvalidToml);
        }
        self.pos += 1;
        Ok(())
    }
    fn string(&mut self) -> Result<String, E> {
        let quote = self.byte().ok_or(E::InvalidToml)?;
        if self.s.as_bytes().get(self.pos..self.pos + 3) == Some(&[quote, quote, quote]) {
            return Err(E::Unsupported);
        }
        self.pos += 1;
        let mut value = String::new();
        loop {
            let b = self.byte().ok_or(E::InvalidToml)?;
            if b == quote {
                self.pos += 1;
                return Ok(value);
            }
            if (b < 0x20 && b != b'\t') || b == 0x7f {
                return Err(E::InvalidToml);
            }
            if quote == b'"' && b == b'\\' {
                self.pos += 1;
                let escape = self.byte().ok_or(E::InvalidToml)?;
                self.pos += 1;
                match escape {
                    b'b' => value.push('\u{8}'),
                    b't' => value.push('\t'),
                    b'n' => value.push('\n'),
                    b'f' => value.push('\u{c}'),
                    b'r' => value.push('\r'),
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'u' | b'U' => {
                        let digits = if escape == b'u' { 4 } else { 8 };
                        let end = self.pos + digits;
                        let hex = self.s.get(self.pos..end).ok_or(E::InvalidToml)?;
                        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                            return Err(E::InvalidToml);
                        }
                        let code = u32::from_str_radix(hex, 16).map_err(|_| E::InvalidToml)?;
                        value.push(char::from_u32(code).ok_or(E::InvalidToml)?);
                        self.pos = end;
                    }
                    _ => return Err(E::InvalidToml),
                }
            } else {
                let c = self.s[self.pos..].chars().next().ok_or(E::InvalidToml)?;
                value.push(c);
                self.pos += c.len_utf8();
            }
        }
    }
    fn keys(&mut self) -> Result<Vec<Key>, E> {
        let mut keys = Vec::new();
        loop {
            self.spaces();
            let start = self.pos;
            let text = if matches!(self.byte(), Some(b'"' | b'\'')) {
                self.string()?
            } else {
                while self
                    .byte()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    self.pos += 1;
                }
                if self.pos == start {
                    return Err(E::InvalidToml);
                }
                self.s[start..self.pos].to_owned()
            };
            if text.len() > MAX_LOCATION_BYTES {
                return Err(E::Limit);
            }
            if text.chars().any(char::is_control) {
                return Err(E::Unsupported);
            }
            keys.push(Key {
                text,
                span: (start, self.pos),
            });
            if keys.len() > DEPTH {
                return Err(E::Limit);
            }
            self.spaces();
            if self.byte() != Some(b'.') {
                return Ok(keys);
            }
            self.pos += 1;
        }
    }
    fn child(&self, table: usize, key: &str) -> Result<Option<usize>, E> {
        match &self.nodes[table].kind {
            Kind::Table(_, fields) => Ok(fields.get(key).copied()),
            _ => Err(E::DuplicateDeclaration),
        }
    }
    fn insert(&mut self, table: usize, key: &Key, id: usize) -> Result<(), E> {
        let Kind::Table(_, fields) = &mut self.nodes[table].kind else {
            return Err(E::DuplicateDeclaration);
        };
        if fields.insert(key.text.clone(), id).is_some() {
            return Err(E::DuplicateDeclaration);
        }
        self.nodes[id].key = Some(key.span);
        Ok(())
    }
    fn table(&mut self, kind: Table, key: &Key) -> Result<usize, E> {
        self.alloc(Kind::Table(kind, BTreeMap::new()), key.span)
    }
    fn descend(&mut self, parent: usize, key: &Key, header: bool) -> Result<usize, E> {
        let id = if let Some(id) = self.child(parent, &key.text)? {
            id
        } else {
            let id = self.table(
                if header {
                    Table::Implicit
                } else {
                    Table::Dotted
                },
                key,
            )?;
            self.insert(parent, key, id)?;
            id
        };
        match &self.nodes[id].kind {
            Kind::Table(Table::Inline, _) => Err(E::DuplicateDeclaration),
            Kind::Table(Table::Header | Table::Implicit, _) if !header => {
                Err(E::DuplicateDeclaration)
            }
            Kind::Table(_, _) => Ok(id),
            Kind::TableArray(items) if header => items.last().copied().ok_or(E::InvalidToml),
            _ => Err(E::DuplicateDeclaration),
        }
    }
    fn assignment(&mut self, mut table: usize, depth: usize) -> Result<(), E> {
        let keys = self.keys()?;
        if depth + keys.len() > DEPTH {
            return Err(E::Limit);
        }
        self.take(b'=')?;
        self.spaces();
        for key in &keys[..keys.len() - 1] {
            table = self.descend(table, key, false)?;
        }
        let key = keys.last().ok_or(E::InvalidToml)?;
        if self.child(table, &key.text)?.is_some() {
            return Err(E::DuplicateDeclaration);
        }
        let id = self.value(depth + keys.len())?;
        self.insert(table, key, id)
    }
    fn header(&mut self) -> Result<usize, E> {
        let start = self.pos;
        self.take(b'[')?;
        let repeated = self.byte() == Some(b'[');
        if repeated {
            self.pos += 1;
        }
        let keys = self.keys()?;
        self.take(b']')?;
        if repeated {
            self.take(b']')?;
        }
        let span = (start, self.pos);
        self.end_line()?;
        let mut parent = 0;
        for key in &keys[..keys.len() - 1] {
            parent = self.descend(parent, key, true)?;
        }
        let key = keys.last().ok_or(E::InvalidToml)?;
        if repeated {
            let array = if let Some(id) = self.child(parent, &key.text)? {
                if !matches!(self.nodes[id].kind, Kind::TableArray(_)) {
                    return Err(E::DuplicateDeclaration);
                }
                id
            } else {
                let id = self.alloc(Kind::TableArray(Vec::new()), span)?;
                self.insert(parent, key, id)?;
                id
            };
            let id = self.alloc(Kind::Table(Table::Header, BTreeMap::new()), span)?;
            let Kind::TableArray(items) = &mut self.nodes[array].kind else {
                return Err(E::InvalidToml);
            };
            items.push(id);
            Ok(id)
        } else if let Some(id) = self.child(parent, &key.text)? {
            let Kind::Table(mode, _) = &mut self.nodes[id].kind else {
                return Err(E::DuplicateDeclaration);
            };
            if *mode != Table::Implicit {
                return Err(E::DuplicateDeclaration);
            }
            *mode = Table::Header;
            self.nodes[id].span = span;
            self.nodes[id].key = Some(key.span);
            Ok(id)
        } else {
            let id = self.alloc(Kind::Table(Table::Header, BTreeMap::new()), span)?;
            self.insert(parent, key, id)?;
            Ok(id)
        }
    }
    fn value(&mut self, depth: usize) -> Result<usize, E> {
        if depth > DEPTH {
            return Err(E::Limit);
        }
        let start = self.pos;
        let kind = match self.byte().ok_or(E::InvalidToml)? {
            b'"' | b'\'' => Kind::String(self.string()?),
            b'[' => {
                self.pos += 1;
                self.trivia()?;
                let mut items = Vec::new();
                while self.byte() != Some(b']') {
                    items.push(self.value(depth + 1)?);
                    self.trivia()?;
                    if self.byte() != Some(b',') {
                        break;
                    }
                    self.pos += 1;
                    self.trivia()?;
                }
                self.take(b']')?;
                Kind::Array(items)
            }
            b'{' => {
                self.pos += 1;
                let id = self.alloc(
                    Kind::Table(Table::Inline, BTreeMap::new()),
                    (start, start + 1),
                )?;
                self.spaces();
                if matches!(self.byte(), Some(b'\r' | b'\n')) {
                    return Err(E::Unsupported);
                }
                if self.byte() != Some(b'}') {
                    loop {
                        self.assignment(id, depth)?;
                        self.spaces();
                        if self.byte() != Some(b',') {
                            break;
                        }
                        self.pos += 1;
                        self.spaces();
                        if matches!(self.byte(), Some(b'\r' | b'\n')) {
                            return Err(E::Unsupported);
                        }
                        if self.byte() == Some(b'}') {
                            return Err(E::InvalidToml);
                        }
                    }
                }
                self.take(b'}')?;
                // Arrays in inline tables must also be single-line in this profile.
                if self.s[start..self.pos].contains(['\r', '\n']) {
                    return Err(E::Unsupported);
                }
                self.nodes[id].span = (start, self.pos);
                return Ok(id);
            }
            _ => {
                while self.byte().is_some_and(|b| {
                    !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}' | b'#')
                }) {
                    self.pos += 1;
                }
                let token = &self.s[start..self.pos];
                match token {
                    "true" => Kind::Bool(true),
                    "false" => Kind::Bool(false),
                    _ => {
                        if token.contains(['.', ':', 'e', 'E'])
                            || ["0x", "0o", "0b"]
                                .iter()
                                .any(|prefix| token.starts_with(prefix))
                            || matches!(token, "inf" | "+inf" | "-inf" | "nan" | "+nan" | "-nan")
                        {
                            return Err(E::Unsupported);
                        }
                        let digits = token.strip_prefix(['+', '-']).unwrap_or(token);
                        if digits.is_empty()
                            || !digits.bytes().all(|b| b.is_ascii_digit() || b == b'_')
                            || digits.starts_with('_')
                            || digits.ends_with('_')
                            || digits.contains("__")
                        {
                            return Err(E::InvalidToml);
                        }
                        let clean = token.replace('_', "");
                        let unsigned = digits.replace('_', "");
                        if unsigned.len() > 1 && unsigned.starts_with('0') {
                            return Err(E::InvalidToml);
                        }
                        clean.parse::<i64>().map_err(|_| E::InvalidToml)?;
                        Kind::Integer
                    }
                }
            }
        };
        self.alloc(kind, (start, self.pos))
    }
    fn parse(mut self) -> Result<Vec<Syntax>, E> {
        self.alloc(Kind::Table(Table::Root, BTreeMap::new()), (0, self.s.len()))?;
        let mut current = 0;
        loop {
            self.trivia()?;
            if self.byte().is_none() {
                return Ok(self.nodes);
            }
            if self.byte() == Some(b'[') {
                current = self.header()?;
            } else {
                self.assignment(current, 0)?;
                self.end_line()?;
            }
        }
    }
}

fn component(key: &str) -> String {
    format!("k:{}", key.replace('~', "~0").replace('/', "~1"))
}
fn location(parent: &str, key: &str) -> String {
    format!("{parent}/{}", component(key))
}

struct Project<'a> {
    syntax: &'a [Syntax],
    nodes: Vec<KnowledgeNode>,
    refs: Vec<ExtractedReference>,
    locations: Vec<String>,
    locator_bytes: usize,
}
impl Project<'_> {
    fn node(&mut self, locator: &str, span: (usize, usize)) -> Result<(), E> {
        if self.nodes.len() == MAX_NODES || locator.len() > MAX_LOCATION_BYTES {
            return Err(E::Limit);
        }
        self.locator_bytes = self
            .locator_bytes
            .checked_add(locator.len())
            .ok_or(E::Limit)?;
        if self.locator_bytes > MAX_SOURCE_BYTES {
            return Err(E::Limit);
        }
        self.nodes.push(KnowledgeNode::new(
            locator,
            SourceSpan::new(span.0, span.1)?,
        )?);
        Ok(())
    }
    fn walk(&mut self, id: usize, locator: String, depth: usize) -> Result<(), E> {
        if depth > DEPTH {
            return Err(E::Limit);
        }
        self.node(&locator, self.syntax[id].span)?;
        if let Some(span) = self.syntax[id].key {
            self.node(&format!("{locator}/@key"), span)?;
        }
        self.locations[id] = locator.clone();
        match &self.syntax[id].kind {
            Kind::Table(_, fields) => {
                for (key, child) in fields {
                    self.walk(*child, location(&locator, key), depth + 1)?;
                }
            }
            Kind::Array(items) | Kind::TableArray(items) => {
                for (n, child) in items.iter().enumerate() {
                    self.walk(*child, format!("{locator}/i:{n}"), depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn fields(&self, id: usize) -> Result<&BTreeMap<String, usize>, E> {
        match &self.syntax[id].kind {
            Kind::Table(_, fields) => Ok(fields),
            _ => Err(E::Unsupported),
        }
    }
    fn field(&self, id: usize, key: &str) -> Option<usize> {
        if let Kind::Table(_, fields) = &self.syntax[id].kind {
            fields.get(key).copied()
        } else {
            None
        }
    }
    fn text(&self, id: usize) -> Result<&str, E> {
        match &self.syntax[id].kind {
            Kind::String(s) => Ok(s),
            _ => Err(E::Unsupported),
        }
    }
    fn reference(
        &mut self,
        from: String,
        spelling: &str,
        target: ReferenceTarget,
    ) -> Result<(), E> {
        if self.refs.len() == MAX_REFERENCES || spelling.len() > MAX_LOCATION_BYTES {
            return Err(E::Limit);
        }
        if spelling.chars().any(char::is_control) {
            return Err(E::Unsupported);
        }
        self.refs
            .push(ExtractedReference::new(&from, spelling, target)?);
        Ok(())
    }
    fn external(&mut self, id: usize) -> Result<(), E> {
        let spelling = self.text(id)?.to_owned();
        self.reference(
            self.locations[id].clone(),
            &spelling,
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        )
    }
    fn string_array(&mut self, id: usize) -> Result<(), E> {
        let Kind::Array(items) = &self.syntax[id].kind else {
            return Err(E::Unsupported);
        };
        for child in items {
            self.external(*child)?;
        }
        Ok(())
    }
    fn dependencies(
        &mut self,
        id: usize,
        aliases: &mut BTreeMap<String, Vec<usize>>,
    ) -> Result<(), E> {
        for (alias, declaration) in self.fields(id)?.clone() {
            aliases.entry(alias.clone()).or_default().push(declaration);
            // The alias is source syntax, not an effective package identity.
            self.reference(
                format!("{}/@key", self.locations[declaration]),
                &alias,
                ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
            )?;
            match &self.syntax[declaration].kind {
                Kind::String(_) => self.external(declaration)?,
                Kind::Table(_, _) => {
                    for (key, value) in self.fields(declaration)?.clone() {
                        match key.as_str() {
                            "features" => self.string_array(value)?,
                            "version" | "path" | "package" | "git" | "branch" | "tag" | "rev"
                            | "registry" | "registry-index" => self.external(value)?,
                            "workspace" => {
                                if !matches!(self.syntax[value].kind, Kind::Bool(true)) {
                                    return Err(E::Unsupported);
                                }
                                self.reference(
                                    self.locations[value].clone(),
                                    "true",
                                    ReferenceTarget::Unresolved(
                                        UnresolvedReference::OutsideCoverage,
                                    ),
                                )?;
                            }
                            "optional" | "default-features" => {
                                if !matches!(self.syntax[value].kind, Kind::Bool(_)) {
                                    return Err(E::Unsupported);
                                }
                            }
                            _ => return Err(E::Unsupported),
                        }
                    }
                }
                _ => return Err(E::Unsupported),
            }
        }
        Ok(())
    }
    fn relations(&mut self) -> Result<(), E> {
        let mut aliases = BTreeMap::<String, Vec<usize>>::new();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(id) = self.field(0, section) {
                self.dependencies(id, &mut aliases)?;
            }
        }
        if let Some(workspace) = self.field(0, "workspace") {
            self.fields(workspace)?;
            if let Some(id) = self.field(workspace, "dependencies") {
                self.dependencies(id, &mut aliases)?;
            }
            for section in ["members", "default-members", "exclude"] {
                if let Some(id) = self.field(workspace, section) {
                    self.string_array(id)?;
                }
            }
        }
        if let Some(targets) = self.field(0, "target") {
            for (cfg, target) in self.fields(targets)?.clone() {
                self.reference(
                    format!("{}/@key", self.locations[target]),
                    &cfg,
                    ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
                )?;
                self.fields(target)?;
                for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(id) = self.field(target, section) {
                        self.dependencies(id, &mut aliases)?;
                    }
                }
            }
        }
        let package = self.field(0, "package");
        if let Some(id) = package {
            for (field, value) in self.fields(id)?.clone() {
                // metadata and arbitrary package subtables are not inheritance declarations.
                if !matches!(
                    field.as_str(),
                    "version"
                        | "authors"
                        | "categories"
                        | "description"
                        | "documentation"
                        | "edition"
                        | "exclude"
                        | "homepage"
                        | "include"
                        | "keywords"
                        | "license"
                        | "license-file"
                        | "readme"
                        | "repository"
                        | "rust-version"
                        | "publish"
                ) {
                    continue;
                }
                if let Some(inherited) = self.field(value, "workspace") {
                    if !matches!(self.syntax[inherited].kind, Kind::Bool(true)) {
                        return Err(E::Unsupported);
                    }
                    self.reference(
                        self.locations[inherited].clone(),
                        "true",
                        ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
                    )?;
                }
            }
        }
        let Some(features) = self.field(0, "features") else {
            return Ok(());
        };
        let declarations = self.fields(features)?.clone();
        let package_scope = package
            .and_then(|p| self.field(p, "name"))
            .is_some_and(|id| matches!(self.syntax[id].kind, Kind::String(_)));
        for array in declarations.values() {
            let Kind::Array(items) = &self.syntax[*array].kind else {
                return Err(E::Unsupported);
            };
            for item in items {
                let spelling = self.text(*item)?.to_owned();
                let target = if !package_scope
                    || spelling
                        .strip_prefix("dep:")
                        .is_some_and(|alias| alias.is_empty() || alias.contains(['/', ':', '?']))
                {
                    ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
                } else if let Some(alias) = spelling.strip_prefix("dep:") {
                    match aliases.get(alias) {
                        Some(ids)
                            if ids.len() == 1
                                && self
                                    .field(0, "dependencies")
                                    .and_then(|d| self.field(d, alias))
                                    == Some(ids[0]) =>
                        {
                            let dep = ids[0];
                            let optional = self
                                .field(dep, "optional")
                                .is_some_and(|id| matches!(self.syntax[id].kind, Kind::Bool(true)));
                            if optional && self.field(dep, "workspace").is_none() {
                                ReferenceTarget::Local(self.locations[dep].clone())
                            } else {
                                ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
                            }
                        }
                        None => ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget),
                        _ => ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
                    }
                } else if spelling.contains(['/', ':', '?']) {
                    ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
                } else if let Some(id) = declarations.get(&spelling) {
                    ReferenceTarget::Local(self.locations[*id].clone())
                } else if aliases.contains_key(&spelling) {
                    ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
                } else {
                    ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
                };
                self.reference(self.locations[*item].clone(), &spelling, target)?;
            }
        }
        Ok(())
    }
}

pub(super) fn extract(bytes: &[u8]) -> Result<(Vec<KnowledgeNode>, Vec<ExtractedReference>), E> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(E::Limit);
    }
    if bytes.is_empty() {
        return Err(E::Unsupported);
    }
    let s = std::str::from_utf8(bytes).map_err(|_| E::InvalidToml)?;
    let syntax = Parser {
        s,
        pos: 0,
        nodes: Vec::new(),
    }
    .parse()?;
    let mut project = Project {
        locations: vec![String::new(); syntax.len()],
        syntax: &syntax,
        nodes: Vec::new(),
        refs: Vec::new(),
        locator_bytes: 0,
    };
    project.walk(0, String::new(), 0)?;
    project.relations()?;
    Ok((project.nodes, project.refs))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_duplicate_and_unsupported_inputs_publish_no_syntax_facts() {
        for source in [
            "x=1\nx=2",
            "'x'=1\n\"\\u0078\"=2",
            "x={a=1,a=2}",
            "[a]\n[a]",
            "a=1\n[a]",
            "[a]\na=1\n[a.a]",
            "a.b=1\n[a]",
            "a={b=1}\n[a.c]",
            "a=[1]\n[[a]]",
            "[[a]]\n[a]",
            "a.b=1\na.b.c=2",
            "[a.b]\nx=1\n[a]\nb.c=2",
            "x='unterminated",
            "x=[1,",
            "x={a=",
            "x=true trailing",
            "[a",
            "x=01",
            "x=1__2",
            "x=_2",
            "x=2_",
            "x=9223372036854775808",
            "x=+",
            "x=",
            "x=\"\\q\"",
            "x=\"\\uD800\"",
            "x=\"\\U00110000\"",
            "x='a\nb'",
            "x=1\ry=2",
            "x={a=1,}",
            "x=[1 2]",
            "x=\"a\u{7f}b\"",
            "# bad\u{1}\nx=1",
            "x=TRUE",
            "[features]\na=[1]",
            "[dependencies]\nx=1",
            "[dependencies]\nx={workspace=false}",
        ] {
            assert!(
                extract(source.as_bytes()).is_err(),
                "accepted invalid fixture"
            );
        }
        for source in [
            "x=1.2",
            "x=0xff",
            "x=nan",
            "x='''multi'''",
            "x=\"\"\"multi\"\"\"",
            "x={\na=1}",
            "x={a=[\n1\n]}",
        ] {
            assert!(
                matches!(extract(source.as_bytes()), Err(E::Unsupported)),
                "unsupported form misclassified"
            );
        }
        assert!(matches!(extract(b"x='\xff'"), Err(E::InvalidToml)));
        assert!(matches!(extract(b""), Err(E::Unsupported)));
    }
    #[test]
    fn strings_keys_tables_and_comments_validate_before_projection() {
        let source = b"\"\" = \"\\b\\t\\n\\f\\r\\\"\\\\\\u00e7\\U0001f642\"\nx=[true, false, +1, -2, 1_000, #comment\r\n [0,],]\n[a.b]\nx='literal\\text'\n[a]\ny=1\n[[list]]\nx=1\n[list.nested]\na=1\n[[list]]\nx=2\n";
        let (nodes, refs) = extract(source).unwrap();
        assert!(refs.is_empty());
        assert!(
            nodes
                .iter()
                .any(|n| n.locator() == "/k:list/i:0/k:nested/k:a")
        );
        assert!(nodes.iter().any(|n| n.locator() == "/k:list/i:1/k:x"));
        // Escaped/literal equivalent identity is compared after decoding.
        assert!(matches!(
            extract(b"'a'=1\n\"\\u0061\"=2"),
            Err(E::DuplicateDeclaration)
        ));
    }
    #[test]
    fn exact_source_node_depth_location_aggregate_and_reference_bounds() {
        let mut bytes = b"#".to_vec();
        bytes.resize(MAX_SOURCE_BYTES, b'a');
        assert!(extract(&bytes).is_ok());
        bytes.push(b'a');
        assert!(matches!(extract(&bytes), Err(E::Limit)));
        let nodes = |n: usize| format!("items=[{}]", vec!["0"; n].join(","));
        assert_eq!(
            extract(nodes(MAX_NODES - 3).as_bytes()).unwrap().0.len(),
            MAX_NODES
        );
        assert!(matches!(
            extract(nodes(MAX_NODES - 2).as_bytes()),
            Err(E::Limit)
        ));
        let deep = |n: usize| format!("x={}0{}", "[".repeat(n), "]".repeat(n));
        assert!(extract(deep(31).as_bytes()).is_ok());
        assert!(matches!(extract(deep(32).as_bytes()), Err(E::Limit)));
        // /k: + name + /@key must fit the existing locator limit exactly.
        let key = "x".repeat(MAX_LOCATION_BYTES - 8);
        assert!(extract(format!("'{key}'=1").as_bytes()).is_ok());
        assert!(matches!(
            extract(format!("'{key}x'=1").as_bytes()),
            Err(E::Limit)
        ));
        let refs = |n: usize| {
            format!(
                "[package]\nname='p'\n[features]\nf=[{}]",
                vec!["'f'"; n].join(",")
            )
        };
        assert_eq!(
            extract(refs(MAX_REFERENCES).as_bytes()).unwrap().1.len(),
            MAX_REFERENCES
        );
        assert!(matches!(
            extract(refs(MAX_REFERENCES + 1).as_bytes()),
            Err(E::Limit)
        ));
        // Exact aggregate locator bytes, independently below other bounds.
        let base = format!("'{}'=[{}]", "a".repeat(1000), vec!["0"; 252].join(","));
        let total: usize = extract(base.as_bytes())
            .unwrap()
            .0
            .iter()
            .map(|n| n.locator().len())
            .sum();
        let remaining = MAX_SOURCE_BYTES - total;
        // A scalar key of K bytes contributes 2*K+11 locator bytes.
        assert_eq!((remaining - 11) % 2, 0);
        let exact = format!("{base}\n'{}'=0", "b".repeat((remaining - 11) / 2));
        let actual: usize = extract(exact.as_bytes())
            .unwrap()
            .0
            .iter()
            .map(|n| n.locator().len())
            .sum();
        assert_eq!(actual, MAX_SOURCE_BYTES);
        // A singleton array contributes 3*K+18. This fixture is exactly N+1.
        assert_eq!((remaining + 1 - 18) % 3, 0);
        let over = format!("{base}\n'{}'=[0]", "b".repeat((remaining + 1 - 18) / 3));
        assert!(matches!(extract(over.as_bytes()), Err(E::Limit)));
    }
    #[test]
    fn metadata_workspace_and_dependency_text_do_not_create_relationships() {
        let source = b"[package]\nname='p'\n[package.metadata]\nworkspace=false\ndependencies={fake='1'}\n[target.'cfg(unknown())'.dependencies]\ndep={version='1'}\n";
        let (_, refs) = extract(source).unwrap();
        assert_eq!(refs.len(), 3); // target key, dependency alias and version only
        assert!(
            refs.iter()
                .all(|r| !r.from().starts_with("/k:package/k:metadata"))
        );
    }
    #[test]
    fn relevant_control_spellings_refuse_and_long_irrelevant_values_remain_bounded_source() {
        assert!(matches!(
            extract(b"[dependencies]\nx={path='a\tprivate'}"),
            Err(E::Unsupported)
        ));
        assert!(matches!(extract(b"\"\\n\"=1"), Err(E::Unsupported)));
        assert!(matches!(
            extract(
                format!("[dependencies]\nx='{}'", "a".repeat(MAX_LOCATION_BYTES + 1)).as_bytes()
            ),
            Err(E::Limit)
        ));
        assert!(
            extract(format!("description='{}'", "a".repeat(MAX_LOCATION_BYTES + 1)).as_bytes())
                .is_ok()
        );
    }
}
