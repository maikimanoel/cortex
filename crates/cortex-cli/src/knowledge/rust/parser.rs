//! Structural item walker and deliberately conservative same-file lexical correspondence.
use super::{
    ExtractionError as E,
    lexer::{self, Kind, Token},
};
use cortex_core::knowledge::*;

struct Scope {
    locator: String,
    parent: Option<usize>,
    module: usize,
    uncertain: bool,
    imports: Vec<String>,
}
struct Decl {
    scope: usize,
    name: String,
    locator: String,
    child: Option<usize>,
    external: bool,
}
struct Occurrence {
    from: String,
    spelling: String,
    path: Vec<String>,
    scope: usize,
    supported: bool,
}
struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    nodes: Vec<KnowledgeNode>,
    refs: Vec<Occurrence>,
    scopes: Vec<Scope>,
    decls: Vec<Decl>,
    locator_bytes: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Module,
    Associated,
    Block,
}
fn name(s: &str) -> &str {
    s.strip_prefix("r#").unwrap_or(s)
}
fn identifier(s: &str) -> bool {
    if s.starts_with("r#") {
        return !matches!(name(s), "_" | "self" | "Self" | "super" | "crate");
    }
    !matches!(
        s,
        "_" | "as"
            | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "gen"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}
fn named_item(keyword: &str) -> bool {
    matches!(
        keyword,
        "mod" | "fn" | "struct" | "enum" | "union" | "trait" | "type" | "const" | "static"
    )
}
impl Parser<'_> {
    fn text(&self, i: usize) -> &str {
        self.tokens.get(i).map_or("", |t| t.text)
    }
    fn node(&mut self, locator: String, start: usize, end: usize) -> Result<String, E> {
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
        self.nodes
            .push(KnowledgeNode::new(&locator, SourceSpan::new(start, end)?)?);
        Ok(locator)
    }
    fn locator(&self, scope: usize, kind: &str, at: usize, suffix: &str) -> String {
        let prefix = &self.scopes[scope].locator;
        format!(
            "{}{kind}@{:06}/{suffix}",
            if prefix.is_empty() {
                String::new()
            } else {
                format!("{prefix}/")
            },
            self.tokens[at].start
        )
    }
    fn scope(&mut self, parent: usize, locator: String, module: bool) -> usize {
        let i = self.scopes.len();
        self.scopes.push(Scope {
            locator,
            parent: Some(parent),
            module: if module {
                i
            } else {
                self.scopes[parent].module
            },
            uncertain: false,
            imports: Vec::new(),
        });
        i
    }
    fn declaration(
        &mut self,
        scope: usize,
        kind: &str,
        at: usize,
        external: bool,
    ) -> Result<usize, E> {
        if self.tokens.get(at).is_none_or(|t| t.kind != Kind::Word) || name(self.text(at)) == "_" {
            return Err(E::InvalidRust);
        }
        if !identifier(self.text(at)) && !(kind == "crate" && self.text(at) == "self") {
            return Err(E::UnsupportedSyntax);
        }
        let locator = self.locator(scope, kind, at, self.text(at));
        let locator = self.node(locator, self.tokens[at].start, self.tokens[at].end)?;
        let d = self.decls.len();
        self.decls.push(Decl {
            scope,
            name: name(self.text(at)).to_owned(),
            locator,
            child: None,
            external,
        });
        Ok(d)
    }
    fn occurrence(
        &mut self,
        scope: usize,
        kind: &str,
        range: std::ops::Range<usize>,
        path: Vec<String>,
        alias: Option<String>,
        supported: bool,
    ) -> Result<(), E> {
        if self.refs.len() == MAX_REFERENCES {
            return Err(E::Limit);
        }
        let spelling = format!(
            "{}{}",
            path.join("::"),
            alias.as_ref().map_or(String::new(), |s| format!(" as {s}"))
        );
        if spelling.len() > MAX_LOCATION_BYTES {
            return Err(E::Limit);
        }
        let suffix = alias
            .as_deref()
            .unwrap_or_else(|| path.last().map_or("", String::as_str));
        let from = self.locator(scope, kind, range.start, suffix);
        let from = self.node(
            from,
            self.tokens[range.start].start,
            self.tokens[range.end - 1].end,
        )?;
        self.refs.push(Occurrence {
            from,
            spelling,
            path,
            scope,
            supported,
        });
        Ok(())
    }
    fn attributes(&mut self, mut i: usize, end: usize, scope: usize) -> Result<usize, E> {
        while self.text(i) == "#" {
            // No attribute is executed or assumed inert, including cfg/derive/doc.
            self.scopes[scope].uncertain = true;
            i += 1;
            if self.text(i) == "!" {
                i += 1;
            }
            if self.text(i) != "[" || i >= end {
                return Err(E::InvalidRust);
            }
            i = self.tokens[i].close + 1;
        }
        Ok(i)
    }
    fn visibility(&self, mut i: usize) -> Result<usize, E> {
        if self.text(i) == "pub" {
            i += 1;
            if self.text(i) == "(" {
                i = self.tokens[i].close + 1;
            }
        }
        Ok(i)
    }
    // The same finite named-item catalog drives dispatch and header refusal.
    // Special forms distinguish item starts from opaque type/signature tokens.
    fn header_item_boundary(&self, i: usize, angles: usize, type_start: bool) -> bool {
        let next_word = self.tokens.get(i + 1).is_some_and(|t| t.kind == Kind::Word);
        match self.text(i) {
            "fn" => next_word, // A bare `fn(...)` is a type, not a named item.
            "union" => next_word && identifier(self.text(i + 1)), // Weak keyword.
            "impl" => angles == 0 && !type_start,
            "static" => i == 0 || self.text(i - 1) != "'",
            "const" => {
                // `*const` is an opaque pointer qualifier, not a named item.
                angles == 0 && (i == 0 || !matches!(self.text(i - 1), "~" | "*")) && next_word
            }
            "extern" => {
                matches!(self.text(i + 1), "{" | "crate")
                    || (self
                        .tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == Kind::Literal)
                        && self.text(i + 2) == "{")
            }
            "macro_rules" => {
                self.text(i + 1) == "!"
                    && self.tokens.get(i + 2).is_some_and(|t| t.kind == Kind::Word)
            }
            "use" | "macro" | "pub" => true,
            keyword => named_item(keyword),
        }
    }
    // Scan a signature/item tail without mistaking nested parameter/const-generic
    // syntax for a new declaration. Expression/type semantics deliberately stay opaque.
    fn terminator(&self, mut i: usize, end: usize) -> Result<usize, E> {
        let mut angles = 0usize;
        let mut type_start = false;
        while i < end {
            if self.header_item_boundary(i, angles, type_start) {
                return Err(E::InvalidRust);
            }
            if angles == 0 {
                if self.text(i) == ">" && i > 0 && self.text(i - 1) == "-" {
                    type_start = true;
                } else if matches!(self.text(i), "<" | "(" | "[")
                    || self.tokens[i].kind == Kind::Literal
                    || (self.tokens[i].kind == Kind::Word
                        && (i == 0 || self.text(i - 1) != "'")
                        && !matches!(self.text(i), "mut" | "const"))
                {
                    // References/lifetimes may precede an opaque impl type.
                    // A consumed type atom/group/impl keyword ends that slot.
                    type_start = false;
                }
            }
            match self.text(i) {
                "(" | "[" => i = self.tokens[i].close + 1,
                "<" => {
                    angles += 1;
                    if angles > lexer::DEPTH {
                        return Err(E::Limit);
                    }
                    i += 1;
                }
                ">" if angles > 0 && (i == 0 || self.text(i - 1) != "-") => {
                    angles -= 1;
                    i += 1;
                }
                "{" if angles > 0 => i = self.tokens[i].close + 1,
                "{" | ";" if angles == 0 => return Ok(i),
                "}" | ")" | "]" => return Err(E::InvalidRust),
                _ => i += 1,
            }
        }
        Err(E::InvalidRust)
    }
    fn use_tree(
        &mut self,
        mut i: usize,
        end: usize,
        scope: usize,
        prefix: &[String],
        depth: usize,
    ) -> Result<usize, E> {
        if depth > lexer::DEPTH {
            return Err(E::Limit);
        }
        let start = i;
        let mut path = prefix.to_vec();
        if self.text(i) == "::" {
            path.push(String::new());
            i += 1;
        }
        loop {
            if i >= end {
                return Err(E::InvalidRust);
            }
            if self.text(i) == "{" {
                let close = self.tokens[i].close;
                if close >= end || close == i + 1 {
                    return Err(E::InvalidRust);
                }
                i += 1;
                loop {
                    i = self.use_tree(i, close, scope, &path, depth + 1)?;
                    if i == close {
                        return Ok(i + 1);
                    }
                    if self.text(i) != "," {
                        return Err(E::InvalidRust);
                    }
                    i += 1;
                    if i == close {
                        return Ok(i + 1);
                    }
                }
            }
            let t = &self.tokens[i];
            if t.kind != Kind::Word && t.text != "*" {
                return Err(E::InvalidRust);
            }
            let glob = t.text == "*";
            if !glob
                && !identifier(t.text)
                && !matches!(t.text, "self" | "super" | "crate" | "Self")
            {
                return Err(E::UnsupportedSyntax);
            }
            path.push(t.text.to_owned());
            i += 1;
            if self.text(i) == "::" {
                if glob {
                    return Err(E::InvalidRust);
                }
                i += 1;
                continue;
            }
            let mut alias = None;
            if self.text(i) == "as" {
                if glob || self.tokens.get(i + 1).is_none_or(|t| t.kind != Kind::Word) {
                    return Err(E::InvalidRust);
                }
                if !identifier(self.text(i + 1)) && self.text(i + 1) != "_" {
                    return Err(E::UnsupportedSyntax);
                }
                alias = Some(self.text(i + 1).to_owned());
                i += 2;
            }
            let bound = alias
                .as_deref()
                .unwrap_or_else(|| path.last().unwrap())
                .to_owned();
            if glob {
                self.scopes[scope].uncertain = true;
            } else if bound != "_" {
                let bound = if name(&bound) == "self" && alias.is_none() {
                    path.iter().rev().nth(1).map_or("self", |s| name(s))
                } else {
                    name(&bound)
                };
                self.scopes[scope].imports.push(bound.to_owned());
            }
            let supported = !glob
                && (path.last().is_none_or(|s| s != "self")
                    || (!prefix.is_empty() && path.len() == prefix.len() + 1));
            self.occurrence(scope, "use", start..i, path, alias, supported)?;
            return Ok(i);
        }
    }
    fn macro_at(&mut self, start: usize, end: usize, scope: usize) -> Result<Option<usize>, E> {
        // Inspect a path once, not each suffix of a long opaque expression path.
        if start > 0
            && (self.text(start - 1) == "::"
                || (self.text(start) == "::" && self.tokens[start - 1].kind == Kind::Word))
        {
            return Ok(None);
        }
        let mut i = start;
        let mut path = Vec::new();
        if self.text(i) == "::" {
            path.push(String::new());
            i += 1;
        }
        while self.tokens.get(i).is_some_and(|t| t.kind == Kind::Word) {
            path.push(self.text(i).to_owned());
            i += 1;
            if self.text(i) != "::" {
                break;
            }
            i += 1;
        }
        if path.is_empty()
            || self.text(i) != "!"
            || path.iter().any(|s| {
                !s.is_empty() && !identifier(s) && !matches!(s.as_str(), "self" | "super" | "crate")
            })
        {
            return Ok(None);
        }
        let mut token_start = start;
        let mut token_end = i;
        i += 1;
        if path == ["macro_rules"] {
            if self.tokens.get(i).is_none_or(|t| t.kind != Kind::Word) {
                return Err(E::InvalidRust);
            }
            if !identifier(self.text(i)) {
                return Err(E::UnsupportedSyntax);
            }
            token_start = i;
            token_end = i + 1;
            path = vec![self.text(i).to_owned()];
            i += 1;
        }
        if i >= end || !matches!(self.text(i), "{" | "[" | "(") {
            // `!=` is an expression operator. A recognized macro path followed
            // by `!` otherwise requires a token tree, even inside an opaque body.
            return if self.text(i) == "=" {
                Ok(None)
            } else {
                Err(E::InvalidRust)
            };
        }
        let close = self.tokens[i].close;
        if close >= end {
            return Err(E::InvalidRust);
        }
        self.scopes[scope].uncertain = true;
        self.occurrence(scope, "macro", token_start..token_end, path, None, false)?;
        Ok(Some(close + 1))
    }
    fn items(&mut self, mut i: usize, end: usize, scope: usize, context: Context) -> Result<(), E> {
        let mut statement_start = true;
        while i < end {
            if self.text(i) == ";" {
                i += 1;
                statement_start = true;
                continue;
            }
            if let Some(next) = self.macro_at(i, end, scope)? {
                i = next;
                continue;
            }
            // Unsupported macro items must not reach the expression fallback:
            // their token bodies are not ordinary blocks containing observed items.
            if self.text(i) == "macro" {
                return Err(E::UnsupportedSyntax);
            }
            if context == Context::Block && !statement_start {
                if self.text(i) == "{" {
                    let close = self.tokens[i].close;
                    let loc = self.locator(scope, "block", i, "");
                    let child = self.scope(scope, loc.trim_end_matches('/').to_owned(), false);
                    self.items(i + 1, close, child, Context::Block)?;
                    i = close + 1;
                    statement_start = true;
                } else if matches!(self.text(i), "(" | "[") {
                    // Nested expressions can contain block-local items too.
                    let close = self.tokens[i].close;
                    self.items(i + 1, close, scope, Context::Block)?;
                    i = close + 1;
                } else {
                    i += 1;
                }
                continue;
            }
            let start = i;
            i = self.attributes(i, end, scope)?;
            if i == end {
                // Only trailing inner attributes are meaningful without an item.
                if self.text(start + 1) == "!" {
                    return Ok(());
                }
                return Err(E::InvalidRust);
            }
            i = self.visibility(i)?;
            if self.text(i) == "use" {
                let next = self.use_tree(i + 1, end, scope, &[], 0)?;
                if self.text(next) != ";" {
                    return Err(E::InvalidRust);
                }
                i = next + 1;
                continue;
            }
            // Qualifiers precede fn/impl/trait/extern, but const/static are items.
            while matches!(self.text(i), "async" | "unsafe" | "default" | "auto")
                || (self.text(i) == "const"
                    && matches!(self.text(i + 1), "fn" | "unsafe" | "async" | "extern"))
            {
                i += 1;
            }
            if self.text(i) == "extern" {
                i += 1;
                if self.text(i) == "crate" {
                    let at = i + 1;
                    self.declaration(scope, "crate", at, true)?;
                    let mut next = at + 1;
                    let alias = if self.text(next) == "as" {
                        next += 2;
                        if self
                            .tokens
                            .get(next - 1)
                            .is_none_or(|t| t.kind != Kind::Word)
                            || (!identifier(self.text(next - 1)) && self.text(next - 1) != "_")
                        {
                            return Err(E::UnsupportedSyntax);
                        }
                        Some(self.text(next - 1).to_owned())
                    } else {
                        None
                    };
                    let imported = name(alias.as_deref().unwrap_or(self.text(at))).to_owned();
                    self.scopes[scope].imports.push(imported);
                    if self.text(next) != ";" {
                        return Err(E::InvalidRust);
                    }
                    self.occurrence(
                        scope,
                        "crate-ref",
                        at..next,
                        vec![self.text(at).to_owned()],
                        alias,
                        false,
                    )?;
                    i = next + 1;
                    continue;
                }
                if self.tokens.get(i).is_some_and(|t| t.kind == Kind::Literal) {
                    i += 1;
                }
                if self.text(i) == "{" {
                    let close = self.tokens[i].close;
                    let loc = self.locator(scope, "extern", i, "");
                    let child = self.scope(scope, loc.trim_end_matches('/').to_owned(), false);
                    self.items(i + 1, close, child, Context::Associated)?;
                    i = close + 1;
                    continue;
                }
                if self.text(i) != "fn" {
                    return Err(E::UnsupportedSyntax);
                }
            }
            let kind = self.text(i).to_owned();
            if context == Context::Block
                && ((kind == "fn" && self.tokens.get(i + 1).is_none_or(|t| t.kind != Kind::Word))
                    || (i > start && kind == "{"))
            {
                // Function-pointer types and qualified block expressions are opaque,
                // including when nested inside a parenthesized expression/type.
                statement_start = false;
                i = start + 1;
                continue;
            }
            if kind == "impl" {
                let term = self.terminator(i + 1, end)?;
                if self.text(term) != "{" {
                    return Err(E::InvalidRust);
                }
                let close = self.tokens[term].close;
                let loc = self.locator(scope, "impl", i, "");
                let child = self.scope(scope, loc.trim_end_matches('/').to_owned(), false);
                self.items(term + 1, close, child, Context::Associated)?;
                i = close + 1;
                continue;
            }
            if named_item(&kind)
                && !(context == Context::Block && kind == "const" && self.text(i + 1) == "{")
            {
                let at = i + 1 + usize::from(kind == "static" && self.text(i + 1) == "mut");
                let d = self.declaration(scope, &kind, at, false)?;
                if matches!(kind.as_str(), "type" | "const" | "static") {
                    if kind != "type" && self.text(at + 1) != ":" {
                        return Err(E::InvalidRust);
                    }
                    let mut next = at + 1;
                    while next < end && self.text(next) != ";" {
                        next = if matches!(self.text(next), "(" | "[" | "{") {
                            self.tokens[next].close + 1
                        } else {
                            next + 1
                        };
                    }
                    if next == end {
                        return Err(E::InvalidRust);
                    }
                    let child = self.scope(scope, self.decls[d].locator.clone(), false);
                    self.items(at + 1, next, child, Context::Block)?;
                    i = next + 1;
                    continue;
                }
                let term = if kind == "mod" {
                    if !matches!(self.text(at + 1), "{" | ";") {
                        return Err(E::InvalidRust);
                    }
                    at + 1
                } else {
                    self.terminator(at + 1, end)?
                };
                if kind == "fn" && !matches!(self.text(at + 1), "(" | "<") {
                    return Err(E::InvalidRust);
                }
                if kind == "fn" && !(at + 1..term).any(|j| self.text(j) == "(") {
                    return Err(E::InvalidRust);
                }
                if matches!(kind.as_str(), "struct" | "enum" | "union" | "trait")
                    && !matches!(self.text(at + 1), "<" | "where" | "(" | "{" | ";" | ":")
                {
                    return Err(E::UnsupportedSyntax);
                }
                if matches!(kind.as_str(), "enum" | "union" | "trait") && self.text(term) != "{" {
                    return Err(E::UnsupportedSyntax);
                }
                if self.text(term) == ";" {
                    if kind == "mod" {
                        self.decls[d].external = true;
                        self.occurrence(
                            scope,
                            "mod-ref",
                            at..at + 1,
                            vec![self.text(at).to_owned()],
                            None,
                            false,
                        )?;
                    }
                    i = term + 1;
                } else {
                    let close = self.tokens[term].close;
                    if matches!(kind.as_str(), "mod" | "fn" | "trait") {
                        let child = self.scope(scope, self.decls[d].locator.clone(), kind == "mod");
                        if kind == "mod" {
                            self.decls[d].child = Some(child);
                        }
                        self.items(
                            term + 1,
                            close,
                            child,
                            match kind.as_str() {
                                "mod" => Context::Module,
                                "trait" => Context::Associated,
                                _ => Context::Block,
                            },
                        )?;
                    }
                    i = close + 1;
                }
                continue;
            }
            if context == Context::Block && i == start {
                statement_start = false;
            } else {
                return Err(E::UnsupportedSyntax);
            }
        }
        Ok(())
    }
    fn uncertain(&self, mut scope: usize) -> bool {
        loop {
            if self.scopes[scope].uncertain {
                return true;
            }
            match self.scopes[scope].parent {
                Some(p) => scope = p,
                None => return false,
            }
        }
    }
    fn resolve(&self, r: &Occurrence) -> ReferenceTarget {
        use UnresolvedReference::{MissingTarget, OutsideCoverage};
        let outside = ReferenceTarget::Unresolved(OutsideCoverage);
        if !r.supported || self.uncertain(r.scope) {
            return outside;
        }
        let path: Vec<_> = r.path.iter().map(|s| name(s)).collect();
        let mut module = self.scopes[r.scope].module;
        let mut i = 0;
        if path.first() == Some(&"self") {
            i = 1;
            if path.get(i) == Some(&"super") {
                return outside;
            }
        } else if path.first() != Some(&"super") {
            return outside;
        }
        while path.get(i) == Some(&"super") {
            let Some(parent) = self.scopes[module].parent else {
                return outside;
            };
            module = self.scopes[parent].module;
            i += 1;
        }
        let segments = &path[i..];
        if segments.is_empty() {
            return outside;
        }
        for (n, part) in segments.iter().enumerate() {
            if self.uncertain(module) {
                return outside;
            }
            if *part == "self" && n + 1 == segments.len() {
                return if self.scopes[module].locator.is_empty() {
                    outside
                } else {
                    ReferenceTarget::Local(self.scopes[module].locator.clone())
                };
            }
            if self.scopes[module].imports.iter().any(|s| s == part) {
                return outside;
            }
            let mut matches = self
                .decls
                .iter()
                .filter(|d| d.scope == module && d.name == *part);
            let Some(d) = matches.next() else {
                return ReferenceTarget::Unresolved(MissingTarget);
            };
            if matches.next().is_some() || d.external {
                return outside;
            }
            if n + 1 == segments.len() {
                return ReferenceTarget::Local(d.locator.clone());
            }
            let Some(child) = d.child else {
                return outside;
            };
            module = child;
        }
        outside
    }
}
pub(super) fn extract(bytes: &[u8]) -> Result<(Vec<KnowledgeNode>, Vec<ExtractedReference>), E> {
    let mut p = Parser {
        tokens: lexer::lex(bytes)?,
        nodes: Vec::new(),
        refs: Vec::new(),
        scopes: vec![Scope {
            locator: String::new(),
            parent: None,
            module: 0,
            uncertain: false,
            imports: Vec::new(),
        }],
        decls: Vec::new(),
        locator_bytes: 0,
    };
    p.node(String::new(), 0, bytes.len())?;
    p.items(0, p.tokens.len(), 0, Context::Module)?;
    let mut refs = Vec::new();
    for r in &p.refs {
        refs.push(ExtractedReference::new(&r.from, &r.spelling, p.resolve(r))?);
    }
    Ok((p.nodes, refs))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn accepted(source: &str) {
        assert!(extract(source.as_bytes()).is_ok(), "source rejected");
    }
    fn limited(source: &str) {
        assert!(
            matches!(extract(source.as_bytes()), Err(E::Limit)),
            "expected atomic limit refusal"
        );
    }
    #[test]
    fn source_node_reference_and_identifier_budgets_have_exact_boundaries() {
        accepted(&" ".repeat(MAX_SOURCE_BYTES));
        limited(&" ".repeat(MAX_SOURCE_BYTES + 1));
        accepted(&format!("{};", "struct A;".repeat(MAX_NODES - 1)));
        limited(&"struct A;".repeat(MAX_NODES));
        accepted(&"use external::Item as Alias;".repeat(MAX_REFERENCES));
        limited(&"use external::Item as Alias;".repeat(MAX_REFERENCES + 1));
        // The lexer itself bounds identifiers; locator metadata can reach its
        // own independent cap first when embedded in an occurrence identity.
        limited(&format!("struct {};", "A".repeat(MAX_LOCATION_BYTES + 1)));
    }
    #[test]
    fn locator_and_spelling_lengths_and_aggregate_metadata_are_bounded() {
        let prefix = "struct@000007/".len();
        accepted(&format!(
            "struct {};",
            "A".repeat(MAX_LOCATION_BYTES - prefix)
        ));
        limited(&format!(
            "struct {};",
            "A".repeat(MAX_LOCATION_BYTES - prefix + 1)
        ));
        // Qualified path spelling can exceed the cap while each word and the
        // final alias/occurrence locator remain short.
        let a = "A".repeat(MAX_LOCATION_BYTES - "self:: as B".len());
        accepted(&format!("use self::{a} as B;"));
        limited(&format!("use self::{a}A as B;"));
        // 64 declarations with 4096-byte locators exactly consume 256 KiB;
        // the root locator is empty. Source bytes still fit the source bound.
        let identifier = "A".repeat(MAX_LOCATION_BYTES - "struct@000007/".len());
        let at_limit = format!("{}\n", format!("struct {identifier};").repeat(64));
        accepted(&at_limit);
        limited(&format!("{at_limit}struct B;"));
    }
    #[test]
    fn module_use_tree_and_signature_nesting_have_exact_boundaries() {
        for (n, succeeds) in [(lexer::DEPTH, true), (lexer::DEPTH + 1, false)] {
            let modules = format!("{}{}", "mod m {".repeat(n), "}".repeat(n));
            let groups = format!("use self::{}Item{};", "{".repeat(n), "}".repeat(n));
            let signature = format!("fn f<T: {}u8{}>() {{}}", "A<".repeat(n - 1), ">".repeat(n));
            for source in [modules, groups, signature] {
                if succeeds {
                    accepted(&source);
                } else {
                    limited(&source);
                }
            }
        }
    }
    #[test]
    fn raw_names_alias_underscore_nested_blocks_and_unicode_trivia_keep_original_ranges() {
        let source = "/* 🦀 */ mod r#type { pub struct r#match; } fn f() { { use self::r#type::{r#match as _}; } }";
        let (nodes, refs) = extract(source.as_bytes()).unwrap();
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].target(), ReferenceTarget::Local(_)));
        let node = nodes
            .iter()
            .find(|n| n.locator() == refs[0].from())
            .unwrap();
        assert_eq!(
            &source.as_bytes()[node.span().start()..node.span().end()],
            b"r#match as _"
        );
    }
    #[test]
    fn opaque_signature_types_and_function_pointer_locals_create_no_false_functions() {
        accepted(
            "pub const fn f<const N: usize, T: Fn() -> usize>(x: [u8; N]) -> impl Fn() -> usize where T: Send { let p: fn() -> usize = || 1; fn real() {} p }",
        );
        let (nodes, _) = extract(b"fn f() { let p: fn() = || {}; fn real() {} }").unwrap();
        assert_eq!(nodes.len(), 3);
        assert!(nodes.iter().any(|n| n.locator().ends_with("/real")));
        let (nodes, _) = extract(b"fn f() { let p: (unsafe extern \"C\" fn(), fn()) = todo!(); async { fn nested() {} }; }").unwrap();
        assert_eq!(nodes.len(), 4); // f, nested, root and the actual todo macro occurrence
        assert!(nodes.iter().any(|n| n.locator().ends_with("/nested")));
        let (_, refs) = extract(b"struct Item; fn f() { if !(true) {} while !(false) { break; } use self::Item as Alias; }").unwrap();
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].target(), ReferenceTarget::Local(_)));
        accepted(&format!(
            "fn f() {{ let _ = {}; }}",
            vec!["segment"; 4096].join("::")
        ));
    }
    #[test]
    fn representative_cortex_adapter_sources_fit_the_declared_structure() {
        for (fixture, bytes) in [
            include_bytes!("../../knowledge.rs").as_slice(),
            include_bytes!("../rust.rs").as_slice(),
            include_bytes!("../json.rs").as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            // The profile is structural, so external imports remain unresolved.
            let (nodes, refs) =
                extract(bytes).unwrap_or_else(|e| panic!("fixture {fixture}: {e:?}"));
            assert!(nodes.len() > 10);
            assert!(!refs.is_empty());
        }
    }
}
