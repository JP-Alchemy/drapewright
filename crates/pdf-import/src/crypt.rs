//! The standard security handler (ISO 32000-2 §7.6.4). Many sewing
//! patterns are "protected" PDFs that open without a password; they are
//! encrypted with an empty user password, and read here exactly as any
//! viewer reads them. A file that needs a password to open is refused.

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Aes256};
use md5::{Digest, Md5};
use sha2::{Sha256, Sha384, Sha512};

use crate::PdfError;
use crate::object::{Dict, Object, Ref, Stream};

const PAD: [u8; 32] = [
    0x28, 0xbf, 0x4e, 0x5e, 0x4e, 0x75, 0x8a, 0x41, 0x64, 0x00, 0x4e, 0x56, 0xff, 0xfa, 0x01, 0x08,
    0x2e, 0x2e, 0x00, 0xb6, 0xd0, 0x68, 0x3e, 0x80, 0x2f, 0x0c, 0xa9, 0xfe, 0x64, 0x53, 0x69, 0x7a,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Identity,
    Rc4,
    Aes128,
    Aes256,
}

#[derive(Clone, Debug)]
pub struct Crypt {
    key: Vec<u8>,
    strings: Method,
    streams: Method,
    encrypt_metadata: bool,
}

fn needs_password() -> PdfError {
    PdfError::Encrypted("This PDF needs a password to open. Open it in a PDF viewer, save a copy without a password, and import that.".into())
}

impl Crypt {
    pub fn new(dict: &Dict, id: &[u8], password: &[u8]) -> Result<Crypt, PdfError> {
        if dict.name(b"Filter") != Some(b"Standard") {
            return Err(PdfError::Encrypted(
                "This PDF is locked with a kind of protection Drapewright cannot open.".into(),
            ));
        }
        let int = |k: &[u8], d: i64| dict.get(k).and_then(Object::as_int).unwrap_or(d);
        let bytes = |k: &[u8]| {
            dict.get(k)
                .and_then(Object::as_bytes)
                .unwrap_or_default()
                .to_vec()
        };
        let (v, r) = (int(b"V", 0), int(b"R", 2));
        let encrypt_metadata = dict
            .get(b"EncryptMetadata")
            .and_then(Object::as_bool)
            .unwrap_or(true);
        let length_bits = int(b"Length", 40);
        let (strings, streams, key_len) = match v {
            1 => (Method::Rc4, Method::Rc4, 5),
            2 | 3 => (
                Method::Rc4,
                Method::Rc4,
                (length_bits / 8).clamp(5, 16) as usize,
            ),
            4 | 5 => {
                let filters = dict.get(b"CF").and_then(Object::as_dict);
                let method = |name: Option<&[u8]>| -> Result<(Method, usize), PdfError> {
                    let name = name.unwrap_or(b"Identity");
                    if name == b"Identity" {
                        return Ok((Method::Identity, 16));
                    }
                    let f = filters
                        .and_then(|cf| cf.get(name))
                        .and_then(Object::as_dict)
                        .ok_or_else(|| PdfError::damaged("an encryption filter is missing"))?;
                    Ok(match f.name(b"CFM") {
                        Some(b"V2") => {
                            let len = f
                                .get(b"Length")
                                .and_then(Object::as_int)
                                .unwrap_or(length_bits);
                            // Written in bytes by some, bits by others.
                            let len = if len > 32 { len / 8 } else { len };
                            (Method::Rc4, len.clamp(5, 16) as usize)
                        }
                        Some(b"AESV2") => (Method::Aes128, 16),
                        Some(b"AESV3") => (Method::Aes256, 32),
                        Some(b"None") | None => (Method::Identity, 16),
                        Some(other) => {
                            return Err(PdfError::Encrypted(format!(
                                "This PDF is encrypted with {}, which Drapewright cannot open.",
                                String::from_utf8_lossy(other)
                            )));
                        }
                    })
                };
                let (strings, a) = method(dict.name(b"StrF"))?;
                let (streams, b) = method(dict.name(b"StmF"))?;
                (strings, streams, a.max(b))
            }
            _ => {
                return Err(PdfError::Encrypted(
                    "This PDF is locked with a kind of protection Drapewright cannot open.".into(),
                ));
            }
        };
        let (o, u) = (bytes(b"O"), bytes(b"U"));
        let key = if r >= 5 {
            key_aes256(password, &u, &bytes(b"UE"), r).ok_or_else(needs_password)?
        } else {
            let p = int(b"P", -1) as i32 as u32;
            let key = key_rc4(password, &o, p, id, key_len, r, encrypt_metadata);
            if !user_password_ok(&key, &u, id, r) {
                return Err(needs_password());
            }
            key
        };
        Ok(Crypt {
            key,
            strings,
            streams,
            encrypt_metadata,
        })
    }

