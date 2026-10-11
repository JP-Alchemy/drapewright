//! PDF in both directions. Printing at true size: the pieces laid out on
//! one big sheet, cut into pages for a home printer (A4, Letter…) that print
//! at exactly 100 % and join edge to edge, or printed whole on large paper
//! at a copy shop. And reading PDF patterns back in (`import_pdf`).

mod butt;
mod import;
mod lettering;
mod loops;
mod marks;
mod plan;
mod sizes;
mod text;
mod tiles;
mod write;

pub use import::import_pdf;
pub use plan::{PlannedPage, PrintError, PrintPlan, SheetPlacement, plan_print};
pub use write::{PdfTexts, export_pdf};
