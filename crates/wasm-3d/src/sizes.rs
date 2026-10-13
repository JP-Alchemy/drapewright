//! Each size of a pattern sewn and measured on a person's figure: how big
//! it is round the bust, waist and hip, and the ease there, to choose a size
//! by. Each size is sewn as Find seams would sew it: panels drawn for one
//! side cut as pairs, and every seam suggested taken.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::{CutInstruction, Piece};
use drapewright_seams::{Seam, drawn_for_one_side, suggest};
use drapewright_sim::garment::{assemble, draped_pieces};
use drapewright_sim::girth::girths;
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
    let Ok(garment) = assemble(&draped.sewn, &seams, SPACING) else {
        return Vec::new();
    };
    if half_round(&garment) {
        return Vec::new();
    }
    let starts = place(&garment, figure);
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
