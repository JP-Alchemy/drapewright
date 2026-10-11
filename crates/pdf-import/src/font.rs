//! Fonts, as far as text needs them: what each character code says (ToUnicode
//! maps, encodings and glyph names) and how far it moves the pen. Glyph
//! shapes are not read.

use std::collections::HashMap;

use crate::encoding::{Base, base_char, glyph_text};
use crate::file::File;
use crate::lexer::{Lexer, Token};
use crate::object::{Dict, Object};

/// One character code shown with a font.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// What it says; empty when not known.
    pub text: String,
    /// Advance, in text space units for a font size of 1.
    pub width: f64,
    /// The single-byte code 32, which word spacing applies to.
    pub is_space: bool,
}

/// A ToUnicode map (§9.10.3).
#[derive(Clone, Debug, Default)]
struct CMap {
    /// Byte lengths of codes, from the code space ranges: (bytes, low, high).
    spaces: Vec<(usize, u32, u32)>,
    single: HashMap<u32, String>,
    ranges: Vec<(u32, u32, Vec<u16>)>,
}

impl CMap {
    fn parse(data: &[u8]) -> CMap {
        let mut map = CMap::default();
        let mut lx = Lexer::new(data);
        let mut section: &[u8] = b"";
        let mut pending: Vec<Token> = Vec::new();
        while let Some(t) = lx.next_token() {
            match t {
                Token::Keyword(
                    k @ (b"begincodespacerange"
                    | b"beginbfchar"
                    | b"beginbfrange"
                    | b"begincidrange"
                    | b"begincidchar"),
                ) => {
                    section = k;
                    pending.clear();
                }
                Token::Keyword(k) if k.starts_with(b"end") => {
                    section = b"";
                    pending.clear();
                }
                Token::ArrayStart if section == b"beginbfrange" && pending.len() == 2 => {
                    // <lo> <hi> [<dst> <dst> …]
                    let mut items = Vec::new();
                    while let Some(t) = lx.next_token() {
                        match t {
                            Token::ArrayEnd => break,
                            Token::String(s) => items.push(s),
                            _ => {}
                        }
                    }
                    if let (Token::String(lo), Token::String(hi)) = (&pending[0], &pending[1]) {
                        let (lo, hi) = (code(lo), code(hi));
                        for (i, dst) in items.iter().enumerate() {
                            let c = lo + i as u32;
                            if c > hi {
                                break;
                            }
                            map.single.insert(c, utf16(dst));
                        }
                    }
                    pending.clear();
                }
                t if !section.is_empty() => {
                    pending.push(t);
                    match (section, pending.as_slice()) {
                        (b"begincodespacerange", [Token::String(lo), Token::String(hi)]) => {
                            map.spaces.push((lo.len().clamp(1, 4), code(lo), code(hi)));
                            pending.clear();
                        }
                        (b"beginbfchar", [Token::String(src), Token::String(dst)]) => {
                            map.single.insert(code(src), utf16(dst));
                            pending.clear();
                        }
                        (b"beginbfchar", [Token::String(src), Token::Name(name)]) => {
                            if let Some(t) = glyph_text(name) {
                                map.single.insert(code(src), t);
                            }
                            pending.clear();
                        }
                        (
                            b"beginbfrange",
                            [Token::String(lo), Token::String(hi), Token::String(dst)],
                        ) => {
                            let units: Vec<u16> = dst
                                .chunks(2)
                                .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                                .collect();
                            if !units.is_empty() {
                                map.ranges.push((code(lo), code(hi), units));
                            }
                            pending.clear();
                        }
                        (_, p) if p.len() >= 3 => pending.clear(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        map
    }

    fn lookup(&self, c: u32) -> Option<String> {
        if let Some(t) = self.single.get(&c) {
            return Some(t.clone());
        }
        let (lo, _, units) = self
            .ranges
            .iter()
            .find(|(lo, hi, _)| (*lo..=*hi).contains(&c))?;
        let mut units = units.clone();
        let last = units.last_mut()?;
        *last = last.wrapping_add((c - lo) as u16);
        Some(String::from_utf16_lossy(&units))
    }
}

fn code(bytes: &[u8]) -> u32 {
    bytes.iter().take(4).fold(0, |v, &b| v << 8 | u32::from(b))
}

fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
        .collect();
    String::from_utf16_lossy(&units)
}

#[derive(Clone, Debug)]
enum Widths {
    Simple {
        first: u32,
        widths: Vec<f64>,
        missing: f64,
    },
    Cid {
        widths: HashMap<u32, f64>,
        default: f64,
    },
}

#[derive(Clone, Debug)]
pub struct Font {
    /// Codes are two bytes (composite fonts with Identity or two-byte maps).
    two_byte: bool,
    to_unicode: Option<CMap>,
    /// For simple fonts: code → text.
    simple: Vec<Option<String>>,
    widths: Widths,
    /// Glyph space to text space: 1/1000, or a Type 3 font's matrix.
    scale: f64,
}

impl Font {
    /// A font that knows nothing: codes read as Latin-1, half an em wide.
    pub fn fallback() -> Font {
        Font {
            two_byte: false,
            to_unicode: None,
            simple: (0..=255u8)
                .map(|c| base_char(Base::WinAnsi, c).map(String::from))
                .collect(),
            widths: Widths::Simple {
                first: 0,
                widths: Vec::new(),
                missing: 500.0,
            },
            scale: 0.001,
        }
    }

    pub fn load(file: &File, dict: &Dict) -> Font {
        let subtype = dict.name(b"Subtype").unwrap_or(b"Type1");
        let to_unicode = file
            .dict_get(dict, b"ToUnicode")
            .and_then(|o| o.as_stream().and_then(|s| file.decode(s).ok()))
            .map(|data| CMap::parse(&data));
        let number = |o: &Object| o.as_f64();
        if subtype == b"Type0" {
            let descendant = file
                .dict_get(dict, b"DescendantFonts")
                .and_then(|a| a.as_array().and_then(|a| a.first().cloned()))
                .and_then(|d| file.resolve(&d).ok());
            let cid = descendant.as_deref().and_then(Object::as_dict);
            let default = cid
                .and_then(|d| d.get(b"DW"))
                .and_then(number)
                .unwrap_or(1000.0);
            let mut widths = HashMap::new();
            if let Some(w) = cid
                .and_then(|d| file.dict_get(d, b"W"))
                .and_then(|w| w.as_array().map(<[Object]>::to_vec))
            {
                let mut i = 0;
                while i < w.len() {
                    let Some(first) = w[i].as_int() else { break };
                    match w.get(i + 1) {
                        Some(Object::Array(list)) => {
                            for (k, v) in list.iter().enumerate() {
                                if let Some(v) = number(v) {
                                    widths.insert(first as u32 + k as u32, v);
                                }
                            }
                            i += 2;
                        }
                        Some(last) => {
                            let (Some(last), Some(v)) =
                                (last.as_int(), w.get(i + 2).and_then(number))
                            else {
                                break;
                            };
                            for c in first..=last.min(first + 65_535) {
                                widths.insert(c as u32, v);
                            }
                            i += 3;
                        }
                        None => break,
                    }
                }
            }
            // One-byte composite fonts are rare; anything named …-H or …-V
            // with two-byte code space ranges reads two bytes.
            let two_byte = to_unicode
                .as_ref()
                .and_then(|m| m.spaces.first())
                .is_none_or(|s| s.0 >= 2);
            return Font {
                two_byte,
                to_unicode,
                simple: Vec::new(),
                widths: Widths::Cid { widths, default },
                scale: 0.001,
            };
        }

        // Simple fonts.
        let encoding = file.dict_get(dict, b"Encoding");
        let base_name = |n: &[u8]| match n {
            b"WinAnsiEncoding" => Some(Base::WinAnsi),
            b"MacRomanEncoding" => Some(Base::MacRoman),
            b"StandardEncoding" => Some(Base::Standard),
            _ => None,
        };
        let symbolic = dict
            .get(b"FontDescriptor")
            .and_then(|d| file.resolve(d).ok())
            .and_then(|d| {
                d.as_dict()
                    .and_then(|d| d.get(b"Flags"))
                    .and_then(Object::as_int)
            })
            .is_some_and(|f| f & 4 != 0 && f & 32 == 0);
        let default_base = if subtype == b"TrueType" || symbolic {
            Base::WinAnsi
        } else {
            Base::Standard
        };
        let base = match encoding.as_deref() {
            Some(Object::Name(n)) => base_name(n).unwrap_or(default_base),
            Some(o) => o
                .as_dict()
                .and_then(|d| d.name(b"BaseEncoding"))
                .and_then(base_name)
                .unwrap_or(default_base),
            None => default_base,
        };
        // Codes the base encoding leaves out are usually meant as Windows
        // text: an embedded font's own encoding often is.
        let mut simple: Vec<Option<String>> = (0..=255u8)
            .map(|c| {
                base_char(base, c)
                    .or_else(|| base_char(Base::WinAnsi, c))
                    .map(String::from)
            })
            .collect();
        if let Some(diffs) = encoding
            .as_deref()
            .and_then(Object::as_dict)
            .and_then(|d| file.dict_get(d, b"Differences"))
            .and_then(|a| a.as_array().map(<[Object]>::to_vec))
        {
            let mut code = 0usize;
            for item in diffs {
                match item {
                    Object::Int(c) => code = c.clamp(0, 255) as usize,
                    Object::Name(n) => {
                        if code < 256 {
                            simple[code] = glyph_text(&n);
                        }
                        code += 1;
                    }
                    _ => {}
                }
            }
        }
        let first = dict
            .get(b"FirstChar")
            .and_then(Object::as_int)
            .unwrap_or(0)
            .clamp(0, 255) as u32;
        let widths: Vec<f64> = file
            .dict_get(dict, b"Widths")
            .and_then(|w| {
                w.as_array()
                    .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect())
            })
            .unwrap_or_default();
        let missing = dict
            .get(b"FontDescriptor")
            .and_then(|d| file.resolve(d).ok())
            .and_then(|d| {
                d.as_dict()
                    .and_then(|d| d.get(b"MissingWidth"))
                    .and_then(number)
            })
            .unwrap_or_else(|| {
                let name = dict.name(b"BaseFont").unwrap_or_default();
                if name.starts_with(b"Courier") {
                    600.0
                } else {
                    500.0
                }
            });
        let scale = if subtype == b"Type3" {
            dict.get(b"FontMatrix")
                .and_then(Object::as_array)
                .and_then(|m| m.first())
                .and_then(number)
                .unwrap_or(0.001)
        } else {
            0.001
        };
        Font {
            two_byte: false,
            to_unicode,
            simple,
            widths: Widths::Simple {
                first,
                widths,
                missing,
            },
            scale,
        }
    }

    fn width(&self, c: u32) -> f64 {
        let w = match &self.widths {
            Widths::Simple {
                first,
                widths,
                missing,
            } => c
                .checked_sub(*first)
                .and_then(|i| widths.get(i as usize))
                .copied()
                .filter(|w| *w > 0.0)
                .unwrap_or(*missing),
            Widths::Cid { widths, default } => widths.get(&c).copied().unwrap_or(*default),
        };
        w * self.scale
    }

    /// The glyphs a shown string's bytes stand for.
    pub fn glyphs(&self, bytes: &[u8]) -> Vec<Glyph> {
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let n = if self.two_byte {
                2.min(bytes.len() - i)
            } else {
                1
            };
            let c = code(&bytes[i..i + n]);
            i += n;
            let text = self
                .to_unicode
                .as_ref()
                .and_then(|m| m.lookup(c))
                .or_else(|| self.simple.get(c as usize).cloned().flatten())
                .unwrap_or_default();
            out.push(Glyph {
                text,
                width: self.width(c),
                is_space: n == 1 && c == 32,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_unicode_maps() {
        let cmap = CMap::parse(
            b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap
            1 begincodespacerange <0000> <FFFF> endcodespacerange
            2 beginbfchar <0003> <0020> <0024> <0041> endbfchar
            2 beginbfrange <0044> <0046> <0061> <0050> <0051> [<0046> <00660069>] endbfrange
            endcmap",
        );
        assert_eq!(cmap.spaces, vec![(2, 0, 0xffff)]);
        assert_eq!(cmap.lookup(0x24).as_deref(), Some("A"));
        assert_eq!(cmap.lookup(0x45).as_deref(), Some("b"));
        assert_eq!(cmap.lookup(0x51).as_deref(), Some("fi"));
        assert_eq!(cmap.lookup(0x99), None);
    }
}
