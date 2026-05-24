mod camera;
mod overlay;
mod pipeline;
mod renderer;
mod tonemap;
mod vertex;

pub use camera::{Atmosphere, Camera, CameraUniform, LocalView};
pub use overlay::{OverlayConfig, OverlayKind};
pub use renderer::Renderer;
pub use vertex::{
    build_star_instance, magnitude_to_render_params, QuadVertex, RenderParams, StarInstance,
    NAKED_EYE_LIMITING_MAGNITUDE, SHADER_INTENSITY_CUTOFF, STAR_QUAD_HALF_PX,
};
