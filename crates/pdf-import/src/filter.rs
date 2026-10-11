//! Stream filters (§7.4): the ones that carry drawings and text. Image
//! filters (DCT, JPX, JBIG2, CCITT) are never needed, since images are not
//! read.

use std::io::Read;

use flate2::read::{DeflateDecoder, ZlibDecoder};

use crate::PdfError;
use crate::object::{Dict, Object};

/// Decodes `data` through each filter in turn. `params` are the matching
/// decode parameters (already resolved).
pub fn decode(
    data: &[u8],
    filters: &[Vec<u8>],
    params: &[Option<Dict>],
) -> Result<Vec<u8>, PdfError> {
    let mut out = data.to_vec();
    for (i, f) in filters.iter().enumerate() {
        let p = params.get(i).and_then(Option::as_ref);
        out = match f.as_slice() {
            b"FlateDecode" | b"Fl" => predict(inflate(&out), p)?,
            b"LZWDecode" | b"LZW" => {
                let early = p
                    .and_then(|p| p.get(b"EarlyChange"))
                    .and_then(Object::as_int)
                    .unwrap_or(1);
                predict(lzw(&out, early != 0), p)?
            }
            b"ASCIIHexDecode" | b"AHx" => ascii_hex(&out),
            b"ASCII85Decode" | b"A85" => ascii85(&out),
            b"RunLengthDecode" | b"RL" => run_length(&out),
            b"Crypt" => out,
            other => {
                return Err(PdfError::Unsupported(format!(
                    "data compressed with {}",
                    String::from_utf8_lossy(other)
                )));
            }
        };
    }
    Ok(out)
}

/// Inflates zlib (or raw deflate) data, keeping whatever decodes before
/// any damage, as viewers do.
fn inflate(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let zlib = data.len() >= 2
        && (u16::from(data[0]) << 8 | u16::from(data[1])) % 31 == 0
        && data[0] & 0x0f == 8;
    if zlib {
        let mut decoder = ZlibDecoder::new(data);
        if decoder.read_to_end(&mut out).is_ok() {
            return out;
        }
        // Keep what came out; fall through only if nothing did.
        if !out.is_empty() {
            return out;
        }
    }
    let mut decoder = DeflateDecoder::new(data);
    let _ = decoder.read_to_end(&mut out);
    out
}

