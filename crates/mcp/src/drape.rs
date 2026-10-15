//! The project's garment draped on the avatar, as the app's Drape does it:
//! pictures of it worn, and how it fits.

use drapewright_body::{Pose, Shape};
use drapewright_fabric::FabricPreset;
use drapewright_project::document::Document;
use drapewright_sim::draping::{self, DrapeError, Draping, Stage};
use drapewright_sim::heatmap::{pull_colour, room_colour};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ToolError;
use crate::render::Picture;
use crate::session::Session;
use crate::text::round1;
use crate::tools::{Output, body_model, measurements_for_avatar};
use crate::worn::{self, Scene, View};

/// How far apart the cloth's points are, mm: as the app drapes without a
/// graphics card.
const SPACING: f64 = 20.0;
/// The fabric's colour, as the app shows it (sRGB, 0 to 1).
const FABRIC: [f32; 3] = [0.36, 0.47, 0.59];
/// How much paler facings and linings are shown, towards white (as in the
/// app).
const INSIDE_PALER: f32 = 0.45;
/// How far above and below a level the cloth is read there, mm.
const BAND: f32 = 30.0;
/// The least cloth points read to tell how the cloth is somewhere.
const FEWEST: usize = 20;
/// The height of the bands the cloth is read in to find where it pulls
/// hardest, mm.
const STRIP: f32 = 100.0;
/// The picture's longer side unless asked otherwise, pixels.
const PICTURE_PX: u32 = 1200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PoseName {
    /// As measured: arms out and down, legs straight.
    #[default]
    Standing,
    /// Arms hanging by the sides.
    ArmsDown,
    /// Both arms reaching forward at shoulder height.
    Reaching,
    /// Both arms raised overhead.
    ArmsUp,
    /// Mid-stride.
    Stepping,
    Sitting,
}

impl PoseName {
    /// The figure moving into it, in words.
    fn words(self) -> &'static str {
        match self {
            PoseName::Standing => "stood as measured",
            PoseName::ArmsDown => "lowered its arms",
            PoseName::Reaching => "reached forward",
            PoseName::ArmsUp => "raised its arms",
            PoseName::Stepping => "stepped forward",
            PoseName::Sitting => "sat down",
        }
    }
}

impl From<PoseName> for Pose {
    fn from(p: PoseName) -> Pose {
        match p {
            PoseName::Standing => Pose::Standing,
            PoseName::ArmsDown => Pose::ArmsDown,
            PoseName::Reaching => Pose::Reaching,
            PoseName::ArmsUp => Pose::ArmsUp,
            PoseName::Stepping => Pose::Stepping,
            PoseName::Sitting => Pose::Sitting,
        }
    }
}

/// What the cloth is coloured by in the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Shows {
    /// The fabric, facings and linings paler.
    #[default]
    Fabric,
    /// How hard it pulls: pale where it hardly does, through yellow and
    /// orange to red at 100 N/m and over (as the app's heatmap).
    Pull,
    /// The room between it and the body: deep blue touching, paler to
    /// white at 40 mm and more.
    Room,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrapeArgs {
    /// Whose figure: the id of a person's measurements to fit the avatar
    /// to (see get_measurements), or "standard" for the standard figure.
    /// Leave out for the first person's measurements, or the standard
    /// figure if there are none.
    person: Option<String>,
    /// Their age in years (25 to 90); if left out, the age kept with their
    /// measurements, or 35.
    age: Option<f64>,
    /// The fabric: cottonLawn, poplin, linen, chambray, denim, twill,
    /// crepe, satin, jersey, ribKnit, woolSuiting or fleece, or the id or
    /// name of one of the project's own fabrics. Leave out for poplin, as
    /// in the app.
    fabric: Option<String>,
    /// Worn open, its buttons undone; leave out to wear it done up.
    open: Option<bool>,
    /// The pose to move into once it has settled standing; leave out for
    /// standing as measured.
    pose: Option<PoseName>,
    /// What the picture colours the cloth by; leave out for the fabric.
    shows: Option<Shows>,
    /// The views to draw, left to right; leave out for front, side and
    /// back.
    views: Option<Vec<View>>,
    /// The picture's longer side, pixels (300 to 2000; default 1200).
    max_px: Option<u32>,
}

