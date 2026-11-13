// What makes a heap of cubes a cloud (`cube_game::clouds`): the light inside it. A cloud is
// drawn as Bevy draws any matt white thing, and then given the sun's light as a cloud has
// it: scattered about within, so it comes out of every side, less where there is much cloud
// overhead; and thrown forward past its edges when the sun is behind it, the silver lining.
// Each cube faces the way the cloud's density falls off there, not the way its own faces
// do, so an edge of the cloud as seen is a cube whose facing is square to the eye.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Which way the sun is (xyz), from the game.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> sun: vec4<f32>;
// The sun's light as a white thing facing it would give it back at a cloud's height, in
// the colour the air has left it, and already at the camera's exposure (rgb).
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> glow: vec4<f32>;

// How much of that light comes out of a cloud's every side, at its top and under its
// thickest part; and how much more at an edge with the sun behind.
const WITHIN: f32 = 0.3;
const UNDER: f32 = 0.3;
const LINING: f32 = 0.55;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);

    // How much cloud is over this cube is in its colour, as a greyer white (blue is the
    // plain amount: 0.57 under the thickest, 1 at the top).
    var open = 1.0;
@if(VERTEX_COLORS)
    open = smoothstep(0.57, 1.0, in.color.b);
    // (With the sun at the horizon or under it the light comes in from the side and from
    // below, and a cloud's foot has as much of it as its top.)
    let level = clamp(1.0 - sun.y / 0.15, 0.0, 1.0);
    let within = WITHIN * mix(mix(UNDER, 0.8, level), 1.0, open);
    // An edge is where the cloud's surface is square to the eye; the sun is behind when the
    // eye looks toward it. The lining is brightest close round the sun and faint elsewhere.
    let edge = pow(1.0 - abs(dot(pbr_input.N, pbr_input.V)), 2.0);
    let toward = max(dot(-pbr_input.V, sun.xyz), 0.0);
    let lining = LINING * edge * (0.2 + 1.8 * pow(toward, 6.0));
    out.color = vec4<f32>(out.color.rgb + glow.rgb * (within + lining), out.color.a);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