    /// The key for one object (Algorithm 1); AES-256 uses the file key.
    fn object_key(&self, r: Ref, method: Method) -> Vec<u8> {
        if method == Method::Aes256 {
            return self.key.clone();
        }
        let mut h = Md5::new();
        h.update(&self.key);
        h.update(&r.num.to_le_bytes()[..3]);
        h.update(r.generation.to_le_bytes());
        if method == Method::Aes128 {
            h.update(b"sAlT");
        }
        let digest = h.finalize();
        digest[..(self.key.len() + 5).min(16)].to_vec()
    }

    fn decrypt(&self, data: &[u8], r: Ref, method: Method) -> Vec<u8> {
        match method {
            Method::Identity => data.to_vec(),
            Method::Rc4 => rc4(&self.object_key(r, method), data),
            Method::Aes128 | Method::Aes256 => aes_cbc_decrypt(&self.object_key(r, method), data),
        }
    }

    /// Decrypts the strings and stream data of object `r`.
    pub fn decrypt_object(&self, object: Object, r: Ref) -> Object {
        match object {
            Object::String(s) => Object::String(self.decrypt(&s, r, self.strings)),
            Object::Array(items) => Object::Array(
                items
                    .into_iter()
                    .map(|o| self.decrypt_object(o, r))
                    .collect(),
            ),
            Object::Dict(d) => Object::Dict(self.decrypt_dict(d, r)),
            Object::Stream(s) => {
                let identity = matches!(
                    s.dict.get(b"Filter"),
                    Some(Object::Name(n)) if n == b"Crypt"
                ) || s
                    .dict
                    .get(b"Filter")
                    .and_then(Object::as_array)
                    .is_some_and(|a| a.first().and_then(Object::as_name) == Some(b"Crypt"));
                let plain = s.dict.name(b"Type") == Some(b"XRef")
                    || (s.dict.name(b"Type") == Some(b"Metadata") && !self.encrypt_metadata)
                    || identity;
                let data = if plain {
                    s.data.clone()
                } else {
                    self.decrypt(&s.data, r, self.streams)
                };
                Object::Stream(std::rc::Rc::new(Stream {
                    dict: self.decrypt_dict(s.dict.clone(), r),
                    data,
                }))
            }
            other => other,
        }
    }

    fn decrypt_dict(&self, d: Dict, r: Ref) -> Dict {
        Dict(
            d.0.into_iter()
                .map(|(k, v)| (k, self.decrypt_object(v, r)))
                .collect(),
        )
    }
}

fn padded(password: &[u8]) -> [u8; 32] {
    let mut out = PAD;
    let n = password.len().min(32);
    out[..n].copy_from_slice(&password[..n]);
    out[n..].copy_from_slice(&PAD[..32 - n]);
    out
}

/// Algorithm 2: the file key from the user password (R2–R4).
fn key_rc4(
    password: &[u8],
    o: &[u8],
    p: u32,
    id: &[u8],
    len: usize,
    r: i64,
    metadata: bool,
) -> Vec<u8> {
    let mut h = Md5::new();
    h.update(padded(password));
    h.update(o);
    h.update(p.to_le_bytes());
    h.update(id);
    if r >= 4 && !metadata {
        h.update([0xff; 4]);
    }
    let mut digest = h.finalize().to_vec();
    if r >= 3 {
        for _ in 0..50 {
            digest = Md5::digest(&digest[..len]).to_vec();
        }
    }
    digest.truncate(len);
    digest
}

/// Algorithms 4 and 5: whether `key` opens the file, checked against /U.
fn user_password_ok(key: &[u8], u: &[u8], id: &[u8], r: i64) -> bool {
    if r == 2 {
        return u.len() >= 32 && rc4(key, &PAD) == u[..32];
    }
    let mut h = Md5::new();
    h.update(PAD);
    h.update(id);
    let mut x = rc4(key, &h.finalize());
    for i in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|b| b ^ i).collect();
        x = rc4(&k, &x);
    }
    u.len() >= 16 && x[..16] == u[..16]
}

