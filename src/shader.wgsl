struct SDFData {
    position: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
}

@group(0) @binding(0) var<uniform> view_matrix: mat4x4<f32>;
@group(0) @binding(1) var<storage, read> sdf_data: array<SDFData>;
@group(0) @binding(2) var<uniform> sdf_data_size: u32;

// FIXME: Implement a more effecient way to render to render SDF objects
@vertex
fn vertex_shader(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Sample a full screen squad
    let x = f32((vertex_index & 1u) * 4u) - 1.0;
    let y = 1.0 - f32((vertex_index & 2u) * 2u);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fragment_shader(@builtin(position) v: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
