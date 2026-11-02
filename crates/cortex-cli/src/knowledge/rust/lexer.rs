//! Private bounded tokenizer. Tokens borrow byte extents; comments never become tokens.
use super::ExtractionError as E;
use cortex_core::knowledge::{MAX_LOCATION_BYTES, MAX_SOURCE_BYTES};

pub(super) const DEPTH: usize = 32;
const TOKENS: usize = 65_536;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Word,
    Literal,
    Punct,
}
pub(super) struct Token<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    pub close: usize,
}
fn ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
fn escape(b: &[u8], at: &mut usize, byte: bool, character: bool) -> Result<(), E> {
    let c = *b.get(*at).ok_or(E::InvalidRust)?;
    *at += 1;
    match c {
        b'\'' | b'"' | b'\\' | b'n' | b'r' | b't' | b'0' => Ok(()),
        b'x' => {
            let h = b.get(*at..*at + 2).ok_or(E::InvalidRust)?;
            if !h.iter().all(u8::is_ascii_hexdigit) || (!byte && h[0] > b'7') {
                return Err(E::InvalidRust);
            }
            *at += 2;
            Ok(())
        }
        b'u' if !byte => {
            if b.get(*at) != Some(&b'{') {
                return Err(E::InvalidRust);
            }
            *at += 1;
            let mut value = 0u32;
            let mut digits = 0;
            while let Some(c) = b.get(*at) {
                if *c == b'}' {
                    break;
                }
                if *c != b'_' {
                    let d = char::from(*c).to_digit(16).ok_or(E::InvalidRust)?;
                    digits += 1;
                    if digits > 6 {
                        return Err(E::InvalidRust);
                    }
                    value = value * 16 + d;
                }
                *at += 1;
            }
            if digits == 0 || b.get(*at) != Some(&b'}') || char::from_u32(value).is_none() {
                return Err(E::InvalidRust);
            }
            *at += 1;
            Ok(())
        }
        b'\n' | b'\r' if !character => {
            if c == b'\r' {
                if b.get(*at) != Some(&b'\n') {
                    return Err(E::InvalidRust);
                }
                *at += 1;
            }
            while b.get(*at).is_some_and(u8::is_ascii_whitespace) {
                *at += 1;
            }
            Ok(())
        }
        _ => Err(E::InvalidRust),
    }
}
fn quoted(s: &str, at: &mut usize, quote: u8, byte: bool) -> Result<(), E> {
    let b = s.as_bytes();
    *at += 1;
    let mut chars = 0;
    loop {
        match b.get(*at).copied().ok_or(E::InvalidRust)? {
            c if c == quote => {
                *at += 1;
                break;
            }
            b'\\' => {
                *at += 1;
                escape(b, at, byte, quote == b'\'')?;
                chars += 1;
            }
            b'\n' | b'\r' | b'\t' if quote == b'\'' => return Err(E::InvalidRust),
            _ => {
                let c = s[*at..].chars().next().ok_or(E::InvalidRust)?;
                if byte && !c.is_ascii() {
                    return Err(E::InvalidRust);
                }
                *at += c.len_utf8();
                chars += 1;
            }
        }
    }
    if quote == b'\'' && chars != 1 {
        return Err(E::InvalidRust);
    }
    Ok(())
}
// Returns the position of a raw literal's opening quote (or None for r#ident).
fn raw_quote(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    if matches!(b.get(j), Some(b'b' | b'c')) {
        j += 1;
    }
    if b.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    while b.get(j) == Some(&b'#') {
        j += 1;
    }
    (b.get(j) == Some(&b'"')).then_some(j)
}
pub(super) fn lex(bytes: &[u8]) -> Result<Vec<Token<'_>>, E> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(E::Limit);
    }
    let s = std::str::from_utf8(bytes).map_err(|_| E::Encoding)?;
    if s.is_empty() {
        return Err(E::InvalidRust);
    }
    let mut tokens: Vec<Token<'_>> = Vec::new();
    let mut stack: Vec<(u8, usize)> = Vec::new();
    let mut i = if s.starts_with('\u{feff}') { 3 } else { 0 };
    if bytes.get(i..i + 2) == Some(b"#!") && bytes.get(i + 2) != Some(&b'[') {
        while bytes.get(i).is_some_and(|b| *b != b'\n') {
            i += 1;
        }
    }
    while i < bytes.len() {
        let start = i;
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"//") {
            while bytes.get(i).is_some_and(|b| *b != b'\n') {
                i += 1;
            }
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"/*") {
            i += 2;
            let mut depth = 1;
            while depth > 0 {
                match bytes.get(i..i + 2) {
                    Some(b"/*") => {
                        depth += 1;
                        if depth > DEPTH {
                            return Err(E::Limit);
                        }
                        i += 2;
                    }
                    Some(b"*/") => {
                        depth -= 1;
                        i += 2;
                    }
                    _ if i < bytes.len() => i += 1,
                    _ => return Err(E::InvalidRust),
                }
            }
            continue;
        }
        let kind;
        if let Some(q) = raw_quote(bytes, i) {
            let r = i + usize::from(matches!(bytes[i], b'b' | b'c'));
            let hashes = q - r - 1;
            if hashes > 255 {
                return Err(E::UnsupportedSyntax);
            }
            i = q + 1;
            loop {
                if i >= bytes.len() {
                    return Err(E::InvalidRust);
                }
                if bytes[i] == b'"'
                    && bytes
                        .get(i + 1..i + 1 + hashes)
                        .is_some_and(|h| h.iter().all(|b| *b == b'#'))
                {
                    i += hashes + 1;
                    break;
                }
                if bytes[start] == b'b' && !bytes[i].is_ascii() {
                    return Err(E::InvalidRust);
                }
                i += 1;
            }
            kind = Kind::Literal;
        } else if bytes[i] == b'"'
            || (matches!(bytes[i], b'b' | b'c') && bytes.get(i + 1) == Some(&b'"'))
        {
            let byte = bytes[i] == b'b';
            if bytes[i] != b'"' {
                i += 1;
            }
            quoted(s, &mut i, b'"', byte)?;
            kind = Kind::Literal;
        } else if bytes[i] == b'b' && bytes.get(i + 1) == Some(&b'\'') {
            i += 1;
            quoted(s, &mut i, b'\'', true)?;
            kind = Kind::Literal;
        } else if bytes[i] == b'\'' {
            let mut j = i + 1;
            while bytes.get(j).is_some_and(|b| ident(*b)) {
                j += 1;
            }
            if j > i + 1 && bytes.get(j) != Some(&b'\'') {
                // Lifetime/label apostrophe is a punctuation token, followed by a word.
                i += 1;
                kind = Kind::Punct;
            } else {
                quoted(s, &mut i, b'\'', false)?;
                kind = Kind::Literal;
            }
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            if bytes.get(i..i + 2) == Some(b"r#") {
                i += 2;
            }
            if !bytes
                .get(i)
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            {
                return Err(E::InvalidRust);
            }
            i += 1;
            while bytes.get(i).is_some_and(|b| ident(*b)) {
                i += 1;
            }
            if i - start > MAX_LOCATION_BYTES {
                return Err(E::Limit);
            }
            kind = Kind::Word;
        } else if bytes[i].is_ascii_digit() {
            i += 1;
            while bytes.get(i).is_some_and(|b| ident(*b)) {
                i += 1;
            }
            kind = Kind::Literal;
        } else if b"{}[]();:,.!?#$%&*+-/<=>@^|~".contains(&bytes[i]) {
            i += 1;
            if bytes.get(start..i + 1) == Some(b"::") {
                i += 1;
            }
            kind = Kind::Punct;
        } else {
            return Err(E::UnsupportedSyntax);
        }
        if tokens.len() == TOKENS {
            return Err(E::Limit);
        }
        let index = tokens.len();
        tokens.push(Token {
            text: &s[start..i],
            start,
            end: i,
            kind,
            close: index,
        });
        if kind == Kind::Punct {
            match bytes[start] {
                b'{' | b'[' | b'(' => {
                    if stack.len() == DEPTH {
                        return Err(E::Limit);
                    }
                    stack.push((bytes[start], index));
                }
                b'}' | b']' | b')' => {
                    let (open, at) = stack.pop().ok_or(E::InvalidRust)?;
                    if !matches!(
                        (open, bytes[start]),
                        (b'{', b'}') | (b'[', b']') | (b'(', b')')
                    ) {
                        return Err(E::InvalidRust);
                    }
                    tokens[at].close = index;
                }
                _ => {}
            }
        }
    }
    if !stack.is_empty() {
        return Err(E::InvalidRust);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_lexer_budgets() {
        assert_eq!(lex(";".repeat(TOKENS).as_bytes()).unwrap().len(), TOKENS);
        assert!(matches!(
            lex(";".repeat(TOKENS + 1).as_bytes()),
            Err(E::Limit)
        ));
        for (n, succeeds) in [(DEPTH, true), (DEPTH + 1, false)] {
            let s = format!("{}{}", "(".repeat(n), ")".repeat(n));
            assert_eq!(lex(s.as_bytes()).is_ok(), succeeds);
            let s = format!("{}{}", "/*".repeat(n), "*/".repeat(n));
            assert_eq!(lex(s.as_bytes()).is_ok(), succeeds);
        }
    }
}
