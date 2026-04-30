struct CompassUniform {
    viewport_size: vec2<f32>,
    center_az_rad: f32,
    fov_x_rad: f32,
    // Distance from the viewport's bottom edge up to the strip's bottom edge, in pixels.
    strip_bottom_px: f32,
    strip_height_px: f32,
    label_top_px: f32,
    label_scale: f32,
    ui_scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    glyphs: vec4<u32>,
};

@group(0) @binding(0)
var<uniform> u: CompassUniform;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) px_xy: vec2<f32>,
};

const TAU: f32 = 6.28318530717958647692;
const PI: f32 = 3.14159265358979323846;
const TICK_STEP_RAD: f32 = 0.26179938779; // 15° in radians
const GLYPH_W: i32 = 5;
const GLYPH_H: i32 = 6;

@vertex
fn vs_main(@location(0) quad_pos: vec2<f32>) -> VsOut {
    // quad_pos in [-1, 1]^2. Map to a horizontal strip near the bottom of the screen.
    let strip_bottom_ndc = -1.0 + (u.strip_bottom_px / u.viewport_size.y) * 2.0;
    let strip_h_ndc = (u.strip_height_px / u.viewport_size.y) * 2.0;
    let t = quad_pos.y * 0.5 + 0.5; // 0 at bottom of strip, 1 at top
    let x = quad_pos.x;
    let y = strip_bottom_ndc + t * strip_h_ndc;

    var out: VsOut;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    // pixel coords: x from left of viewport, y from top of strip
    out.px_xy = vec2<f32>(
        (quad_pos.x * 0.5 + 0.5) * u.viewport_size.x,
        (1.0 - t) * u.strip_height_px,
    );
    return out;
}

fn glyph_pixel(g: u32, x: i32, y: i32) -> bool {
    if (x < 0 || x >= GLYPH_W || y < 0 || y >= GLYPH_H) {
        return false;
    }
    let bit = u32(y * GLYPH_W + x);
    return ((g >> bit) & 1u) == 1u;
}

fn pick_glyph(idx: u32) -> u32 {
    if (idx == 0u) { return u.glyphs.x; }
    if (idx == 1u) { return u.glyphs.y; }
    if (idx == 2u) { return u.glyphs.z; }
    return u.glyphs.w;
}

fn wrap_pi(a: f32) -> f32 {
    return a - TAU * round(a / TAU);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let px = in.px_xy.x;
    let py = in.px_xy.y;

    var alpha: f32 = 0.0;

    // Bottom horizontal line, thickness scales with DPI.
    let line_thickness = u.ui_scale;
    if (py >= u.strip_height_px - line_thickness) {
        alpha = 1.0;
    }

    // Tick marks at the nearest 15° angle to this pixel's azimuth.
    let pixel_az = u.center_az_rad + (px / u.viewport_size.x - 0.5) * u.fov_x_rad;
    let tick_idx = round(pixel_az / TICK_STEP_RAD);
    let tick_az = tick_idx * TICK_STEP_RAD;
    let tick_screen_x = ((tick_az - u.center_az_rad) / u.fov_x_rad + 0.5) * u.viewport_size.x;
    let tick_half_width = u.ui_scale * 0.6;
    if (abs(px - tick_screen_x) < tick_half_width) {
        let tick_int = i32(tick_idx);
        // 90° / 15° = 6 → cardinal ticks every 6 indices.
        let is_cardinal = (tick_int % 6) == 0;
        var tick_height: f32 = 4.0 * u.ui_scale;
        if (is_cardinal) {
            tick_height = 8.0 * u.ui_scale;
        }
        if (py >= u.strip_height_px - tick_height) {
            alpha = 1.0;
        }
    }

    // Cardinal letters (N=0°, E=90°, S=180°, W=270°).
    let scale = u.label_scale;
    let glyph_w_px = f32(GLYPH_W) * scale;
    let glyph_h_px = f32(GLYPH_H) * scale;
    let label_top = u.label_top_px;
    let label_bottom = label_top + glyph_h_px;

    if (py >= label_top && py < label_bottom) {
        for (var c: u32 = 0u; c < 4u; c = c + 1u) {
            let cardinal_az = f32(c) * (PI * 0.5);
            let delta = wrap_pi(cardinal_az - u.center_az_rad);
            let cardinal_x = (delta / u.fov_x_rad + 0.5) * u.viewport_size.x;
            let left = cardinal_x - glyph_w_px * 0.5;
            let rel_x = px - left;
            let rel_y = py - label_top;
            if (rel_x >= 0.0 && rel_x < glyph_w_px) {
                let gx = i32(floor(rel_x / scale));
                let gy = i32(floor(rel_y / scale));
                if (glyph_pixel(pick_glyph(c), gx, gy)) {
                    alpha = 1.0;
                }
            }
        }
    }

    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(alpha, alpha, alpha, alpha);
}
