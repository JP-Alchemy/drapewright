//! Reading pattern pieces from SVG: usvg resolves styles, transforms and
//! units; the paths then go through the shared piece finder in
//! `crate::assemble`.

use std::collections::HashMap;

use drapewright_pattern_core::SourceFormat;
use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
use usvg::tiny_skia_path::PathSegment;

use super::units::{detect_scale, view_box};
use crate::assemble::{CLOSE_TOLERANCE, RawPath, assemble, meaningful_name};
use crate::import::{AllowanceQuestion, ImportError, ImportResult, round_mm, sha256_hex};

/// Labels and classes from the XML, which usvg does not keep.
#[derive(Default)]
struct ElementInfo {
    label: Option<String>,
    class: Option<String>,
    group_label: Option<String>,
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
    let mut infos = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        let group_label = node
            .ancestors()
            .skip(1)
            .filter(|a| a.has_tag_name("g"))
            .find_map(|g| label_of(g).or_else(|| g.attribute("id").map(str::to_string)));
        infos.insert(
            id.to_string(),
            ElementInfo {
                label: label_of(node),
                class: node.attribute("class").map(str::to_string),
                group_label,
            },
        );
    }
    infos
}

/// Maps usvg's canvas pixels to millimetres in the file's own coordinates.
#[derive(Clone, Copy)]
struct ToMm {
    mm_per_unit: f64,
    /// usvg moves the viewBox origin to (0, 0); this puts it back, in mm.
    origin: (f64, f64),
}

fn collect(
    group: &usvg::Group,
    to_mm: ToMm,
    infos: &HashMap<String, ElementInfo>,
    group_name: Option<&str>,
    out: &mut Vec<RawPath>,
) {
    let ToMm {
        mm_per_unit,
        origin,
    } = to_mm;
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                let own = infos
                    .get(g.id())
                    .and_then(|i| i.label.as_deref())
                    .and_then(meaningful_name)
                    .or_else(|| meaningful_name(g.id()));
                collect(g, to_mm, infos, own.as_deref().or(group_name), out);
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
                let hint = [Some(p.id()), label, info.and_then(|i| i.class.as_deref())]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_lowercase();
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
    let tree = usvg::Tree::from_data(data, &usvg::Options::default())
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
    collect(
        tree.root(),
        ToMm {
            mm_per_unit,
            origin,
        },
        &infos,
        None,
        &mut raw,
    );
    let sha256 = sha256_hex(data);
    let (pieces, notes) = assemble(raw, SourceFormat::Svg, &scale, file_name, &sha256);
    Ok(ImportResult {
        allowance: AllowanceQuestion::for_unmeasured(&pieces, &notes, None),
        pieces,
        scale,
        notes,
        sha256,
    })
}
