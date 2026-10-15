//! A garment draped on the figure from start to finish: the pieces sewn by
//! confirmed seams placed round the body, its facings and linings inside
//! it, its buttons done up or not, let go, measured again as it settles,
//! and moved into a pose. The app moves it on a frame at a time (on the
//! graphics card where it can); the assistants' server until it is done.

use drapewright_body::{Body, BodyModel, Pose, Shape};
use drapewright_pattern_core::Piece;
use drapewright_seams::Seam;

use crate::drape::{Drape, Posable, Stepper};
use crate::fabric::Fabric;
use crate::garment::{Garment, GarmentError, assemble, draped_pieces};
use crate::girth::girths;
use crate::inside::with_inside;
use crate::place::{Figure, from_shoulders, half_round, place};

/// A frame of the drape, s.
pub const FRAME: f32 = 1.0 / 60.0;
/// The longest a drape is given to settle, s.
pub const LONGEST: f32 = 20.0;

/// What to drape.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub pieces: &'a [Piece],
    /// The seams: those confirmed are sewn.
    pub seams: &'a [Seam],
    pub fabric: Fabric,
    /// Worn open: its buttons undone.
    pub open: bool,
    /// How far apart the cloth's points are, mm.
    pub spacing: f64,
}

/// Where a drape has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The seams closing round the body.
    Sewing,
    /// Sewn, and coming to rest.
    Settling,
    /// The body moving into a pose, the garment with it.
    Posing,
    /// Hanging still.
    Settled,
    /// Still moving after the longest a drape is given.
    Stopped,
}

/// How big the garment is round the body at one of the figure's levels,
/// measured flat as a sewist measures a finished garment.
#[derive(Clone, Debug, PartialEq)]
pub struct Girth {
    /// `bust`, `waist` or `hip`.
    pub level: &'static str,
    /// The garment round the body there, mm; none where it doesn't reach.
    pub garment: Option<f64>,
    /// The body round there, mm.
    pub body: f64,
    /// How high the level is, mm.
    pub y: f64,
}

/// Why a garment cannot be draped.
#[derive(Debug)]
pub enum DrapeError {
    /// No confirmed seams: nothing is sewn to anything.
    NothingSewn,
    /// The pieces sewn go only half way round the body: a pattern for one
    /// side, its pieces cut once.
    HalfRound,
    Garment(GarmentError),
}

