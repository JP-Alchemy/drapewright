//! The colours the cloth is shown in where it pulls, or by the room
//! between it and the body: as in the app's legend, shared by the app's
//! view and the assistants' pictures.

/// Pull shown at its strongest, N/m.
pub const PULL_SCALE: f32 = 100.0;
/// Room shown up to, mm.
pub const ROOM_SCALE: f32 = 40.0;

const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    ]
}

/// Pale where it hardly pulls, through yellow and orange to red.
const PULL_COLOURS: [(f32, [f32; 3]); 4] = [
    (0.0, rgb(0xe9eef4)),
    (0.25, rgb(0xffe08a)),
    (0.6, rgb(0xff9f43)),
    (1.0, rgb(0xd63031)),
];
/// Deep blue where it touches, paler as it stands away.
const ROOM_COLOURS: [(f32, [f32; 3]); 3] = [
    (0.0, rgb(0x1c64d1)),
    (0.35, rgb(0x7cc4fa)),
    (1.0, rgb(0xf1f3f5)),
];

/// The colour `t` (0 to 1) along a scale of colours.
fn along(scale: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    for pair in scale.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return [0, 1, 2].map(|k| c0[k] + (c1[k] - c0[k]) * f);
        }
    }
    scale[scale.len() - 1].1
}

/// Cloth pulling `pull` N/m (sRGB, 0 to 1).
pub fn pull_colour(pull: f32) -> [f32; 3] {
    along(&PULL_COLOURS, pull / PULL_SCALE)
}

/// Cloth `room` mm from the body (sRGB, 0 to 1).
pub fn room_colour(room: f32) -> [f32; 3] {
    along(&ROOM_COLOURS, room / ROOM_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6)
    }

    #[test]
    fn the_scales_run_from_their_first_colour_to_their_last() {
        assert!(close(pull_colour(0.0), rgb(0xe9eef4)));
        assert!(close(pull_colour(PULL_SCALE * 2.0), rgb(0xd63031)));
        assert!(close(room_colour(-5.0), rgb(0x1c64d1)));
        assert!(close(room_colour(ROOM_SCALE), rgb(0xf1f3f5)));
        let (a, b) = (rgb(0xe9eef4), rgb(0xffe08a));
        let halfway = [0, 1, 2].map(|k| (a[k] + b[k]) / 2.0);
        assert!(close(pull_colour(PULL_SCALE * 0.125), halfway));
    }
}
