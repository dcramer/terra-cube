// Depth and shadows for wind.wesl: the same push, so a swaying leaf's shadow sways with it
// and the depth drawn first matches the picture drawn after. Change both together.
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{
        mesh_functions,
        view_transformations::position_world_to_clip,
    },
};
import bevy_render::globals::Globals;

// Where the prepass keeps the time (the forward pass keeps it elsewhere).
@group(0) @binding(1) var<uniform> globals: Globals;

// The season, as in wind.wesl. This shader is also the one the deferred path draws colour
// with (the default, since water reflects the land), so the season's tint and the snow have
// to be here too: left out, a year went by in one green.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> season: vec4<f32>;

const REACH: f32 = 0.16;
const DIR: vec2<f32> = vec2<f32>(0.8, 0.6);

fn sway(world: vec3<f32>, free: f32, t: f32) -> vec3<f32> {
    let along = dot(world.xz, DIR);
    let gust = 0.35 + 0.65 * (0.5 + 0.5 * sin(along * 0.15 - t * 1.3 + sin(world.z * 0.05 + t * 0.2) * 2.0));
    let flutter = sin(world.x * 2.3 + world.z * 1.7 + world.y * 1.1 + t * 3.7) * 0.4 + sin(world.z * 3.1 - world.x * 1.3 + t * 5.9) * 0.25;
    let push = REACH * free * gust * (0.6 + flutter);
    return vec3<f32>(DIR.x * push, -abs(push) * 0.2, DIR.y * push);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_COLORS)
    out.world_position = vec4<f32>(out.world_position.xyz + sway(out.world_position.xyz, 1.0 - vertex.color.a, globals.time), 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
}

@if(VERTEX_COLORS) {
    let leaf = clamp((1.0 - vertex.color.a) * 2.5, 0.0, 1.0);
    var colour = mix(vertex.color.rgb, vertex.color.rgb * season.rgb, leaf);
@if(VERTEX_NORMALS)
    colour = mix(colour, vec3<f32>(1.1, 1.13, 1.2), season.a * smoothstep(0.5, 0.9, vertex.normal.y));
    out.color = vec4<f32>(colour, 1.0);
}

@if(MOTION_VECTOR_PREPASS) {
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(prev_model, vec4<f32>(vertex.position, 1.0));
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);

    return out;
}
