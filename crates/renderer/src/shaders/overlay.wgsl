// Overlay line shader. One pipeline draws every overlay layer; the per-layer
// uniform supplies the right transform/projection mode and color.

struct OverlayUniform {
    // Perspective mode: full view-projection matrix.
    // All-sky modes: view matrix only; projection is non-linear and happens
    // in this shader from camera-space longitude/latitude.
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
    // [projection_mode, full_sky_scale_x, full_sky_scale_y, full_sky_flag].
    // mode: 0=perspective, 1=Mollweide, 2=Aitoff, 3=Hammer.
    projection_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> overlay: OverlayUniform;

const PI: f32 = 3.14159265359;
const HALF_PI: f32 = 1.57079632679;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) other_position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

fn all_sky_lon_lat_from_view_dir(view_dir: vec3<f32>) -> vec2<f32> {
    let d = normalize(view_dir);
    return vec2<f32>(atan2(d.x, -d.z), asin(clamp(d.y, -1.0, 1.0)));
}

fn mollweide_project(lon: f32, lat: f32) -> vec2<f32> {
    var theta = lat;
    for (var i = 0; i < 6; i = i + 1) {
        let f = 2.0 * theta + sin(2.0 * theta) - PI * sin(lat);
        let fp = 2.0 + 2.0 * cos(2.0 * theta);
        theta = theta - f / max(fp, 1e-4);
        theta = clamp(theta, -HALF_PI, HALF_PI);
    }
    return vec2<f32>((lon / PI) * cos(theta), sin(theta));
}

fn aitoff_project(lon: f32, lat: f32) -> vec2<f32> {
    let half_lon = 0.5 * lon;
    let alpha = acos(clamp(cos(lat) * cos(half_lon), -1.0, 1.0));
    let sinc = select(sin(alpha) / max(alpha, 1e-6), 1.0, abs(alpha) < 1e-6);
    return vec2<f32>(2.0 * cos(lat) * sin(half_lon) / (PI * sinc), sin(lat) / (HALF_PI * sinc));
}

fn hammer_project(lon: f32, lat: f32) -> vec2<f32> {
    let half_lon = 0.5 * lon;
    let denom = sqrt(max(1.0 + cos(lat) * cos(half_lon), 1e-6));
    return vec2<f32>(cos(lat) * sin(half_lon) / denom, sin(lat) / denom);
}

fn all_sky_project(view_dir: vec3<f32>, other_view_dir: vec3<f32>) -> vec4<f32> {
    let lon_lat = all_sky_lon_lat_from_view_dir(view_dir);
    let other_lon_lat = all_sky_lon_lat_from_view_dir(other_view_dir);
    if abs(lon_lat.x - other_lon_lat.x) > PI {
        // The segment crosses the all-sky map seam. Drop both endpoints rather
        // than drawing a spurious line across the entire map ellipse.
        return vec4<f32>(2.0, 2.0, 0.5, 1.0);
    }
    let mode = overlay.projection_params.x;
    var p = mollweide_project(lon_lat.x, lon_lat.y);
    if mode >= 2.5 {
        p = hammer_project(lon_lat.x, lon_lat.y);
    } else if mode >= 1.5 {
        p = aitoff_project(lon_lat.x, lon_lat.y);
    }
    return vec4<f32>(p * overlay.projection_params.yz, 0.5, 1.0);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    if overlay.projection_params.x < -0.5 {
        out.clip_position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        return out;
    }
    if overlay.projection_params.w > 0.5 {
        out.clip_position = all_sky_project(
            (overlay.view_proj * vec4<f32>(input.position, 0.0)).xyz,
            (overlay.view_proj * vec4<f32>(input.other_position, 0.0)).xyz,
        );
    } else {
        out.clip_position = overlay.view_proj * vec4<f32>(input.position, 1.0);
    }
    return out;
}

@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return overlay.color;
}