/// PNG and TIFF predictors (§7.4.4.4).
fn predict(data: Vec<u8>, params: Option<&Dict>) -> Result<Vec<u8>, PdfError> {
    let Some(p) = params else {
        return Ok(data);
    };
    let int = |key: &[u8], default: i64| p.get(key).and_then(Object::as_int).unwrap_or(default);
    let predictor = int(b"Predictor", 1);
    if predictor <= 1 {
        return Ok(data);
    }
    let colors = int(b"Colors", 1).clamp(1, 32) as usize;
    let bits = int(b"BitsPerComponent", 8).clamp(1, 16) as usize;
    let columns = int(b"Columns", 1).clamp(1, 1 << 20) as usize;
    let bpp = (colors * bits).div_ceil(8);
    let row = (colors * bits * columns).div_ceil(8);
    if predictor == 2 {
        if bits != 8 {
            return Ok(data);
        }
        let mut out = data;
        for line in out.chunks_mut(row) {
            for i in bpp..line.len() {
                line[i] = line[i].wrapping_add(line[i - bpp]);
            }
        }
        return Ok(out);
    }
    // PNG: each row starts with its filter type.
    let mut out = Vec::with_capacity(data.len());
    let mut previous = vec![0u8; row];
    for chunk in data.chunks(row + 1) {
        let Some((&kind, rest)) = chunk.split_first() else {
            break;
        };
        let mut line = rest.to_vec();
        line.resize(row, 0);
        for i in 0..row {
            let left = if i >= bpp { line[i - bpp] } else { 0 };
            let up = previous[i];
            let up_left = if i >= bpp { previous[i - bpp] } else { 0 };
            line[i] = match kind {
                0 => line[i],
                1 => line[i].wrapping_add(left),
                2 => line[i].wrapping_add(up),
                3 => line[i].wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8),
                4 => line[i].wrapping_add(paeth(left, up, up_left)),
                _ => line[i],
            };
        }
        out.extend_from_slice(&line[..rest.len().min(row)]);
        previous = line;
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (i16::from(a), i16::from(b), i16::from(c));
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

fn lzw(data: &[u8], early_change: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut table: Vec<Vec<u8>> = (0..=255u8).map(|b| vec![b]).collect();
    table.push(Vec::new()); // 256: clear
    table.push(Vec::new()); // 257: end
    let mut width = 9;
    let mut previous: Option<Vec<u8>> = None;
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in data {
        buffer = buffer << 8 | u32::from(byte);
        bits += 8;
        while bits >= width {
            let code = ((buffer >> (bits - width)) & ((1 << width) - 1)) as usize;
            bits -= width;
            match code {
                256 => {
                    table.truncate(258);
                    width = 9;
                    previous = None;
                    continue;
                }
                257 => return out,
                _ => {}
            }
            let entry = if code < table.len() {
                table[code].clone()
            } else if let Some(p) = &previous {
                let mut e = p.clone();
                e.push(p[0]);
                e
            } else {
                return out;
            };
            out.extend_from_slice(&entry);
            if let Some(p) = previous.take() {
                let mut e = p;
                e.push(entry[0]);
                table.push(e);
            }
            previous = Some(entry);
            let limit = table.len() + usize::from(early_change);
            width = match limit {
                0..=511 => 9,
                512..=1023 => 10,
                1024..=2047 => 11,
                _ => 12,
            };
        }
    }
    out
}

fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut high: Option<u8> = None;
    for &b in data {
        if b == b'>' {
            break;
        }
        let v = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => continue,
        };
        match high.take() {
            Some(h) => out.push(h << 4 | v),
            None => high = Some(v),
        }
    }
    if let Some(h) = high {
        out.push(h << 4);
    }
    out
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut group = [0u8; 5];
    let mut n = 0;
    let data = data.strip_prefix(b"<~").unwrap_or(data);
    for &b in data {
        match b {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0; 4]),
            b'!'..=b'u' => {
                group[n] = b - b'!';
                n += 1;
                if n == 5 {
                    let v = group.iter().fold(0u64, |v, &d| v * 85 + u64::from(d));
                    out.extend_from_slice(&(v as u32).to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for d in group.iter_mut().skip(n) {
            *d = 84;
        }
        let v = group.iter().fold(0u64, |v, &d| v * 85 + u64::from(d));
        out.extend_from_slice(&(v as u32).to_be_bytes()[..n - 1]);
    }
    out
}

fn run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let n = data[i];
        i += 1;
        match n {
            128 => break,
            0..=127 => {
                let end = (i + usize::from(n) + 1).min(data.len());
                out.extend_from_slice(&data[i..end]);
                i = end;
            }
            _ => {
                if let Some(&b) = data.get(i) {
                    out.extend(std::iter::repeat_n(b, 257 - usize::from(n)));
                }
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn flate_with_png_up_predictor() {
        // Two rows of three bytes, the second stored as differences ("up").
        let raw = [0, 1, 2, 3, 2, 1, 1, 1];
        let params = Dict(vec![
            (b"Predictor".to_vec(), Object::Int(12)),
            (b"Columns".to_vec(), Object::Int(3)),
        ]);
        let out = decode(&zlib(&raw), &[b"FlateDecode".to_vec()], &[Some(params)]).unwrap();
        assert_eq!(out, [1, 2, 3, 2, 3, 4]);
    }

    #[test]
    fn ascii_filters_and_run_length() {
        assert_eq!(ascii_hex(b"48 65 6C6C 6F>"), b"Hello");
        assert_eq!(ascii85(b"<~87cURD]j7BEbo7~>"), b"Hello world");
        assert_eq!(
            run_length(&[2, b'a', b'b', b'c', 254, b'z', 128]),
            b"abczzz"
        );
    }

    #[test]
    fn lzw_from_the_specification() {
        // §7.4.4.2 example: 2D 2D 2D 2D 2D 41 2D 2D 2D 42.
        let encoded = [0x80, 0x0b, 0x60, 0x50, 0x22, 0x0c, 0x0c, 0x85, 0x01];
        assert_eq!(
            lzw(&encoded, true),
            [45, 45, 45, 45, 45, 65, 45, 45, 45, 66]
        );
    }
}
