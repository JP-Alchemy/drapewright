//! Content streams (§8, §9): the paths a page paints, with their stroke and
//! fill, and the text it shows, in default user space. Images, shadings and
//! transparency effects are skipped: a pattern is lines and words.

use std::collections::HashMap;
use std::rc::Rc;

use kurbo::{Affine, BezPath, Point, Rect, Shape as _};

use crate::file::File;
use crate::font::Font;
use crate::lexer::{Lexer, Token, is_whitespace};
use crate::object::{Dict, Object};

/// Operators read per page before giving up on a runaway stream.
const MAX_OPERATORS: usize = 20_000_000;
/// Saved graphics states, and nested forms, allowed at once.
const MAX_STACK: usize = 256;
const MAX_FORMS: usize = 24;

/// A painted path.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// In default user space: points, y up.
    pub path: BezPath,
    /// Stroke colour (RGB, 0–1), when stroked.
    pub stroke: Option<[f32; 3]>,
    /// Fill colour, when filled.
    pub fill: Option<[f32; 3]>,
    pub even_odd: bool,
    /// Line width in default user space.
    pub line_width: f64,
    /// Dash lengths in default user space; empty for a solid line.
    pub dash: Vec<f64>,
    /// Layers (optional content groups, by object number) it is drawn in.
    pub layers: Vec<u32>,
    /// What the clip leaves visible, as a box in default user space.
    pub clip: Option<Rect>,
}

/// Text shown by one text-showing operator.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    /// Where the baseline starts and ends, default user space.
    pub start: Point,
    pub end: Point,
    /// The font size, in default user space.
    pub size: f64,
    pub layers: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    pub shapes: Vec<Shape>,
    pub texts: Vec<TextRun>,
}

/// A colour space, enough to tell colours apart.
#[derive(Clone, Debug)]
enum Space {
    Gray,
    Rgb,
    Cmyk,
    Indexed {
        base: Box<Space>,
        lookup: Vec<u8>,
    },
    /// A spot colour whose tint goes linearly (type 2 function) from c0 to c1.
    Tint {
        alternate: Box<Space>,
        c0: Vec<f64>,
        c1: Vec<f64>,
        exponent: f64,
        inputs: usize,
    },
    /// A spot colour we can't convert: more tint, darker.
    TintGray {
        inputs: usize,
    },
    Pattern,
}

impl Space {
    fn components(&self) -> usize {
        match self {
            Space::Gray | Space::Indexed { .. } | Space::Pattern => 1,
            Space::Rgb => 3,
            Space::Cmyk => 4,
            Space::Tint { inputs, .. } | Space::TintGray { inputs } => *inputs,
        }
    }

    fn initial(&self) -> Vec<f64> {
        match self {
            Space::Cmyk => vec![0.0, 0.0, 0.0, 1.0],
            Space::Tint { inputs, .. } | Space::TintGray { inputs } => vec![1.0; *inputs],
            other => vec![0.0; other.components()],
        }
    }

    fn rgb(&self, c: &[f64]) -> [f32; 3] {
        let at = |i: usize| c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let out = match self {
            Space::Gray | Space::Pattern => {
                let g = if matches!(self, Space::Pattern) {
                    0.5
                } else {
                    at(0)
                };
                [g, g, g]
            }
            Space::Rgb => [at(0), at(1), at(2)],
            Space::Cmyk => {
                let k = 1.0 - at(3);
                [(1.0 - at(0)) * k, (1.0 - at(1)) * k, (1.0 - at(2)) * k]
            }
            Space::Indexed { base, lookup } => {
                let n = base.components();
                let i = c.first().copied().unwrap_or(0.0).max(0.0) as usize;
                let comps: Vec<f64> = (0..n)
                    .map(|k| f64::from(lookup.get(i * n + k).copied().unwrap_or(0)) / 255.0)
                    .collect();
                return base.rgb(&comps);
            }
            Space::Tint {
                alternate,
                c0,
                c1,
                exponent,
                ..
            } => {
                let t = at(0).powf(*exponent);
                let comps: Vec<f64> = c0.iter().zip(c1).map(|(a, b)| a + t * (b - a)).collect();
                return alternate.rgb(&comps);
            }
            Space::TintGray { .. } => {
                let g = 1.0 - at(0);
                [g, g, g]
            }
        };
        out.map(|v| v as f32)
    }
}

