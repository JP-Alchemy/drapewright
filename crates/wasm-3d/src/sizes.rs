//! Each size of a pattern sewn and measured on a person's figure: how big
//! it is round the bust, waist and hip, and the ease there, to choose a size
//! by; and the pattern cut in a different size at each, blended between
//! them. Each size is sewn as Find seams would sew it: panels drawn for one
//! side cut as pairs, and every seam suggested taken.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::blend::{Sized, blend};
use drapewright_pattern_core::geom::flatten_segs;
use drapewright_pattern_core::{CutInstruction, Piece};
use drapewright_seams::{Seam, drawn_for_one_side, suggest};
use drapewright_sim::garment::{Garment, assemble, between_legs, draped_pieces};
use drapewright_sim::girth::girths;
use drapewright_sim::heights::{Worn, worn};
use drapewright_sim::place::{Figure, half_round, place};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AvatarSettings;
use crate::drape::{Girth, SPACING};

/// The sizes to measure, on the avatar shaped by `settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SizesRequest {
    pub sizes: Vec<SizePieces>,
    pub settings: AvatarSettings,
}

/// One size's pieces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SizePieces {
    pub size: String,
    pub pieces: Vec<Piece>,
}

/// One size, measured: none where it couldn't be sewn round the body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SizeMeasures {
    pub size: String,
    pub girths: Vec<Girth>,
}

/// Sews each size and measures it on the figure.
pub fn measure_sizes(model: &BodyModel, request: &SizesRequest) -> Vec<SizeMeasures> {
    let body = model.shape(&Shape::from(request.settings.clone()));
    let figure = Figure::of(model, &body);
    request
        .sizes
        .iter()
        .map(|size| SizeMeasures {
            size: size.size.clone(),
            girths: measure(&size.pieces, &figure),
        })
        .collect()
}

fn measure(pieces: &[Piece], figure: &Figure) -> Vec<Girth> {
    let Some((_, garment, starts)) = sewn(pieces, figure) else {
        return Vec::new();
    };
    girths(&garment, &starts, figure)
        .into_iter()
        .zip(&figure.levels)
        .map(|((level, garment), at)| Girth {
            level: level.into(),
            garment,
            body: at.girth,
        })
        .collect()
}

/// Each point of each panel as placed on the figure.
type Starts = Vec<Vec<[f64; 3]>>;

/// The pieces sewn as Find seams would sew them and put on the figure: the
/// pieces as sewn, the garment, and where its points start. None where it
/// can't be sewn round the body.
fn sewn(pieces: &[Piece], figure: &Figure) -> Option<(Vec<Piece>, Garment, Starts)> {
    let mut pieces = pieces.to_vec();
    for id in drawn_for_one_side(&pieces, &[]) {
        if let Some(piece) = pieces.iter_mut().find(|p| p.id == id) {
            as_pairs(piece);
        }
    }
    let seams: Vec<Seam> = suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    let draped = draped_pieces(&pieces, &seams);
    let garment = assemble(&draped.sewn, &seams, SPACING).ok()?;
    if half_round(&garment) {
        return None;
    }
    let starts = place(&garment, figure);
    Some((pieces, garment, starts))
}

/// The size to cut at one of the figure's levels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LevelSize {
    /// "bust", "waist" or "hip".
    pub level: String,
    pub size: String,
}

/// A pattern's sizes, to cut in the size chosen at each level and blend
/// between, on the avatar shaped by `settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlendRequest {
    /// Every size, smallest first.
    pub sizes: Vec<SizePieces>,
    pub settings: AvatarSettings,
    pub at: Vec<LevelSize>,
}

/// The pattern blended: a piece for each piece of the pattern, and those
/// cut in one size, not blended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Blended {
    pub pieces: Vec<Piece>,
    pub notes: Vec<BlendNote>,
}

/// A piece cut in one size, by name, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlendNote {
    pub piece: String,
    pub size: String,
    pub why: OneSize,
}

/// Why a piece is cut in one size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OneSize {
    /// It is all where one size is chosen (a waistband at the waist).
    OneLevel,
    /// It goes round an arm: the size at the top.
    RoundAnArm,
    /// It goes between the legs (a crotch piece): the size at the bottom.
    BetweenLegs,
    /// It isn't sewn round the body (a facing, a pocket): the size at the
    /// top.
    NotWorn,
    /// The pattern hasn't it in every size blended between.
    NotInEverySize,
}

