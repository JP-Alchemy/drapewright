//! Tokens and objects in PDF syntax (§7.2, §7.3), for both the file's
//! objects and content streams. Lenient: real files bend the rules.

use crate::PdfError;
use crate::object::{Dict, Object, Ref};

/// Objects nest no deeper than this, so a hostile file can't exhaust the stack.
const MAX_DEPTH: usize = 64;

pub fn is_whitespace(b: u8) -> bool {
    matches!(b, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

pub fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_regular(b: u8) -> bool {
    !is_whitespace(b) && !is_delimiter(b)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'a> {
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    String(Vec<u8>),
    ArrayStart,
    ArrayEnd,
    DictStart,
    DictEnd,
    /// Anything else: `true`, `obj`, `R`, an operator such as `cm`.
    Keyword(&'a [u8]),
}

#[derive(Clone, Debug)]
pub struct Lexer<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Lexer { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Lexer { data, pos }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Skips whitespace and comments.
    pub fn skip_space(&mut self) {
        while let Some(b) = self.peek() {
            if is_whitespace(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(b) = self.peek() {
                    if b == b'\n' || b == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_space();
        let b = self.peek()?;
        match b {
            b'[' => {
                self.pos += 1;
                Some(Token::ArrayStart)
            }
            b']' => {
                self.pos += 1;
                Some(Token::ArrayEnd)
            }
            b'<' if self.data.get(self.pos + 1) == Some(&b'<') => {
                self.pos += 2;
                Some(Token::DictStart)
            }
            b'>' if self.data.get(self.pos + 1) == Some(&b'>') => {
                self.pos += 2;
                Some(Token::DictEnd)
            }
            b'<' => Some(Token::String(self.hex_string())),
            b'(' => Some(Token::String(self.literal_string())),
            b'/' => Some(Token::Name(self.name())),
            b'{' | b'}' | b')' | b'>' => {
                // Stray delimiters: return them as keywords so callers can skip.
                self.pos += 1;
                Some(Token::Keyword(&self.data[self.pos - 1..self.pos]))
            }
            _ => {
                let start = self.pos;
                while self.peek().is_some_and(is_regular) {
                    self.pos += 1;
                }
                let word = &self.data[start..self.pos];
                Some(number(word).unwrap_or(Token::Keyword(word)))
            }
        }
    }

    fn name(&mut self) -> Vec<u8> {
        self.pos += 1; // the slash
        let mut out = Vec::new();
        while let Some(b) = self.peek() {
            if !is_regular(b) {
                break;
            }
            self.pos += 1;
            if b == b'#'
                && let (Some(h), Some(l)) = (
                    self.data.get(self.pos).copied().and_then(hex),
                    self.data.get(self.pos + 1).copied().and_then(hex),
                )
            {
                out.push(h << 4 | l);
                self.pos += 2;
                continue;
            }
            out.push(b);
        }
        out
    }

    fn hex_string(&mut self) -> Vec<u8> {
        self.pos += 1; // <
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'>' {
                break;
            }
            if let Some(v) = hex(b) {
                match high.take() {
                    Some(h) => out.push(h << 4 | v),
                    None => high = Some(v),
                }
            }
        }
        if let Some(h) = high {
            out.push(h << 4);
        }
        out
    }

    fn literal_string(&mut self) -> Vec<u8> {
        self.pos += 1; // (
        let mut out = Vec::new();
        let mut depth = 1;
        while let Some(b) = self.peek() {
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b);
                }
                b'\\' => {
                    let Some(e) = self.peek() else { break };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            // A line continuation.
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xff) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'\r' => {
                    // An end of line in a string reads as a line feed.
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    out.push(b'\n');
                }
                _ => out.push(b),
            }
        }
        out
    }

    /// Reads one object. With `refs`, `12 0 R` is a reference (file objects);
    /// content streams have none.
    pub fn object(&mut self, refs: bool) -> Result<Object, PdfError> {
        self.object_at_depth(refs, 0)
    }

    fn object_at_depth(&mut self, refs: bool, depth: usize) -> Result<Object, PdfError> {
        if depth > MAX_DEPTH {
            return Err(PdfError::damaged("objects nested too deeply"));
        }
        let token = self
            .next_token()
            .ok_or_else(|| PdfError::damaged("the file ends inside an object"))?;
        self.object_from(token, refs, depth)
    }

    /// The object that starts with `token`.
    pub fn object_from(
        &mut self,
        token: Token<'a>,
        refs: bool,
        depth: usize,
    ) -> Result<Object, PdfError> {
        Ok(match token {
            Token::Int(i) => {
                if refs && let Some(r) = self.reference_after(i) {
                    Object::Ref(r)
                } else {
                    Object::Int(i)
                }
            }
            Token::Real(r) => Object::Real(r),
            Token::Name(n) => Object::Name(n),
            Token::String(s) => Object::String(s),
            Token::ArrayStart => {
                let mut items = Vec::new();
                loop {
                    let t = self
                        .next_token()
                        .ok_or_else(|| PdfError::damaged("an array is not closed"))?;
                    match t {
                        Token::ArrayEnd => break,
                        Token::DictEnd => {}
                        t => items.push(self.object_from(t, refs, depth + 1)?),
                    }
                }
                Object::Array(items)
            }
            Token::DictStart => Object::Dict(self.dict_rest(refs, depth + 1)?),
            Token::Keyword(b"true") => Object::Bool(true),
            Token::Keyword(b"false") => Object::Bool(false),
            Token::Keyword(b"null") => Object::Null,
            Token::Keyword(k) => {
                return Err(PdfError::damaged(format!(
                    "unexpected \"{}\"",
                    String::from_utf8_lossy(k)
                )));
            }
            Token::ArrayEnd | Token::DictEnd => {
                return Err(PdfError::damaged(
                    "unexpected end of an array or dictionary",
                ));
            }
        })
    }

    /// The rest of a dictionary after `<<`.
    pub fn dict_rest(&mut self, refs: bool, depth: usize) -> Result<Dict, PdfError> {
        let mut dict = Dict::default();
        loop {
            let t = self
                .next_token()
                .ok_or_else(|| PdfError::damaged("a dictionary is not closed"))?;
            match t {
                Token::DictEnd => break,
                Token::Name(key) => {
                    let t = self
                        .next_token()
                        .ok_or_else(|| PdfError::damaged("a dictionary is not closed"))?;
                    if t == Token::DictEnd {
                        dict.0.push((key, Object::Null));
                        break;
                    }
                    let value = self.object_from(t, refs, depth)?;
                    dict.0.push((key, value));
                }
                // Junk where a key should be: skip it.
                _ => {}
            }
        }
        Ok(dict)
    }

    /// `gen R` after the object number `num`, if that is what follows.
    fn reference_after(&mut self, num: i64) -> Option<Ref> {
        let save = self.pos;
        let found = (|| {
            let Some(Token::Int(generation)) = self.next_token() else {
                return None;
            };
            let Some(Token::Keyword(b"R")) = self.next_token() else {
                return None;
            };
            Some(Ref {
                num: u32::try_from(num).ok()?,
                generation: u16::try_from(generation).ok()?,
            })
        })();
        if found.is_none() {
            self.pos = save;
        }
        found
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// A number token: `12`, `-3.5`, `.5`, `4.`, or the longest number at the
/// start of slightly broken ones such as `0.5-1`.
fn number(word: &[u8]) -> Option<Token<'static>> {
    let first = *word.first()?;
    if !(first.is_ascii_digit() || matches!(first, b'+' | b'-' | b'.')) {
        return None;
    }
    let mut end = 0;
    let mut dot = false;
    let mut digits = false;
    for (i, &b) in word.iter().enumerate() {
        match b {
            b'+' | b'-' if i == 0 => {}
            // Some writers double the sign: "--5".
            b'-' if i == 1 && word[0] == b'-' => {}
            b'.' if !dot => dot = true,
            b'0'..=b'9' => digits = true,
            _ => break,
        }
        end = i + 1;
    }
    if !digits {
        return None;
    }
    let text = std::str::from_utf8(&word[..end]).ok()?;
    let text = text.strip_prefix("--").map_or(text, |t| t);
    let negative_twice = word.starts_with(b"--");
    if dot {
        let v: f64 = if text.ends_with('.') {
            text.trim_end_matches('.').parse().ok()?
        } else {
            text.parse().ok()?
        };
        Some(Token::Real(if negative_twice { -v } else { v }))
    } else {
        let v: i64 = text.parse().ok().or_else(|| {
            // Too long for an integer: read it as a real.
            text.parse::<f64>().ok().map(|f| f as i64)
        })?;
        Some(Token::Int(if negative_twice { -v } else { v }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(s: &str) -> Vec<Token<'_>> {
        let mut lx = Lexer::new(s.as_bytes());
        std::iter::from_fn(|| lx.next_token()).collect()
    }

    #[test]
    fn numbers_names_and_strings() {
        assert_eq!(
            tokens("12 -3.5 .5 4. +7 --2 1.5-3"),
            vec![
                Token::Int(12),
                Token::Real(-3.5),
                Token::Real(0.5),
                Token::Real(4.0),
                Token::Int(7),
                Token::Int(-2),
                Token::Real(1.5),
            ]
        );
        assert_eq!(tokens("/A#20B"), vec![Token::Name(b"A B".to_vec())]);
        assert_eq!(
            tokens(r"(a\(b\)c\n\101 (d))"),
            vec![Token::String(b"a(b)c\nA (d)".to_vec())]
        );
        assert_eq!(
            tokens("<48 65 6c6c 6F7>"),
            vec![Token::String(b"Hellop".to_vec())]
        );
        assert_eq!(tokens("% note\n q"), vec![Token::Keyword(b"q")]);
    }

    #[test]
    fn objects_with_references() {
        let mut lx = Lexer::new(b"<< /Type /Page /Kids [1 0 R 2 0 R] /N 3 /F 1.5 /T true >>");
        let o = lx.object(true).unwrap();
        let d = o.as_dict().unwrap();
        assert_eq!(d.name(b"Type"), Some(&b"Page"[..]));
        assert_eq!(
            d.get(b"Kids").unwrap().as_array().unwrap()[1],
            Object::Ref(Ref {
                num: 2,
                generation: 0
            })
        );
        assert_eq!(d.get(b"N").unwrap().as_int(), Some(3));
        assert_eq!(d.get(b"T").unwrap().as_bool(), Some(true));
        // Without references, numbers stay numbers.
        let mut lx = Lexer::new(b"[1 0 0 1 0 0]");
        assert_eq!(lx.object(false).unwrap().as_array().unwrap().len(), 6);
    }
}
