
struct Shape {
    start_pos: vec4<f32>,
    end_pos: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    shape_type: u32,
    _padding: u32
}

struct Intersection {
    normal: vec3<f32>,
    hit_pos: vec3<f32>,
    shape_index: i32,
    hit: bool,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) world_pos: vec4<f32>,
    @location(2) normal: vec4<f32>,
    @location(3) color: vec4<f32>,
}

// View data
@group(0) @binding(0) var<uniform> projection_matrix: mat4x4<f32>;
@group(0) @binding(1) var<uniform> view_matrix: mat4x4<f32>;
@group(0) @binding(2) var<uniform> camera_pos: vec4<f32>;
@group(0) @binding(3) var<uniform> resolution: vec2<f32>;
@group(0) @binding(4) var<uniform> object_rotation: mat4x4<f32>;

// Raytrace shape data
@group(0) @binding(5) var<storage, read> shapes: array<Shape>;
@group(0) @binding(6) var<storage, read> num_shapes: u32;

@vertex
fn geometry_vertex_shader(
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
    @builtin(instance_index) i: u32) -> VertexOutput {
    var v: VertexOutput;
    v.world_pos = object_rotation * position;
    v.pos = projection_matrix * view_matrix * v.world_pos;
    v.color = color;
    v.normal = normal;
    return v;
}

@fragment
fn geometry_fragment_shader(v: VertexOutput) -> @location(0) vec4<f32> {
    return phong_lighting(v.world_pos.xyz, v.normal.xyz, v.color.xyz);
}

fn phong_lighting(position: vec3<f32>, normal: vec3<f32>, obj_color: vec3<f32>) -> vec4<f32> {
    let light_pos = vec3<f32>(5.0, 1.0, - 5.0);
    let light_color = vec3<f32>(1.0, 1.0, 1.0);
    let shininess = 64.0;

    let ambient = 0.3 * obj_color.xyz;
    let light_dir = normalize(light_pos - position.xyz);
    let diff = max(dot(normal.xyz, light_dir), 0.0);
    let diffuse = diff * obj_color.xyz * light_color;

    let view_dir = normalize(camera_pos.xyz - position.xyz);
    let halfway = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal.xyz, halfway), 0.0), shininess);
    let specular = 0.15 * spec * light_color;

    return vec4<f32>(ambient + diffuse + specular, 1.0);
}

// Intersection functions modified from here: https://iquilezles.org/articles/intersectors/
fn ray_intersecting_sphere(
    ro: vec3<f32>, rd: vec3<f32>, ce: vec3<f32>, ra: f32) -> Intersection {
    var i: Intersection;
    i.hit = false;

    let oc = ro - ce;
    let b = dot(oc, rd);
    let qc = oc - b * rd;
    var h = ra * ra - dot(qc, qc);
    if (h < 0.0) { return i; }
    h = sqrt(h);

    let dist = -b - h;
    if (dist < 0.0) { return i; } // Inside the sphere

    i.hit = true;
    i.hit_pos = ro + rd * dist;
    i.normal = normalize(i.hit_pos - ce);
    return i;
}

fn capsule_normal(pos: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> vec3<f32> {
    let ba = b - a;
    let pa = pos - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return (pa - h * ba) / r;
}

fn ray_intersecting_capsule(
    ro: vec3<f32>, rd: vec3<f32>, pa: vec3<f32>, pb: vec3<f32>, ra: f32) -> Intersection {
    let  ba = pb - pa;
    let  oa = ro - pa;
    let baba = dot(ba, ba);
    let bard = dot(ba, rd);
    let baoa = dot(ba, oa);
    let rdoa = dot(rd, oa);
    let oaoa = dot(oa, oa);
    let a = baba      - bard * bard;
    var b = baba * rdoa - baoa * bard;
    var c = baba * oaoa - baoa * baoa - ra * ra * baba;
    var h = b * b - a * c;

    var i: Intersection;
    i.hit = false;

    if (h >= 0.0) {
        let t = (-b - sqrt(h)) / a;
        let y = baoa + t * bard;

        // body
        if (y > 0.0 && y < baba) {
            if (t < 0.0) { return i; } // Inside the capsule
            i.hit_pos = ro + rd * t;
            i.normal = capsule_normal(i.hit_pos, pa, pb, ra);
            i.hit = true;
    	    return i;
    	}

        // caps
        var oc = ro - pb;
        if (y <= 0.0) { oc = oa; }
        b = dot(rd, oc);
        c = dot(oc, oc) - ra * ra;
        h = b * b - c;
        if (h > 0.0) {
            let dist = -b - sqrt(h);
            if (dist < 0.0) { return i; } // Inside the capsule

            i.hit_pos = ro + rd * dist;
            i.normal = capsule_normal(i.hit_pos, pa, pb, ra);
            i.hit = true;
            return i;
        }
    }

    return i;
}

fn test_intersections(ray_origin: vec3<f32>, ray_direction: vec3<f32>) -> Intersection {
    for (var i: i32 = 0; i < i32(num_shapes); i++) {
        let s = shapes[i];
        var result: Intersection;

        if (s.shape_type == 0) {
            result = ray_intersecting_sphere(
                ray_origin, ray_direction, s.start_pos.xyz, s.radius);
        } else {
            result = ray_intersecting_capsule(
                ray_origin, ray_direction, s.start_pos.xyz, s.end_pos.xyz, s.radius);
        }

        if (result.hit) {
            result.shape_index = i;
            return result;
        }
    }

    var result: Intersection;
    result.hit = false;
    return result;
}

@vertex
fn raytrace_vertex_shader(
    @builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // Sample a full screen squad (2 triangles)
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0), vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 1.0, -1.0), vec2<f32>( 1.0,  1.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

@fragment
fn raytrace_fragment_shader(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // Get 3D ray direction from the on screen coordinate
    let m = mat3x3<f32>(view_matrix[0].xyz, view_matrix[1].xyz, view_matrix[2].xyz);
    let uv = (position.xy - 0.5 * resolution) / resolution.y;
    let ray_direction = normalize(m * vec3(uv, -1.0));

    let result = test_intersections(camera_pos.xyz, ray_direction);
    if (!result.hit) {
        discard;
    }

    let s = shapes[result.shape_index];
    return phong_lighting(result.hit_pos, result.normal, s.color.xyz);
}
