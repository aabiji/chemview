struct Vertex {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) pos: vec4<f32>,
}

@group(0)
@binding(0)
var<uniform> transform: mat4x4<f32>;

@vertex
fn vertex_shader(
    @location(0) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
) -> Vertex {
    var vertex: Vertex;
    vertex.tex_coord = tex_coord;
    vertex.pos = transform * position;
    return vertex;
}

@fragment
fn fragment_shader(v: Vertex) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
