//! A surface to draw: triangles with smooth normals.

/// Points (mm), their normals, and triangles as three indices each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl Mesh {
    /// A mesh of `triangles` over `positions`, each point's normal the
    /// average of its triangles', weighted by their areas.
    pub fn new(positions: Vec<[f32; 3]>, triangles: &[[u32; 3]]) -> Mesh {
        let mut normals = vec![[0.0f32; 3]; positions.len()];
        for t in triangles {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            // The cross product's length is twice the area: weighted already.
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            for &i in t {
                let m = &mut normals[i as usize];
                m[0] += n[0];
                m[1] += n[1];
                m[2] += n[2];
            }
        }
        for n in &mut normals {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 0.0 {
                *n = [n[0] / l, n[1] / l, n[2] / l];
            }
        }
        Mesh {
            positions,
            normals,
            indices: triangles.iter().flatten().copied().collect(),
        }
    }

    /// The corners of the box round the points used by triangles, mm.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for &i in &self.indices {
            let p = self.positions[i as usize];
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
        (low, high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normals_point_out_of_a_closed_surface() {
        // A tetrahedron, wound outwards.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let mesh = Mesh::new(positions, &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]);
        let centre = [0.25, 0.25, 0.25];
        for (p, n) in mesh.positions.iter().zip(&mesh.normals) {
            let out = (0..3).map(|k| (p[k] - centre[k]) * n[k]).sum::<f32>();
            assert!(out > 0.0, "{p:?} {n:?}");
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((length - 1.0).abs() < 1e-5);
        }
        assert_eq!(mesh.bounds(), ([0.0; 3], [1.0; 3]));
        assert_eq!(mesh.indices.len(), 12);
    }
}
