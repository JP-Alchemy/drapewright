//! Seams: which edges of which pieces are sewn together, how long each side
//! is on the stitching line, how the notches meet, and suggestions for a
//! pattern that has no seams yet. Every suggestion waits to be confirmed;
//! nothing here changes a piece.

mod measure;
mod model;
mod suggest;

pub use measure::{NotchMeeting, SeamProblem, SeamReport, measure, side_length};
pub use model::{Ease, Seam, SeamSide};
pub use suggest::{Reason, Suggestion, sewn_reversed, suggest};
