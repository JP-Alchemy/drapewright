//! Every change to a project is a `Command`. Commands are plain data, so
//! they can come from any tool, be logged, and be replayed.

use std::collections::HashSet;
use std::sync::Arc;

use drapewright_pattern_core::build::reassign_ids;
use drapewright_pattern_core::ops::{self, OpError};
use drapewright_pattern_core::validate::{piece_ids, validate_piece};
use drapewright_pattern_core::{
    Allowance, CornerStyle, CutInstruction, EdgeKind, Grainline, Id, Notch, NotchFrom, NotchKind,
    Piece, Severity,
};
use kurbo::{Affine, Point};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::document::{DisplayUnits, Document, PrintSettings};
use crate::ids::IdAllocator;
use crate::measurements::{EarlierMeasurements, MeasurementSet};
use drapewright_seams::Seam;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Command {
    /// Adds pieces, for example from an import. They get fresh ids.
    AddPieces {
        pieces: Vec<Piece>,
    },
    DeletePieces {
        ids: Vec<Id>,
    },
    RenamePiece {
        piece: Id,
        name: String,
    },
    /// Moves pieces on the work table, mm.
    MovePieces {
        ids: Vec<Id>,
        dx: f64,
        dy: f64,
    },
    /// Turns a piece on the work table about its centre, degrees.
    RotatePiece {
        piece: Id,
        degrees: f64,
    },
    SetAllowance {
        piece: Id,
        allowance: Allowance,
    },
    /// `None` returns the edge to the piece's allowance.
    SetEdgeAllowance {
        piece: Id,
        edge: Id,
        allowance: Option<f64>,
    },
    SetEdgeKind {
        piece: Id,
        edge: Id,
        kind: EdgeKind,
    },
    /// The corner at the end of `edge`.
    SetCornerStyle {
        piece: Id,
        edge: Id,
        style: CornerStyle,
    },
    RenameEdge {
        piece: Id,
        edge: Id,
        name: String,
    },
    /// Splits `edge` at `at` mm from its start, making a new corner there.
    SplitEdge {
        piece: Id,
        edge: Id,
        at: f64,
    },
    /// Joins `edge` with the edge after it.
    JoinEdges {
        piece: Id,
        edge: Id,
    },
    SetGrainline {
        piece: Id,
        grainline: Option<Grainline>,
    },
    AddNotch {
        piece: Id,
        edge: Id,
        at: f64,
        from: NotchFrom,
        kind: NotchKind,
    },
    /// Replaces the notch with the same id.
    SetNotch {
        piece: Id,
        notch: Notch,
    },
    DeleteNotch {
        piece: Id,
        notch: Id,
    },
    SetCut {
        piece: Id,
        cut: Vec<CutInstruction>,
    },
    SetPrintSettings {
        print: PrintSettings,
    },
    RenameProject {
        name: String,
    },
    SetDisplayUnits {
        units: DisplayUnits,
    },
    /// Adds a person's measurements. The set gets a fresh id.
    AddMeasurementSet {
        set: MeasurementSet,
    },
    /// Replaces the measurement set with the same id.
    SetMeasurementSet {
        set: MeasurementSet,
    },
    /// Starts measuring a person again: the current values are kept as the
    /// previous revision, and the new revision starts from them.
    RemeasureSet {
        id: Id,
        taken_on: Option<String>,
    },
    DeleteMeasurementSet {
        id: Id,
    },
    /// Sews edges together: suggestions accepted, or a pair chosen by hand.
    /// Each seam gets a fresh id.
    AddSeams {
        seams: Vec<Seam>,
    },
    /// Replaces the seam with the same id (its direction, name, parts of
    /// edges, or confirming it).
    SetSeam {
        seam: Seam,
    },
    DeleteSeams {
        ids: Vec<Id>,
    },
}

