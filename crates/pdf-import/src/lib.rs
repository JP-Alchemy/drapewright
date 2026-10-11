//! Reads what a sewing pattern PDF draws: each page's paths, with their
//! stroke and fill colours, line widths and dashes, the text it shows, and
//! the layers (optional content groups) they are in, all in the page's own
//! coordinates. Images and visual effects are skipped.
//!
//! Lenient by design: damaged files are read as far as viewers would, and
//! "protected" files that open without a password are read as viewers do.
//! Nothing here allocates without bound or recurses without a limit, so a
//! hostile file can't take the app down.

mod content;
mod crypt;
mod encoding;
mod file;
mod filter;
mod font;
mod lexer;
mod object;

use std::collections::HashSet;
use std::rc::Rc;

use kurbo::{Affine, Rect};
use thiserror::Error;

pub use content::{Drawing, Shape, TextRun};
use file::File;
use object::{Dict, Object, text_string};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PdfError {
    #[error("This is not a PDF file.")]
    NotAPdf,
    #[error("The PDF is damaged: {0}.")]
    Damaged(String),
    #[error("{0}")]
    Encrypted(String),
    #[error("The PDF uses {0}, which Drapewright cannot read yet.")]
    Unsupported(String),
}

impl PdfError {
    pub(crate) fn damaged(message: impl Into<String>) -> Self {
        PdfError::Damaged(message.into())
    }
}

/// A layer (optional content group) the user can show or hide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    /// The group's object number, which shapes refer to.
    pub id: u32,
    pub name: String,
    /// Shown when the file opens.
    pub visible: bool,
}

/// One page's drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The page's area in default user space (points, y up).
    pub media_box: Rect,
    /// What viewers show and print: the media box unless cropped.
    pub crop_box: Rect,
    /// Clockwise degrees the page is turned when shown: 0, 90, 180 or 270.
    pub rotate: i32,
    /// Points per user space unit (almost always 1).
    pub user_unit: f64,
    pub drawing: Drawing,
}

/// Most pages a page tree may hold before it is taken as broken.
const MAX_PAGES: usize = 100_000;

#[derive(Debug)]
pub struct Document {
    file: File,
    /// Each page's dictionary, with inherited attributes filled in.
    pages: Vec<Dict>,
    layers: Vec<Layer>,
}

