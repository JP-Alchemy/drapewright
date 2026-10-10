//! Text on printed pages, in the standard Helvetica fonts that every PDF
//! reader has, so no font is embedded. They cover the letters of Western
//! European languages (Windows-1252); anything else prints as "?", and the
//! print plan lists the names affected.

/// The Windows-1252 code of a character, if it has one.
fn win_ansi(c: char) -> Option<u8> {
    let code = u32::from(c);
    if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
        return u8::try_from(code).ok();
    }
    Some(match c {
        '€' => 0x80,
        '‚' => 0x82,
        'ƒ' => 0x83,
        '„' => 0x84,
        '…' => 0x85,
        '†' => 0x86,
        '‡' => 0x87,
        'ˆ' => 0x88,
        '‰' => 0x89,
        'Š' => 0x8A,
        '‹' => 0x8B,
        'Œ' => 0x8C,
        'Ž' => 0x8E,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '˜' => 0x98,
        '™' => 0x99,
        'š' => 0x9A,
        '›' => 0x9B,
        'œ' => 0x9C,
        'ž' => 0x9E,
        'Ÿ' => 0x9F,
        _ => return None,
    })
}

/// Whether every letter of `s` can be printed.
pub fn encodable(s: &str) -> bool {
    s.chars().all(|c| win_ansi(c).is_some())
}

/// `s` in the fonts' encoding, with "?" for letters they lack.
pub fn encode(s: &str) -> Vec<u8> {
    s.chars().map(|c| win_ansi(c).unwrap_or(b'?')).collect()
}

/// Approximate advance of a letter in Helvetica, in ems: close enough to
/// centre and wrap labels. Nothing is measured from text.
fn advance(c: char) -> f64 {
    match c {
        'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' | '·' => 0.24,
        ' ' | 'f' | 't' | 'I' | '(' | ')' | '[' | ']' | '/' => 0.29,
        'r' | '-' | '‘' | '’' | '“' | '”' | '"' => 0.34,
        'm' | 'M' => 0.83,
        'w' => 0.72,
        'W' => 0.94,
        'A'..='Z' => 0.69,
        _ => 0.56,
    }
}

/// Approximate width of `s` at `size` (any unit), bold being a little wider.
pub fn width(s: &str, size: f64, bold: bool) -> f64 {
    let ems: f64 = s.chars().map(advance).sum();
    ems * size * if bold { 1.06 } else { 1.0 }
}

/// Splits `s` into lines no wider than `max`, at spaces.
pub fn wrap(s: &str, size: f64, max: f64, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && width(&candidate, size, bold) > max {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn western_letters_print_and_others_become_question_marks() {
        assert!(encodable("Rokpand — “Voorkant” × 2 · café"));
        assert_eq!(encode("é€”"), vec![0xE9, 0x80, 0x94]);
        assert!(!encodable("Spódnica przód ś"));
        assert_eq!(encode("aśb"), b"a?b".to_vec());
    }

    #[test]
    fn wrapping_keeps_lines_short() {
        let lines = wrap(
            "Print at 100% and measure the square before cutting",
            10.0,
            120.0,
            false,
        );
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| width(l, 10.0, false) <= 120.0));
        assert_eq!(
            lines.join(" "),
            "Print at 100% and measure the square before cutting"
        );
    }
}
