//! Reading pattern pieces from SVG: usvg resolves styles, transforms and
//! units; the paths then go through the shared piece finder in
//! `crate::assemble`.

use std::collections::HashMap;

use drapewright_pattern_core::{CutInstruction, NotchKind, SourceFormat};
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
use usvg::tiny_skia_path::PathSegment;

use super::freesewing;
use super::units::{detect_scale, view_box};
use crate::assemble::{CLOSE_TOLERANCE, RawNotch, RawPath, assemble, meaningful_name};
use crate::import::{AllowanceQuestion, ImportError, ImportResult, round_mm, sha256_hex};

/// Labels, classes and text from the XML, which usvg does not keep.
#[derive(Default)]
struct ElementInfo {
    label: Option<String>,
    class: Option<String>,
    group_label: Option<String>,
    /// A label FreeSewing puts on annotation paths (`data-text`), such as
    /// "grainline".
    text: Option<String>,
    /// How many to cut, from the text of the piece group the element is in.
    cut: Option<CutInstruction>,
}

/// All the text inside an element, its tspans joined.
fn text_of(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_infos(doc: &roxmltree::Document) -> HashMap<String, ElementInfo> {
    const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
    let label_of = |n: roxmltree::Node| {
        n.attribute((INKSCAPE, "label"))
            .map(str::to_string)
            .or_else(|| {
                n.children()
                    .find(|c| c.has_tag_name("title"))
                    .and_then(|t| t.text())
                    .map(|t| t.trim().to_string())
            })
    };
    // Cutting instructions written in each piece group (FreeSewing).
    let cut_of_group = |g: roxmltree::Node| {
        g.descendants()
            .filter(|t| t.has_tag_name("text"))
            .find_map(|t| freesewing::cut_from_text(&text_of(t)))
    };
    let mut infos = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        let groups = || node.ancestors().skip(1).filter(|a| a.has_tag_name("g"));
        let group_label =
            groups().find_map(|g| label_of(g).or_else(|| g.attribute("id").map(str::to_string)));
        let cut = groups()
            .find(|g| {
                g.attribute("id")
                    .is_some_and(|i| freesewing::part_name(i).is_some())
            })
            .and_then(cut_of_group);
        infos.insert(
            id.to_string(),
            ElementInfo {
                label: label_of(node),
                class: node.attribute("class").map(str::to_string),
                group_label,
                text: node.attribute("data-text").map(str::to_string),
                cut,
            },
        );
    }
    infos
}

/// A symbol placed with `<use>` that means something to a pattern.
#[derive(Clone, Copy)]
enum Symbol {
    Notch(NotchKind),
    /// A logo: decoration, left out.
    Logo,
}

/// usvg turns `<use>` references into anonymous groups. Symbols that mean
/// something (FreeSewing's `#notch`, `#bnotch` and `#logo`) get an id first,
/// so their groups can be found afterwards: returns the text with the ids
/// added, and what each id stands for.
fn tag_symbols(text: &str, doc: &roxmltree::Document) -> (String, HashMap<String, Symbol>) {
    const XLINK: &str = "http://www.w3.org/1999/xlink";
    let mut kinds = HashMap::new();
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for (n, node) in doc
        .descendants()
        .filter(|n| n.has_tag_name("use"))
        .enumerate()
    {
        let href = node
            .attribute((XLINK, "href"))
            .or_else(|| node.attribute("href"));
        let kind = match href {
            Some("#logo") => Symbol::Logo,
            Some(h) => match freesewing::notch_kind(h) {
                Some(k) => Symbol::Notch(k),
                None => continue,
            },
            None => continue,
        };
        if let Some(id) = node.attribute("id") {
            kinds.insert(id.to_string(), kind);
            continue;
        }
        // Just after the tag's name (`<use` or `<svg:use`).
        let start = node.range().start;
        let Some(name_end) =
            text[start..].find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        else {
            continue;
        };
        let id = format!("drapewright-symbol-{n}");
        inserts.push((start + name_end, format!(" id=\"{id}\"")));
        kinds.insert(id, kind);
    }
    let mut out = String::with_capacity(text.len() + inserts.len() * 32);
    let mut last = 0;
    for (at, insert) in inserts {
        out.push_str(&text[last..at]);
        out.push_str(&insert);
        last = at;
    }
    out.push_str(&text[last..]);
    (out, kinds)
}

