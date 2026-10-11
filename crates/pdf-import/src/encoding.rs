//! Character codes to text for fonts without a ToUnicode map: the standard
//! encodings (Annex D) and glyph names from the Adobe Glyph List that
//! patterns use (Latin letters, digits and punctuation).

/// The base encodings a simple font can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Standard,
    WinAnsi,
    MacRoman,
}

const WIN_ANSI_HIGH: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    Some('ƒ'),
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    Some('ˆ'),
    Some('‰'),
    Some('Š'),
    Some('‹'),
    Some('Œ'),
    None,
    Some('Ž'),
    None,
    None,
    Some('‘'),
    Some('’'),
    Some('“'),
    Some('”'),
    Some('•'),
    Some('–'),
    Some('—'),
    Some('˜'),
    Some('™'),
    Some('š'),
    Some('›'),
    Some('œ'),
    None,
    Some('ž'),
    Some('Ÿ'),
];

const MAC_ROMAN_HIGH: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";

/// The character `code` stands for in `base`.
pub fn base_char(base: Base, code: u8) -> Option<char> {
    match (base, code) {
        (_, 0..=31) | (_, 127) => None,
        (Base::Standard, 0x27) => Some('’'),
        (Base::Standard, 0x60) => Some('‘'),
        (_, 32..=126) => Some(char::from(code)),
        (Base::WinAnsi, 0x80..=0x9f) => WIN_ANSI_HIGH[usize::from(code - 0x80)],
        (Base::WinAnsi, _) => Some(char::from(code)),
        (Base::MacRoman, _) => MAC_ROMAN_HIGH.chars().nth(usize::from(code - 0x80)),
        (Base::Standard, _) => standard_high(code),
    }
}

fn standard_high(code: u8) -> Option<char> {
    Some(match code {
        0xa1 => '¡',
        0xa2 => '¢',
        0xa3 => '£',
        0xa4 => '⁄',
        0xa5 => '¥',
        0xa6 => 'ƒ',
        0xa7 => '§',
        0xa8 => '¤',
        0xa9 => '\'',
        0xaa => '“',
        0xab => '«',
        0xac => '‹',
        0xad => '›',
        0xae => 'ﬁ',
        0xaf => 'ﬂ',
        0xb1 => '–',
        0xb2 => '†',
        0xb3 => '‡',
        0xb4 => '·',
        0xb6 => '¶',
        0xb7 => '•',
        0xb8 => '‚',
        0xb9 => '„',
        0xba => '”',
        0xbb => '»',
        0xbc => '…',
        0xbd => '‰',
        0xbf => '¿',
        0xc1 => '`',
        0xc2 => '´',
        0xc3 => 'ˆ',
        0xc4 => '˜',
        0xc5 => '¯',
        0xc6 => '˘',
        0xc7 => '˙',
        0xc8 => '¨',
        0xca => '˚',
        0xcb => '¸',
        0xcd => '˝',
        0xce => '˛',
        0xcf => 'ˇ',
        0xd0 => '—',
        0xe1 => 'Æ',
        0xe3 => 'ª',
        0xe8 => 'Ł',
        0xe9 => 'Ø',
        0xea => 'Œ',
        0xeb => 'º',
        0xf1 => 'æ',
        0xf5 => 'ı',
        0xf8 => 'ł',
        0xf9 => 'ø',
        0xfa => 'œ',
        0xfb => 'ß',
        _ => return None,
    })
}

/// AGL names for Latin-1 U+00A0–U+00FF, in order.
const LATIN1_NAMES: [&str; 96] = [
    "nbspace",
    "exclamdown",
    "cent",
    "sterling",
    "currency",
    "yen",
    "brokenbar",
    "section",
    "dieresis",
    "copyright",
    "ordfeminine",
    "guillemotleft",
    "logicalnot",
    "sfthyphen",
    "registered",
    "macron",
    "degree",
    "plusminus",
    "twosuperior",
    "threesuperior",
    "acute",
    "mu",
    "paragraph",
    "periodcentered",
    "cedilla",
    "onesuperior",
    "ordmasculine",
    "guillemotright",
    "onequarter",
    "onehalf",
    "threequarters",
    "questiondown",
    "Agrave",
    "Aacute",
    "Acircumflex",
    "Atilde",
    "Adieresis",
    "Aring",
    "AE",
    "Ccedilla",
    "Egrave",
    "Eacute",
    "Ecircumflex",
    "Edieresis",
    "Igrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Eth",
    "Ntilde",
    "Ograve",
    "Oacute",
    "Ocircumflex",
    "Otilde",
    "Odieresis",
    "multiply",
    "Oslash",
    "Ugrave",
    "Uacute",
    "Ucircumflex",
    "Udieresis",
    "Yacute",
    "Thorn",
    "germandbls",
    "agrave",
    "aacute",
    "acircumflex",
    "atilde",
    "adieresis",
    "aring",
    "ae",
    "ccedilla",
    "egrave",
    "eacute",
    "ecircumflex",
    "edieresis",
    "igrave",
    "iacute",
    "icircumflex",
    "idieresis",
    "eth",
    "ntilde",
    "ograve",
    "oacute",
    "ocircumflex",
    "otilde",
    "odieresis",
    "divide",
    "oslash",
    "ugrave",
    "uacute",
    "ucircumflex",
    "udieresis",
    "yacute",
    "thorn",
    "ydieresis",
];

