//! The file's structure (§7.5): cross-reference tables and streams,
//! incremental updates, object streams, and, for damaged files, finding
//! the objects by scanning the bytes as viewers do.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::PdfError;
use crate::crypt::Crypt;
use crate::filter;
use crate::lexer::{Lexer, Token, is_delimiter, is_whitespace};
use crate::object::{Dict, Object, Ref, Stream};

/// Where an object is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    Free,
    /// At this byte offset.
    At(usize),
    /// Inside the object stream with this number.
    InStream(u32),
}

/// Most objects one file may hold; a larger count means a broken table.
const MAX_OBJECTS: usize = 10_000_000;

pub struct File {
    data: Vec<u8>,
    /// Where `%PDF-` starts; offsets in some files count from there.
    header: usize,
    entries: HashMap<u32, Entry>,
    pub trailer: Dict,
    cache: RefCell<HashMap<u32, Rc<Object>>>,
    /// Objects being loaded, so a reference loop ends instead of recursing.
    loading: RefCell<Vec<u32>>,
    /// Object numbers found by scanning, for entries that point to the wrong place.
    scanned: RefCell<Option<HashMap<u32, usize>>>,
    crypt: Option<Crypt>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("File")
            .field("bytes", &self.data.len())
            .field("objects", &self.entries.len())
            .finish()
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

impl File {
    pub fn open(data: Vec<u8>, password: &[u8]) -> Result<File, PdfError> {
        let header = find(&data[..data.len().min(1024)], b"%PDF-", 0).ok_or(PdfError::NotAPdf)?;
        let mut file = File {
            data,
            header,
            entries: HashMap::new(),
            trailer: Dict::default(),
            cache: RefCell::new(HashMap::new()),
            loading: RefCell::new(Vec::new()),
            scanned: RefCell::new(None),
            crypt: None,
        };
        let read = file.read_xref_chain();
        if read.is_err() || !file.has_catalog() {
            file.reconstruct();
        }
        if !file.has_catalog() {
            return Err(PdfError::damaged("it has no document catalog"));
        }
        if let Some(encrypt) = file.trailer.get(b"Encrypt").cloned() {
            let encrypt = file.resolve(&encrypt)?;
            let dict = encrypt
                .as_dict()
                .ok_or_else(|| PdfError::damaged("its encryption dictionary is missing"))?
                .clone();
            let id = file
                .trailer
                .get(b"ID")
                .and_then(Object::as_array)
                .and_then(|a| a.first())
                .and_then(Object::as_bytes)
                .unwrap_or_default()
                .to_vec();
            let crypt = Crypt::new(&dict, &id, password)?;
            // Objects read so far (the encryption dictionary) were not encrypted.
            file.cache.borrow_mut().clear();
            file.crypt = Some(crypt);
        }
        Ok(file)
    }

    fn has_catalog(&self) -> bool {
        self.trailer
            .get(b"Root")
            .and_then(Object::as_ref)
            .and_then(|r| self.get(r).ok())
            .is_some_and(|o| o.as_dict().is_some())
    }

    // ------------------------------------------------------------ xref

    fn read_xref_chain(&mut self) -> Result<(), PdfError> {
        let tail = self.data.len().saturating_sub(2048);
        let at = rfind(&self.data[tail..], b"startxref")
            .map(|p| p + tail)
            .ok_or_else(|| PdfError::damaged("no startxref"))?;
        let mut lx = Lexer::at(&self.data, at + 9);
        let Some(Token::Int(start)) = lx.next_token() else {
            return Err(PdfError::damaged("startxref has no offset"));
        };
        let mut next = usize::try_from(start).ok();
        let mut seen = HashSet::new();
        let mut first = true;
        while let Some(offset) = next {
            if !seen.insert(offset) || seen.len() > 512 {
                break;
            }
            let trailer = self
                .read_xref_section(offset)
                .or_else(|_| self.read_xref_section(offset + self.header))?;
            if let Some(stm) = trailer.get(b"XRefStm").and_then(Object::as_int) {
                // A hybrid file: its compressed objects are listed here.
                let _ = self.read_xref_section(stm as usize);
            }
            if first {
                self.trailer = trailer.clone();
                first = false;
            } else {
                for (k, v) in &trailer.0 {
                    if !self.trailer.contains(k) && k != b"Prev" {
                        self.trailer.0.push((k.clone(), v.clone()));
                    }
                }
            }
            next = trailer
                .get(b"Prev")
                .and_then(Object::as_int)
                .and_then(|p| usize::try_from(p).ok());
        }
        Ok(())
    }

