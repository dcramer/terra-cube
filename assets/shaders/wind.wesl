// Wind in what grows: the chunk mesh drawn as Bevy draws any mesh, but with each corner
// pushed along the wind by as much as it is free to move. That is in the corner's colour:
// alpha 1 is held fast (ground, wood), less is looser (a leaf, the tip of a blade of grass).
// The push depends only on where the corner is in the world and the time, so corners that
// meet move together and nothing tears. The same push is in wind_prepass.wesl, which draws
// depth and shadows: change both.
import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};

// The most a corner moves, metres, in the hardest wind; which way the wind blows.
// The season, from the game: how the leaves and grass are tinted (rgb), and how much snow
// lies (a).
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> season: vec4<f32>;
// How hard the wind blows now, 0 to 1 (x), from the game (`cube_sim::sky::wind`).
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> wind: vec4<f32>;

const REACH: f32 = 0.3;
const DIR: vec2<f32> = vec2<f32>(0.8, 0.6);

fn sway(world: vec3<f32>, free: f32, t: f32) -> vec3<f32> {
    // Gusts some forty metres long rolling downwind, and a quick flutter under them.
    let along = dot(world.xz, DIR);
    let gust = 0.35 + 0.65 * (0.5 + 0.5 * sin(along * 0.15 - t * 1.3 + sin(world.z * 0.05 + t * 0.2) * 2.0));
    let flutter = sin(world.x * 2.3 + world.z * 1.7 + world.y * 1.1 + t * 3.7) * 0.4 + sin(world.z * 3.1 - world.x * 1.3 + t * 5.9) * 0.25;
    let push = REACH * wind.x * free * gust * (0.6 + flutter);
    return vec3<f32>(DIR.x * push, -abs(push) * 0.2, DIR.y * push);
}

// A whole tree leans with the wind and comes back, its top most: by how high a corner
// stands over the ground (the second texture place of what grows, `flora::Shape::high`),
// squared, as a pole bends. Slower and longer than the leaves' own stir.
const LEAN: f32 = 0.4;

fn bend(world: vec3<f32>, high: f32, t: f32) -> vec3<f32> {
    let along = dot(world.xz, DIR);
    let swell = 0.55 + 0.45 * sin(along * 0.06 - t * 0.9 + sin(world.x * 0.03 + t * 0.13) * 1.5);
    let up = clamp(high / 14.0, 0.0, 1.3);
    let push = LEAN * wind.x * swell * up * up;
    return vec3<f32>(DIR.x * push, -push * 0.15 * up, DIR.y * push);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var pushed = out.world_position.xyz;
@if(VERTEX_UVS_B)
    pushed += bend(out.world_position.xyz, vertex.uv_b.x, globals.time);
@if(VERTEX_COLORS)
    pushed += sway(out.world_position.xyz, 1.0 - vertex.color.a, globals.time);
    out.world_position = vec4<f32>(pushed, 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);

@if(VERTEX_COLORS) {
    // What grows takes the season's colour, by how much of a leaf it is (the same number
    // that says how free it is to move); and in winter snow lies on whatever faces up.
    let leaf = clamp((1.0 - vertex.color.a) * 2.5, 0.0, 1.0);
    var colour = mix(vertex.color.rgb, vertex.color.rgb * season.rgb, leaf);
@if(VERTEX_NORMALS)
    colour = mix(colour, vec3<f32>(1.1, 1.13, 1.2), season.a * smoothstep(0.5, 0.9, vertex.normal.y));
    out.color = vec4<f32>(colour, 1.0);
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);

    return out;
}