const ASCII_NAMES: [&str; 95] = [
    "space",
    "exclam",
    "quotedbl",
    "numbersign",
    "dollar",
    "percent",
    "ampersand",
    "quotesingle",
    "parenleft",
    "parenright",
    "asterisk",
    "plus",
    "comma",
    "hyphen",
    "period",
    "slash",
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "colon",
    "semicolon",
    "less",
    "equal",
    "greater",
    "question",
    "at",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "bracketleft",
    "backslash",
    "bracketright",
    "asciicircum",
    "underscore",
    "grave",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "braceleft",
    "bar",
    "braceright",
    "asciitilde",
];

const OTHER_NAMES: [(&str, &str); 38] = [
    ("quoteleft", "‘"),
    ("quoteright", "’"),
    ("quotedblleft", "“"),
    ("quotedblright", "”"),
    ("quotesinglbase", "‚"),
    ("quotedblbase", "„"),
    ("guilsinglleft", "‹"),
    ("guilsinglright", "›"),
    ("endash", "–"),
    ("emdash", "—"),
    ("bullet", "•"),
    ("ellipsis", "…"),
    ("dagger", "†"),
    ("daggerdbl", "‡"),
    ("perthousand", "‰"),
    ("trademark", "™"),
    ("fi", "fi"),
    ("fl", "fl"),
    ("ff", "ff"),
    ("ffi", "ffi"),
    ("ffl", "ffl"),
    ("minus", "−"),
    ("fraction", "⁄"),
    ("Euro", "€"),
    ("OE", "Œ"),
    ("oe", "œ"),
    ("Scaron", "Š"),
    ("scaron", "š"),
    ("Zcaron", "Ž"),
    ("zcaron", "ž"),
    ("Ydieresis", "Ÿ"),
    ("florin", "ƒ"),
    ("circumflex", "ˆ"),
    ("tilde", "˜"),
    ("dotlessi", "ı"),
    ("Lslash", "Ł"),
    ("lslash", "ł"),
    ("nonbreakingspace", "\u{a0}"),
];

/// The text a glyph name stands for: AGL names, `uniXXXX` and `uXXXX`.
pub fn glyph_text(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name).ok()?;
    // "a.sc", "f_i": the part before a period names the glyph.
    let base = name.split('.').next().unwrap_or(name);
    if let Some(i) = ASCII_NAMES.iter().position(|n| *n == base) {
        return Some(char::from(32 + i as u8).to_string());
    }
    if let Some(i) = LATIN1_NAMES.iter().position(|n| *n == base) {
        return char::from_u32(0xa0 + i as u32).map(String::from);
    }
    if let Some((_, t)) = OTHER_NAMES.iter().find(|(n, _)| *n == base) {
        return Some((*t).to_string());
    }
    if let Some(hex) = base.strip_prefix("uni")
        && hex.len() >= 4
        && hex.len() % 4 == 0
    {
        let units: Option<Vec<u16>> = hex
            .as_bytes()
            .chunks(4)
            .map(|c| u16::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
            .collect();
        return units.map(|u| String::from_utf16_lossy(&u));
    }
    if let Some(hex) = base.strip_prefix('u')
        && (4..=6).contains(&hex.len())
    {
        return u32::from_str_radix(hex, 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from);
    }
    if base.contains('_') {
        // A ligature of named glyphs: "f_f_i".
        return base.split('_').map(|p| glyph_text(p.as_bytes())).collect();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_encodings() {
        assert_eq!(base_char(Base::WinAnsi, b'A'), Some('A'));
        assert_eq!(base_char(Base::WinAnsi, 0x96), Some('–'));
        assert_eq!(base_char(Base::WinAnsi, 0xe9), Some('é'));
        assert_eq!(base_char(Base::MacRoman, 0x8e), Some('é'));
        assert_eq!(base_char(Base::MacRoman, 0xca), Some('\u{a0}'));
        assert_eq!(base_char(Base::Standard, 0x27), Some('’'));
        assert_eq!(MAC_ROMAN_HIGH.chars().count(), 128);
    }

    #[test]
    fn glyph_names() {
        assert_eq!(glyph_text(b"A").as_deref(), Some("A"));
        assert_eq!(glyph_text(b"seven").as_deref(), Some("7"));
        assert_eq!(glyph_text(b"eacute").as_deref(), Some("é"));
        assert_eq!(glyph_text(b"uni0041").as_deref(), Some("A"));
        assert_eq!(glyph_text(b"u1F600").as_deref(), Some("😀"));
        assert_eq!(glyph_text(b"f_i").as_deref(), Some("fi"));
        assert_eq!(glyph_text(b"a.sc").as_deref(), Some("a"));
        assert_eq!(glyph_text(b"g123"), None);
    }
}