impl Command {
    /// A short description for undo and redo ("Undo rename piece").
    pub fn label(&self) -> &'static str {
        match self {
            Command::AddPieces { .. } => "add pieces",
            Command::DeletePieces { .. } => "delete pieces",
            Command::RenamePiece { .. } => "rename piece",
            Command::MovePieces { .. } => "move",
            Command::RotatePiece { .. } => "turn piece",
            Command::SetAllowance { .. } => "seam allowance",
            Command::SetEdgeAllowance { .. } => "seam allowance",
            Command::SetEdgeKind { .. } => "edge type",
            Command::SetCornerStyle { .. } => "corner shape",
            Command::RenameEdge { .. } => "rename edge",
            Command::SplitEdge { .. } => "add corner",
            Command::JoinEdges { .. } => "remove corner",
            Command::SetGrainline { .. } => "grainline",
            Command::AddNotch { .. } => "add notch",
            Command::SetNotch { .. } => "change notch",
            Command::DeleteNotch { .. } => "delete notch",
            Command::SetCut { .. } => "cutting instructions",
            Command::SetPrintSettings { .. } => "print settings",
            Command::RenameProject { .. } => "rename project",
            Command::SetDisplayUnits { .. } => "units",
            Command::AddMeasurementSet { .. } => "add measurements",
            Command::SetMeasurementSet { .. } => "measurements",
            Command::RemeasureSet { .. } => "measure again",
            Command::DeleteMeasurementSet { .. } => "delete measurements",
            Command::AddSeams { .. } => "sew edges",
            Command::SetSeam { .. } => "change seam",
            Command::DeleteSeams { .. } => "unpick seam",
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "code"
)]
#[ts(export)]
pub enum CommandError {
    #[error("That piece no longer exists.")]
    UnknownPiece { piece: Id },
    #[error("That edge is not on this piece.")]
    UnknownEdge { piece: Id, edge: Id },
    #[error("That notch no longer exists.")]
    UnknownNotch { piece: Id, notch: Id },
    #[error("Those measurements no longer exist.")]
    UnknownMeasurementSet { id: Id },
    #[error("That seam no longer exists.")]
    UnknownSeam { id: Id },
    #[error("{message}")]
    Rejected { message: String },
}

impl From<OpError> for CommandError {
    fn from(e: OpError) -> Self {
        CommandError::Rejected {
            message: e.to_string(),
        }
    }
}

/// What a command changed, so views can update only what they must.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeSet {
    pub added: Vec<Id>,
    pub changed: Vec<Id>,
    pub removed: Vec<Id>,
    /// Project name, settings, print settings, measurements or seams
    /// changed.
    pub document: bool,
}

impl ChangeSet {
    fn changed(id: &Id) -> Self {
        ChangeSet {
            changed: vec![id.clone()],
            ..Default::default()
        }
    }

    /// The change between two document versions (for undo and redo).
    pub fn between(before: &Document, after: &Document) -> Self {
        let mut set = ChangeSet::default();
        for p in &after.pieces {
            match before.piece(&p.id) {
                None => set.added.push(p.id.clone()),
                Some(old) if !Arc::ptr_eq(old, p) => set.changed.push(p.id.clone()),
                _ => {}
            }
        }
        set.removed = before
            .pieces
            .iter()
            .filter(|p| after.piece(&p.id).is_none())
            .map(|p| p.id.clone())
            .collect();
        set.document = before.meta != after.meta
            || before.settings != after.settings
            || before.print != after.print
            || before.measurement_sets != after.measurement_sets
            || before.seams != after.seams;
        set
    }
}

fn piece_mut<'a>(doc: &'a mut Document, id: &Id) -> Result<&'a mut Piece, CommandError> {
    let i = doc
        .piece_index(id)
        .ok_or_else(|| CommandError::UnknownPiece { piece: id.clone() })?;
    Ok(Arc::make_mut(&mut doc.pieces[i]))
}

fn edge_index(piece: &Piece, edge: &Id) -> Result<usize, CommandError> {
    piece
        .outline
        .edge_index(edge)
        .ok_or_else(|| CommandError::UnknownEdge {
            piece: piece.id.clone(),
            edge: edge.clone(),
        })
}

/// Errors in `after` that `before` did not have.
fn new_errors(before: Option<&Piece>, after: &Piece) -> Option<String> {
    let old: HashSet<_> = before
        .map(|b| {
            validate_piece(b)
                .into_iter()
                .filter(|i| i.severity == Severity::Error)
                .map(|i| (i.code, i.item))
                .collect()
        })
        .unwrap_or_default();
    validate_piece(after)
        .into_iter()
        .filter(|i| i.severity == Severity::Error)
        .find(|i| !old.contains(&(i.code, i.item.clone())))
        .map(|i| i.message)
}

/// Applies a command to a document. On error the document is unchanged.
pub fn apply(
    doc: &mut Document,
    command: Command,
    ids: &mut IdAllocator,
) -> Result<ChangeSet, CommandError> {
    let before = doc.clone();
    let result = apply_inner(doc, command, ids);
    match result {
        Ok(set) => {
            // No command may leave a piece with a new error.
            for id in set.added.iter().chain(&set.changed) {
                let after = doc.piece(id).expect("changed piece exists");
                if let Some(message) = new_errors(before.piece(id).map(|p| p.as_ref()), after) {
                    *doc = before;
                    return Err(CommandError::Rejected { message });
                }
            }
            Ok(set)
        }
        Err(e) => {
            *doc = before;
            Err(e)
        }
    }
}