/// Maps usvg's canvas pixels to millimetres in the file's own coordinates.
#[derive(Clone, Copy)]
struct ToMm {
    mm_per_unit: f64,
    /// usvg moves the viewBox origin to (0, 0); this puts it back, in mm.
    origin: (f64, f64),
}

/// What the XML says about elements, which usvg does not keep.
struct Known<'a> {
    infos: &'a HashMap<String, ElementInfo>,
    symbols: &'a HashMap<String, Symbol>,
}

fn collect(
    group: &usvg::Group,
    to_mm: ToMm,
    known: &Known,
    group_name: Option<&str>,
    out: &mut Vec<RawPath>,
    notches: &mut Vec<RawNotch>,
) {
    let ToMm {
        mm_per_unit,
        origin,
    } = to_mm;
    let infos = known.infos;
    for node in group.children() {
        match node {
            usvg::Node::Group(g) if known.symbols.contains_key(g.id()) => {
                match known.symbols[g.id()] {
                    Symbol::Notch(kind) => {
                        // A notch symbol: its centre is where the notch is.
                        let b = g.abs_bounding_box();
                        let (x, y) = (
                            f64::from(b.x() + b.width() / 2.0),
                            f64::from(b.y() + b.height() / 2.0),
                        );
                        notches.push(RawNotch {
                            at: Point::new(
                                round_mm(mm_per_unit * x + origin.0),
                                round_mm(mm_per_unit * y + origin.1),
                            ),
                            kind,
                        });
                    }
                    Symbol::Logo => {
                        // Its paths are marked, so they're left out (and counted).
                        let mut logo = Vec::new();
                        collect(g, to_mm, known, group_name, &mut logo, notches);
                        for mut r in logo {
                            r.hint.push_str(" logo");
                            out.push(r);
                        }
                    }
                }
            }
            usvg::Node::Group(g) => {
                let own = infos
                    .get(g.id())
                    .and_then(|i| i.label.as_deref())
                    .and_then(meaningful_name)
                    .or_else(|| meaningful_name(g.id()));
                collect(g, to_mm, known, own.as_deref().or(group_name), out, notches);
            }
            usvg::Node::Path(p) if p.is_visible() => {
                let t = p.abs_transform();
                let (sx, kx, ky, sy, tx, ty) = (
                    f64::from(t.sx),
                    f64::from(t.kx),
                    f64::from(t.ky),
                    f64::from(t.sy),
                    f64::from(t.tx),
                    f64::from(t.ty),
                );
                let map = |q: usvg::tiny_skia_path::Point| {
                    let (x, y) = (f64::from(q.x), f64::from(q.y));
                    Point::new(
                        round_mm(mm_per_unit * (sx * x + kx * y + tx) + origin.0),
                        round_mm(mm_per_unit * (ky * x + sy * y + ty) + origin.1),
                    )
                };
                let info = infos.get(p.id());
                let label = info.and_then(|i| i.label.as_deref());
                let name = label
                    .and_then(meaningful_name)
                    .or_else(|| meaningful_name(p.id()))
                    .or_else(|| {
                        info.and_then(|i| i.group_label.as_deref())
                            .and_then(meaningful_name)
                    })
                    .or_else(|| group_name.map(str::to_string));
                let hint = [
                    Some(p.id()),
                    label,
                    info.and_then(|i| i.class.as_deref()),
                    info.and_then(|i| i.text.as_deref()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
                let cut = info.and_then(|i| i.cut.clone());
                let reference = (!p.id().is_empty()).then(|| p.id().to_string());
                let filled = p.fill().is_some();

                let mut segs: Vec<PathSeg> = Vec::new();
                let mut start: Option<Point> = None;
                let mut cur = Point::ZERO;
                let mut flush = |segs: &mut Vec<PathSeg>, start: Point, closed: bool| {
                    if segs.is_empty() {
                        return;
                    }
                    let end = segs.last().map_or(start, |s| s.end());
                    let closed = closed || end.distance(start) <= CLOSE_TOLERANCE;
                    if closed {
                        // Snap the end exactly onto the start.
                        if let Some(last) = segs.last_mut() {
                            *last = match *last {
                                PathSeg::Line(l) => PathSeg::Line(Line::new(l.p0, start)),
                                PathSeg::Quad(q) => PathSeg::Quad(QuadBez::new(q.p0, q.p1, start)),
                                PathSeg::Cubic(c) => {
                                    PathSeg::Cubic(CubicBez::new(c.p0, c.p1, c.p2, start))
                                }
                            };
                        }
                    }
                    out.push(RawPath {
                        segs: std::mem::take(segs),
                        closed,
                        filled,
                        hint: hint.clone(),
                        name: name.clone(),
                        reference: reference.clone(),
                        cut: cut.clone(),
                    });
                };
                for seg in p.data().segments() {
                    match seg {
                        PathSegment::MoveTo(q) => {
                            if let Some(s) = start {
                                flush(&mut segs, s, false);
                            }
                            cur = map(q);
                            start = Some(cur);
                        }
                        PathSegment::LineTo(q) => {
                            let q = map(q);
                            if q != cur {
                                segs.push(PathSeg::Line(Line::new(cur, q)));
                            }
                            cur = q;
                        }
                        PathSegment::QuadTo(c, q) => {
                            let q = map(q);
                            segs.push(PathSeg::Cubic(QuadBez::new(cur, map(c), q).raise()));
                            cur = q;
                        }
                        PathSegment::CubicTo(c1, c2, q) => {
                            let q = map(q);
                            segs.push(PathSeg::Cubic(CubicBez::new(cur, map(c1), map(c2), q)));
                            cur = q;
                        }
                        PathSegment::Close => {
                            if let Some(s) = start {
                                if cur != s {
                                    segs.push(PathSeg::Line(Line::new(cur, s)));
                                }
                                flush(&mut segs, s, true);
                                cur = s;
                            }
                        }
                    }
                }
                if let Some(s) = start {
                    flush(&mut segs, s, false);
                }
            }
            _ => {}
        }
    }
}

pub fn import_svg(data: &[u8], file_name: Option<&str>) -> Result<ImportResult, ImportError> {
    if data.starts_with(&[0x1f, 0x8b]) {
        return Err(ImportError::CompressedSvg);
    }
    let unreadable = |detail: String| ImportError::Unreadable {
        format: "SVG",
        detail,
    };
    let text = std::str::from_utf8(data).map_err(|e| unreadable(e.to_string()))?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options)
        .map_err(|e| unreadable(e.to_string()))?;
    if !doc.root_element().has_tag_name("svg") {
        return Err(unreadable(
            "the file does not start with an <svg> element".into(),
        ));
    }
    let scale = detect_scale(text, &doc);
    let infos = element_infos(&doc);
    let (tagged, symbols) = tag_symbols(text, &doc);
    let tree = usvg::Tree::from_str(&tagged, &usvg::Options::default())
        .map_err(|e| unreadable(e.to_string()))?;
    let mm_per_unit = scale.chosen.mm_per_unit;
    let origin = view_box(&doc).map_or((0.0, 0.0), |(x, y, w, h)| {
        let size = tree.size();
        (
            x * f64::from(size.width()) / w * mm_per_unit,
            y * f64::from(size.height()) / h * mm_per_unit,
        )
    });
    let mut raw = Vec::new();
    let mut notches = Vec::new();
    collect(
        tree.root(),
        ToMm {
            mm_per_unit,
            origin,
        },
        &Known {
            infos: &infos,
            symbols: &symbols,
        },
        None,
        &mut raw,
        &mut notches,
    );
    let sha256 = sha256_hex(data);
    let (pieces, notes) = assemble(raw, &notches, SourceFormat::Svg, &scale, file_name, &sha256);
    Ok(ImportResult {
        allowance: AllowanceQuestion::for_unmeasured(&pieces, &notes, None),
        pieces,
        scale,
        sizes: Default::default(),
        notes,
        sha256,
    })
}
