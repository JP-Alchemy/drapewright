//! PDF objects (ISO 32000-1 §7.3).

use std::rc::Rc;

/// An indirect reference: object number and generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ref {
    pub num: u32,
    pub generation: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Object {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    /// Bytes as stored, already decrypted.
    String(Vec<u8>),
    Array(Vec<Object>),
    Dict(Dict),
    Stream(Rc<Stream>),
    Ref(Ref),
}

/// A dictionary, in file order. Later keys win, as most readers do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dict(pub Vec<(Vec<u8>, Object)>);

impl Dict {
    pub fn get(&self, key: &[u8]) -> Option<&Object> {
        self.0.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Sets `key`, replacing any value it had.
    pub fn set(&mut self, key: &[u8], value: Object) {
        self.0.retain(|(k, _)| k != key);
        self.0.push((key.to_vec(), value));
    }

    pub fn name(&self, key: &[u8]) -> Option<&[u8]> {
        self.get(key).and_then(Object::as_name)
    }
}

/// A stream: its dictionary, and its data still encoded (but decrypted).
#[derive(Debug, PartialEq)]
pub struct Stream {
    pub dict: Dict,
    pub data: Vec<u8>,
}

impl Object {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Object::Int(i) => Some(*i),
            Object::Real(r) if r.fract() == 0.0 && r.is_finite() => Some(*r as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Object::Int(i) => Some(*i as f64),
            Object::Real(r) if r.is_finite() => Some(*r),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&[u8]> {
        match self {
            Object::Name(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Object::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Object]> {
        match self {
            Object::Array(a) => Some(a),
            _ => None,
        }
    }

    /// A dictionary, or a stream's dictionary.
    pub fn as_dict(&self) -> Option<&Dict> {
        match self {
            Object::Dict(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }

    pub fn as_stream(&self) -> Option<&Rc<Stream>> {
        match self {
            Object::Stream(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<Ref> {
        match self {
            Object::Ref(r) => Some(*r),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Object::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// A text string (§7.9.2.2): UTF-16BE with a byte order mark, UTF-8 with
/// one, or PDFDocEncoding (taken as Latin-1, which it matches for letters).
pub fn text_string(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    bytes.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_strings_in_each_encoding() {
        assert_eq!(text_string(b"Size 10"), "Size 10");
        assert_eq!(text_string(&[0xfe, 0xff, 0, b'X', 0, b'S']), "XS");
        assert_eq!(text_string(&[0xef, 0xbb, 0xbf, b'M']), "M");
        assert_eq!(text_string(&[0xe9]), "é");
    }
}
