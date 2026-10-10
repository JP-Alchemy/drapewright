//! The document engine: the one owner of a project while it is open.

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::Point;
use serde::{Deserialize, Serialize};
use soom_pattern_core::validate::validate_piece;
use soom_pattern_core::{Id, Issue, IssueCode, Piece, Pt, Severity};
use ts_rs::TS;

use crate::command::{ChangeSet, Command, CommandError, all_ids, apply};
use crate::document::{Document, FORMAT, Meta, ProjectFile, SCHEMA_VERSION, ViewState};
use crate::history::History;
use crate::hit::{Hit, hit_test};
use crate::ids::IdAllocator;
use crate::migrate::{LoadError, load};
use crate::view::PieceView;

/// Undo/redo availability, for toolbar buttons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineState {
    /// Increases with every change, including undo and redo.
    #[ts(type = "number")]
    pub revision: u64,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DispatchOptions {
    /// Commands in a row with the same key form one undo step.
    #[serde(default)]
    pub coalesce: Option<String>,
}

#[derive(Debug)]
pub struct Engine {
    history: History,
    view: ViewState,
    ids: IdAllocator,
    revision: u64,
    generator: String,
    cache: HashMap<Id, (Arc<Piece>, Arc<PieceView>)>,
}

impl Engine {
    /// A new, empty project.
    pub fn new(project_id: &str, name: &str, now: &str, generator: &str) -> Self {
        let doc = Document {
            meta: Meta {
                id: project_id.to_string(),
                name: name.to_string(),
                created: now.to_string(),
                modified: now.to_string(),
                notes: String::new(),
            },
            settings: Default::default(),
            pieces: Vec::new(),
            print: Default::default(),
            extensions: Default::default(),
        };
        Engine::from_document(doc, ViewState::default(), generator)
    }

    fn from_document(doc: Document, view: ViewState, generator: &str) -> Self {
        let ids = IdAllocator::after(all_ids(&doc));
        Engine {
            history: History::new(doc),
            view,
            ids,
            revision: 0,
            generator: generator.to_string(),
            cache: HashMap::new(),
        }
    }

    /// Opens a project file. Problems inside pieces do not stop it opening;
    /// they are returned so the app can show them.
    pub fn open(json: &str, generator: &str) -> Result<(Self, Vec<Issue>), LoadError> {
        let file = load(json)?;
        let mut issues: Vec<Issue> = file
            .document
            .pieces
            .iter()
            .flat_map(|p| validate_piece(p))
            .collect();
        let mut seen = std::collections::HashSet::new();
        for p in &file.document.pieces {
            for id in soom_pattern_core::validate::piece_ids(p) {
                if !seen.insert(id.clone()) && !issues.iter().any(|i| i.item.as_ref() == Some(id)) {
                    issues.push(Issue {
                        severity: Severity::Error,
                        code: IssueCode::DuplicateId,
                        piece: p.id.clone(),
                        item: Some(id.clone()),
                        message: format!("The id “{id}” is used more than once in this project."),
                    });
                }
            }
        }
        Ok((
            Engine::from_document(file.document, file.view, generator),
            issues,
        ))
    }

    /// The project as it would be saved, stamped as modified at `now`.
    pub fn to_file(&self, now: &str) -> ProjectFile {
        let mut document = self.document().clone();
        document.meta.modified = now.to_string();
        ProjectFile {
            format: FORMAT.to_string(),
            schema_version: SCHEMA_VERSION,
            generator: self.generator.clone(),
            document,
            view: self.view.clone(),
        }
    }

    pub fn to_json(&self, now: &str) -> String {
        serde_json::to_string_pretty(&self.to_file(now)).expect("projects always serialise")
    }

    pub fn document(&self) -> &Document {
        self.history.current()
    }

    pub fn view_state(&self) -> &ViewState {
        &self.view
    }

    /// Navigation is not an edit: it changes the view without an undo step.
    pub fn set_view_state(&mut self, view: ViewState) {
        self.view = view;
    }