impl std::fmt::Display for DrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrapeError::NothingSewn => f.write_str("no seams are sewn yet"),
            DrapeError::HalfRound => f.write_str(
                "the pieces sewn go only half way round the body, from a front to a back: \
                 the pieces cut as pairs are cut once",
            ),
            DrapeError::Garment(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DrapeError {}

/// A drape in progress on the figure.
#[derive(Debug)]
pub struct Draping {
    drape: Drape,
    /// The figure as measured, standing.
    body: Body,
    /// The pieces sewn to nothing, by id: left out.
    pub left_out: Vec<String>,
    /// The facings and linings sewn to nothing, by id: left out, as they
    /// go inside.
    pub inside: Vec<String>,
    /// The facings and linings sewn but not placed inside the rest (one
    /// sewn only to another, with nothing to lie on), by id: left out.
    pub unplaced: Vec<String>,
    /// The facings and linings draped inside the rest, by id.
    pub lined: Vec<String>,
    /// How many buttons it has, done up or not.
    pub buttons: usize,
    /// The garment's size round the body at its bust, waist and hip: as
    /// placed, then as it settles standing.
    pub girths: Vec<Girth>,
    /// The garment without its facings and linings (its first panels), its
    /// panels' points as placed, which of them hang from the shoulders, and
    /// the figure: to measure it again as it settles.
    shell: Garment,
    placed: Vec<Vec<[f64; 3]>>,
    hung: Vec<bool>,
    figure: Figure,
    /// Measured as it settled, rather than as placed.
    measured: bool,
    done: Option<Stage>,
    /// The pose asked for, and the pose the body is in or moving into.
    wanted: Pose,
    posed: Pose,
    /// When the drape last set off (sewing, or moving into a pose), s.
    since: f32,
}

/// The garment's size round the body at the figure's levels, its panels'
/// points where `points` has them (mm).
fn measure(shell: &Garment, points: &[Vec<[f64; 3]>], figure: &Figure) -> Vec<Girth> {
    girths(shell, points, figure)
        .into_iter()
        .zip(&figure.levels)
        .map(|((level, garment), at)| Girth {
            level,
            garment,
            body: at.girth,
            y: at.y,
        })
        .collect()
}

impl Draping {
    /// Starts draping on the figure shaped by `shape`: the pieces sewn by
    /// confirmed seams, but for facings and linings and pieces between the
    /// legs, placed round it and let go. Once settled it moves into `pose`.
    pub fn start(
        model: &BodyModel,
        shape: &Shape,
        request: Request,
        pose: Pose,
    ) -> Result<Draping, DrapeError> {
        let draped = draped_pieces(request.pieces, request.seams);
        let sewn = draped.sewn;
        if sewn.is_empty() {
            return Err(DrapeError::NothingSewn);
        }
        let seams: Vec<Seam> = request
            .seams
            .iter()
            .filter(|s| s.confirmed)
            .cloned()
            .collect();
        let shell = assemble(&sewn, &seams, request.spacing).map_err(DrapeError::Garment)?;
        if half_round(&shell) {
            return Err(DrapeError::HalfRound);
        }
        let body = model.shape(shape);
        let figure = Figure::of(model, &body);
        let starts = place(&shell, &figure);
        let girths = measure(&shell, &starts, &figure);
        let placed = starts.clone();
        let hung = from_shoulders(&shell);
        // Its facings and linings inside it, or, if they can't be placed
        // there, left out.
        let ids = |pieces: &[Piece]| pieces.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        let (garment, starts, lined, unplaced) = match with_inside(
            &shell,
            &starts,
            (&sewn, &draped.lined),
            &seams,
            request.spacing,
        ) {
            Some((whole, starts)) => (whole, starts, ids(&draped.lined), Vec::new()),
            None => (shell.clone(), starts, Vec::new(), ids(&draped.lined)),
        };
        let buttons = garment.buttons;
        let garment = if request.open {
            garment.unbuttoned()
        } else {
            garment
        };
        let mut drape = Drape::new(
            &garment,
            &starts,
            request.fabric,
            (&body.vertices, &model.skin()),
        );
        if let Some(posable) = Posable::of(model, &body) {
            drape.posable(posable);
        }
        Ok(Draping {
            drape,
            body,
            left_out: draped.alone,
            inside: draped.inside,
            unplaced,
            lined,
            buttons,
            girths,
            shell,
            placed,
            hung,
            figure,
            measured: false,
            done: None,
            wanted: pose,
            posed: Pose::Standing,
            since: 0.0,
        })
    }

    /// Moves the cloth on with `stepper` from now on (on the graphics
    /// card).
    pub fn use_stepper(&mut self, stepper: Box<dyn Stepper>) {
        self.drape.use_stepper(stepper);
    }

    /// The drape: the cloth, and the body it is on.
    pub fn drape(&self) -> &Drape {
        &self.drape
    }

    /// The figure as measured, standing (mm).
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// The figure as measured: where its shoulders, bust, waist and hip
    /// are.
    pub fn figure(&self) -> &Figure {
        &self.figure
    }

    /// Moves the body into `pose` once the garment has settled (at once if
    /// it has), the garment with it.
    pub fn pose(&mut self, pose: Pose) {
        self.wanted = pose;
        if self.done.is_some() && pose != self.posed {
            self.set_off();
        }
    }

    fn set_off(&mut self) {
        self.drape.pose(self.wanted);
        self.posed = self.wanted;
        self.since = self.drape.time;
        self.done = None;
    }

    /// Whether a frame can be taken now: on the graphics card, not until
    /// it has caught up.
    pub fn ready(&mut self) -> bool {
        self.done.is_some() || self.drape.ready()
    }

    /// Moves the drape on a frame, unless it is done (or the graphics card
    /// is busy).
    pub fn step(&mut self) -> Stage {
        if self.done.is_none() && self.drape.ready() {
            self.drape.step(FRAME);
            if self.drape.settled() {
                // Measured again where the cloth hung from the shoulders has
                // settled on the body: placed flat, it reaches the bust lower
                // than it does over the bust's curve. Cloth hung from the
                // waist is measured where it was placed, as it is worn
                // (settled, a skirt with nothing to hold it may slip).
                if self.posed == Pose::Standing && !self.measured {
                    let x = &self.drape.cloth.x;
                    let firsts = &self.drape.firsts;
                    let points: Vec<Vec<[f64; 3]>> = (0..self.shell.panels.len())
                        .map(|p| {
                            if !self.hung[p] {
                                return self.placed[p].clone();
                            }
                            let first = firsts[p] as usize;
                            (0..self.shell.panels[p].mesh.points.len())
                                .map(|k| x[first + k].map(|c| f64::from(c) * 1000.0))
                                .collect()
                        })
                        .collect();
                    self.girths = measure(&self.shell, &points, &self.figure);
                    self.measured = true;
                }
                if self.wanted == self.posed {
                    self.done = Some(Stage::Settled);
                } else {
                    self.set_off();
                }
            } else if self.drape.time - self.since >= LONGEST {
                self.done = Some(Stage::Stopped);
            }
        }
        self.stage()
    }

    /// Where it has got to.
    pub fn stage(&self) -> Stage {
        self.done.unwrap_or(if self.drape.sewing() {
            Stage::Sewing
        } else if self.drape.moving() {
            Stage::Posing
        } else {
            Stage::Settling
        })
    }

    /// Moves it on until it is done: settled in the pose asked for, or
    /// still moving after the longest it is given.
    pub fn finish(&mut self) -> Stage {
        loop {
            match self.step() {
                stage @ (Stage::Settled | Stage::Stopped) => return stage,
                _ if !self.drape.ready() => self.drape.catch_up(),
                _ => {}
            }
        }
    }
}
