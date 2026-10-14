// The cloth's small steps on the graphics card, as `drapewright-sim` takes
// them on the processor (cloth.rs): each step the points are moved on by
// their velocity and gravity; edges, bends and stitches are held, a colour
// at a time (no two constraints of a colour share a point, so each colour
// is solved at once); the cloth is kept off the body, with friction; cloth
// inside (facings, linings) is kept behind the cloth over it; and the
// velocities are what the points did. Metres, seconds.

struct Params {
    // Gravity (m/s²); w: the step, s.
    gravity: vec4<f32>,
    // The seat's lowest corner; w: 1 if there is a seat.
    seat_low: vec4<f32>,
    // The seat's highest corner; w: how far the body is from field a to b.
    seat_high: vec4<f32>,
    // How far to move the whole cloth (before its first step); w: the
    // fastest a point may go, m/s.
    shift: vec4<f32>,
    // Velocity kept each step, the gap kept off the body, friction, and
    // stitches' compliance over the step squared.
    a: vec4<f32>,
    // How far inside cloth is kept behind the cloth over it, and within how far.
    b: vec4<f32>,
    // Points, the first inside one, 1 if there is a field b.
    counts: vec4<u32>,
}

// A distance field's grid: its first point (w: between points, m); its
// size (w: where its values start in `fields`).
struct Grid {
    low: vec4<f32>,
    size: vec4<u32>,
}

struct Grids {
    a: Grid,
    b: Grid,
}

// The constraints of one colour: where they start, and how many.
struct Range {
    start: u32,
    count: u32,
    pad0: u32,
    pad1: u32,
}

struct Edge {
    i: u32,
    j: u32,
    rest: f32,
    compliance: f32,
}

