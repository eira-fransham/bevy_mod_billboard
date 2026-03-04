#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_bindings::mesh,
    forward_io::{Vertex, VertexOutput},
    render::mesh,
} 

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
//     let camera_right = normalize(vec3<f32>(view.clip_from_world[0][0], view.clip_from_world[1][0], view.clip_from_world[2][0]));
// #ifdef LOCK_Y
//     let camera_up = vec3<f32>(0.0, 1.0, 0.0);
// #else
//     let camera_up = normalize(vec3<f32>(view.clip_from_world[0][1], view.clip_from_world[1][1], view.clip_from_world[2][1]));
// #endif

//     vertex.position = camera_right * vertex.position.x + camera_up * vertex.position.y;

    return mesh::view(vertex);
}
