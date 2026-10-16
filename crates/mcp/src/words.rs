//! The words printed on the pieces in the files they were imported from,
//! for an assistant to name pieces by: the import names a piece by rules
//! (the biggest words beside its number), which a title, a translation or
//! a stray note can fool; an assistant reading them all need not be.

use std::collections::{HashMap, HashSet};

use drapewright_io::pdf::read_printed;
use drapewright_pattern_core::{Piece, PrintedText, SourceFormat};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ToolError;
use crate::session::Session;
use crate::text::round1;
use crate::tools::{Output, unknown_piece};

/// Words printed on this many pieces or more, and on half of those with
/// words or more, are the pattern's (its title, the designer's name).
const ON_MOST: usize = 3;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WordsArgs {
    /// Piece ids; leave out for every piece.
    pieces: Option<Vec<String>>,
    /// Also list, on each piece, the sizes printed along its lines, the
    /// pages' labels for putting them together, and the words printed on
    /// most pieces (otherwise listed once, at the top).
    all: Option<bool>,
}

/// A line of text the same wherever it is printed: its letters, spaces
/// evened, in lower case.
fn key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether a piece's name came from these words.
fn names(piece: &Piece, w: &PrintedText) -> bool {
    let plain = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let (name, words) = (plain(&piece.name), plain(&w.text));
    !words.is_empty() && name.starts_with(&words)
}

fn words_of(piece: &Piece) -> &[PrintedText] {
    piece.source.as_ref().map_or(&[], |s| s.words.as_slice())
}

pub fn piece_labels(session: &mut Session, a: WordsArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let chosen: Vec<&Piece> = match &a.pieces {
        Some(ids) => ids
            .iter()
            .map(|id| doc.piece(id).map(|p| &**p).ok_or_else(|| unknown_piece(id)))
            .collect::<Result<_, _>>()?,
        None => doc.pieces.iter().map(|p| &**p).collect(),
    };
    // The pattern's own words, printed on most of its pieces.
    let mut on: HashMap<String, HashSet<&str>> = HashMap::new();
    let mut texts: HashMap<String, &str> = HashMap::new();
    let with_words = doc
        .pieces
        .iter()
        .filter(|p| !words_of(p).is_empty())
        .count();
    for piece in &doc.pieces {
        for w in words_of(piece) {
            on.entry(key(&w.text)).or_default().insert(&piece.id);
            texts.entry(key(&w.text)).or_insert(&w.text);
        }
    }
    let common: HashSet<&String> = on
        .iter()
        .filter(|(_, p)| p.len() >= ON_MOST && 2 * p.len() >= with_words)
        .map(|(k, _)| k)
        .collect();
    // Not the sizes, printed along every piece's lines.
    let mut on_most: Vec<&str> = common
        .iter()
        .map(|k| texts[*k])
        .filter(|t| read_printed(t).size.is_none())
        .collect();
    on_most.sort_unstable();

    let all = a.all == Some(true);
    let mut pieces: Vec<Value> = Vec::new();
    let mut without: Vec<Value> = Vec::new();
    for piece in chosen {
        let words = words_of(piece);
        if words.is_empty() {
            let pdf = piece
                .source
                .as_ref()
                .is_some_and(|s| s.format == SourceFormat::Pdf);
            without.push(json!({
                "id": piece.id,
                "name": piece.name,
                "why": if pdf {
                    "imported before Drapewright kept the words; import the file again to see them"
                } else {
                    "not imported from a PDF"
                },
            }));
            continue;
        }
        // Biggest first, then down and across the piece.
        let mut order: Vec<&PrintedText> = words.iter().collect();
        order.sort_by(|a, b| {
            b.size
                .total_cmp(&a.size)
                .then(a.at.y.total_cmp(&b.at.y))
                .then(a.at.x.total_cmp(&b.at.x))
        });
        // A piece's number is set out in its biggest type; small numbers
        // are sizes along its lines.
        let biggest = words.iter().map(|w| w.size).fold(0.0, f64::max);
        let mut listed = Vec::new();
        let mut hidden = 0;
        for w in order {
            let read = read_printed(&w.text);
            let everywhere = common.contains(&key(&w.text));
            let kind = if read.number && w.size >= biggest {
                "number"
            } else if !read.cutting.is_empty() {
                "cutting"
            } else if everywhere {
                "on most pieces"
            } else if read.size.is_some() {
                "size"
            } else if read.page {
                "page"
            } else if read.instruction {
                "instruction"
            } else {
                "words"
            };
            if !all && matches!(kind, "size" | "page" | "on most pieces") {
                hidden += 1;
                continue;
            }
            let mut entry = json!({
                "text": w.text,
                "kind": kind,
                "letterMm": round1(w.size),
                "at": [round1(w.at.x), round1(w.at.y)],
                "angle": w.angle.round(),
            });
            if names(piece, w) {
                entry["namedIt"] = json!(true);
            }
            listed.push(entry);
        }
        let mut entry = json!({ "id": piece.id, "name": piece.name, "words": listed });
        if hidden > 0 {
            entry["hidden"] = json!(hidden);
        }
        pieces.push(entry);
    }
    let mut reply = json!({ "onMostPieces": on_most, "pieces": pieces });
    if !without.is_empty() {
        reply["noWordsKept"] = json!(without);
    }
    Ok(Output::json(&reply))
}