    pub fn state(&self) -> EngineState {
        EngineState {
            revision: self.revision,
            can_undo: self.history.undo_label().is_some(),
            can_redo: self.history.redo_label().is_some(),
            undo_label: self.history.undo_label().map(str::to_string),
            redo_label: self.history.redo_label().map(str::to_string),
        }
    }

    pub fn dispatch(
        &mut self,
        command: Command,
        options: DispatchOptions,
    ) -> Result<ChangeSet, CommandError> {
        let label = command.label();
        let mut doc = self.document().clone();
        let set = apply(&mut doc, command, &mut self.ids)?;
        self.history.push(doc, label, options.coalesce);
        self.revision += 1;
        Ok(set)
    }

    pub fn undo(&mut self) -> Option<ChangeSet> {
        let before = self.document().clone();
        if !self.history.undo() {
            return None;
        }
        self.revision += 1;
        Some(ChangeSet::between(&before, self.document()))
    }

    pub fn redo(&mut self) -> Option<ChangeSet> {
        let before = self.document().clone();
        if !self.history.redo() {
            return None;
        }
        self.revision += 1;
        Some(ChangeSet::between(&before, self.document()))
    }

    /// Derived lines and measurements for a piece, recomputed only when the
    /// piece has changed.
    pub fn piece_view(&mut self, id: &str) -> Option<Arc<PieceView>> {
        let piece = self.document().piece(id)?.clone();
        if let Some((cached, view)) = self.cache.get(id)
            && Arc::ptr_eq(cached, &piece)
        {
            return Some(view.clone());
        }
        let view = Arc::new(PieceView::of(&piece));
        self.cache.insert(id.to_string(), (piece, view.clone()));
        Some(view)
    }

    /// What is under `world` (work-table mm), within `tolerance` mm.
    pub fn hit_test(&mut self, world: Pt, tolerance: f64) -> Option<Hit> {
        let pieces: Vec<Arc<Piece>> = self.document().pieces.clone();
        let views: Vec<Arc<PieceView>> = pieces
            .iter()
            .filter_map(|p| self.piece_view(&p.id))
            .collect();
        hit_test(
            pieces
                .iter()
                .map(|p| p.as_ref())
                .zip(views.iter().map(|v| v.as_ref())),
            Point::from(world),
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soom_pattern_core::{Edge, Outline, Segment};

    fn square(name: &str) -> Piece {
        Piece::new(
            "tmp",
            name,
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    Edge::new("a", vec![Segment::line([100.0, 0.0])]),
                    Edge::new("b", vec![Segment::line([100.0, 100.0])]),
                    Edge::new("c", vec![Segment::line([0.0, 100.0])]),
                    Edge::new("d", vec![Segment::line([0.0, 0.0])]),
                ],
            },
        )
    }

    fn engine() -> Engine {
        Engine::new("prj", "Test", "2026-10-10T00:00:00Z", "test")
    }

    #[test]
    fn add_undo_redo() {
        let mut e = engine();
        let set = e
            .dispatch(
                Command::AddPieces {
                    pieces: vec![square("A"), square("B")],
                },
                Default::default(),
            )
            .unwrap();
        assert_eq!(set.added.len(), 2);
        assert_eq!(e.document().pieces.len(), 2);
        assert_eq!(e.state().undo_label.as_deref(), Some("add pieces"));
        let undone = e.undo().unwrap();
        assert_eq!(undone.removed.len(), 2);
        assert!(e.document().pieces.is_empty());
        let redone = e.redo().unwrap();
        assert_eq!(redone.added, set.added);
        assert_eq!(e.state().revision, 3);
    }

