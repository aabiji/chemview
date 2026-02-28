struct InstanceData {
    model: mat4x4<f32>,
    color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) world_pos: vec4<f32>,
    @location(2) normal: vec4<f32>,
    @location(3) color: vec4<f32>,
}

// NOTE: Uniforms must be 16-byte aligned
@group(0) @binding(0) var<uniform> projection_matrix: mat4x4<f32>;
@group(0) @binding(1) var<uniform> view_matrix: mat4x4<f32>;
@group(0) @binding(2) var<uniform> camera_pos: vec4<f32>;
@group(0) @binding(3) var<storage, read> instance_data: array<InstanceData>;

@vertex
fn vertex_shader(
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @builtin(instance_index) instance_index: u32
) -> VertexOutput {
    let instance = instance_data[instance_index];
    var v: VertexOutput;
    v.world_pos = instance.model * position;
    v.pos = projection_matrix * view_matrix * v.world_pos;
    v.normal = normal;
    v.color = instance.color;
    return v;
}

@fragment
fn fragment_shader(v: VertexOutput) -> @location(0) vec4<f32> {
    // Basic phong lighting
    let light_pos = vec3<f32>(5.0, 1.0, 5.0);
    let light_color = vec3<f32>(1.0, 1.0, 1.0);
    let shininess = 64.0;

    let ambient = 0.3 * v.color.xyz;
    let light_dir = normalize(light_pos - v.world_pos.xyz);
    let diff = max(dot(v.normal.xyz, light_dir), 0.0);
    let diffuse = diff * v.color.xyz * light_color;

    let view_dir = normalize(camera_pos.xyz - v.world_pos.xyz);
    let halfway = normalize(light_dir + view_dir);
    let spec = pow(max(dot(v.normal.xyz, halfway), 0.0), shininess);
    let specular = 0.15 * spec * light_color;

    return vec4<f32>(ambient + diffuse + specular, 1.0);
}