/// The fabric named `name`: a preset, or one of the project's own by id or
/// name. Its fabric, and what to call it.
fn fabric(doc: &Document, name: &str) -> Result<(drapewright_fabric::Fabric, String), ToolError> {
    let plain = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    if let Some(own) = doc
        .fabrics
        .iter()
        .find(|f| f.id == name || plain(&f.name) == plain(name))
    {
        return Ok((own.fabric(), own.name.clone()));
    }
    let preset = FabricPreset::ALL
        .into_iter()
        .find(|p| plain(p.name()) == plain(name))
        .ok_or_else(|| {
            let own: Vec<String> = doc
                .fabrics
                .iter()
                .map(|f| format!("\"{}\"", f.name))
                .collect();
            ToolError::new(format!(
                "There is no fabric \"{name}\". The presets are {}{}.",
                FabricPreset::ALL.map(FabricPreset::name).join(", "),
                if own.is_empty() {
                    String::new()
                } else {
                    format!("; the project's own fabrics are {}", own.join(", "))
                }
            ))
        })?;
    Ok((preset.fabric(), words(preset.name())))
}

/// `cottonLawn` as "cotton lawn".
fn words(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_uppercase() {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

fn why(e: DrapeError) -> ToolError {
    ToolError::new(match e {
        DrapeError::NothingSewn => "Nothing is sewn yet, so there is nothing to drape. get_seams \
             suggests seams: ask the user, then sew them with sew_edges."
            .into(),
        DrapeError::HalfRound => "The pieces sewn go only half way round the body, from a front \
             to a back: pieces that should be cut as pairs are cut once. Set them to cut 2 as \
             mirrored pairs with update_piece, then sew the new seams (get_seams suggests them)."
            .into(),
        e => format!("The garment could not be draped: {e}."),
    })
}

/// Pieces by id, with their names.
fn named(doc: &Document, ids: &[String]) -> Value {
    ids.iter()
        .map(|id| {
            let name = doc.piece(id).map_or("", |p| p.name.as_str());
            json!({ "id": id, "name": name })
        })
        .collect()
}

/// How the cloth is between two heights.
#[derive(Clone, Copy, Debug)]
struct Reading {
    /// How hard it pulls, N/m: the middle value, and the least the
    /// hardest-pulled tenth does.
    pull: f32,
    hardest: f32,
    /// The room it leaves the body, mm: the middle value, and the most the
    /// closest twentieth does.
    room: f32,
    least: f32,
}

/// The value a share `at` of the way along `values`, sorted.
fn along(values: &mut [f32], at: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * at).round() as usize]
}

/// The garment's outside as it is now: each of its points' height (mm),
/// how hard it pulls there (N/m) and the room it leaves the body (mm).
struct Outside {
    heights: Vec<f32>,
    pull: Vec<f32>,
    room: Vec<f32>,
}

impl Outside {
    /// The cloth's first `outside` points (those after are facings and
    /// linings).
    fn of(draping: &Draping, outside: usize) -> Self {
        let drape = draping.drape();
        let mut pull = drape.strain().pull;
        let mut room: Vec<f32> = drape.room().iter().map(|r| r * 1000.0).collect();
        pull.truncate(outside);
        room.truncate(outside);
        Outside {
            heights: drape.cloth.x[..outside]
                .iter()
                .map(|p| p[1] * 1000.0)
                .collect(),
            pull,
            room,
        }
    }

    /// How it is between heights `low` and `high`, mm, if enough of it is
    /// there to tell.
    fn read(&self, (low, high): (f32, f32)) -> Option<Reading> {
        let here: Vec<usize> = (0..self.heights.len())
            .filter(|&k| (low..=high).contains(&self.heights[k]))
            .collect();
        if here.len() < FEWEST {
            return None;
        }
        let mut pulls: Vec<f32> = here.iter().map(|&k| self.pull[k]).collect();
        let mut rooms: Vec<f32> = here.iter().map(|&k| self.room[k]).collect();
        Some(Reading {
            pull: along(&mut pulls, 0.5),
            hardest: along(&mut pulls, 0.9),
            room: along(&mut rooms, 0.5),
            least: along(&mut rooms, 0.05),
        })
    }

    /// Where it pulls hardest: the band of heights (mm), and how it is
    /// there.
    fn pulls_most(&self) -> Option<((f32, f32), Reading)> {
        let (low, high) = self
            .heights
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &y| (l.min(y), h.max(y)));
        let mut most: Option<((f32, f32), Reading)> = None;
        let mut from = (low / STRIP).floor() * STRIP;
        while from < high {
            let band = (from, from + STRIP);
            if let Some(r) = self.read(band)
                && most.is_none_or(|(_, m)| r.hardest > m.hardest)
            {
                most = Some((band, r));
            }
            from += STRIP;
        }
        most
    }
}

