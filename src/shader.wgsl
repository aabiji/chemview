struct Vertex {
    @builtin(position) pos: vec3<f32>,
}

@vertex
fn vertex_shader(@location(0) position: vec3<f32>) -> Vertex {
    var vertex: Vertex;
    vertex.pos = position;
    return vertex;
}

@fragment
fn fragment_shader(v: Vertex) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
