//! Reading ASCII DXF into a small typed model: the header values that set
//! units, the block definitions, and the entities patterns use. Everything
//! else is counted, so the importer can say what it left out.

use std::collections::HashMap;

use kurbo::Point;

/// A shape in drawing units, y up, as DXF stores it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    Line {
        a: Point,
        b: Point,
    },
    /// Vertices with the bulge of the segment leaving each (tan of a quarter
    /// of the arc's angle; 0 for straight).
    Polyline {
        vertices: Vec<(Point, f64)>,
        closed: bool,
    },
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Point {
        at: Point,
    },
    Text {
        at: Point,
        height: f64,
        text: String,
    },
    Spline {
        degree: usize,
        knots: Vec<f64>,
        control: Vec<Point>,
        /// One per control point when the spline is rational; empty otherwise.
        weights: Vec<f64>,
        fit: Vec<Point>,
        closed: bool,
    },
    Insert {
        block: String,
        at: Point,
        scale: (f64, f64),
        rotation_deg: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entity {
    pub layer: String,
    pub shape: Shape,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Block {
    pub base: Point,
    pub entities: Vec<Entity>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Drawing {
    /// `$INSUNITS`: 1 inch, 2 feet, 4 mm, 5 cm, 6 m; 0 or absent unitless.
    pub insunits: Option<i64>,
    /// `$MEASUREMENT`: 0 imperial, 1 metric.
    pub measurement: Option<i64>,
    pub blocks: HashMap<String, Block>,
    pub entities: Vec<Entity>,
    /// Entities of kinds Soom does not read.
    pub skipped: u32,
}

/// Decodes DXF bytes: UTF-8 when valid, otherwise Windows-1252, which is
/// what most older CAD programs write.
pub(crate) fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| {
            if (0x80..0xa0).contains(&b) {
                HIGH[(b - 0x80) as usize]
            } else {
                b as char
            }
        })
        .collect()
}

struct Pairs<'a> {
    lines: std::str::Lines<'a>,
}

impl<'a> Iterator for Pairs<'a> {
    type Item = Result<(i32, &'a str), String>;
    fn next(&mut self) -> Option<Self::Item> {
        let code = self.lines.next()?;
        let value = self.lines.next().unwrap_or("");
        Some(
            code.trim()
                .parse::<i32>()
                .map(|c| (c, value.trim()))
                .map_err(|_| format!("expected a group code, found “{}”", code.trim())),
        )
    }
}

/// The group codes of one entity, in order.
type Fields<'a> = Vec<(i32, &'a str)>;

fn num(v: &str) -> f64 {
    v.parse().unwrap_or(0.0)
}

fn field<'a>(fields: &Fields<'a>, code: i32) -> Option<&'a str> {
    fields.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
}

fn point(fields: &Fields, x: i32, y: i32) -> Point {
    Point::new(
        field(fields, x).map_or(0.0, num),
        field(fields, y).map_or(0.0, num),
    )
}

/// MTEXT formatting codes removed, paragraphs as spaces.
fn plain_text(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('P') | Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some(_) => {
                    // \Ffont; \Hheight; and similar run to a semicolon.
                    for d in chars.by_ref() {
                        if d == ';' {
                            break;
                        }
                    }
                }
                None => {}
            },
            '{' | '}' => {}
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