struct Bend {
    i: vec4<u32>,
    k: vec4<f32>,
    // x: compliance.
    compliance: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
// Where each point is; w: its inverse mass (0: held still).
@group(0) @binding(1) var<storage, read_write> x: array<vec4<f32>>;
// Where each was before this step.
@group(0) @binding(2) var<storage, read_write> p: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> v: array<vec4<f32>>;
// The way out from the body where a point touches it; w: 1 if it does.
@group(0) @binding(4) var<storage, read_write> contact: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read> edges: array<Edge>;
@group(0) @binding(6) var<storage, read> bends: array<Bend>;
@group(0) @binding(7) var<storage, read> stitches: array<vec2<u32>>;
// Triangles' corners; w: 1 if it faces out as its corners run, else 0.
@group(0) @binding(8) var<storage, read> tris: array<vec4<u32>>;
// Each point's triangles: where its list starts (indices into this array,
// one more than the points), then the lists.
@group(0) @binding(9) var<storage, read> around: array<u32>;
@group(0) @binding(10) var<uniform> grids: Grids;
@group(0) @binding(11) var<storage, read> fields: array<f32>;
// The outside cloth's outward normal at each of its points.
@group(0) @binding(12) var<storage, read_write> outward: array<vec4<f32>>;
// Each inside point's outside points near it, as `around` is laid out.
@group(0) @binding(13) var<storage, read> layers: array<u32>;
// Each outside point's inside points near it, likewise.
@group(0) @binding(14) var<storage, read> layers_out: array<u32>;
// The way apart where a point rests on the other layer; w: 1 if it does.
@group(0) @binding(15) var<storage, read_write> layered: array<vec4<f32>>;
@group(1) @binding(0) var<uniform> range: Range;

const FAR: f32 = 1e30;

fn step() -> f32 {
    return params.gravity.w;
}

@compute @workgroup_size(64)
fn predict(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if k >= params.counts.x {
        return;
    }
    let xk = x[k];
    p[k] = xk;
    contact[k] = vec4<f32>(0.0);
    layered[k] = vec4<f32>(0.0);
    if xk.w > 0.0 {
        let vk = (v[k].xyz + step() * params.gravity.xyz) * params.a.x;
        v[k] = vec4<f32>(vk, 0.0);
        x[k] = vec4<f32>(xk.xyz + step() * vk, xk.w);
    }
}

@compute @workgroup_size(64)
fn shift(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if k >= params.counts.x {
        return;
    }
    let xk = x[k];
    x[k] = vec4<f32>(xk.xyz + params.shift.xyz, xk.w);
}

@compute @workgroup_size(64)
fn solve_edges(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= range.count {
        return;
    }
    let e = edges[range.start + id.x];
    let xi = x[e.i];
    let xj = x[e.j];
    let d = xi.xyz - xj.xyz;
    let l = length(d);
    if l < 1e-9 || xi.w + xj.w == 0.0 {
        return;
    }
    let n = d / l;
    let alpha = e.compliance / (step() * step());
    let dl = -(l - e.rest) / (xi.w + xj.w + alpha);
    x[e.i] = vec4<f32>(xi.xyz + xi.w * dl * n, xi.w);
    x[e.j] = vec4<f32>(xj.xyz - xj.w * dl * n, xj.w);
}

@compute @workgroup_size(64)
fn solve_bends(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= range.count {
        return;
    }
    let b = bends[range.start + id.x];
    // The curvature `Σ k·x` held at zero, each axis on its own.
    var weight = 0.0;
    var c = vec3<f32>(0.0);
    for (var q = 0u; q < 4u; q = q + 1u) {
        let xq = x[b.i[q]];
        weight = weight + xq.w * b.k[q] * b.k[q];
        c = c + b.k[q] * xq.xyz;
    }
    let alpha = b.compliance.x / (step() * step());
    if weight + alpha == 0.0 {
        return;
    }
    let dl = -c / (weight + alpha);
    for (var q = 0u; q < 4u; q = q + 1u) {
        let xq = x[b.i[q]];
        x[b.i[q]] = vec4<f32>(xq.xyz + xq.w * b.k[q] * dl, xq.w);
    }
}

@compute @workgroup_size(64)
fn solve_stitches(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= range.count {
        return;
    }
    let s = stitches[range.start + id.x];
    let xi = x[s.x];
    let xj = x[s.y];
    let d = xi.xyz - xj.xyz;
    let l = length(d);
    if l < 1e-9 || xi.w + xj.w == 0.0 {
        return;
    }
    let n = d / l;
    let dl = -l / (xi.w + xj.w + params.a.w);
    x[s.x] = vec4<f32>(xi.xyz + xi.w * dl * n, xi.w);
    x[s.y] = vec4<f32>(xj.xyz - xj.w * dl * n, xj.w);
}

// A distance field at `q`, blended between its grid points: the
// gradient, and (w) the distance; w FAR off its grid.
fn sample(g: Grid, q: vec3<f32>) -> vec4<f32> {
    let cell = g.low.w;
    let at = (q - g.low.xyz) / cell;
    let size = g.size.xyz;
    if at.x < 0.0 || at.y < 0.0 || at.z < 0.0 || at.x >= f32(size.x - 1u) || at.y >= f32(size.y - 1u) || at.z >= f32(size.z - 1u) {
        return vec4<f32>(0.0, 1.0, 0.0, FAR);
    }
    let c = vec3<u32>(floor(at));
    let f = at - floor(at);
    let base = g.size.w;
    let sx = size.x;
    let sxy = size.x * size.y;
    let i0 = base + c.x + sx * c.y + sxy * c.z;
    let v000 = fields[i0];
    let v100 = fields[i0 + 1u];
    let v010 = fields[i0 + sx];
    let v110 = fields[i0 + sx + 1u];
    let v001 = fields[i0 + sxy];
    let v101 = fields[i0 + sxy + 1u];
    let v011 = fields[i0 + sxy + sx];
    let v111 = fields[i0 + sxy + sx + 1u];
    let x00 = mix(v000, v100, f.x);
    let x10 = mix(v010, v110, f.x);
    let x01 = mix(v001, v101, f.x);
    let x11 = mix(v011, v111, f.x);
    let y0 = mix(x00, x10, f.y);
    let y1 = mix(x01, x11, f.y);
    let d = mix(y0, y1, f.z);
    let gx = mix(mix(v100 - v000, v110 - v010, f.y), mix(v101 - v001, v111 - v011, f.y), f.z);
    let gy = mix(x10 - x00, x11 - x01, f.z);
    let gz = y1 - y0;
    return vec4<f32>(vec3<f32>(gx, gy, gz) / cell, d);
}

// The way out from a distance and its gradient.
fn outwards(d: f32, g: vec3<f32>) -> vec4<f32> {
    let l = length(g);
    if l > 1e-6 {
        return vec4<f32>(g / l, d);
    }
    return vec4<f32>(0.0, 1.0, 0.0, d);
}

// The seat (a box): how far `q` is outside it, and which way is out.
fn seat(q: vec3<f32>) -> vec4<f32> {
    let low = params.seat_low.xyz;
    let high = params.seat_high.xyz;
    let middle = (low + high) / 2.0;
    let half = (high - low) / 2.0;
    let off = q - middle;
    let r = abs(off) - half;
    let out = max(r, vec3<f32>(0.0));
    let outside = length(out);
    if outside > 0.0 {
        return vec4<f32>(normalize(out * sign(off)), outside);
    }
    var n = vec3<f32>(0.0);
    var d = r.x;
    var k = 0u;
    if r.y >= d {
        d = r.y;
        k = 1u;
    }
    if r.z >= d {
        d = r.z;
        k = 2u;
    }
    n[k] = select(1.0, -1.0, off[k] < 0.0);
    return vec4<f32>(n, d);
}

// What a point at `q` feels: the way out (xyz) and how far it is out of the
// body or the seat, whichever is nearer (w).
fn felt(q: vec3<f32>) -> vec4<f32> {
    let a = sample(grids.a, q);
    var d = a.w;
    var g = a.xyz;
    if params.counts.z == 1u {
        let b = sample(grids.b, q);
        let s = params.seat_high.w;
        if a.w < FAR && b.w < FAR {
            d = a.w + (b.w - a.w) * s;
            g = a.xyz + (b.xyz - a.xyz) * s;
        } else if b.w < FAR {
            d = b.w;
            g = b.xyz;
        }
    }
    var out = vec4<f32>(0.0, 1.0, 0.0, FAR);
    if d < FAR {
        out = outwards(d, g);
    }
    if params.seat_low.w > 0.5 {
        let s = seat(q);
        if s.w < out.w {
            out = s;
        }
    }
    return out;
}

@compute @workgroup_size(64)
fn collide(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if k >= params.counts.x {
        return;
    }
    var xk = x[k];
    if xk.w == 0.0 {
        return;
    }
    let gap = params.a.y;
    let mu = params.a.z;
    let pk = p[k].xyz;
    var touching = vec4<f32>(0.0);
    // The point, then the middles of its triangles: the body is curved
    // between points.
    let to = around[k + 1u];
    for (var q = around[k]; q <= to; q = q + 1u) {
        var at = xk.xyz;
        if q < to {
            let t = tris[around[q]];
            var corners = vec3<f32>(0.0);
            for (var c = 0u; c < 3u; c = c + 1u) {
                if t[c] == k {
                    corners = corners + xk.xyz;
                } else {
                    corners = corners + x[t[c]].xyz;
                }
            }
            at = corners / 3.0;
        }
        let f = felt(at);
        if f.w >= gap {
            continue;
        }
        // Pushed out, with friction (Coulomb, by position): its slide along
        // the body this step cancelled if small for how hard it presses,
        // else cut down by that much.
        let pressed = gap - f.w;
        let n = f.xyz;
        var moved = xk.xyz + pressed * n;
        let along = (moved - pk) - n * dot(moved - pk, n);
        let slide = length(along);
        if slide > 0.0 {
            moved = moved - min(mu * pressed / slide, 1.0) * along;
        }
        xk = vec4<f32>(moved, xk.w);
        touching = vec4<f32>(n, 1.0);
    }
    x[k] = xk;
    contact[k] = touching;
}

@compute @workgroup_size(64)
fn face_out(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    let first = params.counts.y;
    if k >= first {
        return;
    }
    var sum = vec3<f32>(0.0);
    for (var q = around[k]; q < around[k + 1u]; q = q + 1u) {
        let t = tris[around[q]];
        if t.x >= first || t.y >= first || t.z >= first {
            continue;
        }
        let a = x[t.x].xyz;
        let facing = select(-1.0, 1.0, t.w == 1u);
        sum = sum + facing * cross(x[t.y].xyz - a, x[t.z].xyz - a);
    }
    let l = length(sum);
    outward[k] = vec4<f32>(select(vec3<f32>(0.0), sum / l, l > 0.0), 0.0);
}

// How far inside point `xi` falls short of being behind outside point
// `xj` (outward normal `n`) by `apart`; 0 where they are apart enough, or
// not over each other.
fn short_of(xi: vec3<f32>, xj: vec3<f32>, n: vec3<f32>) -> f32 {
    let apart = params.b.x;
    let reach = params.b.y;
    if dot(n, n) == 0.0 {
        return 0.0;
    }
    let off = xi - xj;
    let along = dot(off, n);
    if along < -apart || along > reach || length(off - n * along) > reach {
        return 0.0;
    }
    return along + apart;
}

// Friction between two layers pressed together `short` along `n`: of
// their slide past each other this step (`moved`, the inside's less the
// outside's), how much is held back (all of it, if small for how hard
// they press).
fn held_slip(moved: vec3<f32>, n: vec3<f32>, short: f32) -> vec3<f32> {
    let slip = moved - n * dot(moved, n);
    let slide = length(slip);
    if slide <= 0.0 {
        return vec3<f32>(0.0);
    }
    return min(params.a.z * short / slide, 1.0) * slip;
}

// The outside cloth pushed out from the cloth inside it near it, by its
// share (by mass).
@compute @workgroup_size(64)
fn order_layers_out(@builtin(global_invocation_id) id: vec3<u32>) {
    let j = id.x;
    if j >= params.counts.y {
        return;
    }
    let xj = x[j];
    if xj.w == 0.0 {
        return;
    }
    let n = outward[j].xyz;
    var at = xj.xyz;
    var pushed = false;
    let pj = p[j].xyz;
    for (var q = layers_out[j]; q < layers_out[j + 1u]; q = q + 1u) {
        let i = layers_out[q];
        let xi = x[i];
        let short = short_of(xi.xyz, at, n);
        if short > 0.0 {
            let share = xj.w / (xi.w + xj.w);
            at = at + short * share * n;
            at = at + share * held_slip(xi.xyz - p[i].xyz - (at - pj), n, short);
            pushed = true;
        }
    }
    x[j] = vec4<f32>(at, xj.w);
    // Resting on each other, as on the body: being pushed apart is not speed.
    if pushed {
        layered[j] = vec4<f32>(n, 1.0);
    }
}

// The inside cloth pushed in behind the outside cloth near it, by its
// share (by mass).
@compute @workgroup_size(64)
fn order_layers(@builtin(global_invocation_id) id: vec3<u32>) {
    let first = params.counts.y;
    let i = first + id.x;
    if i >= params.counts.x {
        return;
    }
    let xi = x[i];
    if xi.w == 0.0 {
        return;
    }
    var at = xi.xyz;
    for (var q = layers[id.x]; q < layers[id.x + 1u]; q = q + 1u) {
        let j = layers[q];
        let xj = x[j];
        let n = outward[j].xyz;
        let short = short_of(at, xj.xyz, n);
        if short > 0.0 {
            let share = xi.w / (xi.w + xj.w);
            at = at - short * share * n;
            at = at - share * held_slip(at - p[i].xyz - (xj.xyz - p[j].xyz), n, short);
            layered[i] = vec4<f32>(n, 1.0);
        }
    }
    x[i] = vec4<f32>(at, xi.w);
}

@compute @workgroup_size(64)
fn velocity(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if k >= params.counts.x {
        return;
    }
    var vk = (x[k].xyz - p[k].xyz) / step();
    let speed = length(vk);
    if speed > params.shift.w {
        vk = vk * (params.shift.w / speed);
    }
    // Resting on the body, or on the other layer, a point is not moving
    // towards it or away.
    let c = contact[k];
    if c.w > 0.5 {
        vk = vk - dot(vk, c.xyz) * c.xyz;
    }
    let l = layered[k];
    if l.w > 0.5 {
        vk = vk - dot(vk, l.xyz) * l.xyz;
    }
    v[k] = vec4<f32>(vk, 0.0);
}
