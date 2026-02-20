struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct InstanceData {
    model_matrix: mat4x4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;
@group(0) @binding(1) var<storage, read> instances: array<InstanceData>;

@vertex
fn vertex_shader(
    @location(0) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let instance = instances[instance_index];
    var vertex: VertexOutput;
    vertex.tex_coord = tex_coord;
    vertex.pos = view_projection * instance.model_matrix * position;
    vertex.color = instance.color;
    return vertex;
}

@fragment
fn fragment_shader(v: VertexOutput) -> @location(0) vec4<f32> {
    return v.color;
}