impl Document {
    pub fn load(data: &[u8]) -> Result<Document, PdfError> {
        let file = File::open(data.to_vec(), b"")?;
        let root = file
            .trailer
            .get(b"Root")
            .cloned()
            .ok_or_else(|| PdfError::damaged("it has no document catalog"))?;
        let catalog = file.resolve(&root)?;
        let catalog = catalog
            .as_dict()
            .ok_or_else(|| PdfError::damaged("its catalog is not a dictionary"))?
            .clone();
        let mut pages = Vec::new();
        if let Some(tree) = catalog.get(b"Pages") {
            let mut seen = HashSet::new();
            collect_pages(&file, tree, &Dict::default(), &mut pages, &mut seen, 0);
        }
        if pages.is_empty() {
            return Err(PdfError::damaged("it has no pages"));
        }
        let layers = read_layers(&file, &catalog);
        Ok(Document {
            file,
            pages,
            layers,
        })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Reads page `index` (from 0).
    pub fn page(&self, index: usize) -> Result<Page, PdfError> {
        let dict = self
            .pages
            .get(index)
            .ok_or_else(|| PdfError::damaged("a page is missing"))?;
        let file = &self.file;
        let rect = |key: &[u8]| -> Option<Rect> {
            let a = file.dict_get(dict, key)?;
            let n: Vec<f64> = a.as_array()?.iter().filter_map(Object::as_f64).collect();
            (n.len() == 4).then(|| Rect::new(n[0], n[1], n[2], n[3]).abs())
        };
        let media_box = rect(b"MediaBox").unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
        let crop_box = rect(b"CropBox")
            .map(|c| c.intersect(media_box))
            .filter(|c| c.area() > 0.0)
            .unwrap_or(media_box);
        let rotate = dict
            .get(b"Rotate")
            .and_then(|r| file.resolve(r).ok())
            .and_then(|r| r.as_int())
            .unwrap_or(0)
            .rem_euclid(360) as i32
            / 90
            * 90;
        let user_unit = dict
            .get(b"UserUnit")
            .and_then(Object::as_f64)
            .filter(|u| *u > 0.0)
            .unwrap_or(1.0);
        let resources = file
            .dict_get(dict, b"Resources")
            .and_then(|r| r.as_dict().cloned())
            .unwrap_or_default();
        let mut content = Vec::new();
        if let Some(c) = dict.get(b"Contents") {
            let c = file.resolve(c)?;
            let streams: Vec<Object> = match &*c {
                Object::Array(a) => a.clone(),
                other => vec![other.clone()],
            };
            for s in streams {
                if let Some(stream) = file.resolve(&s).ok().as_deref().and_then(Object::as_stream) {
                    // A damaged stream loses its own drawing, not the page's.
                    if let Ok(data) = file.decode(stream) {
                        content.extend_from_slice(&data);
                        content.push(b'\n');
                    }
                }
            }
        }
        let drawing = content::interpret(file, &content, Rc::new(resources), Affine::IDENTITY);
        Ok(Page {
            media_box,
            crop_box,
            rotate,
            user_unit,
            drawing,
        })
    }
}

/// Inherited page attributes (§7.7.3.4).
const INHERITED: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

fn collect_pages(
    file: &File,
    node: &Object,
    inherited: &Dict,
    out: &mut Vec<Dict>,
    seen: &mut HashSet<u32>,
    depth: usize,
) {
    if depth > 64 || out.len() >= MAX_PAGES {
        return;
    }
    if let Some(r) = node.as_ref()
        && !seen.insert(r.num)
    {
        return;
    }
    let Ok(node) = file.resolve(node) else {
        return;
    };
    let Some(dict) = node.as_dict() else {
        return;
    };
    let mut attrs = inherited.clone();
    for key in INHERITED {
        if let Some(v) = dict.get(key) {
            attrs.set(key, v.clone());
        }
    }
    let kids = dict.get(b"Kids").and_then(|k| file.resolve(k).ok());
    let is_tree = dict.name(b"Type") == Some(b"Pages")
        || (dict.name(b"Type") != Some(b"Page") && kids.is_some());
    if is_tree {
        if let Some(kids) = kids.as_deref().and_then(Object::as_array) {
            for kid in kids {
                collect_pages(file, kid, &attrs, out, seen, depth + 1);
            }
        }
    } else {
        let mut page = dict.clone();
        for key in INHERITED {
            if !page.contains(key)
                && let Some(v) = attrs.get(key)
            {
                page.set(key, v.clone());
            }
        }
        out.push(page);
    }
}

/// The document's layers, in the order its layer panel lists them.
fn read_layers(file: &File, catalog: &Dict) -> Vec<Layer> {
    let Some(props) = file.dict_get(catalog, b"OCProperties") else {
        return Vec::new();
    };
    let Some(props) = props.as_dict() else {
        return Vec::new();
    };
    let groups: Vec<u32> = file
        .dict_get(props, b"OCGs")
        .and_then(|a| {
            a.as_array()
                .map(|a| a.iter().filter_map(Object::as_ref).map(|r| r.num).collect())
        })
        .unwrap_or_default();
    let config = file
        .dict_get(props, b"D")
        .and_then(|d| d.as_dict().cloned())
        .unwrap_or_default();
    let refs = |key: &[u8]| -> HashSet<u32> {
        file.dict_get(&config, key)
            .and_then(|a| {
                a.as_array()
                    .map(|a| a.iter().filter_map(Object::as_ref).map(|r| r.num).collect())
            })
            .unwrap_or_default()
    };
    let (on, off) = (refs(b"ON"), refs(b"OFF"));
    let base_off = config.name(b"BaseState") == Some(b"OFF");
    // The panel's order first, then any group it leaves out.
    let mut order = Vec::new();
    if let Some(o) = file.dict_get(&config, b"Order") {
        flatten_order(file, &o, &mut order, 0);
    }
    for g in &groups {
        if !order.contains(g) {
            order.push(*g);
        }
    }
    order
        .into_iter()
        .filter(|g| groups.contains(g))
        .map(|id| {
            let name = file
                .get(object::Ref {
                    num: id,
                    generation: 0,
                })
                .ok()
                .and_then(|o| {
                    o.as_dict()
                        .and_then(|d| d.get(b"Name"))
                        .and_then(Object::as_bytes)
                        .map(text_string)
                })
                .unwrap_or_default();
            let visible = if base_off {
                on.contains(&id)
            } else {
                !off.contains(&id)
            };
            Layer {
                id,
                name: name.trim().to_string(),
                visible,
            }
        })
        .collect()
}

fn flatten_order(file: &File, o: &Object, out: &mut Vec<u32>, depth: usize) {
    if depth > 16 {
        return;
    }
    match o {
        Object::Ref(r) => {
            // A nested array may itself be an indirect object.
            match file.get(*r).ok().as_deref() {
                Some(Object::Array(a)) => {
                    for item in a {
                        flatten_order(file, item, out, depth + 1);
                    }
                }
                _ => {
                    if !out.contains(&r.num) {
                        out.push(r.num);
                    }
                }
            }
        }
        Object::Array(a) => {
            for item in a {
                flatten_order(file, item, out, depth + 1);
            }
        }
        _ => {}
    }
}
