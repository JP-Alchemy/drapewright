// The avatar, lit softly from above and a little to one side, with a
// faint rim so its outline reads against the background; the cloth draped
// on it, lit alike but matt, on whichever side is seen, each point in its
// own colour (a heatmap, or the fabric's). The floor is a soft shadow
// under the feet.

struct Uniforms {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    light: vec4<f32>,
    figure: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // x: the shadow's radius, mm; y: how dark at its middle.
    floor: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct Lit {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn figure_vertex(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> Lit {
    var out: Lit;
    out.clip = u.view_projection * vec4<f32>(position, 1.0);
    out.normal = normal;
    out.world = position;
    return out;
}

@fragment
fn figure_fragment(in: Lit) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(u.eye.xyz - in.world);
    let l = normalize(u.light.xyz);
    let hemisphere = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    let diffuse = max(dot(n, l), 0.0);
    let h = normalize(l + v);
    let shine = pow(max(dot(n, h), 0.0), 24.0) * 0.08;
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.12;
    let colour = u.figure.rgb * (hemisphere + 0.8 * diffuse) + vec3<f32>(shine + rim);
    return vec4<f32>(colour, 1.0);
}

struct Cloth {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) colour: vec3<f32>,
};

@vertex
fn cloth_vertex(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) colour: vec3<f32>,
) -> Cloth {
    var out: Cloth;
    out.clip = u.view_projection * vec4<f32>(position, 1.0);
    out.normal = normal;
    out.world = position;
    out.colour = colour;
    return out;
}

@fragment
fn cloth_fragment(in: Cloth) -> @location(0) vec4<f32> {
    let v = normalize(u.eye.xyz - in.world);
    // The side facing the eye: a panel's triangles may wind either way.
    var n = normalize(in.normal);
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    let l = normalize(u.light.xyz);
    let hemisphere = mix(u.ground.rgb, u.sky.rgb, 0.5 + 0.5 * n.y);
    let diffuse = max(dot(n, l), 0.0);
    let rim = pow(1.0 - max(dot(n, v), 0.0), 2.0) * 0.06;
    let colour = in.colour * (hemisphere + 0.85 * diffuse) + vec3<f32>(rim);
    return vec4<f32>(colour, 1.0);
}

struct Shadow {
    @builtin(position) clip: vec4<f32>,
    @location(0) at: vec2<f32>,
};

@vertex
fn floor_vertex(@location(0) corner: vec2<f32>) -> Shadow {
    var out: Shadow;
    let r = u.floor.x;
    out.clip = u.view_projection * vec4<f32>(corner.x * r, 0.0, corner.y * r * 0.7, 1.0);
    out.at = corner;
    return out;
}

@fragment
fn floor_fragment(in: Shadow) -> @location(0) vec4<f32> {
    let d = length(in.at);
    let alpha = (1.0 - smoothstep(0.15, 1.0, d)) * u.floor.y;
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}