fn r1(v: f32) -> f64 {
    round1(f64::from(v))
}

pub fn drape(session: &mut Session, a: DrapeArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let model = body_model();
    // Whose figure: a person's, fitted, or the standard one.
    let standard = a.person.as_deref() == Some("standard")
        || (a.person.is_none() && doc.measurement_sets.is_empty());
    let (shape, figure) = if standard {
        (Shape::default(), "the standard figure".to_string())
    } else {
        let person = measurements_for_avatar(doc, a.person.as_deref(), a.age)?;
        let fitted = drapewright_body::fit(model, &person.values, person.woman, person.age);
        (fitted.shape, format!("{}'s measurements", person.name))
    };
    let (cloth, fabric_name) = fabric(doc, a.fabric.as_deref().unwrap_or("poplin"))?;
    let pieces: Vec<_> = doc.pieces.iter().map(|p| (**p).clone()).collect();
    let open = a.open == Some(true);
    let mut draping = Draping::start(
        model,
        &shape,
        draping::Request {
            pieces: &pieces,
            seams: &doc.seams,
            fabric: cloth,
            open,
            spacing: SPACING,
        },
        Pose::Standing,
    )
    .map_err(why)?;
    let stood = draping.finish();
    let standing = draping.drape().time;
    let outside = (0..draping.drape().cloth.x.len())
        .find(|&k| draping.drape().cloth.inside(k))
        .unwrap_or(draping.drape().cloth.x.len());

    // How it fits standing: at each level, and where it pulls most.
    let standing_cloth = Outside::of(&draping, outside);
    let fit: Vec<Value> = draping
        .girths
        .iter()
        .map(|g| {
            let y = g.y as f32;
            let reading = standing_cloth.read((y - BAND, y + BAND));
            json!({
                "level": g.level,
                "garmentMm": g.garment.map(round1),
                "bodyMm": round1(g.body),
                "easeMm": g.garment.map(|m| round1(m - g.body)),
                "pullNPerM": reading.map(|r| json!({ "typical": r1(r.pull), "hardest": r1(r.hardest) })),
                "roomMm": reading.map(|r| json!({ "typical": r1(r.room), "least": r1(r.least) })),
            })
        })
        .collect();
    // The landmark nearest a height: the shoulders, bust, waist or hip.
    let measured = draping.figure();
    let landmarks: Vec<(&str, f32)> = std::iter::once(("shoulders", measured.shoulders))
        .chain(measured.levels.iter().map(|l| (l.name, l.y)))
        .map(|(name, y)| (name, y as f32))
        .collect();
    let nearest = |y: f32| {
        landmarks
            .iter()
            .min_by(|a, b| (a.1 - y).abs().total_cmp(&(b.1 - y).abs()))
            .map(|l| l.0)
    };
    let hardest = |((from, to), r): ((f32, f32), Reading)| {
        json!({
            "fromHeightMm": r1(from),
            "toHeightMm": r1(to),
            "pullNPerM": { "typical": r1(r.pull), "hardest": r1(r.hardest) },
        })
    };
    let pulls_most_standing = standing_cloth.pulls_most().map(|most| {
        let mut said = hardest(most);
        said["nearest"] = json!(nearest((most.0.0 + most.0.1) / 2.0));
        said
    });

    // Then into the pose asked for.
    let pose = a.pose.unwrap_or_default();
    let moved = (pose != PoseName::Standing).then(|| {
        draping.pose(pose.into());
        draping.finish()
    });
    let posed = moved.map(|_| {
        let drape = draping.drape();
        let inside = drape
            .cloth
            .x
            .iter()
            .filter(|&&p| drape.clearance(p) < 0.0)
            .count();
        json!({
            "pose": pose,
            "pullsMost": Outside::of(&draping, outside).pulls_most().map(hardest),
            "clothPointsInsideTheBody": inside,
        })
    });

    let shows = a.shows.unwrap_or_default();
    let views = a.views.unwrap_or_else(|| View::ALL.to_vec());
    let max_px = a.max_px.unwrap_or(PICTURE_PX).clamp(300, 2000);
    let picture = picture(&draping, outside, shows, &views, max_px)?;
    let summary = format!(
        "Draped in {fabric_name} on {figure}{}: {}{}. {}.",
        match (draping.buttons, open) {
            (0, _) => String::new(),
            (n, false) => format!(", {n} buttons done up"),
            (n, true) => format!(", worn open ({n} buttons undone)"),
        },
        match stood {
            Stage::Settled => format!("settled after {standing:.1} s"),
            _ => "still moving after 20 s".into(),
        },
        match moved {
            None => String::new(),
            Some(Stage::Settled) => format!(", then {} and settled", pose.words()),
            Some(_) => format!(", then {} and still moving after 20 s", pose.words()),
        },
        girths(&draping)
    );
    let reply = json!({
        "summary": summary,
        "figure": figure,
        "fabric": fabric_name,
        "settled": moved.unwrap_or(stood) == Stage::Settled,
        "fit": fit,
        "pullsMost": pulls_most_standing,
        "posed": posed,
        "widestSeamGapMm": round1(f64::from(draping.drape().cloth.widest_stitch()) * 1000.0),
        "buttons": draping.buttons,
        "open": open,
        "inside": named(doc, &draping.lined),
        "leftOut": named(doc, &draping.left_out),
        "facingsSewnToNothing": named(doc, &draping.inside),
        "facingsWithNothingToLieOn": named(doc, &draping.unplaced),
        "picture": {
            "views": views.iter().map(|v| v.name()).collect::<Vec<_>>(),
            "shows": shows,
            "mmPerPx": round1(picture.mm_per_px),
        },
    });
    Ok(Output::json(&reply).image(&picture.png))
}

