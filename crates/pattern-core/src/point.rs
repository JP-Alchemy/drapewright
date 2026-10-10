use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

/// A point in millimetres. Stored in files as `[x, y]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, TS)]
#[ts(export, type = "[number, number]")]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn distance(self, other: Pt) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

impl Serialize for Pt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Pt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [x, y] = <[f64; 2]>::deserialize(d)?;
        Ok(Pt { x, y })
    }
}

impl From<Pt> for kurbo::Point {
    fn from(p: Pt) -> Self {
        kurbo::Point::new(p.x, p.y)
    }
}

impl From<kurbo::Point> for Pt {
    fn from(p: kurbo::Point) -> Self {
        Pt { x: p.x, y: p.y }
    }
}

impl From<[f64; 2]> for Pt {
    fn from([x, y]: [f64; 2]) -> Self {
        Pt { x, y }
    }
}