fn apply_inner(
    doc: &mut Document,
    command: Command,
    ids: &mut IdAllocator,
) -> Result<ChangeSet, CommandError> {
    match command {
        Command::AddPieces { pieces } => {
            let mut set = ChangeSet::default();
            for mut piece in pieces {
                reassign_ids(&mut piece, &mut |prefix| ids.next(prefix));
                ops::normalize_orientation(&mut piece);
                set.added.push(piece.id.clone());
                doc.pieces.push(Arc::new(piece));
            }
            Ok(set)
        }
        Command::DeletePieces { ids: gone } => {
            for id in &gone {
                doc.piece_index(id)
                    .ok_or_else(|| CommandError::UnknownPiece { piece: id.clone() })?;
            }
            doc.pieces.retain(|p| !gone.contains(&p.id));
            // Seams sewing a piece that is gone go with it.
            let seams = doc.seams.len();
            doc.seams.retain(|s| !gone.iter().any(|id| s.touches(id)));
            Ok(ChangeSet {
                removed: gone,
                document: doc.seams.len() != seams,
                ..Default::default()
            })
        }
        Command::RenamePiece { piece, name } => {
            piece_mut(doc, &piece)?.name = name;
            Ok(ChangeSet::changed(&piece))
        }
        Command::MovePieces { ids: moved, dx, dy } => {
            for id in &moved {
                let p = piece_mut(doc, id)?;
                p.placement.x += dx;
                p.placement.y += dy;
            }
            Ok(ChangeSet {
                changed: moved,
                ..Default::default()
            })
        }
        Command::RotatePiece { piece, degrees } => {
            let p = piece_mut(doc, &piece)?;
            let c = p.outline.bounds().center();
            let place = |rotation: f64, x: f64, y: f64| {
                Affine::translate((x, y)) * Affine::rotate(rotation.to_radians())
            };
            let world_c = place(p.placement.rotation, p.placement.x, p.placement.y) * c;
            let rotation = (p.placement.rotation + degrees).rem_euclid(360.0);
            let turned: Point = Affine::rotate(rotation.to_radians()) * c;
            p.placement.rotation = rotation;
            p.placement.x = world_c.x - turned.x;
            p.placement.y = world_c.y - turned.y;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetAllowance { piece, allowance } => {
            if allowance.seam < 0.0 || allowance.hem.is_some_and(|h| h < 0.0) {
                return Err(CommandError::Rejected {
                    message: "Seam allowance cannot be negative.".into(),
                });
            }
            piece_mut(doc, &piece)?.allowance = allowance;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetEdgeAllowance {
            piece,
            edge,
            allowance,
        } => {
            if allowance.is_some_and(|a| a < 0.0) {
                return Err(CommandError::Rejected {
                    message: "Seam allowance cannot be negative.".into(),
                });
            }
            let p = piece_mut(doc, &piece)?;
            let i = edge_index(p, &edge)?;
            p.outline.edges[i].allowance = allowance;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetEdgeKind { piece, edge, kind } => {
            let p = piece_mut(doc, &piece)?;
            let i = edge_index(p, &edge)?;
            p.outline.edges[i].kind = kind;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetCornerStyle { piece, edge, style } => {
            let p = piece_mut(doc, &piece)?;
            let i = edge_index(p, &edge)?;
            p.outline.edges[i].end_corner = style;
            Ok(ChangeSet::changed(&piece))
        }
        Command::RenameEdge { piece, edge, name } => {
            let p = piece_mut(doc, &piece)?;
            let i = edge_index(p, &edge)?;
            p.outline.edges[i].name = name;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SplitEdge { piece, edge, at } => {
            unsewn(doc, &piece, &[&edge])?;
            let new_id = ids.next("e");
            ops::split_edge(piece_mut(doc, &piece)?, &edge, at, new_id)?;
            Ok(ChangeSet::changed(&piece))
        }
        Command::JoinEdges { piece, edge } => {
            let p = doc
                .piece(&piece)
                .ok_or_else(|| CommandError::UnknownPiece {
                    piece: piece.clone(),
                })?;
            let i = edge_index(p, &edge)?;
            let next = p.outline.edges[(i + 1) % p.outline.edges.len()].id.clone();
            unsewn(doc, &piece, &[&edge, &next])?;
            ops::join_with_next(piece_mut(doc, &piece)?, &edge)?;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetGrainline { piece, grainline } => {
            piece_mut(doc, &piece)?.grainline = grainline;
            Ok(ChangeSet::changed(&piece))
        }
        Command::AddNotch {
            piece,
            edge,
            at,
            from,
            kind,
        } => {
            let id = ids.next("n");
            let p = piece_mut(doc, &piece)?;
            edge_index(p, &edge)?;
            p.notches.push(Notch {
                id,
                edge,
                at,
                from,
                kind,
                angle: 0.0,
            });
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetNotch { piece, notch } => {
            let p = piece_mut(doc, &piece)?;
            let slot = p
                .notches
                .iter_mut()
                .find(|n| n.id == notch.id)
                .ok_or_else(|| CommandError::UnknownNotch {
                    piece: piece.clone(),
                    notch: notch.id.clone(),
                })?;
            *slot = notch;
            Ok(ChangeSet::changed(&piece))
        }
        Command::DeleteNotch { piece, notch } => {
            let p = piece_mut(doc, &piece)?;
            let before = p.notches.len();
            p.notches.retain(|n| n.id != notch);
            if p.notches.len() == before {
                return Err(CommandError::UnknownNotch { piece, notch });
            }
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetCut { piece, cut } => {
            piece_mut(doc, &piece)?.cut = cut;
            Ok(ChangeSet::changed(&piece))
        }
        Command::SetPrintSettings { print } => {
            doc.print = print;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::RenameProject { name } => {
            doc.meta.name = name;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::SetDisplayUnits { units } => {
            doc.settings.display_units = units;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::AddMeasurementSet { mut set } => {
            set.id = ids.next("m");
            checked(&set)?;
            doc.measurement_sets.push(set);
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::SetMeasurementSet { set } => {
            checked(&set)?;
            let id = set.id.clone();
            *measurement_set_mut(doc, &id)? = set;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::RemeasureSet { id, taken_on } => {
            let set = measurement_set_mut(doc, &id)?;
            set.previous.insert(
                0,
                EarlierMeasurements {
                    revision: set.revision,
                    taken_on: set.taken_on.take(),
                    values: set.values.clone(),
                },
            );
            set.revision += 1;
            set.taken_on = taken_on;
            checked(set)?;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::DeleteMeasurementSet { id } => {
            measurement_set_mut(doc, &id)?;
            doc.measurement_sets.retain(|m| m.id != id);
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::AddSeams { seams } => {
            for mut seam in seams {
                seam.id = ids.next("s");
                checked_seam(doc, &seam)?;
                doc.seams.push(seam);
            }
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::SetSeam { seam } => {
            checked_seam(doc, &seam)?;
            let slot = doc
                .seams
                .iter_mut()
                .find(|s| s.id == seam.id)
                .ok_or_else(|| CommandError::UnknownSeam {
                    id: seam.id.clone(),
                })?;
            *slot = seam;
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
        Command::DeleteSeams { ids: gone } => {
            for id in &gone {
                doc.seam(id)
                    .ok_or_else(|| CommandError::UnknownSeam { id: id.clone() })?;
            }
            doc.seams.retain(|s| !gone.contains(&s.id));
            Ok(ChangeSet {
                document: true,
                ..Default::default()
            })
        }
    }
}

/// Adding or removing a corner on a sewn edge would change what is sewn to
/// what; the seam is unpicked first.
fn unsewn(doc: &Document, piece: &Id, edges: &[&Id]) -> Result<(), CommandError> {
    let sewn = doc.seams.iter().any(|s| {
        [&s.a, &s.b]
            .iter()
            .any(|side| side.piece == *piece && edges.contains(&&side.edge))
    });
    if sewn {
        return Err(CommandError::Rejected {
            message: "This edge is sewn to another. Unpick its seam first, then add or remove the corner."
                .into(),
        });
    }
    Ok(())
}

/// A seam that can be kept: both sides measurable, and not one side sewn to
/// itself.
fn checked_seam(doc: &Document, seam: &Seam) -> Result<(), CommandError> {
    if seam.a == seam.b {
        return Err(CommandError::Rejected {
            message: "An edge can't be sewn to itself; choose its partner.".into(),
        });
    }
    match drapewright_seams::measure(seam, &doc.pieces).problem {
        Some(_) => Err(CommandError::Rejected {
            message: "That seam's edges are not on these pieces.".into(),
        }),
        None => Ok(()),
    }
}

fn measurement_set_mut<'a>(
    doc: &'a mut Document,
    id: &Id,
) -> Result<&'a mut MeasurementSet, CommandError> {
    doc.measurement_sets
        .iter_mut()
        .find(|m| m.id == *id)
        .ok_or_else(|| CommandError::UnknownMeasurementSet { id: id.clone() })
}

fn checked(set: &MeasurementSet) -> Result<(), CommandError> {
    match set.problem() {
        Some(message) => Err(CommandError::Rejected { message }),
        None => Ok(()),
    }
}

/// Every id used anywhere in the document.
pub fn all_ids(doc: &Document) -> impl Iterator<Item = &Id> {
    doc.pieces
        .iter()
        .flat_map(|p| piece_ids(p))
        .chain(doc.measurement_sets.iter().map(|m| &m.id))
        .chain(doc.seams.iter().map(|s| &s.id))
}