/// Algorithm 2.A: the file key for AES-256 (R5, R6), from /U and /UE.
fn key_aes256(password: &[u8], u: &[u8], ue: &[u8], r: i64) -> Option<Vec<u8>> {
    if u.len() < 48 || ue.len() < 32 {
        return None;
    }
    let password = &password[..password.len().min(127)];
    let (hash, validation, key_salt) = (&u[..32], &u[32..40], &u[40..48]);
    let check = if r == 5 {
        Sha256::digest([password, validation].concat()).to_vec()
    } else {
        hash_2b(password, validation, &[])
    };
    if check != hash {
        return None;
    }
    let intermediate = if r == 5 {
        Sha256::digest([password, key_salt].concat()).to_vec()
    } else {
        hash_2b(password, key_salt, &[])
    };
    let cipher = Aes256::new_from_slice(&intermediate).ok()?;
    // Decrypted with no initial vector: CBC from a zero IV.
    let mut previous = [0u8; 16];
    let mut key = Vec::with_capacity(32);
    for chunk in ue[..32].chunks_exact(16) {
        let mut block = aes::Block::try_from(chunk).ok()?;
        cipher.decrypt_block(&mut block);
        key.extend(block.iter().zip(previous).map(|(b, p)| b ^ p));
        previous.copy_from_slice(chunk);
    }
    Some(key)
}

/// Algorithm 2.B: the hash used by revision 6.
fn hash_2b(password: &[u8], salt: &[u8], udata: &[u8]) -> Vec<u8> {
    let mut k = Sha256::digest([password, salt, udata].concat()).to_vec();
    let mut round = 0usize;
    loop {
        let k1: Vec<u8> = [password, &k, udata].concat().repeat(64);
        let e = aes128_cbc_encrypt(&k[..16], &k[16..32], &k1);
        let sum: u32 = e[..16].iter().map(|&b| u32::from(b)).sum();
        k = match sum % 3 {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            _ => Sha512::digest(&e).to_vec(),
        };
        round += 1;
        if round >= 64 && usize::from(*e.last().unwrap_or(&0)) + 32 <= round {
            break;
        }
    }
    k.truncate(32);
    k
}

fn aes128_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    let Ok(cipher) = Aes128::new_from_slice(key) else {
        return Vec::new();
    };
    let mut previous: Vec<u8> = iv.to_vec();
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(16) {
        let mixed: Vec<u8> = chunk.iter().zip(&previous).map(|(a, b)| a ^ b).collect();
        let Ok(mut block) = aes::Block::try_from(mixed.as_slice()) else {
            break;
        };
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
        previous = block.to_vec();
    }
    out
}

/// AES-CBC with the IV in the first 16 bytes and PKCS#5 padding.
fn aes_cbc_decrypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    if data.len() < 32 {
        return Vec::new();
    }
    let (iv, body) = data.split_at(16);
    let body = &body[..body.len() - body.len() % 16];
    let mut out = Vec::with_capacity(body.len());
    let mut previous = iv.to_vec();
    let mut run = |decrypt: &dyn Fn(&mut aes::Block)| {
        for chunk in body.chunks_exact(16) {
            let Ok(mut block) = aes::Block::try_from(chunk) else {
                break;
            };
            decrypt(&mut block);
            out.extend(block.iter().zip(&previous).map(|(b, p)| b ^ p));
            previous = chunk.to_vec();
        }
    };
    if key.len() == 32 {
        if let Ok(c) = Aes256::new_from_slice(key) {
            run(&|b| c.decrypt_block(b));
        }
    } else if let Ok(c) = Aes128::new_from_slice(key) {
        run(&|b| c.decrypt_block(b));
    }
    if let Some(&pad) = out.last() {
        let pad = usize::from(pad);
        if (1..=16).contains(&pad)
            && pad <= out.len()
            && out[out.len() - pad..]
                .iter()
                .all(|&b| usize::from(b) == pad)
        {
            out.truncate(out.len() - pad);
        }
    }
    out
}

fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, usize::from(j));
    }
    let (mut i, mut j) = (0u8, 0u8);
    data.iter()
        .map(|&b| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[usize::from(i)]);
            s.swap(usize::from(i), usize::from(j));
            b ^ s[usize::from(s[usize::from(i)].wrapping_add(s[usize::from(j)]))]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_test_vector() {
        // RFC 6229-style vector: key "Key", plaintext "Plaintext".
        assert_eq!(
            rc4(b"Key", b"Plaintext"),
            [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3]
        );
    }

    #[test]
    fn aes_round_trips_through_cbc() {
        let key = [7u8; 16];
        let iv = [9u8; 16];
        let plain = b"0123456789abcdef0123456789abcdef";
        let encrypted = aes128_cbc_encrypt(&key, &iv, plain);
        let mut stored = iv.to_vec();
        stored.extend(encrypted);
        // Padding a whole block of 16s, as PDF writers do.
        let pad = aes128_cbc_encrypt(&key, &stored[stored.len() - 16..], &[16u8; 16]);
        stored.extend(pad);
        assert_eq!(aes_cbc_decrypt(&key, &stored), plain);
    }
}