#[derive(Clone, Debug)]
struct State {
    ctm: Affine,
    stroke_space: Rc<Space>,
    fill_space: Rc<Space>,
    stroke: [f32; 3],
    fill: [f32; 3],
    line_width: f64,
    dash: Vec<f64>,
    clip: Option<Rect>,
    stroke_alpha: f64,
    fill_alpha: f64,
    char_spacing: f64,
    word_spacing: f64,
    h_scale: f64,
    leading: f64,
    font: Option<Rc<Font>>,
    font_size: f64,
    rise: f64,
}

impl State {
    fn new(ctm: Affine) -> Self {
        State {
            ctm,
            stroke_space: Rc::new(Space::Gray),
            fill_space: Rc::new(Space::Gray),
            stroke: [0.0; 3],
            fill: [0.0; 3],
            line_width: 1.0,
            dash: Vec::new(),
            clip: None,
            stroke_alpha: 1.0,
            fill_alpha: 1.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scale: 1.0,
            leading: 0.0,
            font: None,
            font_size: 0.0,
            rise: 0.0,
        }
    }
}

struct Interpreter<'a> {
    file: &'a File,
    out: Drawing,
    gs: State,
    stack: Vec<State>,
    path: BezPath,
    current: Option<Point>,
    start: Option<Point>,
    clip_pending: bool,
    /// Layers each marked-content level adds.
    marked: Vec<Vec<u32>>,
    /// Forms being drawn, so a form that draws itself stops.
    forms: Vec<u32>,
    tm: Affine,
    tlm: Affine,
    fonts: HashMap<u32, Rc<Font>>,
    operators: usize,
}

/// Interprets a page's content with its resources.
pub fn interpret(file: &File, content: &[u8], resources: Rc<Dict>, ctm: Affine) -> Drawing {
    let mut it = Interpreter {
        file,
        out: Drawing::default(),
        gs: State::new(ctm),
        stack: Vec::new(),
        path: BezPath::new(),
        current: None,
        start: None,
        clip_pending: false,
        marked: Vec::new(),
        forms: Vec::new(),
        tm: Affine::IDENTITY,
        tlm: Affine::IDENTITY,
        fonts: HashMap::new(),
        operators: 0,
    };
    it.run(content, &resources);
    it.out
}

fn numbers(a: &[Object], n: usize) -> Option<Vec<f64>> {
    if a.len() < n {
        return None;
    }
    a[a.len() - n..].iter().map(Object::as_f64).collect()
}

fn matrix(m: &[f64]) -> Affine {
    Affine::new([m[0], m[1], m[2], m[3], m[4], m[5]])
}