    /// Reads the table or stream at `offset`; returns its trailer.
    fn read_xref_section(&mut self, offset: usize) -> Result<Dict, PdfError> {
        let mut lx = Lexer::at(&self.data, offset);
        lx.skip_space();
        if self.data[lx.pos.min(self.data.len())..].starts_with(b"xref") {
            lx.pos += 4;
            return self.read_xref_table(lx.pos);
        }
        let (_, object) = self.parse_at(offset)?;
        let stream = object
            .as_stream()
            .filter(|s| s.dict.name(b"Type") == Some(b"XRef"))
            .ok_or_else(|| PdfError::damaged("no cross-reference table where it should be"))?
            .clone();
        self.read_xref_stream(&stream)?;
        Ok(stream.dict.clone())
    }

    fn read_xref_table(&mut self, pos: usize) -> Result<Dict, PdfError> {
        let mut lx = Lexer::at(&self.data, pos);
        loop {
            lx.skip_space();
            if self.data[lx.pos.min(self.data.len())..].starts_with(b"trailer") {
                lx.pos += 7;
                return match lx.object(true)? {
                    Object::Dict(d) => Ok(d),
                    _ => Err(PdfError::damaged("the trailer is not a dictionary")),
                };
            }
            let (Some(Token::Int(start)), Some(Token::Int(count))) =
                (lx.next_token(), lx.next_token())
            else {
                return Err(PdfError::damaged("a cross-reference table is broken"));
            };
            let (Ok(mut start), Ok(count)) = (u32::try_from(start), usize::try_from(count)) else {
                return Err(PdfError::damaged("a cross-reference table is broken"));
            };
            if count > MAX_OBJECTS {
                return Err(PdfError::damaged("a cross-reference table is broken"));
            }
            for i in 0..count {
                let (
                    Some(Token::Int(offset)),
                    Some(Token::Int(generation)),
                    Some(Token::Keyword(kind)),
                ) = (lx.next_token(), lx.next_token(), lx.next_token())
                else {
                    return Err(PdfError::damaged("a cross-reference entry is broken"));
                };
                // A common mistake: a table starting at 1 whose first entry is
                // the free head of the list, object 0.
                if i == 0 && start == 1 && kind == b"f" && generation == 65535 {
                    start = 0;
                }
                let num = start + i as u32;
                let entry = match kind {
                    b"n" if offset > 0 => Entry::At(offset as usize),
                    _ => Entry::Free,
                };
                self.entries.entry(num).or_insert(entry);
            }
        }
    }

