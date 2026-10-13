//! A garment altered to fit: the room it should have more or less of round
//! the bust, waist and hip taken up at its side seams, as a sewist would
//! let out or take in a garment that doesn't fit. Where on each seam the
//! figure's bust, waist and hip fall comes from putting the garment on it.

use drapewright_body::Shape;
use drapewright_pattern_core::alter::{Alteration, Change, Drawn, alter};
use drapewright_pattern_core::blend::Height;
use drapewright_pattern_core::geom::flatten_segs;
use drapewright_pattern_core::{Id, Piece, Pt};
use drapewright_seams::Seam;
use drapewright_sim::garment::{Garment, assemble, draped_pieces};
use drapewright_sim::heights::{Worn, worn};
use drapewright_sim::place::{Figure, place};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AvatarSettings;
use crate::drape::SPACING;

/// A garment to alter to fit, on the avatar shaped by `settings`: how much
/// bigger round (or smaller, less than 0) it should be at each level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FitRequest {
    pub pieces: Vec<Piece>,
    pub seams: Vec<Seam>,
    pub settings: AvatarSettings,
    pub change: Vec<LevelChange>,
}

/// How much bigger round a garment should be at a level, mm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LevelChange {
    /// "bust", "waist" or "hip".
    pub level: String,
    pub by: f64,
}

/// The steps that alter a garment to fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FitSteps {
    /// Each piece's steps, to add after those it has, in order.
    pub steps: Vec<PieceStep>,
    /// Levels it couldn't be altered at: no seam crosses them.
    pub missed: Vec<String>,
}

/// A step for one piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PieceStep {
    pub piece: Id,
    pub alteration: Alteration,
}

/// Changes smaller than this round the garment are left, mm.
const LEAST: f64 = 2.0;
/// A step moving a seam less than this is left out, mm.
const LEAST_STEP: f64 = 0.3;

/// A level to change: how high it is, and each seam side's share of the
/// change (piece, edge, mm).
struct Wanted {
    level: &'static str,
    y: f64,
    sides: Vec<(usize, Id, f64)>,
}

