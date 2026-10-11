#!/usr/bin/env python3
"""Writes AES-encrypted copies of sizes-layers.pdf, "protected" the way
Acrobat protects PDFs: an owner password only, so they open without one.
CC0: made for Drapewright's tests.

Run from the repository root after make_fixtures.py:
    python3 fixtures/pdf/protect.py

- sizes-layers-aes128.pdf: AES-128 (security handler revision 4)
- sizes-layers-aes256.pdf: AES-256 (revision 6, what Acrobat X+ writes)
- sizes-layers-password.pdf: AES-128 with a password needed to open it
  ("open sesame"), which Drapewright refuses with a plain message

Uses the `openssl` command for AES. Fixed salts and IVs keep the files the
same on every run. Check them with poppler: `pdfinfo` says encrypted, and
`pdftotext` reads the words.
"""
import hashlib
import os
import re
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
PAD = bytes.fromhex("28bf4e5e4e758a4164004e56fffa01082e2e00b6d0683e802f0ca9fe6453697a")
OWNER = b"drapewright"
P = -3900  # print allowed; copy and change not
ID0 = hashlib.md5(b"drapewright sizes").digest()


def aes(mode, key, data, iv=None, pad=True):
    args = ["openssl", "enc", f"-aes-{len(key) * 8}-{mode}", "-K", key.hex()]
    if iv is not None:
        args += ["-iv", iv.hex()]
    if not pad:
        args.append("-nopad")
    return subprocess.run(args, input=data, capture_output=True, check=True).stdout


def rc4(key, data):
    s = list(range(256))
    j = 0
    for i in range(256):
        j = (j + s[i] + key[i % len(key)]) % 256
        s[i], s[j] = s[j], s[i]
    out, i, j = bytearray(), 0, 0
    for b in data:
        i = (i + 1) % 256
        j = (j + s[i]) % 256
        s[i], s[j] = s[j], s[i]
        out.append(b ^ s[(s[i] + s[j]) % 256])
    return bytes(out)


def padded(pw):
    return (pw + PAD)[:32]


def hash_2b(password, salt, udata):
    k = hashlib.sha256(password + salt + udata).digest()
    i = 0
    while True:
        k1 = (password + k + udata) * 64
        e = aes("cbc", k[:16], k1, iv=k[16:32], pad=False)
        k = {0: hashlib.sha256, 1: hashlib.sha384, 2: hashlib.sha512}[sum(e[:16]) % 3](e).digest()
        i += 1
        if i >= 64 and e[-1] <= i - 32:
            return k[:32]


def objects(pdf):
    """(number, body) of each object, in order."""
    return [(int(m.group(1)), m.group(2)) for m in re.finditer(rb"(\d+) 0 obj\n(.*?)\nendobj\n", pdf, re.S)]


def encrypt_object(body, encrypt):
    """Encrypts a body's literal strings and stream data with `encrypt`."""
    m = re.match(rb"(<<.*?>>)\nstream\n(.*)\nendstream$", body, re.S)
    if m:
        data = encrypt(m.group(2))
        head = re.sub(rb"/Length \d+", b"/Length " + str(len(data)).encode(), m.group(1))
        return head + b"\nstream\n" + data + b"\nendstream"
    return re.sub(rb"\(([^)]*)\)", lambda s: b"<" + encrypt(s.group(1)).hex().encode() + b">", body)


def write(name, objs, encrypt_dict):
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}
    for num, body in objs:
        offsets[num] = len(out)
        out += f"{num} 0 obj\n".encode() + body + b"\nendobj\n"
    enc = max(offsets) + 1
    offsets[enc] = len(out)
    out += f"{enc} 0 obj\n".encode() + encrypt_dict + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {enc + 1}\n0000000000 65535 f \n".encode()
    for n in range(1, enc + 1):
        out += f"{offsets[n]:010d} 00000 n \n".encode()
    out += (
        f"trailer\n<< /Size {enc + 1} /Root 1 0 R /Encrypt {enc} 0 R "
        f"/ID [<{ID0.hex()}> <{ID0.hex()}>] >>\nstartxref\n{xref}\n%%EOF\n"
    ).encode()
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(out)
    print("wrote", name, len(out), "bytes")


def aes128(objs, user=b"", name="sizes-layers-aes128.pdf"):
    # Algorithm 3: /O from the owner password.
    key = hashlib.md5(padded(OWNER)).digest()
    for _ in range(50):
        key = hashlib.md5(key).digest()
    o = rc4(key, padded(user))
    for i in range(1, 20):
        o = rc4(bytes(b ^ i for b in key), o)
    # Algorithm 2: the file key from the user password.
    h = hashlib.md5(padded(user) + o + P.to_bytes(4, "little", signed=True) + ID0).digest()
    for _ in range(50):
        h = hashlib.md5(h).digest()
    file_key = h[:16]
    # Algorithm 5: /U.
    u = rc4(file_key, hashlib.md5(PAD + ID0).digest())
    for i in range(1, 20):
        u = rc4(bytes(b ^ i for b in file_key), u)
    u += bytes(16)

    def for_object(num):
        k = hashlib.md5(file_key + num.to_bytes(3, "little") + bytes(2) + b"sAlT").digest()[:16]
        iv = hashlib.md5(b"iv" + num.to_bytes(4, "little")).digest()
        return lambda data: iv + aes("cbc", k, data, iv=iv)

    out = [(n, encrypt_object(b, for_object(n))) for n, b in objs]
    d = (
        b"<< /Filter /Standard /V 4 /R 4 /Length 128 "
        b"/CF << /StdCF << /AuthEvent /DocOpen /CFM /AESV2 /Length 16 >> >> /StmF /StdCF /StrF /StdCF "
        + f"/P {P} /O <{o.hex()}> /U <{u.hex()}> >>".encode()
    )
    write(name, out, d)


def aes256(objs):
    file_key = hashlib.sha256(b"drapewright file key").digest()
    u_val, u_key = b"uvalsalt", b"ukeysalt"
    u = hash_2b(b"", u_val, b"") + u_val + u_key
    ue = aes("cbc", hash_2b(b"", u_key, b""), file_key, iv=bytes(16), pad=False)
    o_val, o_key = b"ovalsalt", b"okeysalt"
    o = hash_2b(OWNER, o_val, u) + o_val + o_key
    oe = aes("cbc", hash_2b(OWNER, o_key, u), file_key, iv=bytes(16), pad=False)
    perms = aes("ecb", file_key, P.to_bytes(4, "little", signed=True) + b"\xff\xff\xff\xffTadb" + b"perm", pad=False)

    def for_object(num):
        iv = hashlib.md5(b"iv" + num.to_bytes(4, "little")).digest()
        return lambda data: iv + aes("cbc", file_key, data, iv=iv)

    out = [(n, encrypt_object(b, for_object(n))) for n, b in objs]
    d = (
        b"<< /Filter /Standard /V 5 /R 6 /Length 256 "
        b"/CF << /StdCF << /AuthEvent /DocOpen /CFM /AESV3 /Length 32 >> >> /StmF /StdCF /StrF /StdCF "
        + f"/P {P} /O <{o.hex()}> /U <{u.hex()}> /OE <{oe.hex()}> /UE <{ue.hex()}> /Perms <{perms.hex()}> >>".encode()
    )
    write("sizes-layers-aes256.pdf", out, d)


if __name__ == "__main__":
    with open(os.path.join(HERE, "sizes-layers.pdf"), "rb") as f:
        objs = objects(f.read())
    aes128(objs)
    aes256(objs)
    aes128(objs, user=b"open sesame", name="sizes-layers-password.pdf")