    #[test]
    fn added_pieces_get_fresh_unique_ids() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A"), square("B")],
            },
            Default::default(),
        )
        .unwrap();
        let ids: Vec<_> = all_ids(e.document()).cloned().collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
        assert!(ids.iter().all(|id| id != "tmp"));
    }

    #[test]
    fn rejected_commands_change_nothing() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A")],
            },
            Default::default(),
        )
        .unwrap();
        let piece = e.document().pieces[0].id.clone();
        let edge = e.document().pieces[0].outline.edges[0].id.clone();
        let revision = e.state().revision;
        let err = e.dispatch(
            Command::SplitEdge {
                piece: piece.clone(),
                edge: edge.clone(),
                at: 0.0,
            },
            Default::default(),
        );
        assert!(err.is_err());
        // A notch past the end of its edge is refused by validation.
        let err = e.dispatch(
            Command::AddNotch {
                piece,
                edge,
                at: 500.0,
                from: soom_pattern_core::NotchFrom::Start,
                kind: soom_pattern_core::NotchKind::Single,
            },
            Default::default(),
        );
        assert!(matches!(err, Err(CommandError::Rejected { .. })), "{err:?}");
        assert_eq!(e.state().revision, revision);
        assert_eq!(e.document().pieces[0].notches.len(), 0);
    }

    #[test]
    fn coalesced_commands_are_one_undo_step() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A")],
            },
            Default::default(),
        )
        .unwrap();
        let id = e.document().pieces[0].id.clone();
        let opts = DispatchOptions {
            coalesce: Some("nudge-1".into()),
        };
        for _ in 0..5 {
            e.dispatch(
                Command::MovePieces {
                    ids: vec![id.clone()],
                    dx: 1.0,
                    dy: 0.0,
                },
                opts.clone(),
            )
            .unwrap();
        }
        assert!((e.document().pieces[0].placement.x - 5.0).abs() < 1e-12);
        e.undo();
        assert!(e.document().pieces[0].placement.x.abs() < 1e-12);
    }

    #[test]
    fn views_are_cached_until_the_piece_changes() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A")],
            },
            Default::default(),
        )
        .unwrap();
        let id = e.document().pieces[0].id.clone();
        let v1 = e.piece_view(&id).unwrap();
        let v2 = e.piece_view(&id).unwrap();
        assert!(Arc::ptr_eq(&v1, &v2));
        e.dispatch(
            Command::RenamePiece {
                piece: id.clone(),
                name: "B".into(),
            },
            Default::default(),
        )
        .unwrap();
        let v3 = e.piece_view(&id).unwrap();
        assert!(!Arc::ptr_eq(&v1, &v3));
    }

    #[test]
    fn hit_testing_finds_corners_edges_and_insides() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A")],
            },
            Default::default(),
        )
        .unwrap();
        let id = e.document().pieces[0].id.clone();
        e.dispatch(
            Command::MovePieces {
                ids: vec![id.clone()],
                dx: 1000.0,
                dy: 0.0,
            },
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            e.hit_test(Pt::new(1100.5, 0.5), 2.0),
            Some(Hit::Corner { .. })
        ));
        assert!(matches!(
            e.hit_test(Pt::new(1050.0, 1.0), 2.0),
            Some(Hit::Edge { .. })
        ));
        assert!(matches!(
            e.hit_test(Pt::new(1050.0, 50.0), 2.0),
            Some(Hit::Piece { .. })
        ));
        // Inside the 15 mm allowance counts as the piece.
        assert!(matches!(
            e.hit_test(Pt::new(1050.0, -10.0), 2.0),
            Some(Hit::Piece { .. })
        ));
        assert_eq!(e.hit_test(Pt::new(50.0, 50.0), 2.0), None);
    }

    #[test]
    fn saving_and_opening_round_trips() {
        let mut e = engine();
        e.dispatch(
            Command::AddPieces {
                pieces: vec![square("A")],
            },
            Default::default(),
        )
        .unwrap();
        let json = e.to_json("2026-10-11T00:00:00Z");
        let (opened, issues) = Engine::open(&json, "test").unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(opened.document().pieces, e.document().pieces);
        assert_eq!(opened.document().meta.modified, "2026-10-11T00:00:00Z");
    }

    #[test]
    fn refuses_newer_and_foreign_files() {
        assert_eq!(
            Engine::open(r#"{"format":"soom-project","schemaVersion":99}"#, "t").unwrap_err(),
            LoadError::TooNew { found: 99 }
        );
        assert_eq!(
            Engine::open(r#"{"format":"other"}"#, "t").unwrap_err(),
            LoadError::NotSoom
        );
        assert!(matches!(
            Engine::open("{", "t").unwrap_err(),
            LoadError::NotJson(_)
        ));
    }
}
