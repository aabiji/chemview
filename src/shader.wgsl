struct SDFData {
    position: vec3<f32>,
    _padding: f32, // Since structs need to be 16 byte aligned
    color: vec3<f32>,
    radius: f32,
}

struct SDFResult {
    index: u32, // index of the hit object
    dist: f32,
}

struct RaymarchResult {
    index: u32,
    hit: u32,
    position: vec3<f32>,
    normal: vec3<f32>,
}

// NOTE: Uniforms must be 16-byte aligned
@group(0) @binding(0) var<uniform> view_matrix: mat3x4<f32>;
@group(0) @binding(1) var<storage, read> sdf_data: array<SDFData>;
@group(0) @binding(2) var<uniform> sdf_data_size: vec2<u32>;
@group(0) @binding(3) var<uniform> resolution: vec2<f32>;
@group(0) @binding(4) var<uniform> camera_pos: vec4<f32>;

// FIXME: Implement a more effecient way to render to render SDF objects
@vertex
fn vertex_shader(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Sample a full screen squad (2 triangles)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0), vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 1.0, -1.0), vec2<f32>( 1.0,  1.0),
    );
    let pos = positions[vertex_index];
    return vec4<f32>(pos, 0.0, 1.0);
}

fn scene_SDF(position: vec3<f32>) -> SDFResult {
    var result: SDFResult; // object with the min distance
    result.dist = 9999.9;
    for (var i: u32 = 0u; i < sdf_data_size.x; i++) {
        let o = sdf_data[i];
        // Sphere function for now
        let dist = length(position - o.position) - o.radius;
        if (dist < result.dist) {
            result.dist = dist;
            result.index = i;
        }
    }
    return result;
}

fn sdf_normal(p: vec3<f32>) -> vec3<f32> {
    let epsilon = 0.001;
    let a1 = vec3<f32>(p.x + epsilon, p.y, p.z);
    let a2 = vec3<f32>(p.x - epsilon, p.y, p.z);
    let b1 = vec3<f32>(p.x, p.y + epsilon, p.z);
    let b2 = vec3<f32>(p.x, p.y - epsilon, p.z);
    let c1 = vec3<f32>(p.x, p.y, p.z + epsilon);
    let c2 = vec3<f32>(p.x, p.y, p.z - epsilon);
    return normalize(vec3<f32>(
        scene_SDF(a1).dist - scene_SDF(a2).dist,
        scene_SDF(b1).dist - scene_SDF(b2).dist,
        scene_SDF(c1).dist - scene_SDF(c2).dist,
    ));
}

fn raymarch(ray_origin: vec3<f32>, ray_direction: vec3<f32>) -> RaymarchResult {
    let max_steps: i32 = 64;
    let epsilon: f32 = 0.001;
    var result: RaymarchResult;
    result.position = ray_origin;
    result.index = 0;
    result.hit = 0;

    for (var step = 0; step < max_steps; step++) {
        let sdf_result = scene_SDF(result.position);
        result.position += ray_direction * sdf_result.dist;

        if (sdf_result.dist < epsilon) {
            result.index = sdf_result.index;
            result.normal = sdf_normal(result.position);
            result.hit = 1;
            return result;
        }
    }

    return result;
}

@fragment
fn fragment_shader(@builtin(position) v: vec4<f32>) -> @location(0) vec4<f32> {
    let m = mat3x3<f32>(view_matrix[0].xyz, view_matrix[1].xyz, view_matrix[2].xyz);
    let uv = (v.xy - 0.5 * resolution) / resolution.y;
    let ray_direction = normalize(m * vec3(uv, -1.0));

    let result = raymarch(camera_pos.xyz, ray_direction);
    if (result.hit == 0) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); } // Background

    // Basic phong lighting
    let light_pos = vec3<f32>(5.0, 0.0, -5.0);
    let light_color = vec3<f32>(1.0, 1.0, 1.0);
    let obj_color = sdf_data[result.index].color;
    let shininess = 64.0;

    let ambient = 0.3 * obj_color;
    let light_dir = normalize(light_pos - result.position);
    let diff = max(dot(result.normal, light_dir), 0.0);
    let diffuse = diff * obj_color * light_color;

    let view_dir = normalize(camera_pos.xyz - result.position);
    let halfway = normalize(light_dir + view_dir);
    let spec = pow(max(dot(result.normal, halfway), 0.0), shininess);
    let specular = 0.15 * spec * light_color;

    return vec4<f32>(ambient + diffuse + specular, 1.0);
}