impl Interpreter<'_> {
    fn run(&mut self, content: &[u8], res: &Rc<Dict>) {
        let mut lx = Lexer::new(content);
        let mut operands: Vec<Object> = Vec::new();
        while let Some(token) = lx.next_token() {
            match token {
                Token::Keyword(b"BI") => {
                    skip_inline_image(&mut lx);
                    operands.clear();
                }
                Token::Keyword(b"true") => operands.push(Object::Bool(true)),
                Token::Keyword(b"false") => operands.push(Object::Bool(false)),
                Token::Keyword(b"null") => operands.push(Object::Null),
                Token::Keyword(op) => {
                    self.operators += 1;
                    if self.operators > MAX_OPERATORS {
                        return;
                    }
                    self.operator(op, &operands, res);
                    operands.clear();
                }
                other => match lx.object_from(other, false, 0) {
                    Ok(o) => operands.push(o),
                    Err(_) => operands.clear(),
                },
            }
            if operands.len() > 64 {
                operands.drain(..operands.len() - 64);
            }
        }
    }

    fn operator(&mut self, op: &[u8], a: &[Object], res: &Rc<Dict>) {
        let n = |i: usize| a.get(i).and_then(Object::as_f64);
        match op {
            b"q" => {
                if self.stack.len() < MAX_STACK {
                    self.stack.push(self.gs.clone());
                }
            }
            b"Q" => {
                if let Some(g) = self.stack.pop() {
                    self.gs = g;
                }
            }
            b"cm" => {
                if let Some(m) = numbers(a, 6) {
                    self.gs.ctm *= matrix(&m);
                }
            }
            b"w" => {
                if let Some(w) = n(0) {
                    self.gs.line_width = w.abs();
                }
            }
            b"d" => {
                if let Some(arr) = a.first().and_then(Object::as_array) {
                    self.gs.dash = arr
                        .iter()
                        .filter_map(Object::as_f64)
                        .map(f64::abs)
                        .collect();
                    if self.gs.dash.iter().all(|d| *d == 0.0) {
                        self.gs.dash.clear();
                    }
                }
            }
            b"gs" => self.ext_state(a, res),
            b"CS" | b"cs" => {
                if let Some(name) = a.first() {
                    let space = Rc::new(self.space(name, res, 0));
                    let colour = space.rgb(&space.initial());
                    if op == b"CS" {
                        self.gs.stroke_space = space;
                        self.gs.stroke = colour;
                    } else {
                        self.gs.fill_space = space;
                        self.gs.fill = colour;
                    }
                }
            }
            b"SC" | b"SCN" | b"sc" | b"scn" => {
                let stroke = op[0] == b'S';
                let space = if stroke {
                    self.gs.stroke_space.clone()
                } else {
                    self.gs.fill_space.clone()
                };
                let comps: Vec<f64> = a.iter().filter_map(Object::as_f64).collect();
                let colour = space.rgb(&comps);
                if stroke {
                    self.gs.stroke = colour;
                } else {
                    self.gs.fill = colour;
                }
            }
            b"G" | b"g" | b"RG" | b"rg" | b"K" | b"k" => {
                let (space, count) = match op {
                    b"G" | b"g" => (Space::Gray, 1),
                    b"RG" | b"rg" => (Space::Rgb, 3),
                    _ => (Space::Cmyk, 4),
                };
                if let Some(c) = numbers(a, count) {
                    let colour = space.rgb(&c);
                    if op[0].is_ascii_uppercase() {
                        self.gs.stroke_space = Rc::new(space);
                        self.gs.stroke = colour;
                    } else {
                        self.gs.fill_space = Rc::new(space);
                        self.gs.fill = colour;
                    }
                }
            }
            b"m" => {
                if let Some(p) = numbers(a, 2) {
                    let p = self.gs.ctm * Point::new(p[0], p[1]);
                    self.path.move_to(p);
                    self.current = Some(p);
                    self.start = Some(p);
                }
            }
            b"l" => {
                if let Some(p) = numbers(a, 2) {
                    let p = self.gs.ctm * Point::new(p[0], p[1]);
                    self.ensure_started();
                    self.path.line_to(p);
                    self.current = Some(p);
                }
            }
            b"c" => {
                if let Some(c) = numbers(a, 6) {
                    let m = self.gs.ctm;
                    let (p1, p2, p3) = (
                        m * Point::new(c[0], c[1]),
                        m * Point::new(c[2], c[3]),
                        m * Point::new(c[4], c[5]),
                    );
                    self.ensure_started();
                    self.path.curve_to(p1, p2, p3);
                    self.current = Some(p3);
                }
            }
            b"v" => {
                if let (Some(c), Some(p0)) = (numbers(a, 4), self.current) {
                    let m = self.gs.ctm;
                    let (p2, p3) = (m * Point::new(c[0], c[1]), m * Point::new(c[2], c[3]));
                    self.path.curve_to(p0, p2, p3);
                    self.current = Some(p3);
                }
            }
            b"y" => {
                if let Some(c) = numbers(a, 4) {
                    let m = self.gs.ctm;
                    let (p1, p3) = (m * Point::new(c[0], c[1]), m * Point::new(c[2], c[3]));
                    self.ensure_started();
                    self.path.curve_to(p1, p3, p3);
                    self.current = Some(p3);
                }
            }
            b"h" => self.close(),
            b"re" => {
                if let Some(r) = numbers(a, 4) {
                    let m = self.gs.ctm;
                    let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
                    let p0 = m * Point::new(x, y);
                    self.path.move_to(p0);
                    self.path.line_to(m * Point::new(x + w, y));
                    self.path.line_to(m * Point::new(x + w, y + h));
                    self.path.line_to(m * Point::new(x, y + h));
                    self.path.close_path();
                    self.current = Some(p0);
                    self.start = Some(p0);
                }
            }
            b"S" => self.paint(true, false, false),
            b"s" => {
                self.close();
                self.paint(true, false, false);
            }
            b"f" | b"F" => self.paint(false, true, false),
            b"f*" => self.paint(false, true, true),
            b"B" => self.paint(true, true, false),
            b"B*" => self.paint(true, true, true),
            b"b" => {
                self.close();
                self.paint(true, true, false);
            }
            b"b*" => {
                self.close();
                self.paint(true, true, true);
            }
            b"n" => self.paint(false, false, false),
            b"W" | b"W*" => self.clip_pending = true,
            b"BT" => {
                self.tm = Affine::IDENTITY;
                self.tlm = Affine::IDENTITY;
            }
            b"Tc" => self.gs.char_spacing = n(0).unwrap_or(0.0),
            b"Tw" => self.gs.word_spacing = n(0).unwrap_or(0.0),
            b"Tz" => self.gs.h_scale = n(0).unwrap_or(100.0) / 100.0,
            b"TL" => self.gs.leading = n(0).unwrap_or(0.0),
            b"Ts" => self.gs.rise = n(0).unwrap_or(0.0),
            b"Tf" => {
                if let (Some(name), Some(size)) = (a.first().and_then(Object::as_name), n(1)) {
                    self.gs.font = self.font(name, res);
                    self.gs.font_size = size;
                }
            }
            b"Td" => {
                if let Some(t) = numbers(a, 2) {
                    self.next_line(t[0], t[1]);
                }
            }
            b"TD" => {
                if let Some(t) = numbers(a, 2) {
                    self.gs.leading = -t[1];
                    self.next_line(t[0], t[1]);
                }
            }
            b"Tm" => {
                if let Some(m) = numbers(a, 6) {
                    self.tm = matrix(&m);
                    self.tlm = self.tm;
                }
            }
            b"T*" => self.next_line(0.0, -self.gs.leading),
            b"Tj" => {
                if let Some(s) = a.first().and_then(Object::as_bytes) {
                    self.show(&[Object::String(s.to_vec())]);
                }
            }
            b"'" => {
                self.next_line(0.0, -self.gs.leading);
                if let Some(s) = a.first().and_then(Object::as_bytes) {
                    self.show(&[Object::String(s.to_vec())]);
                }
            }
            b"\"" => {
                if a.len() >= 3 {
                    self.gs.word_spacing = n(0).unwrap_or(0.0);
                    self.gs.char_spacing = n(1).unwrap_or(0.0);
                    self.next_line(0.0, -self.gs.leading);
                    if let Some(s) = a[2].as_bytes() {
                        self.show(&[Object::String(s.to_vec())]);
                    }
                }
            }
            b"TJ" => {
                if let Some(items) = a.first().and_then(Object::as_array) {
                    self.show(items);
                }
            }
            b"Do" => {
                if let Some(name) = a.first().and_then(Object::as_name) {
                    self.draw_xobject(name, res);
                }
            }
            b"BMC" => self.marked.push(Vec::new()),
            b"BDC" => {
                let layers = match (a.first().and_then(Object::as_name), a.get(1)) {
                    (Some(b"OC"), Some(props)) => self.optional_content(props, res),
                    _ => Vec::new(),
                };
                self.marked.push(layers);
            }
            b"EMC" => {
                self.marked.pop();
            }
            _ => {}
        }
    }

    fn ensure_started(&mut self) {
        if self.current.is_none() {
            // A line with no start point: start where the path is.
            let p = Point::ZERO;
            self.path.move_to(p);
            self.current = Some(p);
            self.start = Some(p);
        }
    }

    fn close(&mut self) {
        if self.current.is_some() {
            self.path.close_path();
            self.current = self.start;
        }
    }

    fn layers(&self) -> Vec<u32> {
        self.marked.iter().flatten().copied().collect()
    }

    fn paint(&mut self, stroke: bool, fill: bool, even_odd: bool) {
        let path = std::mem::take(&mut self.path);
        self.current = None;
        self.start = None;
        if self.clip_pending {
            self.clip_pending = false;
            if !path.elements().is_empty() {
                let b = path.bounding_box();
                self.gs.clip = Some(self.gs.clip.map_or(b, |c| c.intersect(b)));
            }
        }
        if !(stroke || fill) || path.elements().is_empty() {
            return;
        }
        let stroke = stroke && self.gs.stroke_alpha > 0.0;
        let fill = fill && self.gs.fill_alpha > 0.0;
        if !(stroke || fill) {
            return;
        }
        let scale = self.gs.ctm.determinant().abs().sqrt();
        self.out.shapes.push(Shape {
            path,
            stroke: stroke.then_some(self.gs.stroke),
            fill: fill.then_some(self.gs.fill),
            even_odd,
            line_width: self.gs.line_width * scale,
            dash: self.gs.dash.iter().map(|d| d * scale).collect(),
            layers: self.layers(),
            clip: self.gs.clip,
        });
    }

    fn next_line(&mut self, tx: f64, ty: f64) {
        self.tlm *= Affine::translate((tx, ty));
        self.tm = self.tlm;
    }

    fn text_point(&self) -> Point {
        (self.gs.ctm * self.tm) * Point::new(0.0, self.gs.rise)
    }

    /// Shows strings (and, in a TJ array, position adjustments).
    fn show(&mut self, items: &[Object]) {
        let font = self
            .gs
            .font
            .clone()
            .unwrap_or_else(|| Rc::new(Font::fallback()));
        let start = self.text_point();
        let mut text = String::new();
        for item in items {
            match item {
                Object::String(bytes) => {
                    for g in font.glyphs(bytes) {
                        text.push_str(&g.text);
                        let mut tx = g.width * self.gs.font_size + self.gs.char_spacing;
                        if g.is_space {
                            tx += self.gs.word_spacing;
                        }
                        self.tm *= Affine::translate((tx * self.gs.h_scale, 0.0));
                    }
                }
                other => {
                    if let Some(adjust) = other.as_f64() {
                        let tx = -adjust / 1000.0 * self.gs.font_size * self.gs.h_scale;
                        self.tm *= Affine::translate((tx, 0.0));
                        // A gap of more than a fifth of an em reads as a space.
                        if adjust < -200.0 && !text.is_empty() && !text.ends_with(' ') {
                            text.push(' ');
                        }
                    }
                }
            }
        }
        let end = self.text_point();
        let m = (self.gs.ctm * self.tm).as_coeffs();
        let size = m[2].hypot(m[3]) * self.gs.font_size.abs();
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.trim().is_empty() || size <= 0.0 || !size.is_finite() {
            return;
        }
        self.out.texts.push(TextRun {
            text,
            start,
            end,
            size,
            layers: self.layers(),
        });
    }

    /// An entry from a resource category: `/Font /F1`, `/XObject /Im0`…
    fn resource(&self, res: &Dict, category: &[u8], name: &[u8]) -> Option<Object> {
        let cat = self.file.dict_get(res, category)?;
        cat.as_dict()?.get(name).cloned()
    }

    fn font(&mut self, name: &[u8], res: &Dict) -> Option<Rc<Font>> {
        let entry = self.resource(res, b"Font", name)?;
        let key = entry.as_ref().map(|r| r.num);
        if let Some(f) = key.and_then(|k| self.fonts.get(&k)) {
            return Some(f.clone());
        }
        let dict = self.file.resolve(&entry).ok()?;
        let font = Rc::new(Font::load(self.file, dict.as_dict()?));
        if let Some(k) = key {
            self.fonts.insert(k, font.clone());
        }
        Some(font)
    }

    fn ext_state(&mut self, a: &[Object], res: &Dict) {
        let Some(name) = a.first().and_then(Object::as_name) else {
            return;
        };
        let Some(entry) = self.resource(res, b"ExtGState", name) else {
            return;
        };
        let Ok(d) = self.file.resolve(&entry) else {
            return;
        };
        let Some(d) = d.as_dict() else { return };
        if let Some(w) = d.get(b"LW").and_then(Object::as_f64) {
            self.gs.line_width = w.abs();
        }
        if let Some(dash) = d
            .get(b"D")
            .and_then(Object::as_array)
            .and_then(|a| a.first())
            .and_then(Object::as_array)
        {
            self.gs.dash = dash
                .iter()
                .filter_map(Object::as_f64)
                .map(f64::abs)
                .collect();
        }
        if let Some(ca) = d.get(b"CA").and_then(Object::as_f64) {
            self.gs.stroke_alpha = ca;
        }
        if let Some(ca) = d.get(b"ca").and_then(Object::as_f64) {
            self.gs.fill_alpha = ca;
        }
    }

    fn space(&self, spec: &Object, res: &Dict, depth: usize) -> Space {
        if depth > 4 {
            return Space::Gray;
        }
        match spec {
            Object::Name(n) => match n.as_slice() {
                b"DeviceGray" | b"G" | b"CalGray" => Space::Gray,
                b"DeviceRGB" | b"RGB" | b"CalRGB" => Space::Rgb,
                b"DeviceCMYK" | b"CMYK" => Space::Cmyk,
                b"Pattern" => Space::Pattern,
                other => match self.resource(res, b"ColorSpace", other) {
                    Some(o) => match self.file.resolve(&o) {
                        Ok(o) => self.space(&o, res, depth + 1),
                        Err(_) => Space::Gray,
                    },
                    None => Space::Gray,
                },
            },
            Object::Ref(_) => match self.file.resolve(spec) {
                Ok(o) => self.space(&o, res, depth + 1),
                Err(_) => Space::Gray,
            },
            Object::Array(a) => {
                let family = a.first().and_then(Object::as_name).unwrap_or_default();
                let item = |i: usize| a.get(i).and_then(|o| self.file.resolve(o).ok());
                match family {
                    b"ICCBased" => {
                        let n = item(1)
                            .and_then(|s| {
                                s.as_dict()
                                    .and_then(|d| d.get(b"N"))
                                    .and_then(Object::as_int)
                            })
                            .unwrap_or(3);
                        match n {
                            1 => Space::Gray,
                            4 => Space::Cmyk,
                            _ => Space::Rgb,
                        }
                    }
                    b"CalRGB" | b"Lab" => Space::Rgb,
                    b"CalGray" => Space::Gray,
                    b"Indexed" | b"I" => {
                        let base = item(1).map_or(Space::Rgb, |b| self.space(&b, res, depth + 1));
                        let lookup = match item(3).as_deref() {
                            Some(Object::String(s)) => s.clone(),
                            Some(Object::Stream(s)) => self.file.decode(s).unwrap_or_default(),
                            _ => Vec::new(),
                        };
                        Space::Indexed {
                            base: Box::new(base),
                            lookup,
                        }
                    }
                    b"Separation" | b"DeviceN" => {
                        let inputs = if family == b"DeviceN" {
                            item(1)
                                .and_then(|n| n.as_array().map(<[Object]>::len))
                                .unwrap_or(1)
                        } else {
                            1
                        };
                        let alternate =
                            item(2).map_or(Space::Gray, |s| self.space(&s, res, depth + 1));
                        let function = item(3);
                        let f = function.as_deref().and_then(Object::as_dict);
                        match f {
                            Some(f)
                                if inputs == 1
                                    && f.get(b"FunctionType").and_then(Object::as_int)
                                        == Some(2) =>
                            {
                                let list = |k: &[u8], d: f64| -> Vec<f64> {
                                    f.get(k)
                                        .and_then(Object::as_array)
                                        .map(|a| a.iter().filter_map(Object::as_f64).collect())
                                        .unwrap_or_else(|| vec![d])
                                };
                                Space::Tint {
                                    c0: list(b"C0", 0.0),
                                    c1: list(b"C1", 1.0),
                                    exponent: f.get(b"N").and_then(Object::as_f64).unwrap_or(1.0),
                                    alternate: Box::new(alternate),
                                    inputs,
                                }
                            }
                            _ => Space::TintGray { inputs },
                        }
                    }
                    b"Pattern" => Space::Pattern,
                    other => self.space(&Object::Name(other.to_vec()), res, depth + 1),
                }
            }
            _ => Space::Gray,
        }
    }

    /// The layers `/OC` marked content (or a form's /OC) puts its drawing in.
    fn optional_content(&self, props: &Object, res: &Dict) -> Vec<u32> {
        let entry = match props {
            Object::Name(n) => match self.resource(res, b"Properties", n) {
                Some(e) => e,
                None => return Vec::new(),
            },
            other => other.clone(),
        };
        self.layers_of(&entry)
    }

    fn layers_of(&self, entry: &Object) -> Vec<u32> {
        let Ok(resolved) = self.file.resolve(entry) else {
            return Vec::new();
        };
        let Some(d) = resolved.as_dict() else {
            return Vec::new();
        };
        match d.name(b"Type") {
            Some(b"OCMD") => match d.get(b"OCGs") {
                Some(Object::Ref(r)) => vec![r.num],
                Some(Object::Array(a)) => {
                    a.iter().filter_map(Object::as_ref).map(|r| r.num).collect()
                }
                _ => Vec::new(),
            },
            _ => entry.as_ref().map(|r| vec![r.num]).unwrap_or_default(),
        }
    }

    fn draw_xobject(&mut self, name: &[u8], res: &Rc<Dict>) {
        let Some(entry) = self.resource(res, b"XObject", name) else {
            return;
        };
        let num = entry.as_ref().map(|r| r.num);
        if num.is_some_and(|n| self.forms.contains(&n)) || self.forms.len() >= MAX_FORMS {
            return;
        }
        let Ok(object) = self.file.resolve(&entry) else {
            return;
        };
        let Some(stream) = object.as_stream() else {
            return;
        };
        if stream.dict.name(b"Subtype") != Some(b"Form") {
            return;
        }
        let Ok(data) = self.file.decode(stream) else {
            return;
        };
        let form_matrix = stream
            .dict
            .get(b"Matrix")
            .and_then(Object::as_array)
            .and_then(|m| m.iter().map(Object::as_f64).collect::<Option<Vec<f64>>>())
            .filter(|m| m.len() == 6)
            .map_or(Affine::IDENTITY, |m| matrix(&m));
        let form_res = self
            .file
            .dict_get(&stream.dict, b"Resources")
            .and_then(|r| r.as_dict().cloned())
            .map_or_else(|| res.clone(), Rc::new);
        let layers = stream
            .dict
            .get(b"OC")
            .map(|oc| self.layers_of(oc))
            .unwrap_or_default();

        let saved = self.gs.clone();
        let depth = self.stack.len();
        let saved_path = std::mem::take(&mut self.path);
        let (saved_current, saved_start, saved_tm, saved_tlm) =
            (self.current, self.start, self.tm, self.tlm);
        self.gs.ctm *= form_matrix;
        if let Some(b) = stream
            .dict
            .get(b"BBox")
            .and_then(Object::as_array)
            .and_then(|b| b.iter().map(Object::as_f64).collect::<Option<Vec<f64>>>())
            .filter(|b| b.len() == 4)
        {
            let r = self
                .gs
                .ctm
                .transform_rect_bbox(Rect::new(b[0], b[1], b[2], b[3]).abs());
            self.gs.clip = Some(self.gs.clip.map_or(r, |c| c.intersect(r)));
        }
        self.marked.push(layers);
        let marks = self.marked.len();
        if let Some(n) = num {
            self.forms.push(n);
        }
        self.run(&data, &form_res);
        if num.is_some() {
            self.forms.pop();
        }
        self.marked.truncate(marks - 1);
        self.stack.truncate(depth);
        self.gs = saved;
        self.path = saved_path;
        (self.current, self.start, self.tm, self.tlm) =
            (saved_current, saved_start, saved_tm, saved_tlm);
    }
}

/// Skips an inline image: its dictionary, `ID`, the data, and `EI`.
fn skip_inline_image(lx: &mut Lexer) {
    while let Some(t) = lx.next_token() {
        if t == Token::Keyword(b"ID") {
            break;
        }
    }
    let data = lx.data;
    let mut p = lx.pos + 1;
    while p + 1 < data.len() {
        if &data[p..p + 2] == b"EI"
            && is_whitespace(data[p - 1])
            && data.get(p + 2).is_none_or(|&b| is_whitespace(b))
        {
            lx.pos = p + 2;
            return;
        }
        p += 1;
    }
    lx.pos = data.len();
}