/// The pattern cut in the size `request` chooses at each level, blended
/// smoothly between them up and down the figure.
pub fn blend_sizes(model: &BodyModel, request: &BlendRequest) -> Result<Blended, String> {
    let body = model.shape(&Shape::from(request.settings.clone()));
    let figure = Figure::of(model, &body);
    let names: Vec<&str> = request.sizes.iter().map(|s| s.size.as_str()).collect();
    // Each chosen level, top down: how high it is and which size.
    let mut levels: Vec<(f64, usize)> = request
        .at
        .iter()
        .filter_map(|at| {
            let level = figure.levels.iter().find(|l| l.name == at.level)?;
            let size = names.iter().position(|&n| n == at.size)?;
            Some((level.y, size))
        })
        .collect();
    levels.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (Some(&(_, top)), Some(&(_, bottom)), Some(lo), Some(hi)) = (
        levels.first(),
        levels.last(),
        levels.iter().map(|l| l.1).min(),
        levels.iter().map(|l| l.1).max(),
    ) else {
        return Err("no size chosen at any level".into());
    };
    let reference = levels[(levels.len() - 1) / 2].1;
    let index = |h: f64| -> f64 {
        let first = levels[0];
        if h >= first.0 {
            return first.1 as f64;
        }
        for w in levels.windows(2) {
            let ((ya, a), (yb, b)) = (w[0], w[1]);
            if h >= yb {
                let s = ((ya - h) / (ya - yb)).clamp(0.0, 1.0);
                return a as f64 + (b as f64 - a as f64) * s * s * (3.0 - 2.0 * s);
            }
        }
        levels[levels.len() - 1].1 as f64
    };
    // Each size between, sewn: where each piece goes on the figure.
    let worn: Vec<Vec<(String, Worn)>> = (lo..=hi)
        .map(|k| {
            let pieces = &request.sizes[k].pieces;
            sewn(pieces, &figure)
                .map(|(sewn, g, starts)| worn(&g, &starts, &sewn, &figure))
                .unwrap_or_default()
        })
        .collect();
    if worn[reference - lo].is_empty() {
        return Err(format!(
            "size {} can't be sewn round the body",
            names[reference]
        ));
    }
    let worn_as = |k: usize, id: &str| {
        worn[k - lo]
            .iter()
            .find(|(w, _)| w == id)
            .map(|(_, how)| how.clone())
    };

    let mut out = Blended {
        pieces: Vec::new(),
        notes: Vec::new(),
    };
    let base = &request.sizes[reference].pieces;
    for (j, piece) in base.iter().enumerate() {
        // The same piece in each size: shared by every size, or the one of
        // its name (the first, second ... of that name, as here).
        let nth = base[..j].iter().filter(|p| p.name == piece.name).count();
        let members: Option<Vec<&Piece>> = (lo..=hi)
            .map(|k| {
                let pieces = &request.sizes[k].pieces;
                pieces
                    .iter()
                    .find(|p| p.id == piece.id)
                    .or_else(|| pieces.iter().filter(|p| p.name == piece.name).nth(nth))
            })
            .collect();
        let one = |out: &mut Blended, k: usize, why: OneSize, cut: &Piece| {
            out.pieces.push(cut.clone());
            out.notes.push(BlendNote {
                piece: piece.name.clone(),
                size: names[k].into(),
                why,
            });
        };
        let Some(members) = members else {
            one(&mut out, reference, OneSize::NotInEverySize, piece);
            continue;
        };
        if members.iter().all(|m| m.id == piece.id) {
            out.pieces.push(piece.clone());
            continue;
        }
        let height = match worn_as(reference, &piece.id) {
            Some(Worn::Trunk(height)) => height,
            Some(Worn::Arm) => {
                one(&mut out, top, OneSize::RoundAnArm, members[top - lo]);
                continue;
            }
            None if between_legs(piece) => {
                one(&mut out, bottom, OneSize::BetweenLegs, members[bottom - lo]);
                continue;
            }
            None => {
                one(&mut out, top, OneSize::NotWorn, members[top - lo]);
                continue;
            }
        };
        // All of it where one size is chosen: that size as drawn.
        let along = flatten_segs(&piece.outline.path_segs(), 1.0);
        let sizes: Vec<f64> = along.iter().map(|&p| index(height.of(p.into()))).collect();
        let (least, most) = sizes
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, m), &t| (l.min(t), m.max(t)));
        if most - least < 1e-9 && (least - least.round()).abs() < 1e-9 {
            let k = least.round() as usize;
            one(&mut out, k, OneSize::OneLevel, members[k - lo]);
            continue;
        }
        let sized: Vec<Sized> = members
            .iter()
            .enumerate()
            .map(|(i, &m)| Sized {
                piece: m,
                height: match worn_as(lo + i, &m.id) {
                    Some(Worn::Trunk(h)) => Some(h),
                    _ => None,
                },
            })
            .collect();
        match blend(&sized, reference - lo, |h| index(h) - lo as f64) {
            Some(blended) => out.pieces.push(blended),
            None => one(&mut out, reference, OneSize::NotWorn, piece),
        }
    }
    Ok(out)
}

/// Cut as mirrored pairs, a left and a right.
fn as_pairs(piece: &mut Piece) {
    if piece.cut.is_empty() {
        piece.cut.push(CutInstruction::default());
    }
    for cut in &mut piece.cut {
        cut.quantity = 2;
        cut.mirrored = Some(true);
    }
}