/// The garment's size round the bust, waist and hip, and the ease there,
/// in words.
fn girths(draping: &Draping) -> String {
    let said: Vec<String> = draping
        .girths
        .iter()
        .filter_map(|g| {
            let garment = g.garment?;
            let ease = garment - g.body;
            Some(format!(
                "{} {:.1} cm ({})",
                g.level,
                garment / 10.0,
                if ease.abs() < 1.0 {
                    "no ease".to_string()
                } else if ease > 0.0 {
                    format!("{:.1} cm ease", ease / 10.0)
                } else {
                    format!("{:.1} cm smaller than the body", -ease / 10.0)
                }
            ))
        })
        .collect();
    if said.is_empty() {
        "It doesn't reach round the bust, waist or hip".into()
    } else {
        let mut said = said.join(", ");
        said[..1].make_ascii_uppercase();
        said
    }
}

/// The figure wearing the garment as it is now, seen from `views`, the
/// cloth coloured as `shows` (its first `outside` points are its outside).
fn picture(
    draping: &Draping,
    outside: usize,
    shows: Shows,
    views: &[View],
    max_px: u32,
) -> Result<Picture, ToolError> {
    let drape = draping.drape();
    let colours: Vec<[f32; 3]> = match shows {
        Shows::Fabric => (0..drape.cloth.x.len())
            .map(|k| {
                if k >= outside {
                    FABRIC.map(|c| c + (1.0 - c) * INSIDE_PALER)
                } else {
                    FABRIC
                }
            })
            .collect(),
        Shows::Pull => drape
            .strain()
            .pull
            .iter()
            .map(|&p| pull_colour(p))
            .collect(),
        Shows::Room => drape
            .room()
            .iter()
            .map(|&r| room_colour(r * 1000.0))
            .collect(),
    };
    // The body as it is now, and the seat if it sits.
    let mut skin: Vec<[f32; 3]> = match drape.body() {
        Some((_, moved)) => moved.to_vec(),
        None => draping
            .body()
            .vertices
            .iter()
            .map(|p| p.map(|c| c as f32))
            .collect(),
    };
    let mut skin_triangles = body_model().skin();
    if let Some(seat) = drape.seat() {
        let (corners, faces) = seat.mesh();
        let first = skin.len() as u32;
        skin.extend(corners.iter().map(|p| p.map(|c| c * 1000.0)));
        skin_triangles.extend(faces.iter().map(|f| f.map(|i| i + first)));
    }
    let cloth: Vec<[f32; 3]> = drape
        .cloth
        .x
        .iter()
        .map(|p| p.map(|c| c * 1000.0))
        .collect();
    worn::draw(
        &Scene {
            skin: (&skin, &skin_triangles),
            cloth: (&cloth, &drape.cloth.triangles),
            colours: &colours,
        },
        views,
        max_px,
    )
    .map_err(ToolError::new)
}
