//! Random edit sessions: whatever the user does, ids stay unique, pieces
//! stay valid, and undo takes everything back exactly.

use std::collections::HashSet;

use drapewright_pattern_core::testing::arb_piece;
use drapewright_pattern_core::validate::validate_piece;
use drapewright_pattern_core::{Allowance, CornerStyle, EdgeKind, NotchFrom, NotchKind, Severity};
use drapewright_project::command::all_ids;
use drapewright_project::{Command, Engine};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Step {
    Move(f64, f64),
    Rotate(f64),
    Allowance(f64),
    EdgeAllowance(usize, Option<f64>),
    Kind(usize, u8),
    Corner(usize, u8),
    Split(usize, f64),
    Join(usize),
    Notch(usize, f64, bool),
    DeleteNotch(usize),
    Undo,
    Redo,
}

fn arb_step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (-50.0f64..50.0, -50.0f64..50.0).prop_map(|(x, y)| Step::Move(x, y)),
        (-180.0f64..180.0).prop_map(Step::Rotate),
        (0.0f64..30.0).prop_map(Step::Allowance),
        (any::<usize>(), prop::option::of(0.0f64..40.0))
            .prop_map(|(e, a)| Step::EdgeAllowance(e, a)),
        (any::<usize>(), 0u8..3).prop_map(|(e, k)| Step::Kind(e, k)),
        (any::<usize>(), 0u8..6).prop_map(|(e, c)| Step::Corner(e, c)),
        (any::<usize>(), 0.0f64..1.0).prop_map(|(e, f)| Step::Split(e, f)),
        any::<usize>().prop_map(Step::Join),
        (any::<usize>(), 0.0f64..1.0, any::<bool>()).prop_map(|(e, f, end)| Step::Notch(e, f, end)),
        any::<usize>().prop_map(Step::DeleteNotch),
        Just(Step::Undo),
        Just(Step::Redo),
    ]
}

fn command(e: &Engine, step: &Step) -> Option<Command> {
    let piece = e.document().pieces.first()?;
    let id = piece.id.clone();
    let n = piece.outline.edges.len();
    let edge = |i: &usize| piece.outline.edges[i % n].id.clone();
    Some(match step {
        Step::Move(dx, dy) => Command::MovePieces {
            ids: vec![id],
            dx: *dx,
            dy: *dy,
        },
        Step::Rotate(d) => Command::RotatePiece {
            piece: id,
            degrees: *d,
            about: None,
        },
        Step::Allowance(a) => Command::SetAllowance {
            piece: id,
            allowance: Allowance {
                seam: *a,
                ..piece.allowance
            },
        },
        Step::EdgeAllowance(i, a) => Command::SetEdgeAllowance {
            piece: id,
            edge: edge(i),
            allowance: *a,
        },
        Step::Kind(i, k) => Command::SetEdgeKind {
            piece: id,
            edge: edge(i),
            kind: [EdgeKind::Seam, EdgeKind::Hem, EdgeKind::Fold][*k as usize],
        },
        Step::Corner(i, c) => Command::SetCornerStyle {
            piece: id,
            edge: edge(i),
            style: [
                CornerStyle::Auto,
                CornerStyle::Mitre,
                CornerStyle::SquareThis,
                CornerStyle::SquareNext,
                CornerStyle::MirrorThis,
                CornerStyle::MirrorNext,
            ][*c as usize],
        },
        Step::Split(i, f) => {
            let len = piece.outline.edge_geom(i % n).length();
            Command::SplitEdge {
                piece: id,
                edge: edge(i),
                at: f * len,
            }
        }
        Step::Join(i) => Command::JoinEdges {
            piece: id,
            edge: edge(i),
        },
        Step::Notch(i, f, end) => {
            let len = piece.outline.edge_geom(i % n).length();
            Command::AddNotch {
                piece: id,
                edge: edge(i),
                at: f * len,
                from: if *end {
                    NotchFrom::End
                } else {
                    NotchFrom::Start
                },
                kind: NotchKind::Single,
            }
        }
        Step::DeleteNotch(i) => {
            let notch = piece
                .notches
                .get(i % piece.notches.len().max(1))?
                .id
                .clone();
            Command::DeleteNotch { piece: id, notch }
        }
        Step::Undo | Step::Redo => return None,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn random_sessions_keep_the_document_sound(
        piece in arb_piece(),
        steps in prop::collection::vec(arb_step(), 1..40),
    ) {
        prop_assume!(validate_piece(&piece).iter().all(|i| i.severity != Severity::Error));
        let mut e = Engine::new("prj", "Session", "2026-10-10T00:00:00Z", "test");
        e.dispatch(Command::AddPieces { pieces: vec![piece] }, Default::default()).unwrap();
        let start = e.document().clone();
        let mut applied = 0usize;

        for step in &steps {
            match step {
                Step::Undo => {
                    if applied > 0 && e.undo().is_some() {
                        applied -= 1;
                    }
                }
                Step::Redo => {
                    if e.redo().is_some() {
                        applied += 1;
                    }
                }
                _ => {
                    if let Some(cmd) = command(&e, step)
                        && e.dispatch(cmd, Default::default()).is_ok()
                    {
                        applied += 1;
                    }
                }
            }
            let ids: Vec<_> = all_ids(e.document()).collect();
            let unique: HashSet<_> = ids.iter().collect();
            prop_assert_eq!(ids.len(), unique.len(), "duplicate ids after {:?}", step);
            for p in &e.document().pieces {
                let errors: Vec<_> = validate_piece(p).into_iter().filter(|i| i.severity == Severity::Error).collect();
                prop_assert!(errors.is_empty(), "{:?} left errors {:?}", step, errors);
            }
        }

        let end = e.document().clone();
        for _ in 0..applied {
            prop_assert!(e.undo().is_some());
        }
        prop_assert_eq!(&e.document().pieces, &start.pieces);
        for _ in 0..applied {
            prop_assert!(e.redo().is_some());
        }
        prop_assert_eq!(&e.document().pieces, &end.pieces);
    }
}