fn entity_from(kind: &str, fields: &Fields, vertices: &[(Point, f64, i64)]) -> Option<Shape> {
    let flags = field(fields, 70).map_or(0, |v| v.parse::<i64>().unwrap_or(0));
    Some(match kind {
        "LINE" => Shape::Line {
            a: point(fields, 10, 20),
            b: point(fields, 11, 21),
        },
        "LWPOLYLINE" => {
            let mut verts: Vec<(Point, f64)> = Vec::new();
            for &(code, value) in fields {
                match code {
                    10 => verts.push((Point::new(num(value), 0.0), 0.0)),
                    20 => {
                        if let Some(v) = verts.last_mut() {
                            v.0.y = num(value);
                        }
                    }
                    42 => {
                        if let Some(v) = verts.last_mut() {
                            v.1 = num(value);
                        }
                    }
                    _ => {}
                }
            }
            Shape::Polyline {
                vertices: verts,
                closed: flags & 1 != 0,
            }
        }
        "POLYLINE" => {
            // 3D meshes and polyface meshes are not pattern outlines.
            if flags & (16 | 64) != 0 {
                return None;
            }
            Shape::Polyline {
                vertices: vertices
                    .iter()
                    .filter(|(_, _, f)| f & 16 == 0) // skip spline frame points
                    .map(|&(p, b, _)| (p, b))
                    .collect(),
                closed: flags & 1 != 0,
            }
        }
        "ARC" => Shape::Arc {
            center: point(fields, 10, 20),
            radius: field(fields, 40).map_or(0.0, num),
            start_deg: field(fields, 50).map_or(0.0, num),
            end_deg: field(fields, 51).map_or(360.0, num),
        },
        "CIRCLE" => Shape::Circle {
            center: point(fields, 10, 20),
            radius: field(fields, 40).map_or(0.0, num),
        },
        "POINT" => Shape::Point {
            at: point(fields, 10, 20),
        },
        "TEXT" | "MTEXT" => {
            let mut text: String = fields
                .iter()
                .filter(|(c, _)| *c == 3)
                .map(|(_, v)| *v)
                .collect();
            text.push_str(field(fields, 1).unwrap_or(""));
            Shape::Text {
                at: point(fields, 10, 20),
                height: field(fields, 40).map_or(0.0, num),
                text: if kind == "MTEXT" {
                    plain_text(&text)
                } else {
                    text
                },
            }
        }
        "SPLINE" => {
            let mut control = Vec::new();
            let mut fit = Vec::new();
            for &(code, value) in fields {
                match code {
                    10 => control.push(Point::new(num(value), 0.0)),
                    20 => {
                        if let Some(p) = control.last_mut() {
                            p.y = num(value);
                        }
                    }
                    11 => fit.push(Point::new(num(value), 0.0)),
                    21 => {
                        if let Some(p) = fit.last_mut() {
                            p.y = num(value);
                        }
                    }
                    _ => {}
                }
            }
            Shape::Spline {
                degree: field(fields, 71).map_or(3, |v| v.parse().unwrap_or(3)),
                knots: fields
                    .iter()
                    .filter(|(c, _)| *c == 40)
                    .map(|(_, v)| num(v))
                    .collect(),
                control,
                weights: fields
                    .iter()
                    .filter(|(c, _)| *c == 41)
                    .map(|(_, v)| num(v))
                    .collect(),
                fit,
                closed: flags & 1 != 0,
            }
        }
        "INSERT" => Shape::Insert {
            block: field(fields, 2).unwrap_or("").to_string(),
            at: point(fields, 10, 20),
            scale: (
                field(fields, 41).map_or(1.0, num),
                field(fields, 42).map_or(1.0, num),
            ),
            rotation_deg: field(fields, 50).map_or(0.0, num),
        },
        _ => return None,
    })
}