    fn read_xref_stream(&mut self, stream: &Stream) -> Result<(), PdfError> {
        let d = &stream.dict;
        let widths: Vec<usize> = d
            .get(b"W")
            .and_then(Object::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Object::as_int)
                    .map(|w| w.clamp(0, 8) as usize)
                    .collect()
            })
            .unwrap_or_default();
        if widths.len() < 3 {
            return Err(PdfError::damaged(
                "a cross-reference stream has no field widths",
            ));
        }
        let size = d.get(b"Size").and_then(Object::as_int).unwrap_or(0).max(0) as u32;
        let index: Vec<(u32, u32)> = match d.get(b"Index").and_then(Object::as_array) {
            Some(a) => a
                .chunks_exact(2)
                .filter_map(|p| Some((p[0].as_int()? as u32, p[1].as_int()? as u32)))
                .collect(),
            None => vec![(0, size)],
        };
        let data = self.decode_raw(stream)?;
        let row: usize = widths.iter().sum();
        if row == 0 {
            return Ok(());
        }
        let field = |bytes: &[u8]| bytes.iter().fold(0u64, |v, &b| v << 8 | u64::from(b));
        let mut rows = data.chunks_exact(row);
        for (start, count) in index {
            for i in 0..count {
                let Some(r) = rows.next() else {
                    return Ok(());
                };
                let (a, rest) = r.split_at(widths[0]);
                let (b, c) = rest.split_at(widths[1]);
                let kind = if widths[0] == 0 { 1 } else { field(a) };
                let entry = match kind {
                    0 => Entry::Free,
                    1 => Entry::At(field(b) as usize),
                    2 => Entry::InStream(field(b) as u32),
                    _ => continue,
                };
                let _ = c;
                self.entries.entry(start + i).or_insert(entry);
            }
        }
        Ok(())
    }

    /// Finds every `n g obj` in the file, and the trailer, as if there were
    /// no cross-reference table.
    fn reconstruct(&mut self) {
        let offsets = scan_objects(&self.data);
        self.entries = offsets.iter().map(|(&n, &o)| (n, Entry::At(o))).collect();
        self.cache.borrow_mut().clear();
        let mut trailer = Dict::default();
        let mut pos = 0;
        while let Some(p) = find(&self.data, b"trailer", pos) {
            pos = p + 7;
            if let Ok(Object::Dict(d)) = Lexer::at(&self.data, pos).object(true) {
                for (k, v) in d.0 {
                    trailer.set(&k, v);
                }
            }
        }
        // Files with cross-reference streams keep the trailer in those, and
        // most objects in object streams.
        let mut in_streams = Vec::new();
        let mut numbers: Vec<u32> = offsets.keys().copied().collect();
        numbers.sort_unstable();
        for num in numbers {
            let Ok((_, object)) = self.parse_at(offsets[&num]) else {
                continue;
            };
            let Some(stream) = object.as_stream() else {
                continue;
            };
            match stream.dict.name(b"Type") {
                Some(b"XRef") => {
                    for key in [&b"Root"[..], b"Info", b"Encrypt", b"ID"] {
                        if let Some(v) = stream.dict.get(key) {
                            trailer.set(key, v.clone());
                        }
                    }
                }
                Some(b"ObjStm") => in_streams.push((num, stream.clone())),
                _ => {}
            }
        }
        for (num, stream) in in_streams {
            if let Ok(listed) = self.object_stream_numbers(&stream) {
                for n in listed {
                    self.entries.entry(n).or_insert(Entry::InStream(num));
                }
            }
        }
        if !trailer.contains(b"Root") {
            let catalog = self.entries.keys().copied().find(|&n| {
                self.get(Ref {
                    num: n,
                    generation: 0,
                })
                .ok()
                .is_some_and(|o| {
                    o.as_dict()
                        .is_some_and(|d| d.name(b"Type") == Some(b"Catalog"))
                })
            });
            if let Some(n) = catalog {
                trailer.set(
                    b"Root",
                    Object::Ref(Ref {
                        num: n,
                        generation: 0,
                    }),
                );
            }
        }
        self.trailer = trailer;
        self.cache.borrow_mut().clear();
    }

    // ------------------------------------------------------------ objects

    /// The object `n g obj … endobj` at `offset`, with its number.
    fn parse_at(&self, offset: usize) -> Result<(u32, Object), PdfError> {
        let mut lx = Lexer::at(&self.data, offset);
        let (Some(Token::Int(num)), Some(Token::Int(_)), Some(Token::Keyword(b"obj"))) =
            (lx.next_token(), lx.next_token(), lx.next_token())
        else {
            return Err(PdfError::damaged("an object is not where the file says"));
        };
        let num = u32::try_from(num).map_err(|_| PdfError::damaged("a bad object number"))?;
        let object = lx.object(true)?;
        if let (Some(Token::Keyword(b"stream")), Object::Dict(dict)) = (lx.next_token(), &object) {
            let mut start = lx.pos;
            if self.data.get(start) == Some(&b'\r') {
                start += 1;
            }
            if self.data.get(start) == Some(&b'\n') {
                start += 1;
            }
            let declared = dict.get(b"Length").and_then(|l| match l {
                Object::Ref(r) if !self.loading.borrow().contains(&r.num) => {
                    self.get(*r).ok().and_then(|o| o.as_int())
                }
                other => other.as_int(),
            });
            let end = declared
                .and_then(|len| usize::try_from(len).ok())
                .map(|len| start + len)
                .filter(|&end| end <= self.data.len() && self.endstream_at(end))
                .or_else(|| {
                    // A wrong /Length: the data ends at "endstream".
                    let e = find(&self.data, b"endstream", start)?;
                    let mut end = e;
                    if end > start && self.data[end - 1] == b'\n' {
                        end -= 1;
                    }
                    if end > start && self.data[end - 1] == b'\r' {
                        end -= 1;
                    }
                    Some(end)
                })
                .ok_or_else(|| PdfError::damaged("a stream has no end"))?;
            let stream = Stream {
                dict: dict.clone(),
                data: self.data[start..end].to_vec(),
            };
            return Ok((num, Object::Stream(Rc::new(stream))));
        }
        Ok((num, object))
    }

    fn endstream_at(&self, pos: usize) -> bool {
        let mut lx = Lexer::at(&self.data, pos);
        lx.skip_space();
        self.data[lx.pos.min(self.data.len())..].starts_with(b"endstream")
    }

    /// The object a reference points to; `null` if it doesn't exist.
    pub fn get(&self, r: Ref) -> Result<Rc<Object>, PdfError> {
        if let Some(o) = self.cache.borrow().get(&r.num) {
            return Ok(o.clone());
        }
        if self.loading.borrow().contains(&r.num) || self.loading.borrow().len() > 64 {
            return Ok(Rc::new(Object::Null));
        }
        self.loading.borrow_mut().push(r.num);
        let loaded = self.load(r);
        self.loading.borrow_mut().pop();
        let object = Rc::new(loaded?);
        self.cache.borrow_mut().insert(r.num, object.clone());
        Ok(object)
    }

    fn load(&self, r: Ref) -> Result<Object, PdfError> {
        match self.entries.get(&r.num).copied() {
            Some(Entry::At(offset)) => {
                let found = [offset, offset + self.header]
                    .into_iter()
                    .find_map(|o| self.parse_at(o).ok().filter(|(n, _)| *n == r.num))
                    .or_else(|| {
                        let o = *self.scan_index().get(&r.num)?;
                        self.parse_at(o).ok()
                    });
                match found {
                    Some((_, object)) => Ok(self.decrypt(object, r)),
                    None => Ok(Object::Null),
                }
            }
            Some(Entry::InStream(stream)) => self.load_from_object_stream(stream, r.num),
            Some(Entry::Free) | None => Ok(Object::Null),
        }
    }

    fn scan_index(&self) -> HashMap<u32, usize> {
        self.scanned
            .borrow_mut()
            .get_or_insert_with(|| scan_objects(&self.data))
            .clone()
    }

    fn decrypt(&self, object: Object, r: Ref) -> Object {
        match &self.crypt {
            Some(c) => c.decrypt_object(object, r),
            None => object,
        }
    }

    fn object_stream_numbers(&self, stream: &Stream) -> Result<Vec<u32>, PdfError> {
        let data = self.decode_raw(stream)?;
        let n = stream
            .dict
            .get(b"N")
            .and_then(Object::as_int)
            .unwrap_or(0)
            .clamp(0, 1 << 20);
        let mut lx = Lexer::new(&data);
        let mut out = Vec::new();
        for _ in 0..n {
            match (lx.next_token(), lx.next_token()) {
                (Some(Token::Int(num)), Some(Token::Int(_))) => out.push(num as u32),
                _ => break,
            }
        }
        Ok(out)
    }

    /// Loads object `num` from object stream `stream`, caching the others
    /// it holds on the way.
    fn load_from_object_stream(&self, stream: u32, num: u32) -> Result<Object, PdfError> {
        let holder = self.get(Ref {
            num: stream,
            generation: 0,
        })?;
        let Some(s) = holder.as_stream() else {
            return Ok(Object::Null);
        };
        let data = self.decode(s)?;
        let n = s
            .dict
            .get(b"N")
            .and_then(Object::as_int)
            .unwrap_or(0)
            .clamp(0, 1 << 20);
        let first = s
            .dict
            .get(b"First")
            .and_then(Object::as_int)
            .unwrap_or(0)
            .max(0) as usize;
        let mut lx = Lexer::new(&data);
        let mut pairs = Vec::new();
        for _ in 0..n {
            match (lx.next_token(), lx.next_token()) {
                (Some(Token::Int(o)), Some(Token::Int(off))) => {
                    pairs.push((o as u32, off.max(0) as usize))
                }
                _ => break,
            }
        }
        let mut found = Object::Null;
        for (o, off) in pairs {
            let object = Lexer::at(&data, first + off)
                .object(true)
                .unwrap_or(Object::Null);
            if o == num {
                found = object;
            } else if self.entries.get(&o) == Some(&Entry::InStream(stream)) {
                self.cache
                    .borrow_mut()
                    .entry(o)
                    .or_insert_with(|| Rc::new(object));
            }
        }
        Ok(found)
    }

    /// Follows a reference; other objects are returned as they are.
    pub fn resolve(&self, object: &Object) -> Result<Rc<Object>, PdfError> {
        match object {
            Object::Ref(r) => self.get(*r),
            other => Ok(Rc::new(other.clone())),
        }
    }

    /// A dictionary entry, with references followed.
    pub fn dict_get(&self, dict: &Dict, key: &[u8]) -> Option<Rc<Object>> {
        dict.get(key).and_then(|o| self.resolve(o).ok())
    }

    /// A stream's data, decoded.
    pub fn decode(&self, stream: &Stream) -> Result<Vec<u8>, PdfError> {
        self.decode_raw(stream)
    }

    fn decode_raw(&self, stream: &Stream) -> Result<Vec<u8>, PdfError> {
        let resolve = |o: &Object| -> Object {
            match o {
                Object::Ref(r) if !self.loading.borrow().contains(&r.num) => {
                    self.get(*r).map(|o| (*o).clone()).unwrap_or(Object::Null)
                }
                other => other.clone(),
            }
        };
        let filter = stream.dict.get(b"Filter").map(resolve);
        let params = stream
            .dict
            .get(b"DecodeParms")
            .or_else(|| stream.dict.get(b"DP"))
            .map(resolve);
        let filters: Vec<Vec<u8>> = match &filter {
            Some(Object::Name(n)) => vec![n.clone()],
            Some(Object::Array(a)) => a
                .iter()
                .filter_map(|f| resolve(f).as_name().map(<[u8]>::to_vec))
                .collect(),
            _ => Vec::new(),
        };
        let params: Vec<Option<Dict>> = match &params {
            Some(Object::Dict(d)) => vec![Some(d.clone())],
            Some(Object::Array(a)) => a
                .iter()
                .map(|p| match resolve(p) {
                    Object::Dict(d) => Some(d),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        filter::decode(&stream.data, &filters, &params)
    }
}

/// Every `n g obj` in the file: object number → offset (the last one wins).
fn scan_objects(data: &[u8]) -> HashMap<u32, usize> {
    let mut found = HashMap::new();
    let mut pos = 0;
    while let Some(p) = find(data, b"obj", pos) {
        pos = p + 3;
        // The keyword alone: not "endobj", nor "objects".
        let before_ok = p == 0 || is_whitespace(data[p - 1]);
        let after_ok = data
            .get(p + 3)
            .is_none_or(|&b| is_whitespace(b) || is_delimiter(b));
        if !before_ok || !after_ok {
            continue;
        }
        // Back over "gen" and "num".
        let mut i = p;
        let mut fields = [0u64; 2];
        let mut ok = true;
        for field in fields.iter_mut().rev() {
            while i > 0 && is_whitespace(data[i - 1]) {
                i -= 1;
            }
            let end = i;
            while i > 0 && data[i - 1].is_ascii_digit() {
                i -= 1;
            }
            if i == end || end - i > 10 {
                ok = false;
                break;
            }
            *field = std::str::from_utf8(&data[i..end])
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(u64::MAX);
        }
        if ok
            && (i == 0 || is_whitespace(data[i - 1]))
            && let Ok(num) = u32::try_from(fields[0])
        {
            found.insert(num, i);
        }
    }
    found
}