/// A seam side's changes at the levels it crosses: (level, height, mm).
type Levels = Vec<(&'static str, f64, f64)>;

/// One side of a seam to let out or take in: the piece, its edge, and
/// how many times that edge goes round the body (twice for a piece cut
/// as a pair, or on the fold).
struct Side {
    piece: usize,
    edge: Id,
    copies: usize,
}

/// The steps letting out or taking in a garment's side seams to change it
/// round each level by as much as `request` asks.
pub fn fit_steps(model: &drapewright_body::BodyModel, request: &FitRequest) -> FitSteps {
    let body = model.shape(&Shape::from(request.settings.clone()));
    let figure = Figure::of(model, &body);
    let mut out = FitSteps {
        steps: Vec::new(),
        missed: Vec::new(),
    };
    let pieces = &request.pieces;
    let draped = draped_pieces(pieces, &request.seams);
    let Ok(garment) = assemble(&draped.sewn, &request.seams, SPACING) else {
        out.missed = request.change.iter().map(|c| c.level.clone()).collect();
        return out;
    };
    let starts = place(&garment, &figure);
    let heights: Vec<(Id, Height)> = worn(&garment, &starts, &draped.sewn, &figure)
        .into_iter()
        .filter_map(|(id, how)| match how {
            Worn::Trunk(h) => Some((id, h)),
            Worn::Arm => None,
        })
        .collect();
    let height = |piece: &Piece| {
        heights
            .iter()
            .find(|(id, _)| *id == piece.id)
            .map(|(_, h)| *h)
    };

    // Each level: how high it is, and each seam side's share of its change
    // (none at a level to keep as it is, for the changes to taper to).
    let mut wanted: Vec<Wanted> = Vec::new();
    for change in &request.change {
        let Some(level) = figure.levels.iter().find(|l| l.name == change.level) else {
            continue;
        };
        let by = if change.by.abs() < LEAST {
            0.0
        } else {
            change.by
        };
        let sides = sides_at(pieces, &request.seams, &garment, &height, level.y);
        let copies: usize = sides.iter().map(|s| s.copies).sum();
        if copies == 0 {
            if by != 0.0 {
                out.missed.push(change.level.clone());
            }
            continue;
        }
        let each = by / copies as f64;
        wanted.push(Wanted {
            level: level.name,
            y: level.y,
            sides: sides.into_iter().map(|s| (s.piece, s.edge, each)).collect(),
        });
    }
    // Each seam side's changes, top down.
    let mut by_side: Vec<((usize, Id), Levels)> = Vec::new();
    for w in &wanted {
        for (piece, edge, by) in &w.sides {
            let key = (*piece, edge.clone());
            match by_side.iter_mut().find(|(k, _)| *k == key) {
                Some((_, levels)) => levels.push((w.level, w.y, *by)),
                None => by_side.push((key, vec![(w.level, w.y, *by)])),
            }
        }
    }
    // The steps, each placed on the piece as the steps before it leave it.
    let mut shapes: Vec<Option<(Drawn, Vec<Alteration>)>> = vec![None; pieces.len()];
    for ((p, edge), mut levels) in by_side {
        let piece = &pieces[p];
        let Some(h) = height(piece) else {
            continue;
        };
        levels.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (name, at, to, by) in ramps(&levels) {
            if by.abs() < LEAST_STEP {
                continue;
            }
            let (drawn, added) = shapes[p].get_or_insert_with(|| (Drawn::of(piece), Vec::new()));
            let now = alter(drawn, added).unwrap_or_else(|_| drawn.clone());
            let Some(i) = now.outline.edge_index(&edge) else {
                continue;
            };
            let Some(from) = at_height(&now, i, &h, at) else {
                continue;
            };
            let toward = match to {
                Some(y) => at_height(&now, i, &h, y),
                None => far_end(&now, i, &h, at),
            };
            let Some(toward) = toward else {
                continue;
            };
            let change = match name {
                "bust" => Change::BustSide {
                    at: from,
                    to: toward,
                    by,
                },
                "hip" => Change::Hip {
                    at: from,
                    to: toward,
                    by,
                },
                _ => Change::Waist {
                    at: from,
                    to: toward,
                    by,
                },
            };
            let step = Alteration {
                id: format!("fit{}", out.steps.len()),
                on: true,
                change,
            };
            // Kept only if it applies.
            let mut trial = added.clone();
            trial.push(step.clone());
            if alter(drawn, &trial).is_ok() {
                added.push(step.clone());
                out.steps.push(PieceStep {
                    piece: piece.id.clone(),
                    alteration: step,
                });
            }
        }
    }
    out
}

/// A side seam's changes at its levels (top down: name, height, by) as
/// steps that each move it out at one level and beyond, tapering to the
/// next: (level, at, tapering to (none: the seam's far end), by). Each
/// level ends up moved by its own amount.
fn ramps(levels: &[(&'static str, f64, f64)]) -> Vec<(&'static str, f64, Option<f64>, f64)> {
    match levels {
        [] => Vec::new(),
        [(name, y, by)] => vec![(name, *y, None, *by)],
        [(a, ya, da), (b, yb, db)] => vec![(a, *ya, Some(*yb), *da), (b, *yb, Some(*ya), *db)],
        [(a, ya, da), (m, ym, dm), (b, yb, db), ..] => vec![
            // The middle first, tapering down: the top moves with it ...
            (m, *ym, Some(*yb), *dm),
            // ... so the top by the rest.
            (a, *ya, Some(*ym), *da - *dm),
            (b, *yb, Some(*ym), *db),
        ],
    }
}

/// The seam sides crossing height `y`: those of seams between a front and
/// a back (side seams) where there are any, else of every seam between two
/// pieces, round the trunk.
fn sides_at(
    pieces: &[Piece],
    seams: &[Seam],
    garment: &Garment,
    height: &dyn Fn(&Piece) -> Option<Height>,
    y: f64,
) -> Vec<Side> {
    let index = |id: &str| pieces.iter().position(|p| p.id == id);
    let crosses = |p: usize, edge: &str| -> bool {
        let piece = &pieces[p];
        let (Some(h), Some(i)) = (height(piece), piece.outline.edge_index(edge)) else {
            return false;
        };
        let points = piece.outline.edge_geom(i).flatten(0.5);
        let above = points.iter().any(|q| h.of((*q).into()) > y);
        let below = points.iter().any(|q| h.of((*q).into()) < y);
        above && below
    };
    let word = |p: usize, w: &str| pieces[p].name.to_lowercase().contains(w);
    let mut found: Vec<(bool, [(usize, Id); 2])> = Vec::new();
    for seam in seams
        .iter()
        .filter(|s| s.confirmed && s.a.piece != s.b.piece)
    {
        let (Some(a), Some(b)) = (index(&seam.a.piece), index(&seam.b.piece)) else {
            continue;
        };
        if !crosses(a, &seam.a.edge) || !crosses(b, &seam.b.edge) {
            continue;
        }
        let side_seam =
            (word(a, "front") && word(b, "back")) || (word(a, "back") && word(b, "front"));
        found.push((
            side_seam,
            [(a, seam.a.edge.clone()), (b, seam.b.edge.clone())],
        ));
    }
    let any_side = found.iter().any(|f| f.0);
    found
        .into_iter()
        .filter(|f| f.0 || !any_side)
        .flat_map(|(_, sides)| sides)
        .map(|(piece, edge)| {
            let copies = garment
                .panels
                .iter()
                .filter(|panel| panel.piece == pieces[piece].id)
                .flat_map(|panel| &panel.edges)
                .filter(|e| e.edge == edge)
                .count();
            Side {
                piece,
                edge,
                copies,
            }
        })
        .collect()
}

/// Where edge `i` of `shape` is at height `y` on the figure: the first
/// place it crosses it.
fn at_height(shape: &Drawn, i: usize, h: &Height, y: f64) -> Option<Pt> {
    let points = flatten_segs(shape.outline.edge_geom(i).segs(), 0.1);
    points.windows(2).find_map(|w| {
        let (ha, hb) = (h.of(w[0].into()), h.of(w[1].into()));
        ((ha - y) * (hb - y) <= 0.0 && (ha - hb).abs() > 1e-9).then(|| {
            let t = (y - ha) / (hb - ha);
            Pt::new(
                w[0].x + (w[1].x - w[0].x) * t,
                w[0].y + (w[1].y - w[0].y) * t,
            )
        })
    })
}

/// The end of edge `i` further up or down the figure from height `y`.
fn far_end(shape: &Drawn, i: usize, h: &Height, y: f64) -> Option<Pt> {
    let geom = shape.outline.edge_geom(i);
    let (start, end): (Pt, Pt) = (geom.start().into(), geom.end().into());
    Some(if (h.of(start) - y).abs() > (h.of(end) - y).abs() {
        start
    } else {
        end
    })
}