/// Parses ASCII DXF text.
pub(crate) fn parse(text: &str) -> Result<Drawing, String> {
    let pairs: Vec<(i32, &str)> = Pairs {
        lines: text.lines(),
    }
    .collect::<Result<_, _>>()?;
    let mut drawing = Drawing::default();
    let mut i = 0;
    let mut section = "";
    // The block being defined, if inside BLOCKS.
    let mut block: Option<(String, Block)> = None;

    while i < pairs.len() {
        let (code, value) = pairs[i];
        if code != 0 {
            i += 1;
            continue;
        }
        match value {
            "SECTION" => {
                section = pairs
                    .get(i + 1)
                    .filter(|(c, _)| *c == 2)
                    .map_or("", |(_, v)| *v);
                i += 2;
                if section == "HEADER" {
                    // $NAME on code 9, followed by its values.
                    while i < pairs.len() && pairs[i] != (0, "ENDSEC") {
                        if pairs[i].0 == 9 {
                            let name = pairs[i].1;
                            let value = pairs.get(i + 1).and_then(|(_, v)| v.parse::<i64>().ok());
                            match name {
                                "$INSUNITS" => drawing.insunits = value,
                                "$MEASUREMENT" => drawing.measurement = value,
                                _ => {}
                            }
                        }
                        i += 1;
                    }
                }
                continue;
            }
            "ENDSEC" => {
                section = "";
                i += 1;
                continue;
            }
            "EOF" => break,
            "BLOCK" if section == "BLOCKS" => {
                let mut j = i + 1;
                let mut fields = Vec::new();
                while j < pairs.len() && pairs[j].0 != 0 {
                    fields.push(pairs[j]);
                    j += 1;
                }
                let name = field(&fields, 2).unwrap_or("").to_string();
                block = Some((
                    name,
                    Block {
                        base: point(&fields, 10, 20),
                        entities: Vec::new(),
                    },
                ));
                i = j;
                continue;
            }
            "ENDBLK" => {
                if let Some((name, b)) = block.take() {
                    drawing.blocks.insert(name, b);
                }
                i += 1;
                while i < pairs.len() && pairs[i].0 != 0 {
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        if section != "ENTITIES" && section != "BLOCKS" {
            i += 1;
            continue;
        }
        // An entity: its fields run to the next code 0.
        let kind = value;
        let mut j = i + 1;
        let mut fields: Fields = Vec::new();
        while j < pairs.len() && pairs[j].0 != 0 {
            fields.push(pairs[j]);
            j += 1;
        }
        // An old-style POLYLINE is followed by VERTEX entities and SEQEND.
        let mut vertices = Vec::new();
        if kind == "POLYLINE" {
            while j < pairs.len() && pairs[j] == (0, "VERTEX") {
                let mut k = j + 1;
                let mut vf: Fields = Vec::new();
                while k < pairs.len() && pairs[k].0 != 0 {
                    vf.push(pairs[k]);
                    k += 1;
                }
                let flag = field(&vf, 70).map_or(0, |v| v.parse::<i64>().unwrap_or(0));
                vertices.push((point(&vf, 10, 20), field(&vf, 42).map_or(0.0, num), flag));
                j = k;
            }
            if j < pairs.len() && pairs[j] == (0, "SEQEND") {
                j += 1;
                while j < pairs.len() && pairs[j].0 != 0 {
                    j += 1;
                }
            }
        }
        let layer = field(&fields, 8).unwrap_or("0").to_string();
        match entity_from(kind, &fields, &vertices) {
            Some(shape) => {
                let entity = Entity { layer, shape };
                match &mut block {
                    Some((_, b)) => b.entities.push(entity),
                    None => drawing.entities.push(entity),
                }
            }
            None => {
                if !matches!(kind, "ATTDEF" | "ATTRIB" | "VIEWPORT" | "SEQEND") {
                    drawing.skipped += 1;
                }
            }
        }
        i = j;
    }
    Ok(drawing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_units_entities_and_blocks() {
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
                   0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nPOCKET\n10\n0\n20\n0\n\
                   0\nLWPOLYLINE\n8\n1\n70\n1\n90\n3\n10\n0\n20\n0\n10\n100\n20\n0\n42\n0.5\n10\n100\n20\n50\n\
                   0\nENDBLK\n0\nENDSEC\n\
                   0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nPOCKET\n10\n10\n20\n20\n\
                   0\nLINE\n8\nGRAIN\n10\n1\n20\n2\n11\n3\n21\n4\n0\nHATCH\n8\n0\n0\nENDSEC\n0\nEOF\n";
        let d = parse(dxf).unwrap();
        assert_eq!(d.insunits, Some(4));
        let pocket = &d.blocks["POCKET"];
        assert_eq!(
            pocket.entities[0].shape,
            Shape::Polyline {
                vertices: vec![
                    (Point::new(0.0, 0.0), 0.0),
                    (Point::new(100.0, 0.0), 0.5),
                    (Point::new(100.0, 50.0), 0.0)
                ],
                closed: true,
            }
        );
        assert_eq!(d.entities.len(), 2);
        assert_eq!(
            d.entities[1],
            Entity {
                layer: "GRAIN".into(),
                shape: Shape::Line {
                    a: Point::new(1.0, 2.0),
                    b: Point::new(3.0, 4.0)
                }
            }
        );
        assert_eq!(d.skipped, 1);
    }

    #[test]
    fn reads_old_style_polylines_with_vertices() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n8\n1\n66\n1\n70\n1\n\
                   0\nVERTEX\n8\n1\n10\n0\n20\n0\n0\nVERTEX\n8\n1\n10\n5\n20\n0\n0\nVERTEX\n8\n1\n10\n5\n20\n5\n\
                   0\nSEQEND\n8\n1\n0\nENDSEC\n0\nEOF\n";
        let d = parse(dxf).unwrap();
        assert!(
            matches!(&d.entities[0].shape, Shape::Polyline { vertices, closed: true } if vertices.len() == 3)
        );
    }

    #[test]
    fn strips_mtext_formatting() {
        assert_eq!(plain_text("{\\fArial|b1;Front}\\PCut 2"), "Front Cut 2");
    }

    #[test]
    fn decodes_windows_1252() {
        assert_eq!(decode(&[0x44, 0xe9, 0x63, 0x6f, 0x96]), "Déco–");
    }
}
